# Port map

Every unit `rust-anything analyze backend --lang go --whole-app` lists, in port
order. State is updated at each checkpoint; `HANDOFF.md` says what is next.

Regenerate the plan with:

```sh
rust-anything analyze backend --lang go --whole-app
```

| Wave | Unit | LOC | Readiness | Status | Port state |
| ---: | --- | ---: | ---: | --- | --- |
| 0 | `internal/util/monitor/` | 507 | 36 | caution | todo |
| 0 | `internal/biz/theme/wallpaperpack/` | 179 | 79 | ready | ported: diff 212/212, route wired (gate: error-cases exception) |
| 0 | `pkg/storage/` | 127 | 64 | ready | todo |
| 0 | `internal/infra/kvcache/` | 89 | 69 | caution | todo |
| 0 | `internal/web/model/base/` | 73 | 68 | caution | todo |
| 0 | `internal/util/` | 47 | 78 | ready | ported: gate passed, 868/868 (`InArray`/`ToJSONString` kept in Go) |
| 0 | `internal/infra/zaplog/` | 46 | 81 | ready | todo |
| 0 | `internal/web/model/param/commonApi/` | 25 | 100 | ready | todo |
| 0 | `internal/constant/` | 14 | 99 | ready | todo |
| 0 | `internal/web/model/param/systemApi/` | 4 | 100 | ready | todo |
| 1 | `internal/biz/repository/` | 705 | 4 | caution | todo |
| 1 | `internal/biz/cache/` | 150 | 63 | ready | todo |
| 1 | `internal/infra/config/` | 140 | 78 | caution | todo |
| 1 | `pkg/extension/` | 97 | 78 | ready | frozen (ADR-002) |
| 1 | `internal/util/i18n/` | 50 | 87 | ready | todo |
| 2 | `internal/biz/service/` | 811 | 3 | caution | todo |
| 2 | `internal/web/model/response/` | 104 | 70 | ready | todo |
| 2 | `internal/util/jwt/` | 70 | 70 | ready | todo |
| 2 | `internal/web/model/param/panelApi/` | 20 | 100 | ready | todo |
| 3 | `internal/global/` | 40 | 87 | ready | todo |
| 4 | `internal/biz/theme/` | 2507 | 0 | caution | todo |
| 4 | `internal/web/interceptor/` | 235 | 68 | ready | todo |
| 4 | `internal/util/favicon/` | 134 | 73 | caution | todo |
| 5 | `internal/web/router/panel/` | 2411 | 0 | caution | todo |
| 5 | `internal/web/router/system/` | 1874 | 0 | caution | todo |
| 5 | `internal/infra/database/` | 1806 | 0 | caution | todo |
| 6 | `internal/web/router/` | 243 | 49 | caution | todo |
| 7 | `pkg/app/` | 194 | 75 | ready | todo |
| 8 | `main.go/` | 61 | 95 | ready | todo |

## App-level state

- Original recorded: 299 HTTP cases in `rust/app-cases/` (35 named inputs, 264
  traps), `299/299` passing against the Go baseline.
- Front door: `rust/app` (axum), `299/299`, fallback `99.7%` (298/299).
- Recorded cases: 346 (82 named inputs + 264 traps); original 346/346, front
  door 346/346.
- Rust answers directly (43 route registrations):
  - `GET /api/theme/v2/wallpaper/library` -> `rust/wallpaperpack-rs`
  - `GET /api/about`, `GET /api/system/capabilities`, `GET /api/oauth/config`,
    `POST /api/login`, `POST /api/logout`, `GET /api/user/getUser`,
    `GET /api/spaces`, `GET /api/spaces/{id}/search-config`,
    `GET /api/spaces/{id}/groups`, `GET /api/spaces/{id}/items`,
    `GET /api/spaces/{id}/members`, `GET /api/spaces/{id}/public`,
    `GET /api/system/moduleConfig/getByName`, `GET /api/file/getList`,
    `GET /api/spaces/{id}/bookmarks/tree`, `GET /api/panel/users/getList`,
    `GET /api/panel/userConfig/getConfig`, the monitor status readers,
    `GET /api/theme/v2/preference`, `GET /api/theme/v2/space/{id}`,
    the theme reads (`current`, `packages`, `package/{id}`, `mine`, `effective`)
    -> `rust/app`
- Ported library units not yet wired to a route: `rust/util-rs`.
- Fallback rate: 73.4% (254/346). Everything else is still proxied to Go.
