# Yin-Panel Agent Development Guide

This file is private operational context for AI agents. Read it at the start of every task. Treat its commands, paths, ordering, and safety rules as binding unless the user explicitly overrides them.

## Mandatory Agent Protocol

For every code-change task, follow this order without asking the user to repeat it:

1. Inspect `git status --short` and relevant files.
2. Make the smallest scoped change while preserving unrelated worktree changes.
3. By default, locally deploy every application code, configuration, or asset change with `scripts/deploy-local.sh`. It performs the frontend build, backend tests and compilation, replacement, restart, and HTTP verification. Do not use an external backup directory as an implicit deployment target.
4. After deployment, report the local URL and verification results, then wait for the user to validate the running change. Do not commit before the user explicitly confirms that validation passed.
5. Once the user confirms validation passed, automatically commit the approved, reviewed task changes locally on `<username>-dev`; do not ask for a second commit confirmation. Do not push, rebase, or backfill `master` unless explicitly requested.
6. Respect explicit `不要部署` or `只改文件` requests by skipping deployment. Respect explicit `不要提交` or `暂不提交` requests by leaving the changes uncommitted. Report exact checks run and any skipped checks; do not claim success from an unrun or failed check.

### Deployment and commit scope invariant

The source files used for deployment must be exactly the source files approved for the corresponding commit. A successful deployment does not authorize committing only a subset of what was deployed.

- Before deployment, inspect and classify every tracked and untracked change as task-approved, other user work, generated output/cache, secret or machine-local file, or unknown. Do not deploy until the classification is complete.
- Record a `DEPLOYMENT_MANIFEST` containing every approved tracked and untracked source file, plus an explicit exclusion list and reasons. Generated `backend/web`, binaries, logs, caches, Playwright artifacts, `.env.local`, credentials, and machine-specific files are not commit inputs.
- Never run `scripts/deploy-local.sh` from a mixed dirty worktree and then commit only selected files. The build would include the uncommitted files that were omitted from the commit.
- Before committing after user validation, record a `COMMIT_MANIFEST` from the staged files and require `COMMIT_MANIFEST == DEPLOYMENT_MANIFEST`. If the manifests differ, stop and rebuild/redeploy from the corrected scope before committing.
- If all current relevant changes are explicitly approved, include the complete reviewed set in both manifests. If unrelated or unapproved application changes remain, stop and report the unconfirmed file list; do not deploy until the user confirms the complete scope.

Required pre-deployment review:

```bash
git status --short --branch
git diff --stat
git diff --name-only
git ls-files --others --exclude-standard
git diff --check
```

Required pre-commit review after user validation:

```bash
git diff --cached --name-only
git diff --cached --stat
git diff --cached --check
```

When a scope mismatch is found, use this recovery sequence:

```text
STOP deployment/commit
-> re-inspect tracked and untracked files
-> reclassify every change
-> generate one authoritative manifest
-> wait for explicit confirmation of the complete current scope
-> redeploy and wait for user validation
-> commit only after the staged manifest matches the deployment manifest
```

