//! The Rust port of `internal/biz/theme/wallpaperpack/`.
//!
//! `Catalogue` and `Find` read a catalogue that `build.rs` validated and
//! generated from the packaged assets, so both are total functions: the errors
//! Go can return at first load become build failures here instead.
//!
//! `Asset` stays in Go until the storage layer is ported.

#![forbid(unsafe_code)]

use serde_json::{Value, json};

include!(concat!(env!("OUT_DIR"), "/catalogue.rs"));

/// Port of `Catalogue` (`internal/biz/theme/wallpaperpack/pack.go:148`).
pub fn catalogue() -> Value {
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
