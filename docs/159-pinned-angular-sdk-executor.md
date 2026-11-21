# Pinned Angular SDK executor

Historical checkpoint: the later [governed Angular integration](160-governed-angular-task-validation.md) registers the native tool and task validator. Statements below about missing registration describe the original checkpoint.

The Angular executor now performs a real installed Angular build and finite host-owned browser checks in process. **It is not yet registered as an MCP tool or connected to native task validator receipts.** The task profile's `angular_build` check therefore remains explicitly unsupported. This checkpoint does not establish Angular specialist qualification, cross-domain integration, or completion of the original brief.

`mcp/src/angular_check.rs` defines strict versioned owner contracts. `angular_check/runtime.rs` accepts an explicit immutable source snapshot and host-selected installed SDK paths; `browser.mjs` supplies the host DOM oracle, and `browser_profile.mjs` supplies the actual browser OS boundary. No model can supply a shell command, executable path, browser argument, endpoint, or JavaScript evaluation expression through the contract. `completion.rs` validates the contract before owner approval, and `owner_inputs.rs` protects its frozen configuration paths, including hardlink aliases.

The initial executor supports one root application using `@angular/build:application`, explicit TypeScript/HTML inputs, frozen JSON controls and package manifest/lock, empty external assets/styles/polyfills, and exact development build settings. Other builders, SSR, prerender, package scripts/workspaces, binary assets and broader application layouts are unsupported. This deliberate initial surface must not be advertised as general Angular project support.

The source snapshot is limited to 2 MiB. Every JSON input is owner-hashed. Node has an exact version and executable SHA-256; Angular dependencies, the installed package manifest/lock, Playwright driver and headless browser have separate tree identities. Tree hashes include sorted relative names, file contents, executable modes and internal symlink targets. External symlinks, special files and oversized trees fail. SDK identities are rechecked after execution. System and Homebrew dynamic-library closure is not fully content-addressed.

Compilation uses a copied private application tree and a controlled read-only `node_modules` symlink. It has a deny-default Seatbelt profile, no network, no shell, fixed Node/esbuild execution, protected snapshot inputs, and scratch-only writes. Environment inheritance is cleared. The actual existing `HOME` value is forwarded unchanged for OS user lookup; private Angular/XDG configuration disables analytics and cache access to the owner's home. The runtime never installs dependencies.

The host serves only a bounded in-memory map of compiled UTF-8 assets on a held IPv4 loopback socket. The browser gets a fresh disposable profile, no owner credentials, blocked service workers/downloads, a restrictive CSP and same-origin routing. Its separate deny-default OS profile permits only that held loopback port, system/browser SDK reads and disposable-profile writes. It denies new TCP binds, other outbound connections, private control/source reads, shell execution and cross-process argument inspection. Expected DOM values stay in the host controller, outside the browser's readable filesystem. The driver accepts only literal CSS selectors and finite `expect_text`, `click` and `fill` steps. Rust checks the returned raw observations against the frozen cases and applies the privacy gate before hashing or serializing them. Candidate console text and browser exception strings are discarded.

On this macOS host, Chromium's renderer failed with `sandbox initialization failed: Operation not permitted` when attempting its own sandbox under the outer Seatbelt profile. The working executor uses a fixed `--no-sandbox` flag **inside the still-required outer OS sandbox**. It does not fall back to an unconfined browser. This replaces Chromium's usual process-specific inner profiles with one common outer boundary; these are different isolation designs. The exact outer profile is exercised independently with native OS probes rather than inferred from browser routing. Chromium's sandbox initialization implementation is in its [Seatbelt source](https://raw.githubusercontent.com/chromium/chromium/main/sandbox/mac/seatbelt.cc).

The existing .NET process-group watchdog is reused. The trusted Node controller forces the pinned Playwright launcher to retain the owned process group and checks the actual browser PGID before loading candidate code. CPU/file/descriptor limits, finite wall deadlines, bounded pipe draining and sampled group RSS/scratch limits apply. A disappearing temporary cache entry contributes zero to that sample; other observation failures stop execution. RSS limits are sampled enforcement, not total unified-memory measurements. Abrupt parent termination kills owned processes through the existing watchdog; unlike normal cleanup, it cannot guarantee removal of private scratch directories.

