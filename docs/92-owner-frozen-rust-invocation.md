# Owner-frozen Rust invocation

This records the initial adapter checkpoint. [Document 93](93-real-local-component-reuse.md) adds the subsequent approved-source admission repair, actual local-company reuse qualification and newer office bundle. Browser/CEO component-selection integration remains open.

The private ABI adapter from document 91 is now available as native MCP tool `owned_rust_call`, under explicit owner-frozen invocation requirements and existing current governance. This is a new execution boundary, not automatic package promotion, a new model-competence result or industry certification. The broad goal remains active.

## The execution contract

The request contains only `binding_id` and positional `arguments`. The approving owner freezes `required_owned_rust_calls` in the SOW. Each requirement binds its unique id, assigned actor, source path/SHA-256, API SHA-256, function, exact input SHA-256 and an explicitly **owner-declared** historical review-package id. That provenance field is data, not a native assertion that a package qualified. A model cannot select a new path, supply code/compiler flags, change the function/input or turn package metadata into permission.

The normal registry validates the authenticated company/owner/run, worker phase, current policy and optional expected epoch. The new read-only computation also requires an approved active job. Its source read uses the existing descriptor-relative workspace adapter, bounded single-link ordinary files and full digest checks. Its source path becomes a frozen owner input, protected from agent writes and filesystem aliases. No source-authoring or acceptance path may double as an owned library input. The actual current API digest must match before invocation. The existing pure Rust dialect and deny-default compiler/runtime sandbox remain the execution boundary; unsupported isolation fails closed.

`mcp/src/owned_components.rs` handles frozen requirements, custody, invocation evidence and completion. `mcp/src/functions/owned_rust_call.rs` is the thin catalog/contract adapter. The generic typed codec remains in `component_api`, and transport remains in `client`/`transports`. No new dependency was added.

Native completion requires a successful invocation observation **and** its canonical audit receipt for the exact run, epoch, actor, binding, source/API/input and full request/result digests. It rechecks current source bytes and API identity. A completed tool exchange returning `passed=false` cannot satisfy this gate, even though the generic call requirement records the exchange. Internal observation rows without audit receipts, another run's receipts, changed source/API or an old epoch do not qualify. Final task acceptance remains separate independent QA.

Stage views now retain owned-invocation facts and filter them for intermediate actors, while final review requires all frozen actors. Admin admission receives the frozen invocation identities. A CEO plan selecting this tool must have owner-frozen bindings for its assigned actor before intake; the plan itself does not supply authority.

## Transport requirements are implementation metadata

The native client previously special-cased `rust_verify` for its installed compiler and longer exchange deadline. `Tool::runtime_requirements` now supplies trusted implementation metadata. Both Rust tools request the installed compiler and a 110-second native exchange deadline; other tools keep their previous defaults. Model arguments cannot set these values. Sandbox CPU, wall, output and observed-memory limits remain separate and unchanged.

This avoids another tool-name branch when adding an adapter with the same local execution needs. The implementation catalog and basement disclose these requirements, along with request file fields, source, phases, data fields, schemas and effect hints. Integer wire values remain canonical decimal strings to preserve 64-bit values through JavaScript.

## Owner preparation, without activation

`scripts/mcp-owned-invocation.mjs:prepareOwnedInvocation` reads a sealed review package, inspects the exact source through a copied owned helper and prepares a requirement, source-file snapshot and native request. It checks identities, function and arity, clones input data and computes its canonical digest. Native execution still validates the actual types, ranges and full identities. Preparation writes no workspace file, executes no candidate, installs no policy and grants no authority.

The host can use that packet when presenting a complete SOW for owner review. Source staging and invocation must then use the approved run and current actor policy. Existing review packages stay immutable and globally inactive. The browser/CEO preparation flow still needs component-binding selection and real-model reuse qualification; a bare CEO tool selection is deliberately insufficient.

## Verification

`owned-rust-call-final-full-check.log` is a complete `scripts/check.sh` exit-0 run: **433 default Rust tests**, 20 opt-in exclusions, formatting, strict Clippy, actual stdio/SSE, HTTP/Python/Node checks and build. A later two-line basement disclosure addition passed syntax/build in `owned-rust-call-catalog-ui-build.log`; a Python fixture docstring was clarified without changing behavior.

Seven real native tests cover source/API/input and canonical receipt linkage; actor/phase/input/epoch/grant/terminal-job refusal; frozen source writes; changed source/API; failed runtime versus completed exchange; revocation; cross-run isolation; and actual stdio/SSE child execution with installed compiler requirements. Stage-view tests prevent an intermediate pass from becoming global acceptance. Four private owner-utility tests cover preparation, custody and codec behavior. These use unrelated protocol identity functions, not frontier-authored task solutions.

`rust_submission_smoke.py --owned` and `--owned --sse` also passed through the real HTTP controller, native child, source-bound completion gate, admin review and independent QA. Planning/intents/judgments are seeded or simulated, and the author model is deliberately unavailable. Both call and finish are seeded proposals in this variant; it does **not** claim an automatically compiled finish or actual model planning. There are zero author generations. This establishes wiring, not company competence or qualification of an arbitrary package.

The prior successful real-model range-union trial remains evidence for its original bundle `e947053374bf5871744301cbbe8a28aa9ad3e3ac01bc76604f40a3377602c733`. It is not silently relabeled as qualification of this later bundle or new reuse workflow. The failed settlement retest remains retained.

## Current local office

The office at `http://127.0.0.1:8788/?autoconnect=1` was refreshed only after verifying its exact `data/mcp-curriculum.sqlite3` open file, the same six terminal run ids and zero generating plans in their company. The former owned supervisor exited 0. No issued work was interrupted or replayed.

- Current sealed native bundle: `171070ff896668cd5a961f3288989cab554266313dee41b1d56ffbc7b93400e7`.
- Manifest SHA-256: `73a4678ae6a4946607f177fdc5ec32fee144c84d1cf26b0e1ef9e37367760965`.
- Unchanged profile SHA-256: `23b4f92d345691164eebb68202c55732f07c3d228234367542187a063ae04e28`.
- Supervisor exec session: `87889`; local-only SSE, scheduler/activity disabled, same run store, thirteen native functions.

`owned-rust-call-public-pre-refresh.json` and `owned-rust-call-public-office-observation.json` record the read-only checks. The browser was reloaded to replace its stale twelve-function catalog; the B1 disclosure visibly shows thirteen functions, the new tool, schemas and compiler/deadline metadata. The existing themed basement is preserved. Screenshot: `data/mcp-basement-verification/owned-rust-call-catalog.jpg`. Browser extension automation failed, so verification used documented native Chrome controls; no unrelated tabs were modified.

No models were downloaded, no training was performed and no paid provider/tool calls were made for these checks. The offline daily model check was NOT_DUE with latest REVIEW_REQUIRED (`owned-rust-call-daily-model-scan.json`), not a vulnerability-free certificate. Local compute and Codex development credits are separate costs. The next required evidence is an unchanged fresh task where the **actual local CEO** selects roles and a stored component, current native policy admits its frozen invocation, agents produce the result, and independent acceptance plus per-agent completion archive proof pass. Harder compositions, acquisition/training automation, additional hardware/OS isolation, multimodal semantics and industry assurance remain open.
