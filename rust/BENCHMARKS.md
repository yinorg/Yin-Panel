# Benchmarks: Go backend vs the Rust port

The first four sections compare Go with the **front door** while routes were
still being ported. Section 5 was the first standalone comparison. Section 6
supersedes it with repeated, alternating runs and a load generator that rejects
error responses.

Measured on 2026-11-20, Intel Xeon D-1581 @ 1.80 GHz (28 threads), Linux,
loopback HTTP, warm caches. Numbers are one run each; treat ±10% as noise.

The Go backend has since been removed, so the pair scripts that compared the
two builds no longer exist; these numbers are the historical record.

## The baseline that still runs

The Go-versus-Rust pair scripts are gone with the Go tree. The live baseline is
`.github/workflows/perf.yml`: it builds the Rust binary, starts the seeded
service, and runs the E2E repository's k6 scripts, whose thresholds are the
gate (`checks > 0.99`, `http_req_failed < 0.01`, smoke p95 < 500 ms / p99 <
1 s, load p95 < 800 ms / p99 < 1.5 s). It runs nightly and on demand, so a
performance regression is visible even though the comparison arm is gone.

Historical reproduction, when a Go build is still available:

```sh
# unit level: recorded cases, crossing included
rust-anything bench rust/util-rs
rust-anything bench rust/wallpaperpack-rs

# route level: start the front door, then a keep-alive load generator
PORT=3020 ./rust/app/target/release/run-original
./loadgen -url http://127.0.0.1:3020/api/theme/v2/wallpaper/library -n 4000 -c 16
```

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
- **This section disabled Go's read pool** (`sqlite.read_pool_size: -1`). The
  shipped `conf.yaml` leaves the key unset, which enables four read-only
  connections. Section 6 runs both builds with their real configuration, and
  there the `theme/current` gap disappears.

## 6. Repeated, alternating measurement (2026-11-20, M0)

Section 5 used one run per cell, a load generator that counted an HTTP 200 with
a business error as a success, and a Go build whose SQLite read pool was
disabled. Every one of those understated Go or overstated Rust. This section is
the current baseline.

Method:

- one Go and one Rust process, started by `rust/bench/pair.sh` from the **same
  VACUUM-safe database snapshot**, the same `backend/conf.yaml`, the same
  `web/`, each on its own port;
- `enable_monitor: false` on both, so this measures the request path; the
  monitor has its own measurement below;
- `rust/bench/loadgen` counts a request only when the transport, the complete
  body, the status and the JSON envelope `code` all pass; a business error is a
  failure and the run exits non-zero;
- five repetitions per side per endpoint, alternating which side runs first, so
  a drift on the host hits both;
- `rust/bench/compare.sh` reports the median and the observed range. The range
  matters: the machine is shared and this is not a lab.

Reproduce (the pair refuses to start on a busy port and only stops the pids it
started):

```sh
rust/bench/pair.sh up --work /tmp/yin-panel-pair \
    --source-db backend/database/database.db --go-port 3010 --rust-port 3011
rust/bench/compare.sh --go http://127.0.0.1:3010 --rust http://127.0.0.1:3011 \
    --reps 5 --requests 1500 --concurrency 1
rust/bench/compare.sh --go http://127.0.0.1:3010 --rust http://127.0.0.1:3011 \
    --reps 5 --requests 3000 --concurrency 32
rust/bench/pair.sh down --work /tmp/yin-panel-pair
```

### One connection (latency)

| Endpoint | Side | OK/fail | RPS median | RPS range | p50 | p95 | p99 | CPU/req |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `/api/about` | Go | 7,500/0 | 4,650 | 2,880–4,950 | 0.20 ms | 0.28 ms | 0.34 ms | 180 µs |
| | Rust | 7,500/0 | **5,777** | 5,380–5,980 | 0.17 ms | 0.21 ms | 0.26 ms | **60 µs** |
| `/` | Go | 7,500/0 | 3,687 | 3,563–3,759 | 0.25 ms | 0.43 ms | 0.66 ms | 280 µs |
| | Rust | 7,500/0 | **4,865** | 4,747–4,964 | 0.19 ms | 0.28 ms | 0.46 ms | **80 µs** |
| `theme/v2/current` | Go | 7,500/0 | **852** | 822–860 | 1.12 ms | 1.58 ms | 1.98 ms | 1,107 µs |
| | Rust | 7,500/0 | 678 | 647–687 | 1.44 ms | 1.67 ms | 2.02 ms | 1,107 µs |
| `/api/spaces` | Go | 7,500/0 | **1,080** | 1,063–1,106 | 0.90 ms | 1.08 ms | 1.52 ms | 980 µs |
| | Rust | 7,500/0 | 1,035 | 951–1,051 | 0.96 ms | 1.08 ms | 1.20 ms | 913 µs |

### 32 connections (throughput)

| Endpoint | Side | OK/fail | RPS median | RPS range | p50 | p95 | p99 | CPU/req |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `/api/about` | Go | 15,000/0 | 23,764 | 23,295–24,662 | 1.19 ms | 3.02 ms | 4.36 ms | 213 µs |
| | Rust | 15,000/0 | **26,203** | 24,900–27,679 | 1.15 ms | 2.85 ms | 4.33 ms | **70 µs** |
| `/` | Go | 15,000/0 | 7,632 | 7,366–7,751 | 3.86 ms | 8.10 ms | 10.51 ms | 450 µs |
| | Rust | 15,000/0 | **8,233** | 8,180–8,443 | 3.57 ms | 7.75 ms | 10.28 ms | **117 µs** |
| `theme/v2/current` | Go | 15,000/0 | 3,673 | 3,407–3,689 | 8.15 ms | 16.08 ms | 20.03 ms | 1,567 µs |
| | Rust | 15,000/0 | 3,554 | 3,417–3,594 | 8.97 ms | 10.21 ms | 12.28 ms | **1,120 µs** |
| `/api/spaces` | Go | 15,000/0 | **5,836** | 4,165–5,883 | 5.13 ms | 9.56 ms | 12.10 ms | 993 µs |
| | Rust | 15,000/0 | 5,328 | 5,254–5,431 | 5.94 ms | 6.63 ms | 7.20 ms | 750 µs |

