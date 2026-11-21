---
name: building-interactions
description: Connect building sprites, interactive objects and CEO/CFO screens to real scoped runtime events without using inference for animation or accounting.
---

Resolve paths from the repository root. Trace `web/app.js`, `web/assets/office-activity.js`, `room-activity.js`, `office-dashboards.js` and `tower-scene.js` to `crates/harness/src/office_activity.rs`, `observability.rs` and actual persisted events. Read [the baseline](../../../docs/162-local-execution-baseline.md) for coverage limits.

Derive execution state from scoped run/call/tool/checkpoint events. Keep stable actor identity and current task/run IDs. In-flight, completed, failed, cancelled and uncertain states must remain distinguishable. Room selection, sprite movement and hover details use deterministic UI logic, never model calls.

Keep the office tour and walking preview visibly marked as staged presentation. They do not set real inference busy or establish that an agent performed work. Populate screens from runtime receipts/sensors, with unavailable values preserved through totals, history and detail panels. Do not turn a missing charge/token/sensor into zero. CFO enforcement stays in deterministic approved-budget ledgers, not model output or a screen total.

The shared task-status progress projection lives in `mcp/src/task_engine/progress.rs`. It is attached only after current owner/native authorization and evidence checks. Its dispatch counts describe pending requests, not measured inference. The CEO monitor displays only the selected task; its usage remains separate from retained legacy journal totals. Clear monitoring immediately on company/origin/auth changes and mark stale observations inactive.

The CFO company screen reads the owner-authenticated `usage` task operation backed by `crates/harness/src/task_usage.rs`. Aggregate retained jobs/call IDs, not subscriber result copies. Preserve unknown totals for unresolved, conflicting, malformed or truncated records. Keep company task receipts separate from host legacy journals and observer history; a complete retained scan is not lifetime billing coverage. Usage reads do not refresh sources or confer runtime grants. Clear in-flight responses and stale company usage on context/auth changes.

Run `node --test scripts/office-activity.test.mjs scripts/room-activity.test.mjs scripts/ceo-execution.test.mjs scripts/task-activity.test.mjs`, `cargo test -p gents-harness office_activity --offline --locked`, `cargo test -p gents-harness task_usage --offline --locked` and `cargo test -p gents-local-mcp --test task_progress --offline --locked` for affected behavior. Run `npm run build` after UI edits. `python3 scripts/task_workflows_smoke.py --browser` uses the installed browser to check the selected-task monitor and receipt-backed CFO screen against real local HTTP/SQLite with marked mock workers. For visual/interaction claims refresh the actual target; syntax/build success alone is not browser proof. Never start a live company or model solely to animate a preview.
