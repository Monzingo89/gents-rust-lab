# 13 — Final reliability and security additions

The final starter now includes persistent local checkpoints, replay protection, bounded runtime, deterministic acceptance rules, recoverable history and a CI template. These improve the learning harness without claiming to implement a full distributed workflow engine.

## 1. Local recovery

By default the backend creates `data/runs.sqlite3`. The SQLite database is not committed or included in the ZIP. It stores run metadata and observable events, including outputs; treat it as potentially sensitive.

Each event is checkpointed through a blocking SQLite worker so disk I/O does not block an async runtime thread. A `call_started` checkpoint is written before dispatch. Call start/completion/failure records have a stable `call_id` and millisecond timestamps. Snapshot revisions reject stale writes, and deletion tombstones prevent late background writes resurrecting deleted runs. SQLite uses full synchronous commits and a process lock so two harness processes cannot use one journal simultaneously.

If you stop/crash the backend mid-run, restart with the same journal. A previously running job becomes `interrupted`; its history is retained, but it is NOT rerun automatically. An in-flight request might have been billed even when no response was recorded. Inspect provider records before intentionally submitting a new attempt.

```bash
RUN_STORE=data/runs.sqlite3 cargo run -p gents-harness --locked
```

For isolated throwaway execution:

```bash
RUN_STORE=:memory: cargo run -p gents-harness --locked
```

Use the office's Saved runs list to reopen a run. The client stores only the last run ID per backend origin in localStorage, not credentials or task bodies. After reconnecting, it can recover that run's snapshot and resume polling if still active. The API checks ownership again; another user's ID does not grant access. Sign-out clears the visible history and last-run reference. It does not revoke the browser's Microsoft session or undo a provider call.

## 2. Retention and deletion

The global teaching cap is 20 retained runs. New admissions evict the oldest terminal run when necessary; active work is not evicted. Startup cleanup expires terminal records older than `RUN_RETENTION_HOURS` (168 by default; valid 1..8760). This cleanup occurs when the service starts, not as an exact live TTL scheduler.

`GET /api/runs` returns the caller's retained run IDs, statuses and timestamps. `GET /api/runs/<id>` returns a snapshot. `DELETE /api/runs/<id>` removes a terminal record owned by the caller; active work returns 409 until cancelled and stopped. The UI exposes deletion for terminal records.

SQLite file permissions are 0600 on Unix, but the contents are not encrypted by this application. Use FileVault/encrypted storage and protect backups. Deletion empties the live row payload and retains a small run-ID tombstone; it is not guaranteed erasure of backups or disk remnants. Tombstones consume a small amount of disk space per deleted run; offline maintenance may compact them only after the backend is stopped and no writes can still arrive. Read SECURITY.md before persisting enterprise data.

## 3. Idempotent task submission

The UI generates an `Idempotency-Key` UUID per submission and retains it in memory through an ambiguous failed submission. A duplicate key with the same authenticated owner, request and configuration returns the original run ID; a conflicting request returns 409. The duplicate check runs before quota/admission checks. The UI also disables Start while submission is in flight.

```bash
curl -X POST http://127.0.0.1:8787/api/runs \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: my-first-safe-submission' \
  -d '{"task":"Write a brief comparison using the supplied facts","consent":false}'
```

Repeat that command to retrieve the same run rather than spending on another attempt. Use a NEW key for an intentionally new attempt or changed task/configuration. Keys are optional for compatibility, but submissions without them have no replay protection. Keys must be 1..128 ASCII letters, digits, dash or underscore.

The association is owner-scoped, persisted, and retained only as long as its run record. Deletion or expiry removes replay protection. The UI's pending submission key does not survive app termination; use Saved runs/history to inspect ambiguous work after a restart. This is not a guarantee of exactly-once upstream model inference.

## 4. Wall-clock and admission limits

`max_run_seconds` defaults to 900 and accepts 1..3600 in either fixed or judge configuration. The supervisor catches worker panics and enforces the wall-clock deadline. A timeout records `timed_out`; it never records success or automatically retries. Dropping an HTTP future does not guarantee a provider has stopped computing or charging.

User cancellation remains cooperative between calls; the active call can finish and record usage. Deadlines provide the hard runtime bound. A process crash or forced termination instead leaves a recoverable `interrupted` run on next startup.

One caller may have one active run; the process allows two globally. It admits at most six new runs per minute per user. Replay retrieval does not consume a new-run quota. Rate-window bookkeeping is bounded and expires in process memory; it resets on restart. These are lab quotas, not a distributed rate-limiter or a guaranteed monetary cap.