The installed trial uses Node `v24.21.0`, Angular `21.2.26`, TypeScript `5.9.3`, Playwright core `1.64.1` and its headless shell revision `1248`. The isolated public SDK installation lives outside the workspace under `~/Library/Caches/gents-sdk/`. No model was downloaded or loaded, cloud inference used, or paid provider called. Development SDK downloads are separate from runtime execution.

Run from `/Users/rmonzingo/repo-applications/gents-rust-lab`. Each command exits; no terminal or server stays running. The first two commands explicitly prepare public development dependencies if missing; neither is called by a task or MCP tool:

```sh
python3 scripts/prepare_task_angular_sdk.py --node /opt/homebrew/bin/node --install
env -i PATH=/usr/bin:/bin OPENSSL_CONF=/dev/null \
  PLAYWRIGHT_BROWSERS_PATH=/Users/rmonzingo/Library/Caches/gents-sdk/browsers \
  /opt/homebrew/bin/node node_modules/playwright-core/cli.js \
  install chromium-headless-shell --no-remove --no-progress

mkdir -p data/task-angular-live/2026-11-19
GENTS_NODE=/opt/homebrew/bin/node \
GENTS_NODE_VERSION=v24.21.0 \
GENTS_ANGULAR_MODULES=/Users/rmonzingo/Library/Caches/gents-sdk/angular-21.2.26/node_modules \
GENTS_PLAYWRIGHT_DRIVER="$PWD/node_modules/playwright-core" \
GENTS_ANGULAR_BROWSER=/Users/rmonzingo/Library/Caches/gents-sdk/browsers/chromium_headless_shell-1248/chrome-headless-shell-mac-arm64/chrome-headless-shell \
GENTS_ANGULAR_FIXTURE=/Users/rmonzingo/Library/Caches/gents-sdk/angular-21.2.26 \
GENTS_ANGULAR_REPORT="$PWD/data/task-angular-live/2026-11-19/runtime-cases.json" \
  cargo test --offline -p gents-local-mcp --test angular_check -- --ignored --nocapture

GENTS_NODE=/opt/homebrew/bin/node \
GENTS_ANGULAR_MODULES=/Users/rmonzingo/Library/Caches/gents-sdk/angular-21.2.26/node_modules \
GENTS_ANGULAR_COMPILER_REPORT="$PWD/data/task-angular-live/2026-11-19/compiler-boundaries.json" \
  cargo test --offline -p gents-local-mcp --lib actual_compiler_profile -- --ignored --nocapture

python3 scripts/task_angular_boundary_smoke.py --node /opt/homebrew/bin/node \
  --report data/task-angular-live/2026-11-19/browser-boundaries.json
bash scripts/check.sh
```

The live integration test verifies a real build and `0 → click → 1` DOM behavior, a real TypeScript compiler failure, rejection of a compiling `0 → click → 2` implementation despite a false “tests passed” console message, frozen-input/SDK-pin rejection, and rejection of synthetic sensitive DOM content before producing a report. The compiler probes verify seven actual isolation outcomes. The browser probes verify ten actual native OS outcomes, including a positive control for the held asset port and disposable writes. Ordinary CI runs deterministic contract, tree-identity, owner-input and privacy tests and skips installed-SDK checks explicitly.

Public scoped npm URLs exposed an existing address-scanner bug: the old scanner treated the entire registry URL around `@scope` as an email. The scanner now checks address-shaped spans around every `@`, with regression cases retaining rejection of actual addresses inside JSON, URLs and punctuation. No npm URL allowlist or privacy bypass was added. PAN/SSN checks remain in force.

The checkpoint evidence is [`evidence/159-angular-sdk-executor-2026-11-19.json`](evidence/159-angular-sdk-executor-2026-11-19.json). Earlier failed probes and final live reports remain under ignored `data/task-angular-live/2026-11-19/` and `/tmp/gents-angular-*.log`. Current results are bounded test observations; they are not native canonical task receipts or independent model qualification. Native Angular tool/receipt wiring, Cosmos execution evidence, real integration checks, additional existing providers, and fresh independent quality qualification remain open in [`151-task-engine-completion-audit.md`](151-task-engine-completion-audit.md).
