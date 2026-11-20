# Benchmarks: Go backend vs the Rust port

The first four sections compare Go with the **front door** while routes were
still being ported. Section 5 is the current one: **two standalone services**,
Go and pure Rust, on the same host, database and frontend.

Measured on 2026-11-20, Intel Xeon D-1581 @ 1.80 GHz (28 threads), Linux,
loopback HTTP, warm caches. Numbers are one run each; treat ±10% as noise.

Reproduce with:

```sh
# unit level: recorded cases, crossing included
rust-anything bench rust/util-rs
rust-anything bench rust/wallpaperpack-rs

# route level: start the front door, then a keep-alive load generator
PORT=3020 ./rust/app/target/release/run-original
./loadgen -url http://127.0.0.1:3020/api/theme/v2/wallpaper/library -n 4000 -c 16
./loadgen -url http://127.0.0.1:3002/api/theme/v2/wallpaper/library -n 4000 -c 16
```

The load generator is a small Go program (`net/http`, keep-alive,
`Accept-Encoding: identity`, 4000 requests, 16 connections) that also samples
the serving process's `/proc/<pid>/stat` CPU time and RSS.

## 1. Unit level: `rust-anything bench`

"Rust ns/call" includes the crossing (cgo call plus the generated binary wire),
which is how the Go backend will call these units until each one is ported with
its callers.

| Unit | Function | Go ns/call | Rust ns/call | Speedup |
| --- | --- | ---: | ---: | ---: |
| `internal/util` | `Md5` | 277 | 2,084 | 0.13x |
| | `PathExists` | 2,672 | 2,438 | 1.10x |
| | `PasswordEncryption` | 758 | 5,116 | 0.15x |
| | `RedirectURL` | 88 | 660 | 0.13x |
| `wallpaperpack` | `Catalogue` | 49 | 1,333,780 | 0.00x |
| | `Find` | 1,699 | 2,618 | 0.65x |

Verdict from the tool: do not ship these units for speed. A per-call bridge
crossing costs more than the function does, and Go returns `Catalogue` as a
struct copy (slice headers, no allocation) where Rust must materialise a JSON
tree. `Catalogue` was 4.83 ms/call before the port cached the built tree; it is
1.33 ms/call now, which is still dominated by cloning the value for the wire.
These units belong with their callers, not behind a bridge.

## 2. Route level: `GET /api/theme/v2/wallpaper/library`

94,430-byte JSON body, byte-identical between the Go service and the Rust
route (compared with `cmp`). The Rust route answers directly; it does not proxy.

| Build | `c` | RPS | p50 | p99 | CPU/req | Cores | RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Go (as shipped) | 1 | 391 | 2.28 ms | 4.40 ms | 3,940 µs | 1.54 | 61 MiB |
| Rust, as ported | 1 | 279 | 3.52 ms | 4.15 ms | 3,562 µs | 0.98 | 9 MiB |
| Rust, cached body | 1 | 3,077 | 0.27 ms | 0.69 ms | 80 µs | 0.25 | 9 MiB |
| Go (as shipped) | 16 | 2,145 | 7.16 ms | 13.15 ms | 5,667 µs | 12.2 | 69 MiB |
| Rust, as ported | 16 | 2,396 | 6.36 ms | 11.95 ms | 5,552 µs | 13.3 | 36 MiB |
| Rust, cached body | 16 | 3,861 | 3.73 ms | 8.93 ms | 109 µs | 0.42 | 10 MiB |

Same-work comparison (both rebuild the response per request, `c=16`): Rust is
about 12% ahead on throughput and latency with the same CPU per request and
roughly half the resident memory. Single-request latency favours Go.

The `as ported` row is a single 600-request run of the build before the body
cache; the other rows are 1,000 (`c=1`) and 4,000 (`c=16`) request runs.

The cached-body row is not a language result: the payload is fully determined by
the catalogue compiled into the binary, so the Rust handler serialises it once.
Go's handler builds a `gin.H` tree and JSON-encodes 94 KB on every request; it
could cache the same bytes. On the cached route the load generator becomes the
bottleneck (Rust used 0.42 of 28 cores while pushing 348 MiB/s), so 3,861 RPS is
a floor, not a ceiling.

## 3. Cost of the front door (proxied route)

`GET /api/about`, `c=16`, 4,000 requests.

