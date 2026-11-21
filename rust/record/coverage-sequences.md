# Stateful route coverage

Some routes require state returned by an earlier request or an alternate
recorder-only config and therefore cannot be represented by the default
independent request rows in `inputs.jsonl`.

| Method | Route | Success evidence | Failure evidence |
| --- | --- | --- | --- |
| GET | `/api/theme/v2/preview/:token` | `python3 rust/record/replay-preview-sequence.py` uploads a valid package to each implementation and follows that implementation's returned token. | `b0-get-api-theme-v2-preview-token-failure-2` |
| GET | `/api/theme/v2/preview/:token/assets/*name` | The same sequence reads a declared package resource using the returned token. | `b0-get-api-theme-v2-preview-token-assets-name-failure-2` |
| POST | `/api/system/monitor/getDiskMountpoints` | `python3 rust/record/replay-monitor-sequence.py` runs both servers with monitoring enabled and compares the partition list exactly. | `b0-post-api-system-monitor-getdiskmountpoints-failure-2` |
| POST | `/api/system/monitor/getDiskStateByPath` | The same enabled-monitor sequence compares the `/` disk usage response exactly. | `b0-post-api-system-monitor-getdiskstatebypath-failure-2` |
| POST | `/api/system/monitor/getSnapshot` | The same enabled-monitor sequence compares snapshot keys, field types and stable CPU model; sampled numeric metrics are time-varying. | `b0-post-api-system-monitor-getsnapshot-success-1` |

The sequence runner compares Go and Rust responses after normalizing only the
random token and the URL derived from it. Each implementation receives the
same package but uses its own generated token; production token generation
remains cryptographically random.