Do not use `git reset --hard`, `git checkout --`, `git stash`, deletion, or selective guessing to hide unapproved worktree changes.

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
- If the user approves including existing code, stage the complete reviewed set together, inspect `git diff --cached --stat` and `git diff --cached --check`, then commit. If the user does not approve inclusion, stop and wait for scope confirmation; do not use another worktree to bypass the decision.
- The reviewed file set used for deployment and the reviewed file set staged for commit must be the same manifest. A dirty worktree with additional application changes cannot be deployed until all relevant changes are explicitly approved for the same commit.
- If the current local non-`master` branch is the intended development branch and the user requests normalization, rename that local branch to `<username>-dev`; do not recreate it from another commit or push it merely because it was renamed. Keep `<username>-dev` local by default and do not require a remote upstream.
- The default code-change lifecycle is **local deployment, user validation, then automatic local commit**. Deployment success alone is not user validation. Keep the task changes uncommitted while the user checks the running service.
- The prompt `本地提交`, `只提交`, or `提交到开发分支` authorizes a local commit, but the default lifecycle still requires successful deployment and the user's explicit validation before committing. These prompts do not authorize push, rebase, or backfill. The prompt `不要提交` or `暂不提交` overrides automatic commit and leaves the changes uncommitted after verification.
- The prompt `不要部署` or `只改文件` overrides default local deployment. Continue to preserve and verify the requested changes, but do not deploy them.
- The prompt `回灌主分支`, `把代码回灌到 master`, or `走 Git 规范到远程 master` means the **backfill master** mode. Only then fetch the remote, rebase `<username>-dev`, fast-forward merge it into `master`, and push `master`.
- The prompt `提交并回灌`, `完成 1-2`, or `提交后推送远程` means execute local commit mode followed by backfill master mode.
- In local commit mode, after staging only the reviewed files, inspect `git diff --cached --stat` and `git diff --cached --check`, commit on `<username>-dev`, and verify the result with `git status --short --branch`, `git branch -vv`, and `git ls-remote --heads origin`. Do not switch to `master` or push as part of this mode.
- A commit message describes the change and nothing else. Never add a `Co-Authored-By` trailer, a "Generated by" or "Generated with" line, an assistant, model, or tool name, an emoji signature, or any other attribution, credit, or metadata the user did not explicitly ask for. Never alter the `author` or `committer` identity either; the user is the sole author of every commit. If the user wants attribution, they will ask for it, and the exact wording is theirs to choose.
- Write the commit message in the language and style the existing history uses. Do not invent a house style, a sign-off, or a trailer convention.
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
- Rebase only after the source worktree is clean and the user-approved change boundary is committed. If unrelated user changes prevent a checkout or rebase, stop and report the blocker instead of using another worktree to bypass the scope review.
- Resolve rebase conflicts only on `<source-branch>`. Never resolve conflicts on `master`, and never use stash, reset, discard, or overwrite commands to hide unrelated user changes.
- Use `<username>-dev` as `<source-branch>` by default. If the user explicitly requested a temporary task branch, that branch may be used instead. Ordinary merge and `--no-ff` are prohibited; `master` accepts only fast-forward integration.
- Run the required build, test, and `git diff --check` verification after the final rebase and before the fast-forward merge. If `origin/master` advances or the merge/push fails, return to `<source-branch>`, fetch, rebase, verify, and retry.
- Do not proactively push non-`master` branches to the remote. If the user explicitly requests a non-`master` push, push only the branch they specified.
- Never use `--force` or `--force-with-lease` when pushing `master`. The only exception is a history rewrite the user has explicitly and specifically authorised, such as correcting commit messages; even then use `--force-with-lease` rather than `--force`, record the pre-rewrite commit ids and tree hashes before rewriting, and afterwards verify that every rewritten commit's tree hash is unchanged and that nothing but the intended metadata differs. Report the before and after ids.
- Existing merge commits and existing remote branches are not rewritten or deleted by this policy unless the user explicitly requests a separate migration.
- After a local commit, merge, or push, verify with `git status --short --branch`, `git branch -vv`, and `git ls-remote --heads origin`; report retained local branches, remote branches deleted by explicit request, and any stale upstream tracking references.

## Home Rendering Boundary (Core vs Theme)

The home page is rendered by a theme package. The Core supplies only what a theme cannot safely do for itself. This section is the binding summary; the full statement lives in `frontend/THEME_SYSTEM.md`.

### Core responsibilities (frontend)

| # | Responsibility | Content |
| --- | --- | --- |
| C1 | Data channel | Talks to the backend, fetches and mutates data, projects it into the snapshot handed to the theme. **A theme never talks HTTP.** |
| C2 | Policy and trust boundary | Holds the JWT, enforces a permission per command, validates arguments. **A theme never holds a token.** |
| C3 | Bootstrap and degradation | Startup loading state, theme consent prompt, and the minimum viable fallback view when the theme cannot render. |
| C4 | Application shell | Login, public access codes, theme recovery, settings and admin surfaces — everything that is not the home's own rendering. |

### Theme responsibilities

All presentation and interaction of the home page: layout, styling, and which affordances it exposes.

### Data access: typed capability commands

A theme reaches data only through typed commands (`api.commands.execute('item.create', {...})`). Each command declares the permission it needs and the Core enforces it, so the backend HTTP shape is not part of the theme contract. The snapshot is scoped by the same permission set, so an ungranted theme renders read-only and empty instead of leaking data.

### Surfaces the Core keeps

A theme invokes them; the Core renders them: `editor.open` (item editor — privileged file upload and icon selection), `commandCenter.open` (cross-module navigation), `ui.openCoreSurface('theme-settings')` (application shell).

### Decision rule

> Needs a privilege (token, file, routing) **or** must exist when no theme is present → **Core**
> Pure presentation and interaction → **theme**

### Where this lives in the code

`frontend/src/views/home/index.vue` is the host: it wires the `frontend/src/core/home/use*` composables (environment, monitor, theme runtime, data, modals, public access, commands) and renders the Core-owned chrome around the theme frame. The theme renders inside a sandboxed, cross-origin frame, so the Core cannot measure inside it; the theme reports its layout back through the `layout.report` command for the monitor band.

## Project Facts

- Core repository: `https://github.com/yinorg/Yin-Panel`
- Browser extension repository: `https://github.com/yinorg/yin-panel-extension`
- E2E project: configure its location with `YIN_PANEL_E2E_DIR`; it is separate from Core.
- Frontend: `frontend/`
- Backend: `backend/`
- Frontend production output: `backend/web/`
- Local runtime directory: repository `backend/` by default; `YIN_PANEL_RUNTIME_DIR` may explicitly override it
- Local binary: `$YIN_PANEL_RUNTIME_DIR/yin-panel`
- Local HTTP port: `${YIN_PANEL_PORT:-3002}`
- Current local service is a standalone process, not a Docker container.

## Test Placement and Ownership