| Path | RPS | p50 | p99 | CPU/req (total) |
| --- | ---: | ---: | ---: | ---: |
| Go directly | 32,064 | 0.34 ms | 1.89 ms | 205 µs |
| Through the Rust front door | 23,596 | 0.55 ms | 1.91 ms | 395 µs (185 + 210 in the Go child) |

The front door costs about 26% throughput, +0.21 ms p50, and roughly doubles CPU
per request while a route is still proxied. That is the price of the incremental
architecture and it disappears per route as routes move to Rust; it is not a
reason to skip it, but it does mean a fully proxied deployment is slower than
the Go-only one today.

## 4. Startup, memory and size

Same configuration on both sides (monitor and static server off, empty SQLite):

| | Boot to first 200 | Idle RSS |
| --- | ---: | ---: |
| Go, standalone | 87 ms | 53 MiB |
| Rust front door (spawns that Go process) | 124 ms | 9 MiB + 51 MiB child |

With the production `conf.yaml` (`enable_monitor: true`), Go's boot is 1,059 ms;
the monitor's first snapshot dominates and is unrelated to language. Two boots
against an already migrated database were 1,063 ms, so it is not the migration.

| Artifact | Size |
| --- | ---: |
| Go binary (local build) | 177.9 MiB |
| Rust front door | 3.6 MiB |
| `libwallpaperpack_rs.so` | 0.7 MiB |

A pure Rust service at this stage would be about 9 MiB resident against Go's
53 MiB; today the front door adds its 9 MiB on top of the Go process it
supervises.

## Caveats

- `Accept-Encoding: identity` on both sides. The Go service gzips API responses;
  the front door has no compression layer yet, so a gzip comparison would charge
  Go CPU for bandwidth it actually saves.
- The 94,430-byte body is JSON of a static catalogue; it is the most favourable
  possible shape for the cached-route result.
- The unit bench measures the bridge, not the Rust code: it is evidence about
  the migration boundary, not about Rust.
- One run per configuration on a shared machine; use the commands above to
  re-measure on the target hardware before quoting these numbers elsewhere.

## 5. Pure Rust vs Go, service to service (2026-11-20)

Both binaries run standalone, one process each, on the same host: Intel Xeon
D-1581 @ 1.80 GHz (28 threads), SQLite on local disk, the same `backend/web`
build, and two `VACUUM`-equivalent snapshots of the same database (3 accounts,
5 items, 17 groups, 12 Spaces). `enable_monitor: false`,
`enable_static_server: true`, identical `conf.yaml` apart from the port.

Load: `rust/bench/loadgen` (keep-alive, `Accept-Encoding: identity`), reporting
client-side RPS and latency plus the **server's** own CPU time and RSS read from
`/proc/<pid>`. `c=1` is latency, `c=32` is throughput. One run per cell; treat
±10% as noise.

```sh
go build -o /tmp/loadgen ./rust/bench/loadgen
/tmp/loadgen -url http://127.0.0.1:3010/api/about -n 8000 -c 32 -pid <go pid>
```

### Latency, one connection

| Endpoint | Go RPS | p50 | p99 | CPU/req | Rust RPS | p50 | p99 | CPU/req | Rust vs Go |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `GET /api/about` (no DB) | 4,828 | 0.19 ms | 0.42 ms | 193 µs | 7,032 | 0.13 ms | 0.23 ms | 60 µs | **1.46x** |
| `GET /` (index.html) | 4,015 | 0.23 ms | 0.59 ms | 300 µs | 5,548 | 0.17 ms | 0.26 ms | 87 µs | **1.38x** |
| `GET /assets/*.js` | 3,570 | 0.25 ms | 0.73 ms | 327 µs | 5,664 | 0.17 ms | 0.28 ms | 87 µs | **1.59x** |
| `GET items (DB)` | 1,313 | 0.75 ms | 1.44 ms | 850 µs | 1,331 | 0.75 ms | 0.93 ms | 692 µs | 1.01x |
| `GET theme/current` | 1,039 | 0.91 ms | 1.87 ms | 1,233 µs | 864 | 1.15 ms | 1.38 ms | 1,058 µs | 0.83x |
| `GET theme asset (93 KB)` | 1,704 | 0.54 ms | 1.31 ms | 1,025 µs | 1,654 | 0.59 ms | 0.74 ms | 450 µs | 0.97x |
| `GET wallpaper library` | 405 | 2.17 ms | 4.39 ms | 3,950 µs | 5,074 | 0.19 ms | 0.28 ms | 75 µs | **12.5x** |
| `POST /api/login` | 1,721 | 0.56 ms | 1.27 ms | 675 µs | 2,334 | 0.43 ms | 0.54 ms | 325 µs | **1.36x** |
| `GET /api/spaces` | 1,143 | 0.86 ms | 1.45 ms | 953 µs | 1,210 | 0.86 ms | 1.03 ms | 827 µs | 1.06x |

