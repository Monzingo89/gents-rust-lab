---
name: harness-routing
description: Trace or change task intake, workflow selection and local model admission while keeping CEO proposals subject to host policy and shared budgets.
---

Resolve paths from the current repository root. Inspect `crates/harness/src/task_frontdoor.rs`, `task_workflows.rs`, `task_native.rs` and `ceo_fastpath.rs`; follow shared services under `mcp/src/task_engine/`. Read [the baseline](../../../docs/162-local-execution-baseline.md) and the relevant API contract in `docs/150-adaptive-local-task-engine.md` or `docs/155-native-asynchronous-tasks.md`.

Try authorization-aware exact cache/source extraction before model admission. Bind cache identity to the original request, output/context/constraints, tenant/subject, current access/policy/profile/runtime and source snapshot. Recheck freshness and revocation on delivery. Never cache generated drafts as verified facts.

Treat CEO/model routing as a proposal. Host code validates registered templates, acyclic bounded dependencies, evaluated compatible profiles, tool intersections, model pins, context/memory feasibility, deadlines and remaining call/token budgets. Unsupported actions remain in existing governed executors; a hint cannot create an executor or grant.

Reuse `RuntimePool`, `local_runtime` and the existing task factory. Profiles are metadata, not separate resident models. Use measured whole-path latency when supplied; otherwise preserve the admitted default and label routing optimality unknown. Keep local-only policy and no automatic paid fallback. A new paid route requires an explicit owner spending policy before dispatch.

Before ranking latency, size the known registered root packets with the shared `StageInput::for_node` factory and filter against resolved input-byte ceilings as well as evaluated profile and memory bounds. Later dependency/tool growth and actual tokenizer context still require dispatch checks. Analysis-only proposals defer worker context validation. Treat owner-configured latency as a ranking input; benchmark provenance, comparable workloads and optimality need separate evidence.

Use the shared `model_pins.rs` admission service for grounded and workflow model identities. Do not construct unrelated executor settings to check a pin. Warm identity caches must recheck root/path security metadata and company adapter scope; successful hashing grants no execution authority.

Each newly launched asynchronous job holds `office_activity::ForegroundWork` through execution and cleanup so optional efficiency research yields immediately. Coalesced subscribers share the job's guard. Keep research cancellation reset and foreground admission under the same small priority gate; shutdown closes it permanently for that service instance. This gate carries no task grants or call/token budgets. Preserve the existing runtime pool and durable research interval. `python3 scripts/task_workflows_smoke.py --grounded` verifies the older grounded adapter through HTTP alongside the workflow/native checks.

Verify with `cargo test -p gents-local-mcp --test intake_replans --offline --locked` and the affected HTTP/native smoke. `python3 scripts/local_evaluate.py` runs the bounded fixture baseline. Mocks validate orchestration, not task-solving competence. Read actual result status and verification state.
