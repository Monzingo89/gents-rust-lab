# The Gents company: admin judge, tools and Operations

From the repository root:

```sh
npm run company
```

The command checks the installed local models, builds and starts the Rust harness on `127.0.0.1:8788`, onboards company documents if needed, starts an optional email listener, keeps the Mac awake, and opens the browser. Keep the terminal running and the Mac connected to power. Inference stays local. Public websites, model downloads and configured email are deliberate network tools.

## The office

The game-style office retains Strategy/CEO, Knowledge, Training, Production/All Hands and Quality, and adds Operations. Every Gent wears a suit; faces, hair, glasses, accessories, ties and role colors distinguish them. `ZZZZ` means that character is resting. Activity bubbles use recorded model/tool state, not private chain-of-thought.

The large status light is green while company work or training is active, yellow when a question is waiting or the harness disconnects, and red when no work is running. The browser can answer persistent questions and continue the same job. During a disconnect it keeps retrying rather than cancelling the work.

## How work executes

The browser freezes and approves the request. A separate local admin call reviews task admission, then every proposed tool action against the original request and actual recent evidence. The default admin model is the already installed Qwen3-8B; no new weights or paid model API are needed.

Law, CTO, PM and Architecture contribute plans. The engineer performs a tool loop with recorded results; QA can require revisions and the engineer continues. Every tool intent is checkpointed before execution. Full request/decision/result records are retained as numbered `tool-NNNNNN.json` files under the run directory. The model can retrieve older results using `journal_read`.

Invalid or denied tools do not execute. The model can propose a different approach. Persistent execution errors become browser questions. An admin model is an additional review, not a guarantee that every judgment is correct. Fixed path, budget and integration controls remain enforced by code.

Supported tools are described in `config/company-tools.json`:

- Public web search with a second search source when the first presents a challenge; public HTTPS pages; isolated headless Chrome, page actions and screenshots. Responses are untrusted evidence. Browser requests resolve and connect to the same checked public address. Private networks, local harness endpoints and credential forwarding are blocked. Account login sessions and browser downloads are not shared with the user's normal browser.
- Workspace file reads/writes, historical tool results, and real shell/code/test commands. Commands run in a macOS sandbox that blocks network and access to personal files outside approved runtime/workspace paths. It also blocks writes outside the run workspace and signals to other applications. Generated code never receives email or HF credentials.
- Resource measurements, Hugging Face discovery, revision-pinned downloads into `./models`, retained hashes, GGUF quantization and local MLX 2/4/8-bit conversion. Conversion uses the same GPU queue as inference/training. Original weights are retained.
- Real local training preparation, execution, progress, held-out evaluation, and adapter promotion with test evidence. Training is described below.
- Email reading, requests, and sending using configured addresses and local credentials. Email actions pass the admin judge. Missing configuration becomes a browser question.
- Explicit process termination. The CEO can inspect processes and ask to stop an exact same-user PID. The operator sees its identity and types `APPROVE`; the broker rechecks UID/start time/executable before sending SIGTERM. It does not guess that an unrelated app has no unsaved work. The company automatically stops its own model workers when switching models, training, converting, or waiting for an answer.

This toolset supports useful local work but does not provide unrestricted desktop control, a universal deployment connector, purchasing authority, or access to every signed-in account. Add the appropriate scoped integration for a task that needs one. Killing a process frees memory; it does not free disk space. Model and download tools check free disk, reserve working room, and can choose a smaller model; they do not delete personal files.

## Resource recovery and long runs

One local backend is loaded at a time. GGUF loading tries full GPU offload, reduced offload, then CPU loading. After repeated worker failure the company can use the smaller installed admin model and record that capability changed. Operations can select another installed model, download a suitable model, or quantize a copy. CPU execution may be much slower; no amount of orchestration creates physical RAM or disk capacity.

Company sessions have a 30-day safety deadline; local model requests and training/conversion jobs have 24-hour limits. Tool failures are returned as evidence for repair. A problem that still needs a person produces a yellow question instead of being silently reported complete.

`autonomy.json` persists admission, completed role plans, the selected worker, tool intent and outstanding question. The ordinary company journal persists events and run state. Restarting `npm run company` recovers interrupted tool-enabled runs. An action interrupted while in flight may already have taken effect: the company asks what happened and never automatically replays it. This matters particularly for outgoing email.

For changed integration credentials, select **Reload integrations & restart office** in the question panel. The launcher reloads `.env.local` and then `.env.company` (company values take precedence), restarts the harness, and recovers the job. The outstanding question retains its ID; answer it after reloading. Restart controls require the supervised `npm run company` launcher and local authentication. Stopping the terminal stops the company; starting the command again recovers saved work.

