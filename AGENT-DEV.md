# Yin-Panel Agent Development Guide

This file is private operational context for AI agents. Read it at the start of every task. Treat its commands, paths, ordering, and safety rules as binding unless the user explicitly overrides them.

## Mandatory Agent Protocol

For every code-change task, follow this order without asking the user to repeat it:

1. Inspect `git status --short` and relevant files.
2. Make the smallest scoped change while preserving unrelated worktree changes.
3. For frontend-only changes, run the frontend build only. Do not compile or restart the backend.
4. For backend changes, build the frontend first only when the backend serves a changed frontend bundle; then format, test, and compile the backend.
5. If the user asks to update the running service, deploy only the changed artifact: replace `web/` for frontend-only changes, or replace the binary for backend changes. Restart only when the binary changed; preserve timestamped backups and verify port `3002` after a restart.
6. Report exact checks run and any skipped checks. Do not claim success from an unrun or failed check.

Do not stop at a plan when the user asked to execute. Do not re-ask for paths, build order, or restart procedure already specified here.

Every command must run in the directory stated by its `cd` or tool working-directory setting. Before running Go commands, verify `go.mod` is present in the current directory. Before running npm commands, verify `package.json` is present. Never infer that a prior tool call's `cd` persists into a later tool call.

## Git Collaboration Rules

- Keep `master` usable at all times. Never make code changes, commits, or direct pushes on `master`.
- At the start of a task, inspect the current branch and worktree before fetching or switching branches.
- When the worktree is clean, fetch remote updates, switch to `master`, and fast-forward it with `git pull --ff-only origin master`. Create a temporary task branch as `<username>/<YYYYMMDD-HHmm>` from the synchronized `master`.
- When `master` has uncommitted or untracked changes, do not pull, reset, discard, or overwrite them. Create a temporary branch as `<username>/<YYYYMMDD-HHmm>` from the current `master` and carry the complete worktree into that branch before continuing.
- Before merging, summarize the final scope from the original `master` fork point and rename the branch to `<username>/<short-summary>`.
- Use lowercase English and hyphens in `short-summary`, normally 2 to 4 words describing the main theme. Do not enumerate files, dates, or every individual feature in the branch name; record those details in commits and the pull request.
- Use descriptive prefixes such as `feature/`, `fix/`, `refactor/`, and `docs/` only when they clarify the task; the username is required for temporary and final task branches.
- To determine the final summary, inspect `git merge-base master HEAD`, `git log --oneline <base>..HEAD`, `git diff <base>...HEAD`, `git status --short`, `git diff --stat`, and `git diff --cached --stat`. Include intended untracked files in the review before renaming.
- Complete the required build, test, and `git diff --check` verification on the task branch before merging.
- Merge through a pull request or an equivalent non-direct merge workflow. After a successful merge, delete both the local and remote task branches.
- Verify that `master` remains buildable and usable after the merge. Keep the task branch available until any failed verification is resolved.

## Project Facts

- Repository: `https://github.com/yinorg/Yin-Panel`
- Frontend: `frontend/`
- Backend: `backend/`
- Frontend production output: `backend/web/`
- Current local runtime directory: `/home/hsy/project/backup/backend`
- Current local binary: `/home/hsy/project/backup/backend/yin-panel`
- Current local HTTP port: `3002`
- Current local service is a standalone process, not a Docker container.

## Before Changes

1. Run `git status --short` and preserve unrelated user changes.
2. Search the repository before changing branding, API fields, database models, or migrations.
3. Never delete or reset databases, uploads, backups, or generated files without explicit permission.
4. Keep user-facing translations synchronized across `frontend/src/locales/`.

## Standard Build Order

For a combined release, build the frontend first, then build and test the backend:

```bash
cd /home/hsy/project/Yin-Panel/frontend
npm ci                         # only when node_modules is absent or stale
npm run build-only             # writes production files to ../backend/web

cd /home/hsy/project/Yin-Panel/backend
gofmt -w <changed-go-files>
go test ./...
mkdir -p /tmp/yin-panel-build
go build -o /tmp/yin-panel-build/yin-panel .
```

`npm run type-check` may report pre-existing project-wide TypeScript errors. Do not claim it passed unless it exits successfully. `npm run build-only` is the required frontend acceptance check.

For frontend-only work, stop after `npm run build-only` and deploy `backend/web/` if a running instance must be updated. Do not run `go build` or restart the service.

For backend-only work, skip the frontend entirely. Run `gofmt`, `go test ./...`, and `go build`; deploy only the backend binary and restart the service. Do not replace `web/`.

## Local Deployment

The service runs from `/home/hsy/project/backup/backend`, with that directory as its working directory so `conf.yaml` is found. Never replace its database or uploads with repository copies.

After successful builds, replace only the changed runtime artifact. Preserve old binaries, but do not keep backups of frontend static files:

```bash
run_dir=/home/hsy/project/backup/backend
stamp=$(date +%Y%m%d-%H%M%S)
cp -a "$run_dir/yin-panel" "$run_dir/yin-panel.previous-$stamp"
cp -a /tmp/yin-panel-build/yin-panel "$run_dir/yin-panel"
rm -rf "$run_dir/web"
cp -a /home/hsy/project/Yin-Panel/backend/web "$run_dir/web"
```

Identify the listener with `ss -ltnp | grep ':3002'`. Stop only the Yin-Panel process, then start it detached from the terminal:

```bash
kill <yin-panel-pid>
cd /home/hsy/project/backup/backend
setsid ./yin-panel > yin-panel.log 2>&1 < /dev/null &
```

Verify the process and HTTP service:

```bash
ss -ltnp | grep ':3002'
curl --noproxy '*' -fsS http://127.0.0.1:3002/ -o /tmp/yin-panel-home.html
tail -30 /home/hsy/project/backup/backend/yin-panel.log
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
cd /home/hsy/project/Yin-Panel/backend && go test ./...
cd /home/hsy/project/Yin-Panel/frontend && npm run build-only
cd /home/hsy/project/Yin-Panel && git diff --check
```

For frontend-only changes, run `cd /home/hsy/project/Yin-Panel/frontend && npm run build-only` and `cd /home/hsy/project/Yin-Panel && git diff --check`; skip backend tests and backend compilation.

After deployment, verify port `3002`, the root page, the startup log, and the changed feature through the running HTTP service. Report every skipped check and its reason.
