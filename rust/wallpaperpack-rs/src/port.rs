//! The Rust port of `internal/biz/theme/wallpaperpack/`.
//!
//! `Catalogue` and `Find` read a catalogue that `build.rs` validated and
//! generated from the packaged assets, so both are total functions: the errors
//! Go can return at first load become build failures here instead.
//!
//! `Asset` stays in Go until the storage layer is ported.

#![forbid(unsafe_code)]

use std::sync::OnceLock;

use serde_json::{Value, json};

include!(concat!(env!("OUT_DIR"), "/catalogue.rs"));

static CATALOGUE_VALUE: OnceLock<Value> = OnceLock::new();

/// Port of `Catalogue` (`internal/biz/theme/wallpaperpack/pack.go:148`).
pub fn catalogue() -> Value {
    CATALOGUE_VALUE.get_or_init(build_catalogue).clone()
}

/// Built once, like the Go package's `once.Do(load)`. Rebuilding the tree per
/// call cost about 4.9 ms against Go's cached struct return.
fn build_catalogue() -> Value {
    let categories: Vec<Value> = CATALOGUE
        .iter()
        .map(|(id, name, items)| {
            json!({
                "id": id,
                "name": name,
                "items": items.iter().map(entry_value).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "version": VERSION,
        "categories": categories,
    })
}

/// Port of `Find` (`internal/biz/theme/wallpaperpack/pack.go:156`).
pub fn find(id: String) -> Value {
    if id.is_empty() {
        return json!([empty_entry(), false]);
    }
    for (_, _, items) in CATALOGUE {
        for entry in *items {
            if entry.0 == id {
                return json!([entry_value(entry), true]);
            }
        }
    }
    json!([empty_entry(), false])
}

/// One entry in the field order the Go struct marshals.
fn entry_value(entry: &Entry) -> Value {
    json!({
        "id": entry.0,
        "name": entry.1,
        "kind": entry.2,
        "source": entry.3,
        "poster": entry.4,
    })
}

/// What Go returns when `Find` misses: the zero value of `Entry`.
fn empty_entry() -> Value {
    json!({
        "id": "",
        "name": "",
        "kind": "",
        "source": "",
        "poster": "",
    })
}

/// `Asset(name)`: one packaged file and its media type, embedded at build time
/// exactly like Go's `embed.FS`. The name is resolved against the packaged
/// index, so a traversal or an unpackaged path is absent.
pub fn asset(name: String) -> Option<(Vec<u8>, String)> {
    let name = name.trim_start_matches('/').to_string();
    if name.is_empty()
        || name.contains('\\')
        || name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return None;
    }
    ASSETS
        .iter()
        .find(|(asset, _, _)| *asset == name)
        .map(|(_, media_type, content)| (content.to_vec(), media_type.to_string()))
}
