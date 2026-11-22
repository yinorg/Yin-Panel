# Repository layout

The repository is split by responsibility, not by language:

| Path | Contents |
| --- | --- |
| `backend/` | Rust workspace: the `yin-panel` server, `db-tool`, the wallpaper crate, recorded cases, parity/coverage checks and the replay harness |
| `frontend/` | Vue application and theme SDK |
| `docker/` | The only deployment-asset directory: `Dockerfile`, compose file, `conf.yaml.example` and the Helm chart under `helm/` |
| `docs/` | User-facing translations under language folders, and engineering documents under `engineering/` |
| `scripts/` | Repository automation (`deploy-local.sh`, `replay.sh`, release checks) |
| `runtime/` | Local runtime state only (database, uploads, built web, binary); never tracked |
| `.github/` | Pull-request, database, nightly, E2E, performance and release workflows |

Rules:

- `distribution/` and top-level `shared/` no longer exist: deployment assets live in `docker/`, and shared test fixtures live next to their consumer (`frontend/tests/fixtures/`).
- Engineering docs are named after their subject and live in `docs/engineering/`; component READMEs stay with the component.
- `runtime/` is generated and ignored. Nothing under it belongs in a commit.
- `rust-anything` expects `rust/app-cases`; `scripts/replay.sh` provides that view without keeping a `rust/` directory.
