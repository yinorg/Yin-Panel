//! Theme package v2: archive parsing, manifest validation and installation.
//!
//! Mirrors `internal/biz/theme/package_v2.go`. The archive is a ZIP holding
//! `manifest.json`, two DTCG documents, the declared resources and an optional
//! Ed25519 signature.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub const MAX_ARCHIVE: usize = 32 << 20;
pub const MAX_EXPANDED: u64 = 48 << 20;
pub const MAX_FILES: usize = 512;
pub const MAX_SETTINGS_SCHEMA: usize = 256 * 1024;
const DTCG_VERSION: &str = "2025.10";
const DTCG_SCHEMA: &str = "https://www.designtokens.org/schemas/2025.10/format.json";
const MAX_PREVIEWS: usize = 3;
/// `OfficialPublicKey` in `common.go`, hex.
const OFFICIAL_PUBLIC_KEY: &str =
    "c57e23e3f7bafbe6a797e45d73931e4cd82f7e3479c2de5be8c4c74322871c78";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    pub format: String,
    #[serde(default, rename = "formatVersion")]
    pub format_version: i64,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, rename = "themeApi")]
    pub theme_api: String,
    #[serde(default)]
    pub core: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repository: String,
    #[serde(default)]
    pub tokens: TokenSet,
    #[serde(default, rename = "defaultScheme")]
    pub default_scheme: String,
    #[serde(default)]
    pub entrypoints: Entrypoints,
    #[serde(default)]
    pub runtime: Runtime,
    #[serde(default)]
    pub contributes: Contributions,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<ThemeSettings>,
    #[serde(
        default,
        rename = "wallpaperCatalogue",
        skip_serializing_if = "String::is_empty"
    )]
    pub wallpaper_catalogue: String,
    #[serde(default)]
    pub resources: Vec<Resource>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenSet {
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub documents: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entrypoints {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub script: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    #[serde(
        default,
        rename = "supportedModes",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub supported_modes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contributions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<String>,
    #[serde(
        default,
        rename = "homeLayout",
        skip_serializing_if = "Option::is_none"
    )]
    pub home_layout: Option<HomeLayout>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomeLayout {
    #[serde(default)]
    pub kind: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub editable: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<Permission>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional: Vec<Permission>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permission {
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub origins: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeSettings {
    #[serde(default)]
    pub schema: String,
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default, rename = "mediaType")]
    pub media_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct Package {
    pub manifest: Manifest,
    pub tokens: BTreeMap<String, serde_json::Value>,
    pub files: BTreeMap<String, (String, Vec<u8>)>,
    pub verified: bool,
    pub revision: String,
}

impl Package {
    /// `PackagePreviewPublicV2`: the manifest with preview asset URLs.
    pub fn preview_public(&self, token: &str) -> serde_json::Value {
        let mut manifest = self.manifest.clone();
        for resource in &mut manifest.resources {
            let escaped: Vec<String> = resource.path.split('/').map(path_escape).collect();
            resource.url = format!(
                "/api/theme/v2/preview/{}/assets/{}",
                path_escape(token),
                escaped.join("/")
            );
        }
        let mut manifest = serde_json::to_value(manifest).unwrap_or(serde_json::Value::Null);
        if manifest
            .get("resources")
            .and_then(serde_json::Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            manifest["resources"] = serde_json::Value::Null;
        }
        serde_json::json!({
            "manifest": manifest,
            "tokens": self.tokens,
            "verified": self.verified,
            "revision": self.revision,
        })
    }
}

pub fn path_escape(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        let keep = byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_'
                    | b'.'
                    | b'~'
                    | b'+'
                    | b'$'
                    | b'&'
                    | b'!'
                    | b'*'
                    | b'\''
                    | b'('
                    | b')'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
            );
        if keep {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn safe_path(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('\\')
        && !name.starts_with('/')
        && name != ".."
        && !name.starts_with("../")
        && !name.contains("/../")
        && !name.ends_with("/..")
        && !name.contains("//")
        && name != "."
}

fn valid_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 2 || bytes.len() > 128 {
        return false;
    }
    if !(bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit()) {
        return false;
    }
    bytes.iter().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
    })
}

