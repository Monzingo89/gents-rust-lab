# Cache-first CEO entry and preserved owner requests

The existing CEO/UI endpoint now checks eligible exact answers and source extraction before requiring a local CEO model. The same entry retains the person's exact ask, explicit context, constraints, output and proposed acceptance through clarification and the frozen execution draft. This advances the [active architecture refactor](148-ceo-cache-first-architecture.md); it does not finish its broader RAG, registry, workflow, model-discovery or scheduling requirements.

## One entry, preserved context

`crates/harness/src/ceo_fastpath.rs` adapts the shared task repository to the existing `/api/companies/{company}/planning/{id}` endpoint. Eligible initial requests can return `answered`, with a host-owned `TaskRequest`, source-supported `TaskResult`, original user message and citation text in the same durable CEO conversation history. The model check occurs afterward. Thus an eligible source or exact-cache answer does not need a model to be configured or loaded.

The adapter refuses to shortcut attachments, owned invocations, duty constraints, explicit planning modes, repair messages or follow-up context. Typed additional criteria, context, output language and action intent pass through the shared cache eligibility checks. A miss rolls back its tentative task record and continues into the original planner rather than leaving a second disconnected task identity. Unknown fields cannot silently disappear into a source answer. There are no worker starts, grants, SOW approvals or tool calls on this path.

Saved answers use the current task repository on read: source deletion, correction, expiry or changed allowed sources redact the old answer and evidence. Verbatim source support remains distinct from independent factual correctness. Curated source associations are owner supplied; no historical answer is hardcoded into production.

The office UI renders a saved source answer without execution controls and provides a new-conversation action. It preserves the exact original textarea bytes. The main request desk now carries its previously dropped **Done means…** lines as `task_input.proposed_acceptance`; these extra criteria prevent reuse of an unconstrained cached answer.

## Typed request before planning

An optional strict `task_input` object on the existing CEO API uses the shared Rust `TaskInput`. Its `original_ask` must equal the initial message exactly. Host code generates the task UUID and canonical version-one `TaskRequest`. The original request stays unchanged across later clarification/repair messages. Existing clients may omit the object. Earlier conversations migrate from their first retained user message; if that original is absent, the host requests a new complete conversation without inventing it or invoking a model.

The prompt view retains the typed input once, leaving redundant original/objective strings in canonical storage. Explicit text-only planning receives those owner fields too. Other planning requests can receive at most eight current owner-scoped lexical passages within the existing evidence budget, with actual source coordinates and hashes. They are marked unassessed reference candidates; a lexical score does not establish sufficiency, factual truth or permission. This is retrieval-aware planning, **not yet a generated RAG answer**. Evidence preparation failure produces a recorded failed turn without dispatching inference.

When the owner has supplied explicit acceptance criteria, the first planning turn can propose ready work immediately. It does not have to spend a separate turn asking the owner to repeat criteria already supplied. Essential missing intent can still produce a question. The native decoder retains the question/ready distinction, and the host rejects a ready proposal that omits or rewrites any supplied criterion. Unstructured legacy requests retain their earlier clarification behavior. This change does not authorize execution or make a model's proposed checks independent evidence.

When a ready conversation becomes an intake draft, `company.rs` freezes the complete host-retained request as `ceo_task_request` and records its SHA-256 alongside the existing CEO proposal, revision and policy controls. The signed intake digest therefore binds the original request even when the execution idea is a separate compiled description. Caller-supplied mismatches, forged request digests and task provenance without a retained planning session are rejected. Existing pre-migration conversations without typed provenance remain distinguishable; no original request is fabricated for them.

Execution review also includes a deep copy of that typed request in its expected intake. The compiled worker task contains the retained owner context, constraints, format and criteria as explicitly untrusted requirement data. This prevents supplied facts from disappearing between planning and execution. The existing 10,000-byte task bound applies to the complete compiled context; oversize input fails instead of losing fields. Changing any retained owner context during draft freezing is rejected before approval. Task criteria are cleared on company/connection changes and sign-out.

## Observed verification

The actual HTTP fixture launches a private harness with a mock model that is **never invoked**. Eight simultaneous task requests produce one deterministic extraction and seven exact hits. The actual CEO endpoint then returns a source answer and a subsequent cache hit without passing its real-model requirement. Both answers appear in saved CEO history and become redacted after source deletion. No job dispatch, policy installation or model inference occurs.

The browser fixture uses the actual office page and HTTP endpoints, without intercepted page routes. It confirms visible source text, a saved-cache status, no execution controls, and no browser errors or run dispatches. Adding an extra Done means criterion sends that exact criterion in the typed input and falls through to planning. The fixture intentionally has no usable local CEO, so this constrained request fails clearly instead of serving the old answer.

A separate **real local Qwen3-8B** trial supplied a synthetic one-sentence task, two source facts and two explicit criteria. It produced a ready plan on its first turn:

| Measurement | Observed value |
| --- | --- |
| Recorded local model replies | 1 |
| Reported input / output tokens | 517 / 146 |
| Wall time through saved planning response | 6.32 seconds |
| Proposed staffing | One configured writer, empty tool list |
| Owner criteria | Both retained verbatim |
| Provider fees | $0 |
| Worker execution / native tool calls | None |

The owner request remained byte-exact and its context/format/criteria stayed intact. This tests intake and contract formation; the requested sentence was not executed, independently judged or claimed complete. It is one example, not a model ranking, routing benchmark, general accuracy result or evidence that this is the smallest suitable model. Backend process RSS and actual generation metadata are retained separately; no GPU/energy savings are inferred.

The same saved plan was then reopened under the same frozen harness binary and source snapshot. A single **unapproved draft job** retained the exact original task ID/request and its digest. Changed-request, changed-digest and missing-session attempts were rejected through the real HTTP API. No additional planning call, policy install, approval, worker dispatch or native tool call occurred. [Evidence and scoped limits](evidence/local-ceo-task-entry-2026-11-18.json) distinguish the real local trial from the synthetic HTTP/browser fixtures.

## Commands and remaining work

From the repository root:

```sh
cargo test -p gents-harness ceo_planning --offline --locked
cargo test -p gents-local-mcp --test task_engine --offline --locked
cargo build -p gents-harness --offline --locked
python3 scripts/task_frontdoor_smoke.py
python3 scripts/task_frontdoor_smoke.py --browser
```

The last command requires the already installed Playwright package and Chrome. Both fixture commands start and stop private harnesses; neither invokes a model. No process is left running by these checks. Verification passed: 586 workspace Rust tests (27 opt-in tests ignored), strict offline Clippy, formatting, the workspace build, ten focused JavaScript checks, 32 source-boundary Python checks, the real HTTP/browser fixture and the web build. The usual workspace format/lint/test/build checks still apply. The live model trial is retained private evidence, not an automatically repeated CI test.

Still required by the full brief: shared typed `destructure_task`/routing APIs and schemas; one-generator grounded answers; evidence sufficiency and contradiction checks; evaluated semantic reuse and embeddings; complete configured-model discovery; actual distinct registered workflow executors; specialist profiles; unified task budgets/resources/cancellation/coalescence; native MCP task APIs; isolated optimization interfaces; and the full held-out evaluation/completion audit. Existing execution approvals and source/weight quarantine remain intact.
