# Governed Angular task validation

`angular_check` is now a registered native MCP tool, and the `angular_build/v1` task validator calls it through the existing asynchronous task service. The actual native stdio and SSE trials use the installed Angular compiler and headless browser. Model workers in these protocol trials are mocks: a passing tool check does not establish Angular expertise, independent acceptance or production profile admission.

Implementation locations: `mcp/src/functions/angular_check.rs` registers the typed tool; `mcp/src/registry_angular.rs` owns detached dispatch/commit; `mcp/src/angular_check/native.rs` binds sources and canonical receipts. `crates/harness/src/task_workflows.rs` supplies task admission, scheduling, budgets and delivery checks; `mcp/src/task_engine/native.rs` revalidates native status. `mcp/tests/angular_native_check.rs`, `scripts/task_angular_validation_smoke.py` and `scripts/verify_task_angular_reports.py` contain the installed-SDK and independent journal checks.

The copied-source executor and its initial supported project layout are described in [`159-pinned-angular-sdk-executor.md`](159-pinned-angular-sdk-executor.md). That document's original in-process checkpoint predates this integration. The executor still supports the explicit standard browser-application subset; unsupported builders/layouts are rejected rather than treated as passing.

The owner supplies an `angular_check_contracts` entry in approved intake, plus `angular_check` and `files:read` grants for its assigned worker. The host configures absolute installed `GENTS_NODE`, `GENTS_ANGULAR_MODULES`, `GENTS_PLAYWRIGHT_DRIVER` and `GENTS_ANGULAR_BROWSER` paths. SDKs must be outside the actor's writable workspace. Runtime execution never installs packages or accepts model-selected executables, browser flags or endpoints. The contract, SDK content pins and finite DOM cases are the same strict typed inputs as the in-process executor.

Native arguments are `contract_id` and host-bound `expected_source_identity`. The existing company operator binds the latter before independent action review; asynchronous validators bind it from their frozen admission. The identity includes the exact captured input hashes, frozen contract, parent approval, company/run/owner/actor/phase, policy revision, canonical host SDK paths and implementation digests. Another actor/run or changed source/configuration cannot borrow it. Frozen JSON controls retain the existing file-write/hardlink protections.

The registry uses two short authorization transactions around detached SDK work. It commits a dispatch reservation, releases SQLite's writer, then verifies the full pinned SDK contents and runs compilation/browser checks in disposable scratch. A second transaction rechecks active-job state, approval, policy revision, current grants and the captured source identity. The observation and canonical tool audit commit together. A rejected or lost result is recorded conservatively and cannot become a passing task receipt.

Successful and failed canonical results carry a separate SDK metadata seal. A seal records sorted SDK file identities, sizes, modes, link targets, modification times and kernel change times. The tool verifies full content hashes before and after execution and requires an unchanged seal across that run. Current-result checks compare a fresh seal with that content-verified observation. Restoring an old modification time after an equal-size edit still changes the seal through kernel change time. SDK/system dynamic-library closure outside the pinned trees remains explicitly unverified.

Recursive SDK scans are forbidden inside a database transaction. `ReceiptAuthority::for_scope` captures an authorized SDK observation before the writer transaction and delegates all access decisions to the original authority. Its opaque, short-lived observation is unavailable to model JSON and grants no permissions. The commit/status transaction still checks current source, policy, approval and canonical audit custody. Missing/expired observations fail closed without an in-transaction rescan. Raw in-process callers can use the same wrapper; the default `Authority` trait extension preserves existing adapters. No observation is cached across RPC requests.

The first integrated trial exposed SQLite contention when SDK content checks held the writer. A metadata-only version still failed under concurrent load during result delivery. The final design moves both content and recursive metadata scans outside writers rather than increasing database timeouts or weakening current-source checks. Failed trial logs and an isolated failed-trial database are retained in the checkpoint evidence.

The browser controller holds the selected asset port on both IPv4 and IPv6, matching the OS profile's localhost grant, and serves only the compiled in-memory asset map. A fixed renderer-process limit supplements the sampled group limits. The required outer Seatbelt profile remains active; Chromium's own non-nestable inner sandbox is disabled inside that outer profile. Candidate console messages and exceptions do not pass the host oracle. Privacy is checked on bounded raw observations before hashes or results are persisted.