### Throughput, 32 connections

| Endpoint | Go RPS | p50 | p99 | CPU/req | Rust RPS | p50 | p99 | CPU/req | Rust vs Go |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `GET /api/about` | 23,002 | 1.27 ms | 4.31 ms | 280 µs | 24,869 | 1.14 ms | 4.88 ms | 71 µs | 1.08x |
| `GET /` | 19,384 | 1.55 ms | 5.36 ms | 434 µs | 22,174 | 1.30 ms | 5.10 ms | 110 µs | 1.14x |
| `GET /assets/*.js` | 19,834 | 1.54 ms | 4.65 ms | 432 µs | 26,299 | 1.13 ms | 3.67 ms | 106 µs | **1.33x** |
| `GET items (DB)` | 7,102 | 4.07 ms | 11.69 ms | 883 µs | 7,249 | 4.37 ms | 5.36 ms | 597 µs | 1.02x |
| `GET theme/current` | 5,330 | 5.72 ms | 12.34 ms | 1,323 µs | 3,764 | 8.22 ms | 14.96 ms | 1,065 µs | 0.71x |
| `GET theme asset (93 KB)` | 8,705 | 2.97 ms | 13.36 ms | 888 µs | 8,446 | 3.77 ms | 4.74 ms | 480 µs | 0.97x |
| `GET wallpaper library` | 3,161 | 9.30 ms | 22.90 ms | 5,827 µs | 21,643 | 1.44 ms | 4.41 ms | 107 µs | **6.8x** |
| `POST /api/login` (c=8) | 9,662 | 0.56 ms | 3.22 ms | 608 µs | 20,447 | 0.35 ms | 1.64 ms | 233 µs | **2.1x** |
| `GET /api/spaces` | 5,866 | 5.11 ms | 12.24 ms | 989 µs | 5,508 | 5.75 ms | 6.89 ms | 726 µs | 0.94x |

### Startup, memory, size

| | Go | Rust |
| --- | ---: | ---: |
| Boot to first `200` (monitor off, 3 runs) | 62 ms | 39 ms |
| Idle RSS | 59.6 MiB | 13–17 MiB |
| RSS after the heavy runs above | 94 MiB | 67 MiB |
| Threads | 35 | 33 |
| Binary | 177.9 MiB | 147 MiB |

The Rust resident set grows from ~15 MiB to ~67 MiB once SQLite has cached pages;
Go starts at ~60 MiB and peaks near 94 MiB on the same work.

### What the numbers mean

- **Where Rust wins clearly**: the embedded wallpaper catalogue (the response is
  built once, so Go re-encodes 94 KB per request), login (MD5×3 + JWT), static
  files, and the tiny-JSON route. Per request Rust costs 2–5x less CPU.
- **Where they are level**: the SQLite-backed reads. Both are dominated by the
  database; `/api/spaces` and `items` land within a few percent.
- **Where Go still wins**: `GET /api/theme/v2/current`, by ~30% at `c=32`.
  Rust parses the 7.5 KB manifest and 6 KB token document out of SQLite and
  re-serialises them per request through a 4-connection pool; Go unmarshals into
  structs and marshals back on a single serialised connection. It is the same
  work, and Rust uses *less* CPU per request (1,065 µs vs 1,323 µs) but queues
  more under load.

### Caveats, stated rather than hidden

- **Go logs every request** (`[GIN] ... | 4.24ms | GET "/api/spaces"`) to stdout;
  the Rust build logs none. That alone costs Go CPU and latency per request, and
  is a feature difference, not a language one.
- Go runs the gin middleware chain (security headers, `no-store`, gzip
  negotiation, recovery) on every request; Rust sets the headers directly. The
  gzip middleware was verified not to compress with `Accept-Encoding: identity`.
- The wallpaper-library result is the most favourable shape possible for Rust
  (a cached constant body). Go could cache it too; it does not today.
- Two processes on one shared machine, warm page cache, one run per cell.
- `/api/spaces` was measured **after** fixing an N+1 in the Rust handler (two
  extra queries per Space); before the fix it was 744 RPS at `c=1` and 3,852 at
  `c=32`. Go always batched, and its comment says so.
