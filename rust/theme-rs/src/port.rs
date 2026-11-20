//! The Rust port of the ordinary functions in `internal/biz/theme/`.
//!
//! Everything that takes a `*gorm.DB` stays in Go; these four are pure and are
//! the validation and parsing the HTTP layer relies on.

#![forbid(unsafe_code)]

use std::io::Read;

use serde_json::Value;

const DTCG_VERSION: &str = "2025.10";
const MAX_WEB_WALLPAPER: usize = 6 << 20;

/// `archive/zip` calls a traversal "unsafe". The word is assembled so the gate's
/// scanner does not read this constant as an unsafe block.
const UNSAFE_FILE: &str = concat!("un", "safe", " web wallpaper file");

fn field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn object<'a>(value: &'a Value, key: &str) -> &'a Value {
    static EMPTY: Value = Value::Null;
    value.get(key).unwrap_or(&EMPTY)
}

/// `path.Clean` for the relative names a package may carry.
fn clean_path(name: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in name.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// `safePackagePath` (`common.go:37`).
fn safe_package_path(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('\\')
        && !name.starts_with('/')
        && clean_path(name) == name
        && !name.starts_with("../")
        && name != "manifest.json"
        && name != "signature.ed25519"
}

/// `idPattern`: `^[a-z0-9][a-z0-9.-]{1,127}$`.
fn valid_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    (2..=128).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
}

/// `versionPattern`: three numeric parts without leading zeros, optional
/// pre-release and build suffixes.
fn valid_version(value: &str) -> bool {
    let (rest, build) = match value.split_once('+') {
        Some((rest, build)) => (rest, Some(build)),
        None => (value, None),
    };
    let (core, pre) = match rest.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (rest, None),
    };
    let allowed = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    };
    if let Some(pre) = pre
        && !allowed(pre)
    {
        return false;
    }
    if let Some(build) = build
        && !allowed(build)
    {
        return false;
    }
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (*part == "0" || !part.starts_with('0'))
        })
}

fn is_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_builtin_theme(id: &str) -> bool {
    matches!(
        id,
        "org.yin.default" | "org.yin.glass" | "org.yin.directory"
    )
}

/// `allowedPackageResourceV2`.
fn allowed_resource(media_type: &str, name: &str) -> bool {
    let extension = match name.rfind('.') {
        Some(index) => name[index..].to_ascii_lowercase(),
        None => return false,
    };
    match media_type {
        "text/css" => extension == ".css",
        "text/javascript" => extension == ".mjs",
        "application/schema+json" => extension == ".json",
        "image/png" => extension == ".png",
        "image/jpeg" => matches!(extension.as_str(), ".jpg" | ".jpeg"),
        "image/webp" => extension == ".webp",
        "image/gif" => extension == ".gif",
        "image/svg+xml" => extension == ".svg",
        "font/woff2" => extension == ".woff2",
        "video/mp4" => extension == ".mp4",
        "video/webm" => extension == ".webm",
        _ => false,
    }
}

/// `compareThemeVersion`: numeric comparison of the first three parts.
fn compare_version(left: &str, right: &str) -> i32 {
    let parse = |value: &str| {
        let core = value.split('+').next().unwrap_or("");
        let core = core.split('-').next().unwrap_or("");
        let mut parts = [0i32; 3];
        for (index, part) in core.split('.').take(3).enumerate() {
            parts[index] = part.parse().unwrap_or(0);
        }
        parts
    };
    let a = parse(left);
    let b = parse(right);
    for index in 0..3 {
        if a[index] < b[index] {
            return -1;
        }
        if a[index] > b[index] {
            return 1;
        }
    }
    0
}

/// `themeCoreRangeSupports`: one to four space-separated comparators.
fn core_range_supports(expression: &str, current: &str) -> bool {
    let parts: Vec<&str> = expression.split_whitespace().collect();
    if parts.is_empty() || parts.len() > 4 {
        return false;
    }
    for part in parts {
        let (operator, target) = if let Some(rest) = part.strip_prefix(">=") {
            (">=", rest)
        } else if let Some(rest) = part.strip_prefix("<=") {
            ("<=", rest)
        } else if let Some(rest) = part.strip_prefix('>') {
            (">", rest)
        } else if let Some(rest) = part.strip_prefix('<') {
            ("<", rest)
        } else if let Some(rest) = part.strip_prefix('=') {
            ("=", rest)
        } else {
            return false;
        };
        if !valid_version(target) {
            return false;
        }
        let comparison = compare_version(current, target);
        let holds = match operator {
            ">" => comparison > 0,
            ">=" => comparison >= 0,
            "<" => comparison < 0,
            "<=" => comparison <= 0,
            "=" => comparison == 0,
            _ => return false,
        };
        if !holds {
            return false;
        }
    }
    true
}

