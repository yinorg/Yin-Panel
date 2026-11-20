//! Turns the packaged wallpaper catalogue into Rust statics.
//!
//! Go validates the catalogue once, on first use, and returns an error when it
//! is malformed. The pack is compiled into the binary, so the Rust port
//! performs that same validation here, at build time: a broken catalogue fails
//! the build instead of every call. `src/port.rs` therefore has no error path
//! that a recorded case could not reach, and the emitted data is exactly the
//! filtered catalogue Go would expose.

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let assets = manifest.join("../../backend/internal/biz/theme/wallpaperpack/assets");
    println!("cargo:rerun-if-changed={}", assets.display());

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let index = index_of(&assets);
    let document: serde_json::Value = serde_json::from_slice(
        &fs::read(assets.join("catalogue.json")).expect("reading assets/catalogue.json"),
    )
    .expect("decoding assets/catalogue.json");

    let version = document
        .get("version")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    assert!(
        !version.is_empty(),
        "wallpaper pack catalogue has no version"
    );

    let mut generated = String::new();
    generated.push_str(&format!(
        "pub static VERSION: &str = {};\n",
        literal(version)
    ));
    generated.push_str("#[allow(clippy::redundant_static_lifetimes)]\npub type Entry = (&'static str, &'static str, &'static str, &'static str, &'static str);\n");
    generated.push_str("pub static CATALOGUE: &[(&str, &str, &[Entry])] = &[\n");

    let categories = document
        .get("categories")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for category in categories {
        let id = text(&category, "id");
        let name = text(&category, "name");
        generated.push_str(&format!("    ({}, {}, &[\n", literal(&id), literal(&name)));
        for item in category
            .get("items")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let entry = (
                text(&item, "id"),
                text(&item, "name"),
                text(&item, "kind"),
                text(&item, "source"),
                text(&item, "poster"),
            );
            if valid(&entry, &index).is_err() {
                continue;
            }
            generated.push_str(&format!(
                "        ({}, {}, {}, {}, {}),\n",
                literal(&entry.0),
                literal(&entry.1),
                literal(&entry.2),
                literal(&entry.3),
                literal(&entry.4),
            ));
        }
        generated.push_str("    ]),\n");
    }
    generated.push_str("];\n");

    let mut entries = String::new();
    for (name, media_type) in &index {
        // The artwork is compiled in, as Go's `embed.FS` does: the asset route
        // then needs no runtime directory next to the binary.
        let absolute = assets.join(name);
        entries.push_str(&format!(
            "    ({}, {}, include_bytes!({})),\n",
            literal(name),
            literal(media_type),
            literal(&absolute.to_string_lossy())
        ));
    }
    generated.push_str(&format!(
        "pub static ASSETS: &[(&str, &str, &[u8])] = &[\n{entries}];\n"
    ));

    fs::write(out.join("catalogue.rs"), generated).expect("writing the generated catalogue");
}

fn text(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// A Rust string literal for `value`.
fn literal(value: &str) -> String {
    format!("{value:?}")
}

/// `(relative name, media type)` for every packaged file, mirroring Go's walk of
/// the embedded assets.
fn index_of(root: &Path) -> Vec<(String, &'static str)> {
    let mut found = Vec::new();
    walk(root, root, &mut found);
    found.sort();
    found
}

fn walk(root: &Path, dir: &Path, found: &mut Vec<(String, &'static str)>) {
    let mut children: Vec<_> = fs::read_dir(dir)
        .expect("reading the assets directory")
        .map(|entry| entry.expect("an asset entry").path())
        .collect();
    children.sort();
    for path in children {
        if path.is_dir() {
            walk(root, &path, found);
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .expect("an asset under the pack root")
            .to_string_lossy()
            .replace('\\', "/");
        if relative == "catalogue.json" {
            continue;
        }
        if let Some(media_type) = media_type_for(&relative) {
            found.push((relative, media_type));
        }
    }
}

fn media_type_for(name: &str) -> Option<&'static str> {
    let dot = name.rfind('.')?;
    match name[dot..].to_ascii_lowercase().as_str() {
        ".jpg" | ".jpeg" => Some("image/jpeg"),
        ".png" => Some("image/png"),
        ".html" | ".htm" => Some("text/html"),
        _ => None,
    }
}

/// Port of `valid`: both files must be packaged and the media type must match
/// the entry's kind.
fn valid(
    entry: &(String, String, String, String, String),
    index: &[(String, &'static str)],
) -> Result<(), ()> {
    let (id, name, kind, source, poster) = entry;
    if id.is_empty() || name.is_empty() || source.is_empty() {
        return Err(());
    }
    let media = |name: &str| {
        index
            .iter()
            .find(|(asset, _)| asset == name)
            .map(|(_, media_type)| *media_type)
    };
    match kind.as_str() {
        "page" if media(source) != Some("text/html") => return Err(()),
        "image" if !media(source).is_some_and(|kind| kind.starts_with("image/")) => return Err(()),
        "page" | "image" => {}
        _ => return Err(()),
    }
    if !poster.is_empty() && !media(poster).is_some_and(|kind| kind.starts_with("image/")) {
        return Err(());
    }
    Ok(())
}
