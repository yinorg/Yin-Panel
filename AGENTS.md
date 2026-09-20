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
- By default, use a local long-lived development branch named `<username>-dev`; normally create it once and do not delete it proactively.
- Unless the user explicitly requests it, do not proactively create a temporary task branch. If the user explicitly requests one, it may be created and used for that task.
- If `<username>-dev` does not exist and no temporary branch was requested, synchronize local `master` with `origin/master` first, then create `<username>-dev` from that commit.
- Before any `fetch`, `checkout`, branch rename, `rebase`, merge, or push, inspect the current worktree with `git status --short --branch`, `git diff --stat`, and the untracked-file list. If it is dirty, do not switch branches or rewrite refs until the changes have been reviewed.
- For a dirty worktree, inspect the complete tracked diff and every untracked file, classify changes as task-related, existing user work, generated output, or unclear, and report that classification before staging anything. The default is to ask the user how to handle existing code; never stage, commit, stash, reset, discard, delete, or overwrite it without explicit approval.
- If the user approves including existing code, stage only the reviewed files, keep one coherent feature together, inspect `git diff --cached --stat` and `git diff --cached --check`, then commit. If the user does not approve inclusion, preserve the original worktree and use a clean temporary worktree for the task and integration.
- If the current local non-`master` branch is the intended development branch and the user requests normalization, rename that local branch to `<username>-dev`; do not recreate it from another commit or push it merely because it was renamed. Keep `<username>-dev` local by default and do not require a remote upstream.
- Ordinary code-change tasks default to the **local commit** mode: after implementation and required verification, commit the approved changes on `<username>-dev` and stop. Do not automatically rebase, merge, or push.
- The prompt `本地提交`, `只提交`, or `提交到开发分支` means local commit mode only. The prompt `不要提交`, `只改文件`, or `暂不提交` overrides the default and leaves the changes uncommitted after verification.
- The prompt `回灌主分支`, `把代码回灌到 master`, or `走 Git 规范到远程 master` means the **backfill master** mode. Only then fetch the remote, rebase `<username>-dev`, fast-forward merge it into `master`, and push `master`.
- The prompt `提交并回灌`, `完成 1-2`, or `提交后推送远程` means execute local commit mode followed by backfill master mode.
- In local commit mode, after staging only the reviewed files, inspect `git diff --cached --stat` and `git diff --cached --check`, commit on `<username>-dev`, and verify the result with `git status --short --branch`, `git branch -vv`, and `git ls-remote --heads origin`. Do not switch to `master` or push as part of this mode.
- In backfill master mode, before changing refs inspect `git status --short --branch`, `git diff --stat`, and the untracked-file list, then run:
  ```bash
  git fetch origin
  git checkout <source-branch>
  git rebase origin/master
  # Run required verification after the final rebase.
  git checkout master
  git merge --ff-only <source-branch>
  git push origin master
  ```
- Rebase only after the source worktree is clean and the user-approved change boundary is committed. If unrelated user changes prevent a checkout or rebase, use a clean temporary worktree for verification and integration instead of using stash, reset, discard, or overwrite commands.
- Resolve rebase conflicts only on `<source-branch>`. Never resolve conflicts on `master`, and never use stash, reset, discard, or overwrite commands to hide unrelated user changes.
- Use `<username>-dev` as `<source-branch>` by default. If the user explicitly requested a temporary task branch, that branch may be used instead. Ordinary merge and `--no-ff` are prohibited; `master` accepts only fast-forward integration.
- Run the required build, test, and `git diff --check` verification after the final rebase and before the fast-forward merge. If `origin/master` advances or the merge/push fails, return to `<source-branch>`, fetch, rebase, verify, and retry.
- Do not proactively push non-`master` branches to the remote. If the user explicitly requests a non-`master` push, push only the branch they specified.
- Never use `--force` or `--force-with-lease` when pushing `master`.
- Existing merge commits and existing remote branches are not rewritten or deleted by this policy unless the user explicitly requests a separate migration.
- After a local commit, merge, or push, verify with `git status --short --branch`, `git branch -vv`, and `git ls-remote --heads origin`; report retained local branches, remote branches deleted by explicit request, and any stale upstream tracking references.

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