Task execution reserves an estimated 3 GiB native process-group allowance through the existing scheduler and charges the real MCP invocation to the parent task tool budget. The result records actual elapsed time and sampled group RSS rather than invented GPU or inference savings. A same-task check can be reused only with a current canonical receipt. A failed/lost invocation gets a not-run marker that prevents implicit SDK replay during replanning. Source changes require a fresh admitted request. A completed validated task is not a verified answer-cache substitute for a new required build.

At delivery and native status, the host rechecks canonical result/audit hashes, source hashes, SDK seal, actor, parent approval and policy. Changes invalidate the answer and clear its validation receipts. A compiling application with the wrong DOM result produces a failed check. An absent configured/authorized check is `not_run`; Cosmos and integration validators remain unsupported. Generated answers remain unverified even when these finite Angular checks pass.

Legacy company review now includes the frozen Angular contract, source-bound deduplication and unchanged actor/grant scoping. CEO-selected Angular work requires its assigned actor's frozen contract. Unrelated legacy prompts omit the unavailable Angular capability schema. This repaired a prompt-budget regression without changing the context budget or truncating the original mission. The native catalog remains available under its existing grant checks.

The live component test exercises actual stdio/SSE SDK runs, source/identity changes, wrong UI plus a false console success message, audit corruption, actor/phase denial, concurrent owner writes, and revocation during unlocked SDK execution. The task trial additionally proves source/audit invalidation at status, privacy rejection without a canonical result or replay, and cancellation after observing a real running browser, followed by cleanup of its entire owned process group. Expected DOM cases are frozen host inputs; all workers are explicitly mocks.

From `/Users/rmonzingo/repo-applications/gents-rust-lab`, use the SDK setup in document 159 if needed. Then run these terminating commands; no terminal/server stays running:

```sh
mkdir -p data/task-angular-native-live/2026-11-19
export GENTS_NODE=/opt/homebrew/bin/node
export GENTS_NODE_VERSION=v24.21.0
export GENTS_ANGULAR_MODULES=/Users/rmonzingo/Library/Caches/gents-sdk/angular-21.2.26/node_modules
export GENTS_PLAYWRIGHT_DRIVER="$PWD/node_modules/playwright-core"
export GENTS_ANGULAR_BROWSER=/Users/rmonzingo/Library/Caches/gents-sdk/browsers/chromium_headless_shell-1248/chrome-headless-shell-mac-arm64/chrome-headless-shell
export GENTS_ANGULAR_FIXTURE=/Users/rmonzingo/Library/Caches/gents-sdk/angular-21.2.26

cargo build --workspace --locked
GENTS_ANGULAR_NATIVE_REPORT="$PWD/data/task-angular-native-live/2026-11-19/component.json" \
  cargo test --offline -p gents-local-mcp --test angular_native_check -- --ignored --nocapture
python3 scripts/task_angular_validation_smoke.py \
  --sdk-record data/task-angular-native-live/2026-11-19/component.json \
  --report data/task-angular-native-live/2026-11-19/task-stdio.json
python3 scripts/task_angular_validation_smoke.py --sse \
  --sdk-record data/task-angular-native-live/2026-11-19/component.json \
  --report data/task-angular-native-live/2026-11-19/task-sse.json
python3 scripts/verify_task_angular_reports.py \
  data/task-angular-native-live/2026-11-19/task-stdio.json \
  data/task-angular-native-live/2026-11-19/task-sse.json \
  --output data/task-angular-native-live/2026-11-19/verification.json
bash scripts/check.sh
```

These SDK environment variables contain paths/versions, not credentials. Each task trial creates an isolated temporary company and local port, and saves its report plus an archived SQLite database at the requested report path. It does not operate the default server or publish source. The checkpoint evidence is [`evidence/160-governed-angular-validation-2026-11-19.json`](evidence/160-governed-angular-validation-2026-11-19.json). The complete original scope remains active in [`151-task-engine-completion-audit.md`](151-task-engine-completion-audit.md): Cosmos/query/RU and real cross-domain integration, broader providers and independent quality qualification still require work.