## 5. Deterministic acceptance gates

Expand Deterministic acceptance checks in the task UI. Required and forbidden lines are exact case-sensitive substrings checked by Rust against the final candidate. Up to 16 checks are allowed, each at most 500 bytes. In judge mode they apply in addition to the frozen definition of done.

For example:

```json
{
  "task": "Recommend a vendor from the supplied facts",
  "consent": true,
  "definition_of_done": ["Support all four users within the $40 budget"],
  "acceptance_checks": {
    "required_substrings": ["Vendor B"],
    "forbidden_substrings": ["guaranteed savings"]
  }
}
```

The planner and judge receive these requirements. Rust evaluates them and a failed rule vetoes an AI `accepted` verdict, emits `acceptance_veto`, and sends the failure into the next routing round. The judge cannot waive the rule. If the round/call limit is exhausted, the result remains incomplete.

These simple checks are demonstrative, not semantic proof. For real software work, add typed deterministic validators for unit-test results, schemas, required artifacts and evidence provenance. Do not execute arbitrary tests or shell commands supplied by an LLM. Fixed mode rejects nonempty deterministic acceptance rules because it has no completion/repair judge.

## 6. Provider and input boundaries

Model configuration validates HTTPS URLs, port 443, no embedded credentials/fragments, and approved provider hostnames. Only `api-version` query parameters are allowed; authentication belongs in headers. Redirects are disabled. Provider responses are capped at 2 MiB and text at 65,536 characters. Every model call has a global 128,000-character prompt ceiling; judge-mode context may impose a lower configured ceiling. These are character/resource guards, not tokenizer-accurate context admission.

Default hosts cover the documented OpenRouter router, HF router/dedicated endpoints, Azure Databricks workspace hosts and listed Azure AI endpoint families. If your approved provider uses a custom domain, operators may set an explicit comma-separated `PROVIDER_ALLOWED_HOSTS` list of additional exact hosts. That is an operator trust decision, not a model-controlled option. DNS/IP egress policy still belongs in network controls for higher-assurance deployments.

Entra signing-key refresh is throttled for unknown keys, token size is bounded, and responses use no-store/nosniff/no-referrer headers. Only the three known UI assets are served; unknown paths do not expose arbitrary files in the web folder. HTTP model failures produce correlated `call_failed` events without exposing provider response bodies. Failed/aborted calls may have unknown usage; the totals are not a complete billing ledger.

## 7. Azure deployment implications

The Docker image creates a writable `/app/data` directory for its non-root user. This makes the sample start, but a container's local writable layer is ephemeral. Provision and mount a suitable durable volume and set `RUN_STORE=/data/runs.sqlite3` if you want the journal retained across container replacement. Protect volume permissions and backups; test actual restart durability. SQLite/file-lock behavior on remote filesystems must be validated for your environment.

Keep one replica and one active revision for this starter. A local file or shared mount is not a design for distributed worker coordination. For reliable multi-replica Azure execution, replace Store with a transactional shared repository, add a queue, leases and per-step idempotency; reconcile ambiguous provider calls rather than blindly replaying them. The code does not auto-provision a durable volume or shared database.

## 8. CI and final verification

The supplied `.github/workflows/ci.yml` runs on push/pull request once you put this folder in a GitHub repository. It uses SHA-pinned official checkout/setup-node actions, read-only repository permissions, no persisted checkout credentials and a 15-minute job timeout. It installs the tested Rust toolchain, runs formatting/lint/tests, exercises execution/recovery, builds the web UI and checks npm advisories. No cloud/model/signing credentials are required. It has not been run in your GitHub account.

Run locally:

```bash
npm ci
bash scripts/check.sh
```

The added `scripts/reliability_smoke.py` checks persistent duplicate submissions, conflicting keys, admission limits, active deletion refusal, call IDs/timestamps, forced-crash recovery without replay, history/deletion, run timeout and deterministic rejection of an AI approval. All use mock inference; no paid calls are made.

Before release, scan Rust dependencies using your chosen `cargo audit`/advisory tooling, review licenses, update pins deliberately and run native Mac/iOS validation. Dependency pinning and a green mock test suite do not prove production security. The native iOS authentication bridge and App Store account/device steps from chapter 09 remain required.

**Checkpoint:** force a restart, recover an interrupted run, repeat an idempotency key without launching new work, and demonstrate that Rust can veto a judge's approval.
