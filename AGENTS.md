# Yin-Panel Agent Development Guide

This file is private operational context for AI agents. Read it at the start of every task. Treat its commands, paths, ordering, and safety rules as binding unless the user explicitly overrides them.

## Mandatory Agent Protocol

For every code-change task, follow this order without asking the user to repeat it:

1. Inspect `git status --short` and relevant files.
2. Make the smallest scoped change while preserving unrelated worktree changes.
3. For frontend-only changes, run the frontend build only. Do not compile or restart the backend.
4. For backend changes, build the frontend first only when the backend serves a changed frontend bundle; then format, test, and compile the backend.
5. If the user asks to update the running service, deploy only the changed artifact: replace `web/` for frontend-only changes, or replace the binary for backend changes. Restart only when the binary changed; preserve timestamped backups and verify the configured port after a restart.
6. Report exact checks run and any skipped checks. Do not claim success from an unrun or failed check.

Do not stop at a plan when the user asked to execute. Do not re-ask for paths, build order, or restart procedure already specified here.

Every command must run in the directory stated by its `cd` or tool working-directory setting. Before running Go commands, verify `go.mod` is present in the current directory. Before running npm commands, verify `package.json` is present. Never infer that a prior tool call's `cd` persists into a later tool call.

## Git Collaboration Rules

- Keep `master` usable at all times. Never make code changes, commits, or direct pushes on `master`.
- At the start of a task, inspect the current branch and worktree before fetching or switching branches.
- Determine `<username>` from `git config user.name`, converted to lowercase. Never use the Linux login name.
- When the worktree is clean, fetch remote updates, switch to `master`, and fast-forward it with `git pull --ff-only origin master`. Create a temporary task branch as `<username>/<YYYYMMDD-HHmm>` from the synchronized `master`.
- When `master` has uncommitted or untracked changes, do not pull, reset, discard, or overwrite them. Create a temporary branch as `<username>/<YYYYMMDD-HHmm>` from the current `master` and carry the complete worktree into that branch before continuing.
- Before merging, first merge the latest `master` into the temporary branch, run the required local verification, summarize the final scope, and rename the branch to `<username>/<short-summary>`.
- Use lowercase English and hyphens in `short-summary`, normally 2 to 4 words describing the main theme. Do not enumerate files, dates, or every individual feature in the branch name; record those details in commits and the pull request.
- Use descriptive prefixes such as `feature/`, `fix/`, `refactor/`, and `docs/` only when they clarify the task; the username is required for temporary and final task branches.
- To determine the final summary, inspect `git merge-base master HEAD`, `git log --oneline <base>..HEAD`, `git diff <base>...HEAD`, `git status --short`, `git diff --stat`, and `git diff --cached --stat`. Include intended untracked files in the review before renaming.
- Complete the required build, test, and `git diff --check` verification on the task branch before merging.
- Merge the final task branch into `master`, preferably through a pull request or an equivalent non-direct merge workflow.
- After merging, verify that `master` remains buildable and usable. Only after that verification succeeds, delete both the local and remote task branches, then push the merged `master`.
- If the task branch was already pushed under its temporary name, push the renamed final branch first and delete the old remote name only after the new branch is confirmed complete.
- Never delete a task branch before confirming that its commits exist in `master`.

## Project Facts

- Core repository: `https://github.com/yinorg/Yin-Panel`
- Browser extension repository: `https://github.com/yinorg/yin-panel-extension`
- E2E project: configure its location with `YIN_PANEL_E2E_DIR`; it is separate from Core.
- Frontend: `frontend/`
- Backend: `backend/`
- Frontend production output: `backend/web/`
- Local runtime directory: `YIN_PANEL_RUNTIME_DIR` (required for deployment)
- Local binary: `$YIN_PANEL_RUNTIME_DIR/yin-panel`
- Local HTTP port: `${YIN_PANEL_PORT:-3002}`
- Current local service is a standalone process, not a Docker container.

All paths outside this repository are machine-specific. Use `git rev-parse --show-toplevel`, `YIN_PANEL_RUNTIME_DIR`, `YIN_PANEL_EXTENSION_DIR`, and `YIN_PANEL_E2E_DIR`; never commit a developer home directory or machine-specific absolute path.

Load local-only values for a shell session with:

```bash
repo_root="$(git rev-parse --show-toplevel)"
env_file="${YIN_PANEL_ENV_FILE:-$repo_root/.env.local}"
if [ -f "$env_file" ]; then
    set -a
    . "$env_file"
    set +a
fi
```

`.env.example` documents supported variables. `.env.local` may contain test credentials and must remain untracked. Never print `SUDO_PASSWORD`, put it in command arguments, or write it to logs; prefer `sudo -v` or an OS credential helper when possible.

## Before Changes

