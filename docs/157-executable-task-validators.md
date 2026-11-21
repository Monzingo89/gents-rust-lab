# Executable task validators and native SDK receipts

The task factory now attaches the registered profile's validators to every node, together with input custody and output shape. Previously it always attached only `output_shape`. The executor runs the union once after all nodes return, retaining per-check results in the final task result. Unknown validators and dropped profile checks are rejected. Mixed-profile graphs must retain each node's own required checks. This does not qualify a profile or turn a generated answer into a verified factual answer.

## Registry and states

`mcp/src/task_engine/validators.rs` implements a finite registry:

| Validator | Current execution and meaning |
| --- | --- |
| `input_custody/v1` | Checks task ID and the exact original input digest against the plan. Does not prove that a draft fully answers the request. |
| `output_shape/v1` | All scheduled nodes returned bounded, schema-checked answers; requested JSON parses. Does not establish factual or functional correctness. |
| `current_citations/v1` | Quotes belong to the actual node evidence and its TTL is current. Delivery/status still recheck access and source bytes. Membership is not entailment, coverage, contradiction resolution or historical expertise. |
| `dotnet_build/v1` | Executes the existing owner-frozen native offline SDK check when the task has a native actor/run, current grants and admission-bound inputs. Otherwise explicitly `not_run`. |
| `dotnet_api/v1` | Separately granted owner-frozen finite HTTP cases against the actual copied application; see [158](158-frozen-dotnet-api-checks.md). A build-only contract cannot execute it. |
| `angular_build/v1`, `cosmos_receipts/v1`, `integration_receipts/v1` | Explicitly `unsupported`; no runtime implementation is advertised by these profile names. |
| `independent_acceptance/v1` | Explicitly `not_run`; model self-assessment is not independent acceptance. |

An actual native check failure blocks answer delivery. An unrun or unsupported check remains visible and cannot be promoted to a pass. A successful compile does not change `verification_state` to `verified` or enter the verified answer cache. The overall task-engine goal remains active; Angular/Cosmos integration, broader API support and independent production-quality qualification remain open.

`ExecutionPolicy.permitted_validators` is trusted host configuration, separate from the permitted read tools exposed to a model. For native tasks, `dotnet_build` becomes available only through the current actor's `dotnet_check` grant. The SDK check independently requires `files:read` and an exact owner-frozen contract. Model JSON cannot populate this policy or add an executable validator. Profile registration is metadata-only and bounded at 4096 profiles.

## Native .NET path

Use the owner contract described in [`156-offline-dotnet-build-checks.md`](156-offline-dotnet-build-checks.md), an explicitly enabled/allowlisted native task worker, and a worker model admitted for the `dotnet` profile. Start an ordinary native `execute_task` request with `hints.domain="dotnet"`. The fixture's mock model is only protocol infrastructure; its evaluation metadata does not admit any real specialist.

At admission the host reads authorized input bytes and freezes the exact build/source identity for each matching contract. This identity joins the workflow coalescing key. A source change before a later model or SDK dispatch stops that admission; it does not silently select new inputs. The current implementation freezes runtime checks for the profile selected at admission. If model intake later chooses a runtime check with no admission binding, that check remains `not_run`; it cannot acquire authority or bind new code through model output.

After the workflow finishes, the host reserves a scheduler slot and 2 GiB for the SDK process group (a conservative enforcement-limit estimate, not a measurement). Each actual native request charges the same task-wide tool budget used by read tools. Intake, worker, repair and revision model calls remain charged to the existing task and parent-run ledgers. The model cannot request extra tool or model allowance. Duplicate in-flight subscribers can share this context-compatible execution; a completed runtime-validated workflow is not reused as a substitute for another build. A cached fact does not satisfy an owner-required SDK check, so that answer-cache result is ineligible for the runtime-required path.

Only one build runs per contract after a three-stage pipeline, rather than one for each node. Global cancellation/deadline checks continue during validation; dropping the native client kills its owned MCP process, and the SDK pipe watchdog stops the compile group. Failed/lost checks are not automatically replayed or used as a reason to regenerate unchanged code. The `dotnet_check` path does not run a candidate application. The separately granted API path in [158](158-frozen-dotnet-api-checks.md) executes only frozen HTTP checks. Native action tools remain outside the read-only model tool loop.

## Short transactions around SDK work