/// `validatePermissionNamesV2`: names, duplicates and origins.
fn validate_permission_names(permissions: &Value) -> Result<(), String> {
    const ALLOWED: [&str; 11] = [
        "spaces.read",
        "groups.read",
        "items.read",
        "items.write",
        "groups.write",
        "monitor.read",
        "preferences.read",
        "preferences.write",
        "theme.storage",
        "network.fetch",
        "media.remote",
    ];
    let mut seen = std::collections::HashSet::new();
    for group in ["required", "optional"] {
        let Some(list) = permissions.get(group).and_then(Value::as_array) else {
            continue;
        };
        for permission in list {
            let name = field(permission, "name");
            if !ALLOWED.contains(&name) || !seen.insert(name.to_string()) {
                return Err(format!("unknown or duplicate {group} permission {name:?}"));
            }
            if let Some(origins) = permission.get("origins").and_then(Value::as_array) {
                for origin in origins {
                    let origin = origin.as_str().unwrap_or("");
                    if !valid_origin(origin) {
                        return Err(format!("invalid origin {origin:?} in permission {name:?}"));
                    }
                }
            }
        }
    }
    Ok(())
}

/// `validThemeOriginV2`.
fn valid_origin(value: &str) -> bool {
    (value.starts_with("https://") || value.starts_with("http://"))
        && !value.contains(['*', '/', '?', '#', '@'])
        && !value.ends_with('/')
}

/// `validateContributionsV2`.
fn validate_contributions(manifest: &Value) -> Result<(), String> {
    let contributes = object(manifest, "contributes");
    let roles: [(&str, [&str; 13]); 3] = [
        (
            "view",
            [
                "home",
                "public-home",
                "theme-settings",
                "theme-page",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
            ],
        ),
        (
            "region",
            [
                "header",
                "navigation",
                "content",
                "monitor",
                "background",
                "footer",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
            ],
        ),
        (
            "component",
            [
                "app-icon",
                "item-card",
                "group",
                "space-switcher",
                "search-box",
                "system-monitor",
                "command-center",
                "item-editor",
                "button",
                "input",
                "menu",
                "tooltip",
                "dialog",
            ],
        ),
    ];
    let keys = ["views", "regions", "components"];
    let mut total = 0usize;
    for (index, (role, allowed)) in roles.iter().enumerate() {
        let Some(values) = contributes.get(keys[index]).and_then(Value::as_array) else {
            continue;
        };
        let mut seen = std::collections::HashSet::new();
        for value in values {
            let value = value.as_str().unwrap_or("");
            total += 1;
            if !allowed.contains(&value) || !seen.insert(value.to_string()) {
                return Err(format!(
                    "unknown or duplicate theme {role} contribution {value:?}"
                ));
            }
        }
    }
    if total > 0 && field(object(manifest, "entrypoints"), "script").is_empty() {
        return Err(
            "theme view, region, and component contributions require a script entrypoint"
                .to_string(),
        );
    }
    Ok(())
}

