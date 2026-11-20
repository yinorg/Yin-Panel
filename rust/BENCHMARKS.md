# Benchmarks: Go backend vs the Rust port

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