`mcp/src/registry_dotnet.rs` separates the long SDK operation from SQLite's writer lock:

1. A short immediate transaction checks phase, current policy, approved active parent and native contract execution rules; captures explicit source inputs and installed SDK identity; and records a dispatch reservation.
2. The SDK compiles its copied scratch snapshot without an open database transaction. Status reads, worker lease refreshes and owner governance changes remain available.
3. A second immediate transaction rechecks policy revision, parent approval, current grants, contract rules and exact source identity. Only then do the build observation and canonical MCP receipt commit together.

A source change or owner revocation during compilation prevents commitment of a successful result. A lost audit commit leaves no authoritative build observation. The durable dispatch is not proof of a build, a returned answer or model inference, and does not authorize replay after restart.

The live component regression writes to SQLite and changes source after an actual SDK dispatch, proving the writer is available and the stale snapshot cannot commit. A second actual SDK dispatch loses its grant during execution and cannot commit against the old revision. Both use the installed SDK and retain the existing macOS deny-default scratch sandbox.

## Results, status and compatibility

`TaskResult.validation_receipts` is an optional additive field, absent when empty. Existing stored task results deserialize with an empty list. Generated schemas are updated in `config/schemas/task-engine-v1.json`. Each compact receipt records the validator/tool/contract, actor-policy epoch through the native scope, source identity, input-snapshot digest, canonical argument/result hashes, managed artifact hash, outcome, elapsed time and available sampled process-group RSS. Full per-file hashes remain in the canonical native build result, avoiding duplicated oversized MCP frames.

Native status rechecks the current contract, source bytes, executable hash, grants and canonical audit record before exposing a build claim. Source edits, SDK changes or missing canonical receipts invalidate it and clear the answer and runtime receipts. A different actor cannot retrieve the task. The configured trusted MCP client optionally forwards the installed SDK path for status/cancel validation; ordinary task status does not require installing .NET.

SDK phase metrics now use `null` when no sample exists and include `resource_samples`. This replaces the earlier zero-initialized sampled maxima for very short phases. Nonzero RSS samples remain process-group observations, not total system/Metal/unified memory. SDK/runtime library closure is still not fully content-addressed, and scratch-path debug metadata can change DLL bytes for identical sources. Those limits are not hidden by a passing check.

## Verification commands

```bash
cd /Users/rmonzingo/repo-applications/gents-rust-lab
cargo build --offline --workspace
cargo test --offline -p gents-local-mcp --test task_validators
python3 scripts/task_validation_smoke.py
python3 scripts/task_validation_smoke.py --sse
GENTS_DOTNET=/opt/homebrew/bin/dotnet python3 scripts/task_validation_smoke.py --dotnet-live
GENTS_DOTNET=/opt/homebrew/bin/dotnet python3 scripts/task_validation_smoke.py --dotnet-live --sse
GENTS_DOTNET=/opt/homebrew/bin/dotnet \
  cargo test --offline -p gents-local-mcp --test dotnet_check -- --include-ignored --nocapture
CARGO_NET_OFFLINE=true bash scripts/check.sh
```

All commands exit when finished; no server terminal stays running. The live .NET fixture requires already-installed SDK `10.0.401` and an explicit absolute runtime path. It does not download a missing SDK. `--report <new-json-path>` retains actual task results and runtime binary hashes in an existing parent directory. Ordinary CI runs the no-SDK native checks over both transports, explicitly reporting absent/unsupported checks rather than pretending to compile.

The actual SDK task fixture verifies three stages/one validation, shared tool accounting, a passing build, a compiler failure that blocks the task, repeated completion requiring a new build, current-source invalidation, missing-receipt invalidation, actor separation and a source change during a pipeline preventing SDK dispatch. Worker replies are mocked; these are execution/custody tests, not model-quality or API-integration benchmarks. Provider charges and actual model inference are zero.

Checkpoint: [`evidence/157-task-validators-2026-11-19.json`](evidence/157-task-validators-2026-11-19.json) records the passing full repository check (671 Rust passes, 28 opt-in skips), separate installed-SDK component/task trials, and actual stdio/SSE runtime binary hashes. Six canonical SDK results were independently hash-checked against native audits, task references, approved intake digests and current implementation/input identities. The first full-check failure is retained: the old workspace fixture assumed acceptance-row order. Its repair checks the still-unrun independent acceptance row by name; it does not weaken that requirement.