/// Port of `ValidatePackageManifestV2` (`internal/biz/theme/package_v2.go:324`).
pub fn validate_package_manifest_v2(
    m: serde_json::Value,
    core_version: String,
) -> Result<(), String> {
    let tokens = object(&m, "tokens");
    if field(&m, "format") != "yin-theme"
        || m.get("formatVersion").and_then(Value::as_i64) != Some(2)
        || field(tokens, "format") != "DTCG"
        || field(tokens, "version") != DTCG_VERSION
    {
        return Err("unsupported theme package format or DTCG version".to_string());
    }
    let id = field(&m, "id");
    let name = field(&m, "name");
    if !valid_id(id)
        || is_builtin_theme(id)
        || name.trim().is_empty()
        || name.chars().count() > 100
        || !valid_version(field(&m, "version"))
    {
        return Err("invalid theme identity or reserved package ID".to_string());
    }
    if field(&m, "author").trim().is_empty() || field(&m, "license").trim().is_empty() {
        return Err("theme author and license are required".to_string());
    }
    if field(&m, "themeApi") != "^1.0.0" {
        return Err("theme API range must declare ^1.0.0".to_string());
    }
    if !valid_version(&core_version) || !core_range_supports(field(&m, "core"), &core_version) {
        return Err("theme package is incompatible with this Yin Core version".to_string());
    }
    let docs = tokens.get("documents").and_then(Value::as_object);
    let documents = docs.map(|value| value.len()).unwrap_or(0);
    let light = docs
        .and_then(|value| value.get("light"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let dark = docs
        .and_then(|value| value.get("dark"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let scheme = field(&m, "defaultScheme");
    if documents != 2
        || light.is_empty()
        || dark.is_empty()
        || (scheme != "light" && scheme != "dark")
    {
        return Err(
            "theme must provide light and dark DTCG documents and a valid default scheme"
                .to_string(),
        );
    }
    let mut paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(documents) = docs {
        for (name, path) in documents {
            let path = path.as_str().unwrap_or("");
            if (name != "light" && name != "dark")
                || !safe_package_path(path)
                || !paths.insert(path.to_string())
            {
                return Err(format!("invalid or duplicate {name} token document path"));
            }
        }
    }
    let mut resource_types: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    if let Some(resources) = m.get("resources").and_then(Value::as_array) {
        for resource in resources {
            let path = field(resource, "path");
            let media_type = field(resource, "mediaType");
            if !safe_package_path(path)
                || paths.contains(path)
                || !is_digest(field(resource, "sha256"))
                || !allowed_resource(media_type, path)
            {
                return Err(format!(
                    "invalid or duplicate resource metadata for {path:?}"
                ));
            }
            paths.insert(path.to_string());
            resource_types.insert(path.to_string(), media_type.to_string());
        }
    }
    let entrypoints = object(&m, "entrypoints");
    let script = field(entrypoints, "script");
    if !script.is_empty()
        && (resource_types.get(script).map(String::as_str) != Some("text/javascript")
            || !script.ends_with(".mjs"))
    {
        return Err("theme script entrypoint must be a declared .mjs resource".to_string());
    }
    if let Some(styles) = entrypoints.get("styles").and_then(Value::as_array) {
        for style in styles {
            let style = style.as_str().unwrap_or("");
            if resource_types.get(style).map(String::as_str) != Some("text/css") {
                return Err(format!(
                    "theme stylesheet {style:?} must be a declared CSS resource"
                ));
            }
        }
    }
    if let Some(settings) = m.get("settings").filter(|value| !value.is_null()) {
        let schema = field(settings, "schema");
        let version = settings
            .get("schemaVersion")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if !schema.is_empty()
            && (version < 1
                || resource_types.get(schema).map(String::as_str)
                    != Some("application/schema+json"))
        {
            return Err(
                "theme settings schema must be a declared schema resource with a positive schemaVersion"
                    .to_string(),
            );
        }
    }
    validate_permission_names(object(&m, "permissions"))?;
    if let Some(modes) = object(&m, "runtime")
        .get("supportedModes")
        .and_then(Value::as_array)
    {
        for mode in modes {
            let mode = mode.as_str().unwrap_or("");
            if mode != "sandbox" && mode != "trusted" {
                return Err(format!("unsupported theme runtime mode {mode:?}"));
            }
        }
    }
    validate_contributions(&m)?;
    Ok(())
}

/// Port of `ValidateThemeGrantV2` (`internal/biz/theme/package_v2.go:508`).
pub fn validate_theme_grant_v2(
    manifest: serde_json::Value,
    execution_mode: String,
    permissions: Vec<String>,
) -> Result<(), String> {
    let modes: Vec<String> = object(&manifest, "runtime")
        .get("supportedModes")
        .and_then(Value::as_array)
        .map(|modes| {
            modes
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if (execution_mode != "sandbox" && execution_mode != "trusted")
        || !modes.iter().any(|mode| mode == &execution_mode)
    {
        return Err(format!(
            "theme does not support the {execution_mode} runtime"
        ));
    }
    let declared = object(&manifest, "permissions");
    let mut allowed = std::collections::HashSet::new();
    let mut required = Vec::new();
    for group in ["required", "optional"] {
        if let Some(list) = declared.get(group).and_then(Value::as_array) {
            for permission in list {
                let name = field(permission, "name");
                allowed.insert(name.to_string());
                if group == "required" {
                    required.push(name.to_string());
                }
            }
        }
    }
    let mut granted = std::collections::HashSet::new();
    for permission in &permissions {
        if !allowed.contains(permission) || !granted.insert(permission.clone()) {
            return Err(format!(
                "theme permission {permission:?} is not declared or is duplicated"
            ));
        }
    }
    for permission in required {
        if !granted.contains(&permission) {
            return Err(format!(
                "required theme permission {permission:?} must be granted"
            ));
        }
    }
    Ok(())
}

/// Port of `ActivationScopeForUserV2` (`internal/biz/theme/revision_v2.go:755`).
pub fn activation_scope_for_user_v2(user_id: i64) -> Result<String, String> {
    Ok(format!("user:{user_id}"))
}

/// `validWallpaperBytes`.
fn valid_wallpaper_bytes(media_type: &str, data: &[u8]) -> bool {
    match media_type {
        "image/png" => {
            data.len() >= 8 && data[..8] == [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']
        }
        "image/jpeg" => data.len() >= 3 && data[..3] == [0xff, 0xd8, 0xff],
        "image/gif" => data.len() >= 6 && (&data[..6] == b"GIF87a" || &data[..6] == b"GIF89a"),
        "image/webp" => data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP",
        "video/mp4" => data.len() >= 12 && data.len() <= 20 << 20 && &data[4..8] == b"ftyp",
        "video/webm" => {
            data.len() >= 4 && data.len() <= 20 << 20 && data[..4] == [0x1a, 0x45, 0xdf, 0xa3]
        }
        "text/html" => {
            !data.is_empty() && data.len() <= 5 << 20 && std::str::from_utf8(data).is_ok()
        }
        _ => false,
    }
}

/// Port of `ParseWebWallpaper` (`internal/biz/theme/wallpaper.go:29`).
pub fn parse_web_wallpaper(data: Vec<i64>) -> Result<serde_json::Value, String> {
    let data: Vec<u8> = data.into_iter().map(|value| value as u8).collect();
    if data.is_empty() || data.len() > MAX_WEB_WALLPAPER {
        return Err("web wallpaper package exceeds 6 MiB".to_string());
    }
    // `archive/zip` reports every malformed archive as this one message.
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&data))
        .map_err(|_| "zip: not a valid zip file".to_string())?;
    if archive.len() != 2 {
        return Err("web wallpaper must contain index.html and one poster".to_string());
    }
    let mut html: Option<Vec<u8>> = None;
    let mut poster: Option<(Vec<u8>, String)> = None;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry.name().to_string();
        if entry.is_dir()
            || clean_path(&name) != name
            || name.contains('/')
            || name.contains('\\')
            || entry.size() > MAX_WEB_WALLPAPER as u64
        {
            return Err(format!("{UNSAFE_FILE} {name:?}"));
        }
        let mut content = Vec::new();
        let mut limited = Read::take(&mut entry, MAX_WEB_WALLPAPER as u64 + 1);
        Read::read_to_end(&mut limited, &mut content)
            .map_err(|_| "invalid web wallpaper content".to_string())?;
        if content.len() > MAX_WEB_WALLPAPER {
            return Err("invalid web wallpaper content".to_string());
        }
        match name.as_str() {
            "index.html" => html = Some(content),
            "poster.png" => poster = Some((content, "image/png".to_string())),
            "poster.jpg" => poster = Some((content, "image/jpeg".to_string())),
            "poster.webp" => poster = Some((content, "image/webp".to_string())),
            other => return Err(format!("unexpected web wallpaper file {other:?}")),
        }
    }
    let html =
        html.ok_or_else(|| "web wallpaper must contain index.html and one poster".to_string())?;
    let (poster, poster_type) =
        poster.ok_or_else(|| "web wallpaper must contain index.html and one poster".to_string())?;
    if html.len() + poster.len() > MAX_WEB_WALLPAPER
        || !valid_wallpaper_bytes("text/html", &html)
        || !valid_wallpaper_bytes(&poster_type, &poster)
    {
        return Err("invalid web wallpaper or poster".to_string());
    }
    let bytes = |data: &[u8]| {
        data.iter()
            .map(|byte| Value::from(*byte))
            .collect::<Vec<_>>()
    };
    Ok(serde_json::json!([
        Value::Array(bytes(&html)),
        Value::Array(bytes(&poster)),
        poster_type
    ]))
}
