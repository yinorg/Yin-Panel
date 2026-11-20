# Glossary

Go terms in this backend and the Rust names that replace them. Add an entry
when a port introduces a name that is not an obvious translation.

| Go | Rust | Note |
| --- | --- | --- |
| `gin.RouterGroup` | `axum::Router` | routes are registered per unit in `rust/app/src/units/mod.rs` |
| `gin.HandlerFunc` | `axum` layer / `async fn` handler | one layer per Gin middleware |
| `gin.Context` | `axum::extract::{Request, State}` | `State` carries the original for the proxy |
| `c.JSON(status, body)` | `(StatusCode, Json(body))` | header order and content type are set explicitly |
| `c.ShouldBindJSON` | `Json<T>` extractor | rejection body differs; covered by recorded cases |
| `response.SuccessData` | `{"code":0,"data":…,"msg":"OK"}` | key order is sorted, like Go maps |
| `gorm.DB` | `sqlx::Pg/MySql/SqlitePool` | ADR-003 |
| `gorm.ErrRecordNotFound` | `sqlx::Error::RowNotFound` | |
| `zap.Logger` | `tracing` | |
| `go-cache` | `moka` | |
| `gopsutil` | `sysinfo` | |
| `goquery` | `scraper` | favicon unit |
| `santhosh-tekuri/jsonschema` | `jsonschema` | theme manifest validation |
| `embed.FS` | `include_bytes!` / generated statics | wallpaper pack validates in `build.rs` |
| `wallpaperpack.Pack` | `(&str, &str, &[Entry])` statics | `rust/wallpaperpack-rs/build.rs` |
| `wallpaperpack.Entry` | 5-tuple `Entry` | same field order as the Go struct |
| `once.Do(load)` | `build.rs` (compile time) | ADR-006 |
| `i18n.LangInit` | (not ported yet) | Go exits the process on a missing file |
| fallback | proxy to the Go binary | counted in `rust/app/src/original.rs` |