## Email configuration

Copy `.env.company.example` to `.env.company` using your editor and configure one provider. The launcher loads the ignored `.env.local` first, then `.env.company`; existing keys can stay in `.env.local`. Reloading also removes settings deleted from these files. Never put credentials in tasks, answers, source control, logs or chat.

For Resend sending, configure `GENTS_EMAIL_PROVIDER=resend`, `RESEND_API_KEY`, and `GENTS_EMAIL_FROM` with a verified sending identity. `GENTS_EMAIL_DEFAULT_TO` tells the agents your preferred recipient; it must also appear in `GENTS_EMAIL_ALLOWED_RECIPIENTS`. A Gmail destination does not need a Gmail API key. Sending through your verified domain does not grant access to Gmail or Google-hosted incoming mail.

Inbound Resend requests additionally require `GENTS_EMAIL_INBOX`, the exact dedicated address actually routed to Resend. Both the sender and the receiving address are checked on message listings and body retrievals. Existing Google/Microsoft MX routing is not replaced by this application. Use a separate receiving subdomain if the root domain already hosts personal/business mail. Resend verification flags alone do not establish that current DNS delivers inbound mail to Resend. The code does not provision DNS, mailboxes, or purchase a plan.

For a dedicated IMAP/SMTP mailbox, configure `GENTS_EMAIL_PROVIDER=imap`, `GENTS_EMAIL_USER`, `GENTS_EMAIL_PASSWORD`, `GENTS_EMAIL_FROM`, `GENTS_IMAP_HOST`, `GENTS_SMTP_HOST` and optional TLS SMTP port (default 465). Use a provider-supported app password or dedicated credentials. Normal account passwords and unsupported OAuth-only mailboxes are not substitutes; an OAuth-only Gmail/Microsoft 365 setup needs its provider authorization flow added.

Set `GENTS_EMAIL_ALLOWED_SENDERS` to the request senders and `GENTS_EMAIL_ALLOWED_RECIPIENTS` to permitted outgoing recipients. `GENTS_EMAIL_AUTO_START=1` enables the local 30-second inbox listener, which deduplicates messages and converts allowlisted messages into frozen tasks when the company is free. Treat sender matching as a routing rule, not cryptographic sender authentication; use a dedicated mailbox and its provider spam/authentication controls. Old mail in that configured inbox can also be imported; choose a dedicated folder for new requests.

Resend outgoing requests receive an idempotency key. SMTP has no universal delivery-idempotency contract, so interrupted sends always require inspection. No real email has been sent or received as verification until the user configures a mailbox and provides a test recipient.

## Recurring local-agent work

Agents can use the admin-reviewed `schedule` tool to create, inspect, pause, or resume daily jobs in an IANA timezone. These are generic task triggers: the scheduler does not research or generate a report. Every occurrence is a new local-agent run with fresh admission and per-action review, under the original approved request. Schedule configuration, occurrence IDs, and admission state persist in the company's `schedules.sqlite3`; retries reconcile the frozen job rather than launching another copy. Revoked/blocked/archived parent authorization prevents further admissions. Previous dates missed while offline are skipped, not replayed in a burst.

An admin-reviewed `schedule` update supplies the full replacement specification and must come from the original creating run. It preserves enabled/paused state and occurrence history. The recurring acceptance criteria must cover the actual recurring work, not merely schedule registration. Preparation time and delivery time are separate.

## Loop recovery and honest progress

Every 200 steps now emits a non-blocking progress checkpoint instead of requiring a generic human answer. On recovery, only the exact obsolete step-count question is retired automatically; genuine operator questions and uncertain in-flight effects remain protected. A third identical tool request within the last sixteen proposals is rejected before execution. Already successful identical schedule creation or email sends are not replayed as new work. An installed PM model receives the original task, recorded operator answers, actual tool index and rejection to propose a different next action; a successful recovery retains that worker model. Four unsuccessful recovery proposals request targeted operator/model help rather than executing hundreds of repeated actions. Malformed proposals and denied actions enter the same bounded recovery path. Admin review is still required before any action executes.

Recovery now rotates distinct installed CTO/PM/engineer/admin model weights, excluding the currently stuck model when alternatives exist; different display names for the same weights are not counted as alternatives. Recovery context deduplicates actual executed requests and separates blocked action/argument pairs from evidence, without copying repeated rejected summaries back into the prompt. The versioned context upgrade can retry the old runtime-capability stall once, but cannot clear authorization questions, pending side effects or a subsequent failure under the same recovery version. This does not guarantee that the installed models are capable of every task.