- Keep unit and subsystem integration tests with the code they exercise: backend tests belong under `backend/`, and frontend tests belong under `frontend/` using the local framework conventions.
- Put cross-repository acceptance tests and real-browser workflows in the separate E2E repository at `YIN_PANEL_E2E_DIR`, under its `tests/` directory.
- Scenarios that load the browser extension, exercise `chrome_url_overrides`, or verify Core plus extension behavior belong in E2E, even when the reported defect is in extension code.
- Do not copy Core or extension source into the E2E repository. Load the tested extension through `YIN_PANEL_EXTENSION_DIR` and record the tested source commit in CI or the test run context.
- When a change spans repositories, keep implementation and fast local tests in their owning repositories; add the cross-repository regression test to E2E.
- For changes spanning Core and E2E, maintain separate manifests for each repository and verify each repository's deployment/test scope equals its own commit scope. A Core deployment does not imply that E2E changes were committed.
- Keep Playwright reports, traces, screenshots, videos, and other generated test output ignored and untracked.

All paths outside this repository are machine-specific. Use `git rev-parse --show-toplevel`, `YIN_PANEL_RUNTIME_DIR`, `YIN_PANEL_EXTENSION_DIR`, and `YIN_PANEL_E2E_DIR`; never commit a developer home directory or machine-specific absolute path.

## Local Environment

At the start of every task, load `.env.local` before running shell commands. Each tool call starts a new shell session, so repeat this setup in every command that uses local paths, credentials, proxies, E2E, installation, deployment, or privileged operations. Do not decide a local variable is unset by checking only the inherited process environment.

Use this setup from the repository root:

```bash
repo_root="$(git rev-parse --show-toplevel)"
env_file="${YIN_PANEL_ENV_FILE:-$repo_root/.env.local}"
if [ ! -f "$env_file" ]; then
    printf 'Local environment file not found: %s\n' "$env_file" >&2
    exit 1
fi
set +x
set -a
. "$env_file"
set +a
# Keep sudo credentials in this shell only; do not pass them to child processes.
export -n SUDO_PASSWORD SUDO_PASSWD 2>/dev/null || true
```

`.env.example` documents supported variables. `.env.local` may contain credentials and proxy URLs and must remain untracked. Never print or log these values.

- For E2E work, load the environment first, then require `YIN_PANEL_E2E_DIR` to be non-empty and resolve relative paths from `repo_root`. Confirm the resolved directory exists before reporting E2E as unavailable or running its tests.
- Network commands must inherit the configured `http_proxy`, `https_proxy`, and `all_proxy` values from the loaded environment. If a network operation fails, verify the command inherited the proxy configuration and retry with it before reporting a network blocker. Do not print proxy values.
- For sudo, accept `SUDO_PASSWORD` or the existing local alias `SUDO_PASSWD`. Keep the credential unexported and authenticate through standard input; never put it in command arguments or logs. Use this in the shell session that needs privilege:

  ```bash
  sudo_password="${SUDO_PASSWORD:-${SUDO_PASSWD:-}}"
  if [ -n "$sudo_password" ]; then
      sudo -S -v <<< "$sudo_password"
      sudo_status=$?
      unset sudo_password SUDO_PASSWORD SUDO_PASSWD
      [ "$sudo_status" -eq 0 ] || exit "$sudo_status"
  else
      sudo -v
  fi
  ```

  The cached sudo credential can then be used for privileged commands. If no password is configured, use an OS credential helper or the normal interactive `sudo -v` flow.

## Before Changes

1. Run `git status --short` and preserve unrelated user changes.
2. Search the repository before changing branding, API fields, database models, or migrations.
3. Never delete or reset databases, uploads, backups, or generated files without explicit permission.
4. Keep user-facing translations synchronized across `frontend/src/locales/`.

## Standard Build Order

For a combined release, run `scripts/deploy-local.sh`. It builds the frontend first, then tests and compiles the backend, replaces both runtime artifacts, restarts the service, and verifies the running HTTP service.

`npm run type-check` may report pre-existing project-wide TypeScript errors. Do not claim it passed unless it exits successfully. `npm run build-only` is the required frontend acceptance check.

The local update SOP intentionally rebuilds and deploys both artifacts every time so the running frontend and backend cannot become version-mismatched.

## Local Deployment

Local deployment is the default for application code, configuration, and asset changes. After a successful deployment, wait for the user's explicit validation before committing; once validated, commit the reviewed task files locally without asking again. Documentation-only changes that do not affect runtime artifacts do not require a deployment unless the user asks for one.

The service runs from the repository `backend/` directory by default, with that directory as its working directory so `conf.yaml` is found. `YIN_PANEL_RUNTIME_DIR` can explicitly override the target. Never replace its database or uploads with repository copies.

Run the complete local deployment with:

```bash
./scripts/deploy-local.sh
```

The script requires `backend/conf.yaml` before it stops any running process. After all builds and tests pass, it stops only the Yin-Panel listener, uses `sudo` to force-remove the old binary and `web/`, then starts the new binary from the target directory. It does not create backups and never touches database, upload, or configuration files.

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
