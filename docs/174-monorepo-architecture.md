# Verified monorepo boundaries

Inspected November 21, 2026 at HEAD `28a8da823231b792030eff5b6172dba29768442f`, branch `codex/feature/public-research-view`. This is a monorepo: the first-party control plane, orchestration, MCP server, runtime adapters, configuration, building UI, desktop shell and website are in one Git repository. Cargo workspace membership is a narrower boundary than Git ownership.

## Git and editor boundary

- `git rev-parse --show-toplevel` and `pwd -P` both resolve to `/Users/rmonzingo/repo-applications/gents-rust-lab`. Origin is `https://github.com/Monzingo89/gents-rust-lab.git`.
- The running VS Code window is titled `config.yaml — gents-rust-lab`. Its Explorer root is `gents-rust-lab`, with `crates`, `mcp`, `web`, `config`, `scripts` and `src-tauri` directly underneath the repository path. It has opened the root, not a component subfolder. The active editor file, `~/.continue/config.yaml`, is a user-level assistant configuration outside the repository; it is not the opened project or an application component.
- A full non-symlink-following walk of 13,248 directories, including ignored model, dependency, build and data directories, found only the root `.git`. No nested `.git` file/directory, submodule/gitlink, `.gitmodules` or external directory symlink was found. `.claude/skills/*` links resolve to this checkout's canonical `.agents/skills/*`.
- `git worktree list --porcelain` also reports `/Users/rmonzingo/.local/share/kilo/worktree-pool/gents-rust-lab-a4320e7c5003/foam-tornado`, detached at `8828798683ca0816d980871aa8048c88eb9b8a75`. That is another checkout of this same repository, not an independently tracked first-party component. It was not modified or used as delivery evidence.

## First-party source inventory

The counts below come from `git ls-files` before this audit's changes. Each listed component's existing entrypoints resolve to this same Git root.

| Component | Actual source and entrypoints | Tracked files |
| --- | --- | ---: |
| Rust harness and orchestrator | `crates/harness/src/main.rs`, `workflow.rs`, `autonomy.rs`, `coordination/`, `company.rs`, `ceo_planning.rs`, `company_factory/`, `task_frontdoor.rs`, `task_workflows.rs` | 123 under `crates/harness` |
| Native MCP and shared task engine | `mcp/src/main.rs`, `lib.rs`, `functions/`, `transports/`, `task_engine/`; tests in `mcp/tests/` | 143 under `mcp` |
| Model integrations | Harness `provider.rs`, `local_runtime.rs`, `runtime_pool.rs`, `mlx_prompt.rs`; `scripts/mlx_prompt_tokens.py`, `setup-local.sh`, `train_role.py`; `mcp/media/worker.py` | Included in harness/MCP/scripts counts |
| Launchers, legacy broker and local checks | `scripts/start-company.mjs`, `run-company.sh`, `run-harness.mjs`, `company_tools.py`, `gents.py`, `check.sh` | 299 under `scripts` |
| Configuration and policies | `config/mock.json`, `company.local.json`, `live.example.json`, `schemas/`, `inference-requirements.txt`; root `.env*.example` templates | 43 under `config` |
| Custom building UI | `web/index.html`, `app.js`, `assets/tower-scene.js`, `task-workbench.js`, `office-activity.js`, `office-dashboards.js` | 70 under `web` |
| Optional desktop shell | `src-tauri/Cargo.toml`, `src/main.rs`, `src/lib.rs`, `tauri.conf.json` | 60 excluding vendor sources |
| Introduction website | `site/src/`, `server/`, `content.mjs`, `achievements.mjs`; builders under `scripts/` | 15 under `site` |

Orchestration is implemented inside the harness and shared MCP task engine; there is no missing standalone `orchestrator` crate to create. These distinct authority and execution contracts remain as described in [the local execution baseline](162-local-execution-baseline.md).

The starting checkout has substantial existing modified and untracked work. In particular, `crates/harness/src/local_runtime/native_cancel.rs`, `model_pins.rs`, `task_usage.rs`, `mcp/src/task_engine/progress.rs`, `mcp/tests/task_progress.rs`, `web/assets/task-activity.js`, the lifecycle/evaluation scripts, `AGENTS.md` and `.agents/`/`.claude/` are present but **not yet tracked**. They are inside the correct Git boundary and are not ignored. Their modified callers depend on some of these files. A commit containing only already-tracked modifications would omit required source. This audit does not stage, commit or publish existing work.

Untracked `.codex/config.toml` and `.continue/gents.yaml` also appeared during the audit. They are repository-local assistant configuration, not additional application components, and were left untouched.

No separately versioned first-party component or external first-party path dependency was found. This is bounded to the filesystem, manifests, launchers and configuration paths inspected here. No tracked Open WebUI entrypoint was located; Docker's daemon is unreachable, so any separately operated Open WebUI deployment remains unknown rather than a verified missing project source directory.

## Cargo and frontend ownership

