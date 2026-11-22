//! The Rust port of `pkg/storage/`.
//!
//! `Local` is a struct with methods, so the bridge keeps it in Go; the content
//! addressing rule is a plain function and is ported here.

#![forbid(unsafe_code)]

/// Port of `ContentAddressedKey` (`pkg/storage/key.go:10`).
///
/// The digest is lower-cased and trimmed, the extension gains its dot, and the
/// key is `<aa>/<bb>/<digest><ext>` so one directory never holds every object.
pub fn content_addressed_key(hex_digest: String, extension: String) -> Result<String, String> {
    let digest = hex_digest.trim().to_lowercase();
    if digest.chars().count() != 64 {
        return Err("SHA-256 digest must contain 64 hexadecimal characters".to_string());
    }
    if !digest
        .chars()
        .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
    {
        return Err("SHA-256 digest must be hexadecimal".to_string());
    }
    let mut extension = extension.trim().to_lowercase();
    if !extension.is_empty() && !extension.starts_with('.') {
        extension = format!(".{extension}");
    }
    if extension.contains('/') || extension.contains('\\') {
        return Err("invalid file extension".to_string());
    }
    Ok(format!(
        "{}/{}/{}{}",
        &digest[0..2],
        &digest[2..4],
        digest,
        extension
    ))
}
