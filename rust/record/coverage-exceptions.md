# Recording coverage exceptions

The following registered routes are public read-only projections. Their
responses do not depend on caller-controlled request data, so an invalid cookie
does not produce a meaningful route-level failure. Infrastructure failures
(for example, an unavailable database) are not recorded as request outcomes.

| Method | Route | Evidence |
| --- | --- | --- |
| GET | `/api/about` | `system/about.go` returns the build/runtime information without authentication. |
| GET | `/api/oauth/config` | `system/oauth.go` returns public OAuth configuration without authentication. |
| GET | `/api/system/capabilities` | `system/capabilities.go` returns the static capability projection without authentication. |
| GET | `/api/theme/v2/current` | `system/theme.go` resolves the current public theme; an invalid session does not change the response contract. |
| GET | `/api/theme/v2/effective` | `system/theme.go` resolves the public effective theme; an invalid session does not change the response contract. |
| GET | `/api/theme/v2/packages` | `system/theme.go` lists publicly available theme packages. |
| GET | `/api/theme/v2/wallpaper/library` | `system/theme.go` returns the packaged public wallpaper catalogue. |

These routes remain covered by successful replay cases. This exception does not
apply to asset routes, where missing content is a meaningful 404, or to
authenticated admin/read routes.