1. Run `git status --short` and preserve unrelated user changes.
2. Search the repository before changing branding, API fields, database models, or migrations.
3. Never delete or reset databases, uploads, backups, or generated files without explicit permission.
4. Keep user-facing translations synchronized across `frontend/src/locales/`.

## Standard Build Order

For a combined release, build the frontend first, then build and test the backend:

```bash
repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root/frontend"
npm ci                         # only when node_modules is absent or stale
npm run build-only             # writes production files to ../backend/web

cd "$repo_root/backend"
gofmt -w <changed-go-files>
go test ./...
mkdir -p /tmp/yin-panel-build
go build -o /tmp/yin-panel-build/yin-panel .
```

`npm run type-check` may report pre-existing project-wide TypeScript errors. Do not claim it passed unless it exits successfully. `npm run build-only` is the required frontend acceptance check.

For frontend-only work, stop after `npm run build-only` and deploy `backend/web/` if a running instance must be updated. Do not run `go build` or restart the service.

For backend-only work, skip the frontend entirely. Run `gofmt`, `go test ./...`, and `go build`; deploy only the backend binary and restart the service. Do not replace `web/`.

## Local Deployment

The service runs from `$YIN_PANEL_RUNTIME_DIR`, with that directory as its working directory so `conf.yaml` is found. Require this variable before deployment. Never replace its database or uploads with repository copies.

After successful builds, replace only the changed runtime artifact. A running Linux executable cannot be overwritten reliably and may fail with `Text file busy`; always stop the current Yin-Panel process and confirm its PID has exited before replacing the binary. If the process does not exit, do not replace the binary or kill unrelated processes; report the blocker. Preserve old binaries, but do not keep backups of frontend static files:

```bash
repo_root="$(git rev-parse --show-toplevel)"
run_dir="${YIN_PANEL_RUNTIME_DIR:?Set YIN_PANEL_RUNTIME_DIR before deployment}"
stamp=$(date +%Y%m%d-%H%M%S)
cp -a "$run_dir/yin-panel" "$run_dir/yin-panel.previous-$stamp"
pid=$(ss -ltnp | awk '/:'"${YIN_PANEL_PORT:-3002}"' / {match($0,/pid=[0-9]+/); if (RSTART) {print substr($0,RSTART+4,RLENGTH-4); exit}}')
test -n "$pid"
kill "$pid"
for i in $(seq 1 100); do
    if ! kill -0 "$pid" 2>/dev/null; then
        break
    fi
    sleep 0.1
done
if kill -0 "$pid" 2>/dev/null; then
    echo "Yin-Panel process did not exit" >&2
    exit 1
fi
cp -a /tmp/yin-panel-build/yin-panel "$run_dir/yin-panel"
rm -rf "$run_dir/web"
cp -a "$repo_root/backend/web" "$run_dir/web"
```

Identify the listener with `ss -ltnp | grep ":${YIN_PANEL_PORT:-3002}"`. Resolve and record the Yin-Panel PID before stopping it. Stop only that process, confirm it exited as shown above, then start it detached from the terminal:

```bash
kill <yin-panel-pid>
cd "$run_dir"
setsid ./yin-panel > yin-panel.log 2>&1 < /dev/null &
```

Verify the process and HTTP service:

```bash
ss -ltnp | grep ":${YIN_PANEL_PORT:-3002}"
curl --noproxy '*' -fsS "http://127.0.0.1:${YIN_PANEL_PORT:-3002}/" -o /tmp/yin-panel-home.html
tail -30 "$run_dir/yin-panel.log"
```

Do not start the binary from another working directory; it will exit because it cannot find `conf.yaml`.

## Versioning and Tags

Before creating a release tag:

1. Update `backend/internal/global/global.go`, especially `global.VERSION`.
2. Build the frontend so generated frontend metadata is current.
3. Build and test the backend after the frontend build.
4. Run `git diff --check`, inspect `git diff`, and confirm old branding and old version strings are gone.
5. Create the tag only after all checks pass.

The Docker workflow publishes standard version tags such as `0.3.1`. It expects frontend output in `backend/web` and a backend binary in `build-output/yin-panel`. Do not tag before both artifacts are rebuilt.

## Database and Identity Rules

- Space permissions are keyed by `SpaceMember.UserID`, never by email text.
- `mail` is the user login identity and display email; `name` is the nickname.
- Existing databases may contain the legacy `username` column. Migrations must be idempotent, backward-compatible, and must not overwrite user data blindly.
- Test migrations against both fresh and existing databases.

## Final Verification

For combined or backend changes, run:

```bash
repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root/backend" && go test ./...
cd "$repo_root/frontend" && npm run build-only
cd "$repo_root" && git diff --check
```

For frontend-only changes, run the equivalent commands from the repository root and `git diff --check`; skip backend tests and backend compilation.

After deployment, verify `${YIN_PANEL_PORT:-3002}`, the root page, the startup log, and the changed feature through the running HTTP service. Report every skipped check and its reason.
