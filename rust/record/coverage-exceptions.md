# Recording coverage exceptions

The following registered routes are public read-only projections. Their
responses do not depend on caller-controlled request data, so an invalid cookie
does not produce a meaningful route-level failure. Infrastructure failures
(for example, an unavailable database) are not recorded as request outcomes.

| Method | Route | Evidence |
| --- | --- | --- |
| GET | `/api/about` | `system/about.go` returns the build/runtime information without authentication. |
| GET | `/api/oauth/config` | `system/oauth.go` returns public OAuth configuration without authentication. |
| GET | `/api/oauth/:provider` | `system/oauth.go` registers this only when `oauth.enable` is true and redirects to the provider; the authorization URL carries runtime-generated `state`, `nonce` and PKCE values that a fixed recorded request cannot reproduce. |
| GET | `/api/oauth/:provider/callback` | Same router condition; success requires an identity provider to exchange a real authorization code, so no static case can produce the redirect and session cookie. |
| GET | `/api/system/capabilities` | `system/capabilities.go` returns the static capability projection without authentication. |
| GET | `/api/theme/v2/current` | `system/theme.go` resolves the current public theme; an invalid session does not change the response contract. |
| GET | `/api/theme/v2/effective` | `system/theme.go` resolves the public effective theme; an invalid session does not change the response contract. |
| GET | `/api/theme/v2/packages` | `system/theme.go` lists publicly available theme packages. |
| GET | `/api/theme/v2/wallpaper/library` | `system/theme.go` returns the packaged public wallpaper catalogue. |

These routes remain covered by successful replay cases. This exception does not
apply to asset routes, where missing content is a meaningful 404, or to
authenticated admin/read routes.

## Runtime-dependent success paths

These routes have a recorded failure case but no static success case, because a
successful response cannot be produced from a fixed recorded request: the value
it needs is generated at runtime. A dedicated sequence script compares the Go
build with the Rust binary live instead.

| Method | Route | Evidence |
| --- | --- | --- |
| GET | `/api/theme/v2/preview/:token` | The token is 24 random bytes minted by the preview upload; `rust/record/replay-preview-sequence.py` uploads a package, then compares the token GET and asset GET between Go and Rust. |
| GET | `/api/theme/v2/preview/:token/assets/*name` | Same sequence script; it reports `asset bytes=… content-type=…` for both builds. |
| POST | `/api/system/monitor/getSnapshot` | Success needs `enable_monitor: true`; `rust/record/replay-monitor-sequence.py` boots both builds with monitoring enabled and compares the response shape. |
| POST | `/api/system/monitor/getDiskMountpoints` | Same monitor sequence; it reports the mountpoint entry count for both builds. |
| POST | `/api/system/monitor/getDiskStateByPath` | Same monitor sequence; it reports the disk-state entry count for both builds. |