Recovery input is a structured task object, not an escaped JSON transcript, and does not add historical company retrieval. For GGUF structured decoding, blocked tool families are removed from the action enum for the recovery turn only; the local model must choose a different available tool. Normal turns retain the full catalog, and every resulting proposal still passes through the independent admin judge and runtime checks. This is a temporary restriction, never an authorization override. MLX still relies on the proposal rejection guard because it does not use the GGUF JSON-schema decoder.

Admin action review also receives a structured task object with deduplicated historical answers and actual recent tool observations. Previously answered question text is not reinserted as a fresh question. A repeated already-answered admin clarification is an invalid review, not permission to execute: the tool remains unexecuted and recovery proposes another approach. The specific legacy schedule-discovery question can be retired when its original operator answer is retained; this never clears process confirmations or uncertain side-effect intents and never authorizes an action.

Tool indexes retain journal references across the whole run, including earlier successes and failures; the worker can use `journal_read` for full evidence. Previous model outputs are not reintroduced as factual company memory in tool workflows. CEO monitoring skips operator waits and in-flight tools, and its published status comes from the executed-tool index. Local CEO model output is limited to advisory next actions, not evidence of completed work. A schedule record never proves research, report generation, email queuing, delivery or inbox placement.

The public RSS search fallback rejects responses with no query-topic keyword matches instead of presenting generic news homepages as topical research. This is only a relevance heuristic: even matching snippets are discovery pointers, never verified claims. The agents must fetch and inspect original sources.

The harness owns the scheduler process and stops it at shutdown/restart. Keep `npm run company` and the computer running for fresh research. A daily preparation start is not a guaranteed completion time: agents should prepare sufficiently early, then use `email_send` with an explicit future `scheduled_at` timestamp to let Resend hold the finished message for delivery. Past/naive timestamps are rejected rather than silently sent immediately. `email_status` reads the provider's actual scheduling/delivery status. Provider queuing is not proof of inbox placement. Agents must retain sources and verify evidence; links alone do not guarantee a claim is true.

New tasks retrieve their own handoffs and operator-ingested reference documents, not other jobs' outputs. Explicit operator `recall` can reintroduce prior outputs as reference material. Admission/action judges do not receive historical company retrieval; the current operator task defines their scope.

## Downloads and training

Public model downloads need no key. Gated/private Hugging Face models may require a read-scoped `HF_TOKEN` in `.env.company` and acceptance of the model license. Downloads use the `hf` CLI, an immutable commit revision and exact weight/tokenizer/config filenames. Executable repository files are excluded; model loading does not enable remote code.

Training requires local Safetensors/MLX weights and reviewed, licensed, company-scoped examples, including distinct train/validation/test prompts. The existing MLX worker trains a LoRA adapter while retaining base weights and reports actual baseline and adapter held-out losses. A GGUF alone is not a trainable Safetensors directory.

The company may propose training through the Training Room. It must not invent review, provenance or data rights. For larger datasets the tool can use a JSONL dataset directory in the workspace via `dataset_path`. A training candidate is not automatically called better: promotion requires improved held-out loss, two distinct successful shell validation steps for role and general regression tests, privacy/license review, exact adapter hashes and admin approval. The existing promotion code rechecks lineage and retains rollback adapters. If evidence is insufficient the company asks for data or review. Lower test loss alone does not establish general usefulness or faster inference.

## Verification

`bash scripts/check.sh` checks Rust formatting, clippy, unit tests, existing lifecycle/reliability/company/audit checks, the real HTTP/sandbox autonomy smoke flow, Python boundaries and the UI build. `npm run test:office` checks browser admission, status lights, answering and responsive layout against a running office.

The HTTP autonomy smoke uses simulated model decisions and actual file/command execution. Real model behavior has a separate opt-in test:

```sh
LOCAL_MODEL_PORT=8796 cargo test actual_admin_judge_returns_structured_decision --locked -- --ignored --nocapture
npm run test:company:local
```

Do not use a model port already occupied by another process. A successful local admin smoke establishes that the configured weights can produce the decision schema; it does not establish universal task-completion accuracy or guaranteed model improvement.

The local company smoke runs a real Qwen3-8B admin/operator/QA sequence on a tiny write/read fixture, verifies the artifact independently, chooses unused ports, and retains its uniquely scoped test artifacts under `data/companies/local-smoke-*/`. It does not send email, train weights or download full models.
