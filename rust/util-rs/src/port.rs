//! The Rust port of `internal/util/`.
//!
//! `InArray` and `ToJSONString` stayed in Go: the first is generic per type, the
//! second accepts `any`. Both are ported once their callers are ported.

#![forbid(unsafe_code)]

use md5::{Digest, Md5};

/// Port of `Md5` (`internal/util/base.go:12`).
pub fn md5(str: String) -> Result<String, String> {
    let digest = Md5::digest(str.as_bytes());
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Port of `PathExists` (`internal/util/base.go:17`).
pub fn path_exists(path: String) -> Result<bool, String> {
    match std::fs::metadata(&path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("stat {path}: {}", go_io_message(&path, &error))),
    }
}

/// Go reports a `*PathError` with the C library's message, and that text reaches
/// API responses through `err.Error()`. Rust phrases the same failures
/// differently, so the message is rebuilt the way Go prints it.
fn go_io_message(path: &str, error: &std::io::Error) -> String {
    if path.contains('\0') {
        return "invalid argument".to_string();
    }
    let known = match error.raw_os_error() {
        Some(13) => Some("permission denied"),
        Some(20) => Some("not a directory"),
        Some(21) => Some("is a directory"),
        Some(22) => Some("invalid argument"),
        Some(36) => Some("file name too long"),
        Some(40) => Some("too many levels of symbolic links"),
        _ => None,
    };
    match known {
        Some(message) => message.to_string(),
        None => error
            .to_string()
            .split(" (os error")
            .next()
            .unwrap_or_default()
            .to_lowercase(),
    }
}

/// Port of `PasswordEncryption` (`internal/util/base.go:38`).
pub fn password_encryption(password: String) -> Result<String, String> {
    md5(md5(md5(password)?)?)
}

/// Port of `RedirectURL` (`internal/util/base.go:54`).
pub fn redirect_url(root_url: String, provider: String) -> Result<String, String> {
    Ok(format!("{root_url}/api/oauth/{provider}/callback"))
}
