# Retained company task usage on the CFO screen

November 21, 2026. Task-engine inference bypasses the legacy run-journal reply path. The previous CFO cost screen therefore excluded it. Adding task result measurements would double-count coalesced subscribers and could turn interrupted work into zero usage. This slice adds a numeric projection over existing durable receipts; no ledger, service, crate, scheduler or inference route was added.

## Behavior and authority

The existing authenticated owner task endpoint accepts `{"op":"usage"}` at `POST /api/companies/{company}/tasks`. The `company::connect_readonly` connection factory verifies company ownership before a deferred read transaction. It opens SQLite with read-only/no-follow flags and query-only mode, without invoking the existing writer's archive/index/schema maintenance. The operation does not refresh source dependencies, ingest context, install schemas or grant native actor access. It returns aggregate numbers for that company's retained workflow and grounded jobs, including native actor jobs. It exposes no prompts, source passages, errors, actor/model identities or call IDs. Native MCP callers gain no new tool or permission.

`crates/harness/src/task_usage.rs` reads each retained job once, then deduplicates workflow receipts by `(job_id, call_id)` and grounded completions by job ID. Subscriber/result rows are not summed. Identical receipt duplicates count once; conflicting duplicates, orphan returns and malformed records invalidate aggregate certainty. Native completion receipts establish actual model calls; fixtures establish zero generation. A request record alone does neither.

The scan is bounded to 256 jobs per executor kind (512 total) and 4,096 receipt events, with a one-record overflow probe. An exceeded bound makes totals unknown rather than pretending to cover the omitted records. Oversized/malformed audit bodies also invalidate certainty. Grounded jobs without a completion receipt and workflow requests without matching returns leave aggregate model usage, tokens and fees unknown. Missing token values remain unknown even when all calls are fixtures. Older owned-native/fixture receipts can establish zero provider fees from their native/fixture proof; unproven receipts cannot.

`coverage_complete` means the bounded **retained receipt scan** completed without those integrity/limit problems. It does not mean every dispatched request returned, every measurement exists, or lifetime history is retained. The projection deliberately excludes legacy run journals, the efficiency observer and external runtimes such as an unverified Open WebUI deployment. It does not estimate savings or electricity cost. New workflow return records retain reported provider cost and latency; grounded return metadata identifies fixtures explicitly. Older receipt formats remain readable.

The floor-nine CFO screen displays this company's task provider fees and expands into task calls, tokens, native/fixture replies, unresolved requests and scope. Legacy host journal/observer totals remain separately labelled facts and existing floor-five screens. No cross-scope grand total is invented. Unknown company fees display `UNAVAILABLE`. Selected-task status is still separate from company totals.

The UI clears company usage synchronously on company, connection, token or sign-out changes. An epoch check rejects an older in-flight response after reset. Disconnection, a mismatched origin/company or observations older than 15 seconds suppress the task receipt view. A failed task-usage read does not disable independent host sensor telemetry. Animations, accounting and polling perform no model calls.

CFO enforcement remains in the existing deterministic approved-intake and inference-reservation ledgers. This read-only display cannot approve work, increase a budget, refund a reservation or authorize paid runtime calls.

## Evidence

Three new Rust tests cover scope/payload isolation, job/call deduplication, missing/conflicting/unresolved/orphan/truncated records, owner denial and the absence of source/schema side effects. An intentionally unarchived cold record and missing search index remain untouched even after a denied owner read; a write through the read-only connection fails. Existing workflow/native smokes independently compare receipt counts, distinguish fixture token unknowns and require unknown usage after a revoked request. The owner fixture observed **29 returned fixture calls across 10 jobs**, zero actual generation, $0 provider fees and unknown tokens.

The workspace passed **708 Rust tests**, with 33 opt-in tests ignored. Strict offline all-target Clippy, focused UI tests, the Vite production build and 32 broker/governance tests passed. All eight bounded local suite commands passed in **126.55 seconds**; these include real HTTP/SQLite and native stdio/SSE control paths with mock workers. They establish execution/accounting contracts, not model competence. The browser probe additionally checks the CFO screen against the actual usage API, desktop/mobile task layout and company reset. Source hashes, exact outcomes and screenshot hashes are in [the evidence record](evidence/retained-task-usage-2026-11-21.json).

```sh
python3 scripts/local_evaluate.py
python3 scripts/task_workflows_smoke.py --browser
cargo test -p gents-harness task_usage --offline --locked
npm run build
```

No real model or paid provider was invoked by this slice. Actual model latency, RSS/context, general task completion and total hardware/electricity/development costs were not measured here; see [the prior bounded real-model trial](165-shared-model-pins.md) for its separate scope. The full platform-dependent `scripts/check.sh` was not rerun. Existing company services/configuration were not restarted or replaced. Broader accounting retention, external runtimes and asynchronous-task yielding by optional research remain on [the bounded backlog](162-local-execution-baseline.md).