- Root `Cargo.toml` has resolver `2`, members `crates/harness` and `mcp`, and explicitly excludes `src-tauri`. Both default members use the root `Cargo.lock` and `target/`.
- `crates/harness/Cargo.toml` depends on `gents-local-mcp = { path = "../../mcp" }`. No first-party Cargo path dependency escapes the Git root. Other core dependencies resolve through the registry/lockfile.
- `cargo metadata --offline --locked --no-deps --format-version 1`, invoked from the root and through each member manifest, confirms these two members and the same workspace root. The MCP package owns its library plus server, SQLite bridge, Rust acceptance and Rust component binaries; these are targets, not additional crates.
- `src-tauri/Cargo.toml` resolves as its own `gents-office` package with `src-tauri/Cargo.lock` and `src-tauri/target/`. Its `patch.crates-io` entries point to the retained GLib/GTK macro backports under `src-tauri/vendor/`. Keeping that platform-specific dependency/patch graph independent preserves current desktop commands and core checks.
- Root `package.json`/`package-lock.json` define one private npm package, `gents-office-lab`, with no npm `workspaces` field or pnpm/Yarn workspace configuration. `npm run dev` and `npm run build` operate on `web/`; Tauri's before-dev/build hooks call these same commands. No frontend package belongs in Cargo.
- The second non-dependency `package.json` is ignored `site/deploy/package.json`. `scripts/build-vercel-introduction.mjs` generates it from tracked site/server sources and `site/dist/`. It is deployment output, not a separately maintained frontend package workspace.

## Third-party and ignored material

- `src-tauri/vendor/{glib-0.18.5,glib-macros-0.18.5,gtk3-macros-0.18.2}` are 157 tracked third-party source/provenance files, not our application crates or nested repositories. Preserve their notices and reviewed provenance; see [vendor/README.md](../src-tauri/vendor/README.md). Their standalone validation lockfiles are ignored intentionally.
- `node_modules/`, `.venv*/`, Cargo registry/toolchains and Homebrew applications are dependencies. The current `llama-server` resolves outside the repository to `/opt/homebrew/Cellar/llama.cpp/0.7.0/bin/llama-server`; Ollama resolves to `/Applications/Ollama.app/Contents/Resources/ollama`. Our Rust/Python adapters are in this repository; these installed runtimes are not first-party source to import.
- `.gitignore` already excludes `models/`, model weight extensions, `.env`/private environment variants, `config/local.json`, signing material, `data/`, SQLite stores, `target/`, `node_modules/`, `.venv*/`, `__pycache__/`, `dist/`, `site/deploy/` and `.vercel/`. `.mypy_cache/` currently also ignores itself through its generated `.gitignore`. Environment examples remain tracked.
- Ignored source-shaped files were inspected: `data/release-session-1/{source-scan,indexed-source}/` are release audit snapshots; `data/poc-runs/*/expense-cli/` and `data/local-showcase/authorization-qualification/` are task/candidate artifacts. They are not missing runtime workspace members. `site/dist/`, `site/deploy/` and `src-tauri/gen/` are generated output. No ignored canonical first-party runtime source directory was found.
- `git ls-files --cached --ignored --exclude-standard` returned no tracked ignored files. Index checks found no tracked model weights, private environment files, signing keys, build outputs or dependency directories.

## Concrete repair and validation

The existing container builder copied root Cargo files and `crates/` only. Reproducing exactly that source layout caused Cargo metadata to fail with exit 101 because `mcp/Cargo.toml` was absent. Even with that member present, both crates embed configuration, the approved skill catalog/bodies and `scripts/python_snapshot_check.py` at compile time.

`infra/Dockerfile` now copies `mcp/`, `config/`, the shared skill source directory and the embedded Python helper into the build stage before its existing locked release build. `.dockerignore` now also excludes downloaded weights, local Python environments/caches, generated frontend/deployment output and private key material. The runtime command, authentication settings, workspace membership, lockfiles and component boundaries are unchanged. `AGENTS.md` records these boundaries for subsequent refactoring.

Fresh validation for this audit:

- Root `cargo check --workspace --all-targets --offline --locked`: passed before and after the repair.
- Root/member/desktop offline locked Cargo metadata: passed; membership and path dependency roots match the inventory above.
- The repaired build-stage `COPY` inputs were reproduced in a temporary directory, omitting private `config/local.json`. All 85 literal `include_str!`/`include_bytes!` paths under first-party Rust sources resolved there. Metadata and workspace/all-target checks passed with the existing build cache. Additionally, `cargo check --release -p gents-harness --offline --locked` compiled the staged inputs in a separate target directory, passing in 24.78 seconds including staging. This was a compiler check on macOS, not a linked Linux image build.
- `npm run build`: passed, with 43 Vite modules transformed; output remains ignored `dist/`.
- `git diff --check`: passed. Hash comparison of the 1,251 original files confirmed only the five intended existing-file edits; 1,246 other files and HEAD were preserved. The sixth audit file is this new document. Separately appearing assistant configuration was left untouched.
- No Docker daemon is reachable at the current local socket. These source-layout and macOS compiler checks do not establish a Linux image build, container startup, desktop GUI behavior or deployment. No model, paid API, download, production service restart or deployment was invoked.

## Decisions kept separate

No Git-history migration is needed for the located first-party components. Moving Tauri into the core Cargo workspace would require a separate review of the combined lockfile, desktop-only patches, platform prerequisites and CI scope. Bringing an external service or task-produced candidate into canonical source likewise needs an explicit ownership/provenance decision. Do not remove the Kilo worktree, erase Git metadata or move repositories as part of routine simplification. A full Linux container build and its supported runtime configuration remain a separate validation/deployment task.
