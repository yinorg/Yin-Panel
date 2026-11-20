//! The Rust port of `internal/web/model/response/`.
//!
//! Only `GetErrorMsgByCode` is bridgeable: every other function takes a
//! `*gin.Context`, so it stays in Go. The map is the source of truth for the
//! codes the HTTP layer sends.

#![forbid(unsafe_code)]

/// `ErrorCodeMap` in `ErrorCode.go`.
const ERROR_CODES: &[(i64, &str)] = &[
    (1000, "Not logged in yet"),
    (1003, "Incorrect username or password"),
    (1004, "Account disabled or not activated"),
    (1005, "No current permission for operation"),
    (1006, "Account does not exist"),
    (1007, "Old password error"),
    (1200, "Database error"),
    (1201, "Please keep at least one"),
    (1202, "No data record found"),
    (1300, "Upload failed"),
    (1301, "Unsupported file format"),
    (1400, "Parameter format error"),
];

/// Port of `GetErrorMsgByCode` (`internal/web/model/response/apiReturn.go:83`).
///
/// Answers `[message, found]`: an unknown code is not an error, so the port
/// returns `Ok` with `false` rather than `Err`.
pub fn get_error_msg_by_code(code: i64) -> Result<serde_json::Value, String> {
    match ERROR_CODES.iter().find(|(value, _)| *value == code) {
        Some((_, message)) => Ok(serde_json::json!([message, true])),
        None => Ok(serde_json::json!(["", false])),
    }
}
