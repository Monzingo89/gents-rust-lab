# 07 — Metrics, office events and production extensions

## 1. Read metrics correctly

Judge calls count too: routing and completion use the same usage records and appear in run totals. Each completed call has a model/provider label, latency, provider response ID when present, and a `Usage` object. Expand the activity entry or export the event JSON.

| Field | Meaning | How to aggregate |
|---|---|---|
| `input` | Provider-reported prompt tokens | Sum completed calls with known values |
| `output` | Provider-reported completion tokens | Sum completed calls with known values |
| `cached_input` | Cached subset of input, when reported | Do not add it to input |
| `reasoning` | Reasoning subset of output under the supported schema | Do not add it to output |
| `cost_usd` | Provider-reported response cost | Sum known values, preserve unknown remainder |
| `latency_ms` | Client-observed full-call latency | Compare per call; not time to first token |
| `source` | Provenance or missing-data explanation | Retain in exports |

The normalized schema assumes the documented chat-completions usage convention. Confirm semantics before adapting another provider. Missing fields are `null`, not zero. A partial sum is displayed as a known amount plus unknown usage; it is not your total bill. Infrastructure charges and failed/abandoned requests are outside response-level accounting.

## 2. Observe execution without claiming hidden reasoning

Expand the planner's response to see its declared plan. Expand worker calls to see returned work and summaries. Observe `call_started`, `call_completed`, `plan_accepted` and `agent_completed` events. An API response is currently non-streamed; the app displays its text once the call finishes. Polling happens every 700 ms, so very brief states may not be visibly visited.

To add streaming later, consume provider SSE in the adapter, emit text delta events and reconcile final usage. Keep private reasoning excluded by default. Capture a provider-approved concise summary if the provider exposes one and its terms permit it; never manufacture a hidden-thought transcript.

## 3. Extend from a lab to a reliable harness

Chapter 13 now implements a single-process SQLite journal and client history/resume. The queue, shared database and distributed execution work below remain later exercises:

1. **Persistent state:** the local SQLite Store now checkpoints run events, IDs and terminal status. Extend that repository boundary to a shared transactional database for distributed execution. Store owner, run, call, event sequence and terminal state. On restart, mark in-flight calls interrupted rather than reissuing them automatically. Later use Azure PostgreSQL or Cosmos DB behind the same port. Verify a mid-run restart.
2. **Durable execution:** enqueue work using Service Bus, give each step a stable ID, checkpoint before/after calls and handle leases. Enforce idempotency for external actions. An interrupted inference request may have been billed; exact-once inference is not guaranteed.
3. **Cost admission:** give each model a capability record and current pricing provenance. Reserve an upper bound before a call, including input and maximum output. Reject models with unknown pricing when a hard admission budget is required. Reconcile actual usage and account for provider billing delays. Keep provider spending controls as a second layer.
4. **Tools:** define a separate `Tool` trait with validated arguments, user-scoped authorization, timeout and explicit side-effect classification. Start with a pure calculator. Add `tool_started`/`tool_completed` events. Do not execute shell commands taken from model text.
5. **RAG:** implement the `Retriever` port from chapter 04. Index sample documents with source IDs and ACLs. Test both positive and denied access. Pass only authorized passages into worker context.
6. **Telemetry:** add OpenTelemetry spans for run, agent and call; export aggregate metrics to Azure Monitor/Application Insights. Exclude prompt/output bodies by default. Store opaque run/call IDs, provider, latency, usage completeness and error class.
7. **Parallel agents:** use a dependency DAG. Schedule only ready nodes using a Tokio semaphore and isolated input snapshots. Keep a per-user and per-provider concurrency quota. Test cancellation, failures and fan-in deterministically before removing the eight-agent teaching cap.
8. **Regression evaluation:** version a small prompt set and acceptance rules. Run it against each model configuration before changing defaults. Track quality, latency, failed plans and cost completeness.

## 4. Separate app lifetime from work lifetime

For macOS, the hosted backend can continue while the app is closed. For iOS, assume the app will be suspended; do not depend on a continuously running Rust worker on the phone. The cloud harness owns long tasks. Before mobile release, persist runs server-side and store only resumable run IDs client-side, then reconnect after suspension or network loss. The local journal and run-ID reconnect path now support history recovery (chapter 13). A hosted deployment still needs a durable volume/shared database and physical-device validation.

**Checkpoint:** explain why input + output is different from input + output + reasoning, and why “unknown cost” must not be shown as a zero-dollar run.
