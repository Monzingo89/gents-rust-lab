# Scoped task progress in the building

Registered workflows already retained dispatch, return, tool, plan and validation events in the company audit journal. The building's activity display only read legacy run events, so a task executing through the owner task workbench could leave its CEO monitor idle.

The existing task-status result now has an optional `progress` field. `mcp/src/task_engine/progress.rs` supplies the same projection for owner HTTP status and native MCP status, after their current authorization/source checks. No new service, table, execution engine, model call, grant or paid provider is introduced.

## Progress and authority

The projection joins the subscriber and workflow job by **tenant, subject and task ID**. Native subjects retain their actor/run-private custody. Owner reads still validate the frozen job hash, current runtime, full source snapshot and access scope. Native reads still validate current grants, policy epoch, parent approval, worker lease, source bytes and runtime receipts. Invalidated evidence returns no progress metadata. A different actor/run/company cannot borrow the stream.

Only fixed host event types, journal sequence/time, call UUID and bounded host node ID are returned. Prompts, model answers, source quotes, tool arguments/results, raw errors, paths and native process details are excluded. The SQL projection does not load raw event bodies into the returned packet. It scans at most 257 matching records, exposes at most the most recent 16 events, and marks scan truncation. Missing/malformed identities or incomplete scan coverage produce unknown counters rather than false zeroes.

Model requests and replies are deduplicated by call ID. Coalesced subscribers point to the same job identity; cancelling one subscriber makes that subscriber inactive even if its peer continues. Terminal cancellation, interruption or failure never remains animated as an active request. Historical unresolved dispatches remain observable separately. **Dispatch is not proof of actual inference or acceptance.** Running/interrupted/cancelled reads with recorded dispatches leave actual call/token usage unknown rather than preserving initial zero values. Final task measurements remain separate from journal progress counts.

The generated schema is refreshed using the existing command:

```sh
target/debug/gents-local-mcp --task-schemas > config/schemas/task-engine-v1.json
```

The generated-schema equality test remains unchanged. Old persisted task results remain readable because the new field is optional and omitted when absent. Exact/source answers do not acquire a synthetic workflow stream. The older grounded-answer path remains separate pending lifecycle parity evidence.

## Building behavior

The task workbench publishes only status/identity/numeric progress to its building monitor. It renders recent host events as text, preserving source evidence and independent acceptance as separate sections. The CEO sprite is labelled **Selected task monitor**; it represents a deterministic view of that task, not an additional CEO model invocation. Its existing active animation follows running status and stops at terminal status. The CEO room's selected-task screen shows the host phase.

The selected company/origin/auth context bounds this display. Clear view, company change, connection change and logout clear the monitor without cancelling the underlying task. A failed status request clears the activity cue. Observations older than 15 seconds become stale/inactive when the display next updates. No model output chooses actor movement or accounting state. The optional office tour remains labelled staged presentation.

CFO and expanded dashboard facts show selected-task calls/cost separately from the retained-journal totals. This avoids double counting shared jobs or presenting the older journal view as a complete billing ledger. Runtime budgets and enforcement remain in the existing deterministic ledgers. This slice does not aggregate every historical task or expose other actors' private tasks in the building.

## Verification and remaining scope

```sh
cargo test -p gents-local-mcp --test task_progress --test native_task_queue --offline --locked
node --test scripts/task-activity.test.mjs scripts/office-activity.test.mjs scripts/room-activity.test.mjs scripts/ceo-execution.test.mjs
python3 scripts/task_workflows_smoke.py --browser
python3 scripts/task_native_smoke.py --sse
python3 scripts/local_evaluate.py
```

The fixed Rust projection tests cover parallel pending calls, duplicate event IDs, terminal subscribers, cross-subject/tenant/job joins, invalidated evidence, payload exclusion, malformed JSON, bounded history and incomplete-count handling. Existing owner/native smokes now also assert shared progress identities, finished request/return counts and metadata suppression after source/access invalidation. Their original governance, cancellation, budget and restart assertions remain intact.

The installed-Chrome browser check observes actual HTTP/SQLite workflow events with marked mock workers: pending request, three replies, inactive completion, the rendered CEO screen and immediate company-reset cleanup. Existing inert-untrusted-text and mobile layout checks remain. The browser also caught a stale manifest label waiting for an animation frame; reset now updates it synchronously. A screenshot is retained privately in `data/task-architecture-verification/task-runtime-pending.png`.

These tests validate orchestration, custody and presentation. They do not qualify real-model task completion, factual accuracy, peak/GPU memory or actual model routing quality. No models were loaded. Full evidence and measurement scope are in [scoped-task-progress-2026-11-21.json](evidence/scoped-task-progress-2026-11-21.json). Broader billing aggregation, all-task building views and grounded/RAG implementation consolidation remain in the backlog.