fn valid_version(value: &str) -> bool {
    let core = value.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.chars().all(|ch| ch.is_ascii_digit())
                && (part == &"0" || !part.starts_with('0'))
        })
}

fn is_digest(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

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
        _ => false,
    }
}

fn is_builtin_theme(id: &str) -> bool {
    matches!(
        id,
        "org.yin.default" | "org.yin.glass" | "org.yin.directory"
    )
}

/// `themeCoreRangeSupports`: enough of a semver range for the `>=x.y.z` forms
/// packages declare.
/// `themeCoreRangeSupports`: one to four space-separated comparators, each with
/// a full version. A bare version, `^`, `*` and comma lists are not accepted —
/// the Go validator rejects them.
fn core_range_supports(expression: &str, version: &str) -> bool {
    let Some(current) = parse_version(version) else {
        return false;
    };
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
        let Some(expected) = parse_version(target) else {
            return false;
        };
        let holds = match operator {
            ">" => current > expected,
            ">=" => current >= expected,
            "<" => current < expected,
            "<=" => current <= expected,
            _ => current == expected,
        };
        if !holds {
            return false;
        }
    }
    true
}

fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let core = value.trim_start_matches('v').split(['-', '+']).next()?;
    let mut parts = core.split('.');
    Some((
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}

/// `validateDTCGDocument202510`, narrowed to the structure this Core depends
/// on: the schema identifier, one JSON document, and every token carrying a
/// `$value` with a `$type` available from itself or a group.
pub fn validate_dtcg_document(data: &[u8]) -> Result<(), String> {
    let document: serde_json::Value =
        serde_json::from_slice(data).map_err(|error| error.to_string())?;
    if document.get("$schema").and_then(serde_json::Value::as_str) != Some(DTCG_SCHEMA) {
        return Err("$schema must identify DTCG 2025.10".to_string());
    }
    if !document.is_object() {
        return Err("DTCG document must be a JSON object".to_string());
    }
    walk_tokens(&document, None)
}

fn walk_tokens(node: &serde_json::Value, inherited_type: Option<&str>) -> Result<(), String> {
    let Some(object) = node.as_object() else {
        return Ok(());
    };
    let declared_type = object
        .get("$type")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let type_name = declared_type.as_deref().or(inherited_type);
    if object.contains_key("$value") {
        let Some(kind) = type_name else {
            return Err("token has no inherited $type".to_string());
        };
        if !matches!(
            kind,
            "color"
                | "dimension"
                | "fontFamily"
                | "fontWeight"
                | "duration"
                | "cubicBezier"
                | "number"
                | "strokeStyle"
                | "border"
                | "transition"
                | "shadow"
                | "gradient"
                | "typography"
        ) {
            return Err(format!("token has unknown DTCG type {kind:?}"));
        }
        return Ok(());
    }
    for (key, value) in object {
        if key.starts_with('$') {
            continue;
        }
        walk_tokens(value, type_name)?;
    }
    Ok(())
}

/// `ValidatePackageManifestV2`.
pub fn validate_manifest(manifest: &Manifest, core_version: &str) -> Result<(), String> {
    if manifest.format != "yin-theme"
        || manifest.format_version != 2
        || manifest.tokens.format != "DTCG"
        || manifest.tokens.version != DTCG_VERSION
    {
        return Err("unsupported theme package format or DTCG version".to_string());
    }
    if !valid_id(&manifest.id)
        || is_builtin_theme(&manifest.id)
        || manifest.name.trim().is_empty()
        || manifest.name.chars().count() > 100
        || !valid_version(&manifest.version)
    {
        return Err("invalid theme identity or reserved package ID".to_string());
    }
    if manifest.author.trim().is_empty() || manifest.license.trim().is_empty() {
        return Err("theme author and license are required".to_string());
    }
    if manifest.theme_api != "^1.0.0" {
        return Err("theme API range must declare ^1.0.0".to_string());
    }
    if !valid_version(core_version) || !core_range_supports(&manifest.core, core_version) {
        return Err("theme package is incompatible with this Yin Core version".to_string());
    }
    if manifest.tokens.documents.len() != 2
        || manifest
            .tokens
            .documents
            .get("light")
            .map(String::as_str)
            .unwrap_or("")
            .is_empty()
        || manifest
            .tokens
            .documents
            .get("dark")
            .map(String::as_str)
            .unwrap_or("")
            .is_empty()
        || !matches!(manifest.default_scheme.as_str(), "light" | "dark")
    {
        return Err(
            "theme must provide light and dark DTCG documents and a valid default scheme"
                .to_string(),
        );
    }
    let mut paths = std::collections::HashSet::new();
    for (scheme, name) in &manifest.tokens.documents {
        if !matches!(scheme.as_str(), "light" | "dark")
            || !safe_path(name)
            || !paths.insert(name.clone())
        {
            return Err(format!("invalid or duplicate {scheme} token document path"));
        }
    }
    let mut resource_types: HashMap<String, String> = HashMap::new();
    for resource in &manifest.resources {
        if !safe_path(&resource.path)
            || paths.contains(&resource.path)
            || !is_digest(&resource.sha256)
            || !allowed_resource(&resource.media_type, &resource.path)
        {
            return Err(format!(
                "invalid or duplicate resource metadata for {:?}",
                resource.path
            ));
        }
        paths.insert(resource.path.clone());
        resource_types.insert(resource.path.clone(), resource.media_type.clone());
    }
    if !manifest.entrypoints.script.is_empty()
        && (resource_types
            .get(&manifest.entrypoints.script)
            .map(String::as_str)
            != Some("text/javascript")
            || !manifest.entrypoints.script.ends_with(".mjs"))
    {
        return Err("theme script entrypoint must be a declared .mjs resource".to_string());
    }
    for style in &manifest.entrypoints.styles {
        if resource_types.get(style).map(String::as_str) != Some("text/css") {
            return Err(format!(
                "theme stylesheet {style:?} must be a declared CSS resource"
            ));
        }
    }
    if let Some(settings) = &manifest.settings {
        if settings.schema_version < 1
            || resource_types.get(&settings.schema).map(String::as_str)
                != Some("application/schema+json")
        {
            return Err(
                "theme settings schema must be a declared schema resource with a positive schemaVersion"
                    .to_string(),
            );
        }
    }
    let allowed_names = [
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
    let mut seen_permissions = std::collections::HashSet::new();
    for (group, list) in [
        ("required", &manifest.permissions.required),
        ("optional", &manifest.permissions.optional),
    ] {
        for permission in list {
            if !allowed_names.contains(&permission.name.as_str())
                || !seen_permissions.insert(permission.name.clone())
            {
                return Err(format!(
                    "unknown or duplicate {group} permission {:?}",
                    permission.name
                ));
            }
            for origin in &permission.origins {
                if !valid_origin(origin) {
                    return Err(format!(
                        "invalid origin {origin:?} in permission {:?}",
                        permission.name
                    ));
                }
            }
        }
    }
    for mode in &manifest.runtime.supported_modes {
        if !matches!(mode.as_str(), "sandbox" | "trusted") {
            return Err(format!("unsupported theme runtime mode {mode:?}"));
        }
    }
    validate_contributions(manifest)?;
    Ok(())
}

fn valid_origin(value: &str) -> bool {
    (value.starts_with("https://") || value.starts_with("http://"))
        && !value.contains(['*', '/', '?', '#', '@'])
        && !value.ends_with('/')
}

fn validate_contributions(manifest: &Manifest) -> Result<(), String> {
    let roles: [(&str, &[String], &[&str]); 3] = [
        (
            "view",
            &manifest.contributes.views,
            &["home", "public-home", "theme-settings", "theme-page"],
        ),
        (
            "region",
            &manifest.contributes.regions,
            &[
                "header",
                "navigation",
                "content",
                "monitor",
                "background",
                "footer",
            ],
        ),
        (
            "component",
            &manifest.contributes.components,
            &[
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
    for (role, values, allowed) in roles {
        let mut seen = std::collections::HashSet::new();
        for value in values {
            if !allowed.contains(&value.as_str()) || !seen.insert(value) {
                return Err(format!("unknown or duplicate theme {role} {value:?}"));
            }
        }
    }
    if let Some(layout) = &manifest.contributes.home_layout {
        if !matches!(layout.kind.as_str(), "standard" | "directory") {
            return Err(format!("unknown theme home layout {:?}", layout.kind));
        }
    }
    let contributions = manifest.contributes.views.len()
        + manifest.contributes.regions.len()
        + manifest.contributes.components.len();
    if contributions > 0 && manifest.entrypoints.script.is_empty() {
        return Err(
            "theme view, region, and component contributions require a script entrypoint"
                .to_string(),
        );
    }
    Ok(())
}

/// `ParsePackageArchiveV2`.
pub fn parse_archive(data: &[u8], confirm_unverified: bool) -> Result<Package, String> {
    if data.is_empty() || data.len() > MAX_ARCHIVE {
        return Err("theme package must be between 1 byte and 32 MiB".to_string());
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|error| {
        let detail = if error.to_string().contains("Could not find EOCD") {
            "zip: not a valid zip file"
        } else {
            return format!("invalid theme package archive: {error}");
        };
        format!("invalid theme package archive: {detail}")
    })?;
    if archive.len() > MAX_FILES {
        return Err("theme package contains too many files".to_string());
    }
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut expanded: u64 = 0;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("invalid theme package archive: {error}"))?;
        let raw = entry.name().to_string();
        let name = raw.strip_prefix("./").unwrap_or(&raw).to_string();
        if !safe_path(&name) || name.is_empty() {
            return Err(format!("unsafe theme package path {raw:?}"));
        }
        if entry.is_dir() {
            continue;
        }
        if files.contains_key(&name) {
            return Err(format!("duplicate theme package path {raw:?}"));
        }
        expanded += entry.size();
        if expanded > MAX_EXPANDED {
            return Err("expanded theme package exceeds 48 MiB".to_string());
        }
        let mut content = Vec::new();
        entry
            .by_ref()
            .take(MAX_EXPANDED + 1)
            .read_to_end(&mut content)
            .map_err(|error| error.to_string())?;
        if content.len() as u64 > MAX_EXPANDED {
            return Err("expanded theme package exceeds 48 MiB".to_string());
        }
        files.insert(name, content);
    }

    let manifest_bytes = files
        .get("manifest.json")
        .ok_or_else(|| "manifest.json is required".to_string())?;
    let manifest: Manifest = serde_json::from_slice(manifest_bytes)
        .map_err(|error| format!("invalid v2 theme manifest: {error}"))?;
    let core_version = crate::appcfg::version_name()
        .trim_start_matches('v')
        .to_string();
    validate_manifest(&manifest, &core_version)?;

    let mut tokens = BTreeMap::new();
    let mut used: std::collections::HashSet<String> = ["manifest.json", "signature.ed25519"]
        .iter()
        .map(|value| value.to_string())
        .collect();
    for (scheme, name) in &manifest.tokens.documents {
        let doc = files
            .get(name)
            .ok_or_else(|| format!("missing {scheme} token document {name:?}"))?;
        validate_dtcg_document(doc)
            .map_err(|error| format!("invalid {scheme} DTCG document: {error}"))?;
        used.insert(name.clone());
        tokens.insert(
            scheme.clone(),
            serde_json::from_slice(doc).map_err(|error| error.to_string())?,
        );
    }
    let mut packaged: BTreeMap<String, (String, Vec<u8>)> = BTreeMap::new();
    for resource in &manifest.resources {
        let content = files
            .get(&resource.path)
            .ok_or_else(|| format!("missing theme resource {:?}", resource.path))?;
        used.insert(resource.path.clone());
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(content);
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        if hex != resource.sha256.to_ascii_lowercase() {
            return Err(format!("resource digest mismatch for {:?}", resource.path));
        }
        if !allowed_resource(&resource.media_type, &resource.path) {
            return Err(format!(
                "resource type is not allowed for {:?}",
                resource.path
            ));
        }
        packaged.insert(
            resource.path.clone(),
            (resource.media_type.clone(), content.clone()),
        );
    }
    if let Some(settings) = &manifest.settings {
        match packaged.get(&settings.schema) {
            Some((media_type, content))
                if settings.schema_version >= 1
                    && media_type == "application/schema+json"
                    && content.len() <= MAX_SETTINGS_SCHEMA => {}
            _ => {
                return Err(
                    "theme settings require a declared schema and positive schemaVersion"
                        .to_string(),
                );
            }
        }
    }
    let mut entrypoints = vec![manifest.entrypoints.script.clone()];
    entrypoints.extend(manifest.entrypoints.styles.iter().cloned());
    if let Some(settings) = &manifest.settings {
        entrypoints.push(settings.schema.clone());
    }
    for name in entrypoints {
        if name.is_empty() {
            continue;
        }
        if !used.contains(&name) {
            return Err(format!(
                "entrypoint {name:?} is not a declared theme resource"
            ));
        }
    }
    for name in files.keys() {
        if !used.contains(name) {
            return Err(format!("undeclared theme package file {name:?}"));
        }
    }
    if !manifest.entrypoints.script.is_empty()
        && !manifest
            .runtime
            .supported_modes
            .iter()
            .any(|mode| mode == "sandbox" || mode == "trusted")
    {
        return Err("script themes must declare a supported runtime mode".to_string());
    }

    let verified = match files.get("signature.ed25519") {
        Some(signature) => {
            use base64::Engine;
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(String::from_utf8_lossy(signature).trim())
                .map_err(|_| "invalid theme signature encoding".to_string())?;
            if decoded.len() != 64 {
                return Err("invalid theme signature encoding".to_string());
            }
            let payload = signature_payload(&files);
            verify_signature(&payload, &decoded)?
        }
        None => {
            if !confirm_unverified {
                return Err("theme is unsigned; explicit confirmation is required".to_string());
            }
            false
        }
    };
    let revision = package_revision(&files);
    Ok(Package {
        manifest,
        tokens,
        files: packaged,
        verified,
        revision,
    })
}

/// `packageRevisionV2`: SHA-256 over the sorted file names and their digests.
fn package_revision(files: &BTreeMap<String, Vec<u8>>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for (name, content) in files {
        if name == "signature.ed25519" {
            continue;
        }
        let digest = Sha256::digest(content);
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        hasher.update(format!("{name}\u{0}{hex}\n").as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn signature_payload(files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for (name, content) in files {
        if name == "signature.ed25519" {
            continue;
        }
        let digest = Sha256::digest(content);
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        hasher.update(format!("{name}\u{0}{hex}\n").as_bytes());
    }
    hasher.finalize().to_vec()
}

fn verify_signature(payload: &[u8], signature: &[u8]) -> Result<bool, String> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let key_bytes = hex_bytes(OFFICIAL_PUBLIC_KEY)?;
    let key = VerifyingKey::from_bytes(
        key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "invalid official key".to_string())?,
    )
    .map_err(|_| "invalid official key".to_string())?;
    let signature =
        Signature::from_slice(signature).map_err(|_| "theme signature is invalid".to_string())?;
    key.verify(payload, &signature)
        .map(|_| true)
        .map_err(|_| "theme signature is invalid".to_string())
}

fn hex_bytes(value: &str) -> Result<Vec<u8>, String> {
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16).map_err(|_| "invalid hex".to_string())
        })
        .collect()
}

// ---------------------------------------------------------------- previews

struct Preview {
    package: Package,
    expires_at: Instant,
}

static PREVIEWS: Mutex<Vec<(String, Preview)>> = Mutex::new(Vec::new());

pub fn save_preview(package: Package) -> Result<String, String> {
    // `SavePackagePreviewV2` uses 24 random bytes: the token is the only thing
    // protecting a preview, so it must not be predictable.
    use rand::RngCore;
    let mut raw = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut raw);
    let token: String = raw.iter().map(|byte| format!("{byte:02x}")).collect();
    let mut store = PREVIEWS.lock().map_err(|_| "preview store".to_string())?;
    let now = Instant::now();
    store.retain(|(_, preview)| preview.expires_at > now);
    while store.len() >= MAX_PREVIEWS {
        store.remove(0);
    }
    store.push((
        token.clone(),
        Preview {
            package,
            expires_at: now + Duration::from_secs(600),
        },
    ));
    Ok(token)
}

pub fn get_preview(token: &str) -> Option<Package> {
    let mut store = PREVIEWS.lock().ok()?;
    let now = Instant::now();
    store.retain(|(_, preview)| preview.expires_at > now);
    store
        .iter()
        .find(|(key, _)| key == token)
        .map(|(_, preview)| preview.package.clone())
}