### What changed from section 5

- `theme/v2/current` is **level**, not "Go wins by 30%". With Go's read pool
  enabled it reaches ~3,670 RPS at `c=32` against Rust's ~3,550, and at `c=1`
  the two use exactly the same CPU per request (1,107 µs). The remaining
  difference is latency, not computation, and the earlier pool-queueing
  explanation is not supported by this measurement.
- `/api/spaces` is Rust ~0.91x at `c=32` but with a tighter tail (p99 7.2 ms vs
  Go's 12.1 ms) and less CPU per request. Go's range at `c=32` (4,165–5,883)
  shows the host noise; a single run could report either side as the winner.
- The absolute RPS of `/` is lower than section 5 for both builds because this
  pair runs beside the live service and a second listener on a disk with under
  3 GiB free. Compare the two sides, not the absolute numbers.

### Monitor snapshot, measured separately

The live deployment ran with `enable_monitor: true`. Before the change every
`POST /api/system/monitor/getSnapshot` waited for a one-second CPU sample:

```
200 1.003714s
200 1.003670s
200 1.003717s
```

After serving the snapshot from a three-second background collection, the same
call on the same deployment:

```
200 0.001022s
200 0.000775s
200 0.000716s
```

A collection still takes about a second, as Go's `cpu.Percent(time.Second, true)`
does, but it happens once before the listener opens and then off the request
path, which is exactly what `internal/biz/cache/monitor.go` does.

### Post-optimisation cross-check

The runs above were taken before the compression and range work. Re-running
`compare.sh` with the same pair after those changes (5 repetitions, `c=32`,
3,000 requests) keeps every endpoint in the same band:

| Endpoint | Go RPS | Rust RPS | Rust CPU/req | Go CPU/req |
| --- | ---: | ---: | ---: | ---: |
| `/api/about` | 22,937 | 25,963 | 77 µs | 293 µs |
| `/` | 7,747 | 8,096 | 127 µs | 457 µs |
| `theme/v2/current` | 3,660 | 3,569 | 1,123 µs | 1,520 µs |
| `/api/spaces` | 5,649 | 5,063 | 793 µs | 1,023 µs |

`/api/gzip` now costs Rust CPU on compressed routes and saves the client
90% of the bytes; the figures above are measured **without** `Accept-Encoding:
gzip`, so they isolate the uncompressed path. `/` moved from 117 µs to 127 µs
per request because the file is stat'ed to decide whether to read it inline or
off the async workers; that stat is what keeps a 500 KB asset from blocking a
runtime thread, and the small-file path stays inline because offloading a
13 KB file measured 66 µs worse with no p99 gain.


### Gzip CPU/size trade-off

The default Rust zlib encoder was slower than Go's pooled writer on JSON. Tested
with the same 3-repetition, c=32 method and `Accept-Encoding: gzip`, Rust's
`Compression::fast()` changes:

| Endpoint | Go RPS | Rust RPS | Go gzip bytes | Rust gzip bytes | Rust CPU/req | Go CPU/req |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `/api/about` | 10,862 | 10,997 | 123 | 104 | 160 µs | 240 µs |
| `theme/v2/current` | 3,219 | 2,930 | 1,546 | 1,827 | 1,395 µs | 1,305 µs |
| wallpaper library | 1,260 | 1,927 | 8,530 | 10,273 | 630 µs | 6,540 µs |

The level-1 trade is now active: it preserves decoded bytes and headers, brings
the small JSON route to throughput parity, and lets the large immutable
catalogue use 79% less CPU per request than Go, at 1.7 KB more wire bytes. The
wallpaper body is still 89% smaller than its 94,430-byte uncompressed form.
`theme/current` remains a known gzip CPU/latency gap; the one-query join reduced
its DB round trips but did not move the c=32 median outside the noise. Caching
it is not yet justified: CPU at c=1 is close to Go on identity encoding, while
compression, not repeated query count, is the largest measured c=32 cost.

### Release-gate baseline (2026-11-21)

The consolidated `scripts/verify-rust-release.sh` run uses the same database
snapshot, five alternating repetitions, and validates every response.
Identity-encoded median RPS from the clean integrated run:

| Endpoint | c=1 Go | c=1 Rust | c=32 Go | c=32 Rust | Rust/Go at c=32 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `/api/about` | 4,612 | 5,970 | 24,890 | 25,729 | 1.03x |
| `/` | 3,300 | 4,378 | 7,392 | 7,903 | 1.07x |
| `theme/v2/current` | 842 | 675 | 3,639 | 3,532 | 0.97x |
| `/api/spaces` | 1,070 | 1,054 | 5,834 | 5,366 | 0.92x |

The one-query `theme/current` path preserves its response in parity checks but
does not improve the high-concurrency median beyond measurement noise. The
remaining `/api/spaces` throughput gap is paired with lower Rust CPU/request
and p99 (753 µs and 7.02 ms versus Go's 997 µs and 11.83 ms). No additional
cache or pool tuning is justified by this run alone.
