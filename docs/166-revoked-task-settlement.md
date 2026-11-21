# Terminal settlement after native task revocation

November 21, 2026. The previous native smoke revised policy immediately after queuing. That proved invalidation but could run before model dispatch. It now waits for one committed `task_model_dispatch` with no matching return before changing the epoch. The stronger test failed: the executor stopped but its final checkpoint attempted current source authorization, failed, and left the workflow job `running`.

## Runtime change

`task_workflows.rs` now handles expired native custody before attempting normal result publication. After stopping this job's session, a private host helper validates the exact tenant/subject/job, stored checkpoint digest and frozen job content inside one immediate transaction. It marks the job completed with `evidence_invalidated`, redacts answer/evidence/artifacts/plan/analysis/acceptance receipts and updates only its bound subscribers and native queue rows. An already cancelled subscriber remains cancelled. The existing terminal audit kind and retention policy are reused.

This lifecycle write neither regrants model/source/tool access nor reconstructs a current data-access scope. It stores no new generated payload, performs no inference and cannot reuse a changed, corrupt or already terminal checkpoint. Failed checkpoint integrity still stops without replay. The job's original frozen input remains in its existing private durable record for investigation. Optional unresolved model usage, tokens, memory and latency remain unknown; dispatch reservations do not prove actual inference. Existing local-only admission establishes zero provider fees.

Regranting a later epoch cannot revive the original task. Normal successful publication keeps its existing current-source, workspace, native-receipt and policy checks. Owner-only execution and other execution engines are unchanged.

## Structured handoff recovery

The existing memory handoff already stores a typed binding to source ID/hash, accepted record hash, sender, recipient, run and policy revision. The test now serializes this binding to a checkpoint, closes the company database, reopens it and retrieves the same original through `SqliteHandoffs::revalidate`. The existing forgery, changed bytes, other owner/run/recipient, revocation and regrant denials then run against that reopened connection. No cached summary becomes authority, and the model-facing packet gains no database identifiers or expected downstream answer.

This verifies one existing record-handoff recovery contract; it does not qualify semantic recall, arbitrary agent context transfer or every executor's crash recovery.

## Verification

The strengthened actual stdio and SSE transport smokes use delayed **mock workers**, HTTP governance and private SQLite stores. Both observed one pending request before revocation, a persisted redacted terminal job, exactly one dispatch and zero admitted return checkpoints, and continued invalidation after regrant. Restart also waits for a pending request and retains the existing no-replay assertion. No model weights are loaded by these checks; zero return checkpoints do not prove that an external backend incurred zero work.

A new Rust test rejects another tenant/subject and a corrupted checkpoint, preserves a cancelled subscriber, checks payload redaction and unknown usage, and rejects a second terminal write without adding another event. The memory binding reopen test passes. Full command results, source hashes, retained regression log hash and local suite measurements are in [the evidence record](evidence/revoked-task-settlement-2026-11-21.json).

The final workspace run passed **705 Rust tests**, with 33 opt-in tests ignored. Strict offline all-target Clippy, 32 broker/governance tests and two evaluator failure/timeout tests passed. All eight local suite checks passed in **130.49 seconds**, including the final persisted-checkpoint and pending-restart assertions on both transports. These are command durations including setup/teardown, not model latency. Formatting, diff checks and canonical/Claude/Kimi/CodeGPT discovery links passed. The platform-dependent full `scripts/check.sh` and a new visual browser session were not run in this slice.

From the repository root, the fixed local suite includes both stronger native checks:

```sh
python3 scripts/local_evaluate.py
cargo test -p gents-harness terminal_cleanup --offline --locked
cargo test -p gents-harness memory_handoff --offline --locked
```

This slice adds no model call, download, provider fallback or production configuration change. Real-model task competence, revocation during actual backend inference, peak/GPU memory and total hardware/electricity/development costs remain unknown for this slice. See [the prior real-model trial](165-shared-model-pins.md) for its separately measured scope. Existing company services were not restarted.
