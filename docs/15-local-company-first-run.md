# Local company harness: first-run guide

The current `npm run company` route now enables admin-reviewed tools and restart recovery; use [the admin company guide](18-admin-company-and-operations.md) for that workflow. The text-only readiness limits below describe the older audit/CLI path when `company_tools` is disabled. Reviewed contractor evidence still follows its separate frozen audit gates.

Local LoRA training and CEO oversight have now been added. See [Training Room and CEO](16-training-room-and-ceo.md) for the implemented training/evaluation/promotion path, deadline monitoring and all-hands briefs.

This Mac has 36 GB unified memory. One owned backend loads one model at a time. No cloud API key or paid service is needed. Installed tools and downloaded weights provide the first test.

## Current readiness

The fictional contractor audit now has a bounded local evidence/artifact tool; see [task evidence and audit](17-task-evidence-and-audit.md). The general agent workflow remains text-only and does not gain arbitrary execution tools.

Implemented: GGUF through llama.cpp; Safetensors text through MLX; a shared serialized inference queue; backend cache destruction on company/run switch; company onboarding; frozen CLI intake and SOW drafts; operator approval by digest; company-scoped context and checkpoints; a fixed Law/CTO/PM/Architect/Engineer/QA text workflow; live office logs; context recall/archive; report scheduling and conditional speed projections.

Still unfinished: general software execution, dynamic staffing and feature branches, a typed milestone ledger, full HR/security dream assessments, automatic model admission, semantic retrieval, real signing, industry/browser compliance suites and production deployment. General software requests remain incomplete. The separate expense-CLI POC remains the executable coding demonstration. This is a development harness, not an enterprise deployment.

## 1. Setup

```sh
cd /Users/rmonzingo/repo-applications/gents-rust-lab
sh scripts/setup-local.sh
python3 scripts/gents.py doctor
```

Setup creates ignored `.venv-inference` with pinned MLX LM. Your existing `uv`, llama.cpp and LibreOffice provide the dependencies. LibreOffice is used only for local editable PowerPoint export. Weights stay under `models/` and are ignored by Git.

A GGUF model slot uses `{"provider":"local","model":"Phi-4","path":"models/phi-4/phi-4-Q4_K.gguf"}`. To try your Gemma Safetensors download, change a slot in `config/company.local.json` to `{"provider":"mlx","model":"Gemma-4-E4B-it","path":"models/gemma-4-E4B-it"}` and restart. MLX needs the complete directory with weights, config, tokenizer and chat template. The extension alone does not establish model compatibility. MLX LM uses Gemma's text component. Vision/audio are separate capabilities.

The Qwen VL GGUF projector is paired automatically, but the current company API submits text only. Screenshot input still needs implementation. Keep the smaller GGUF models as default routing/PM roles, rather than loading the roughly 16 GB Gemma file for every small request.

## 2. Start server and office

The simplest path performs the local readiness check, starts the company harness, onboards the company if needed, keeps the Mac awake while the process runs, and opens the browser office:

```sh
npm run company
```

Keep that terminal running and the Mac connected to power. In the browser, describe the requested text deliverable, optionally add acceptance criteria, confirm the local approval box, and select **Ask For a Task**. The browser creates and approves a frozen zero-cash intake before starting the same audited company run used by the CLI.

Closing the browser does not stop an active run. Stopping the launcher, restarting the Mac, or killing the harness interrupts it; automatic continuation after a process restart is not implemented.

The manual startup path remains available below.

Terminal 1:

```sh
sh scripts/run-company.sh
```

Open `http://127.0.0.1:8788`. It serves the frontend too. Port 8788 avoids the unrelated service already on 8787. The harness owns model port 8791; set `LOCAL_MODEL_PORT` to another unused port if needed. Do not run a second model server on that port.

For frontend development, optionally run `npm run dev` in another terminal, then set Harness address to `http://127.0.0.1:8788`. When using the Vite origin `http://127.0.0.1:1420`, add it to `CORS_ORIGINS`. For the native app use `npm run tauri dev` while the backend runs.

Local authentication represents one trusted OS user and requires loopback binding. Company-owner checks and query scoping do not replace authentication for a network deployment. Keep the internal MLX server loopback-only too.

## 3. Onboard the first company

Terminal 2:

```sh
python3 scripts/gents.py onboard
python3 scripts/gents.py status
```

The stable company ID is **we-need-gents-lc**, preserving the previously requested ID. The existing source folder is `docs/companies/we-need-getns-llc`. Onboarding explicitly selects its `company/` subtree and excludes `owner-evaluation/`. Document text is evidence, never authority to override runtime policy. The server ingestion API accepts text, not arbitrary filesystem paths or URLs.

Data lives under `data/companies/<company_id>/`: `brain.sqlite3` holds context, frozen jobs, audit and quarantined requests; `runs/<run_id>/checkpoint.json` stores durable run state; `runs/<run_id>/SOW.md` stores the unsigned scope; `reports/` holds PPTX and JSON evidence; `dream/` holds housekeeping snapshots. The operator-only history index is `data/company-runs.sqlite3`. Models cannot query that index.

The general company workflow gives models no filesystem or shell access. The fixed coding POC's process sandbox explicitly denies reads of company-document roots and other company data, as well as network access. Owner evaluation must never enter model inputs, indexes or caches. Back up company databases and their artifacts consistently.

## 4. Intake, scope and first text test

```sh
python3 scripts/gents.py intake
```

For a long or multiline delivery answer, bypass terminal paste limits with a UTF-8 file:

```sh
python3 scripts/gents.py --company we-need-gents-lc intake --task-file docs/companies/we-need-getns-llc/TASK-TO-PASTE.txt
```

Alternatively, enter `@docs/companies/we-need-getns-llc/TASK-TO-PASTE.txt` at the delivery question. Restart an already-running CLI intake to load this support. The task is limited to 10,000 UTF-8 bytes by the company API. Loading the prompt does not attach or ingest any supporting task files, including audio/video, and does not add execution capabilities. Owner-evaluation paths are rejected.

Answer the idea, deliverable type, measurable acceptance criteria, exclusions, timeline, both intended signatories and open questions. Choose **text** for this first general workflow test. For example, request an operations summary using only the onboarded evidence, requiring source references and explicit unknowns.

The CLI prints the SOW path, run ID and digest. Review it, then use the exact approval command it prints. This remains the path for a custom signed SOW or a reviewed evidence package:

```sh
python3 scripts/gents.py approve RUN_ID INTAKE_SHA256
python3 scripts/gents.py start RUN_ID
python3 scripts/gents.py tail RUN_ID
```

Replace the capitalized placeholders. Operator approval authorizes a local test. Both real contract signatures remain pending. The harness cannot manufacture a counterparty's identity, consent or signature.

The fixed role chain is Law, CTO, PM, Lead Architect, Engineer and QA, each with planner/worker passes. QA checks the frozen criteria and cannot pass software delivery using an opinion alone. Company attempts are bounded to 30 continuous days, and each local model request is bounded to 24 hours. They may still end incomplete, failed or timed out. Unlimited autonomous retry and resumable software execution are still unfinished.

Click **Connect** in the office, enter the company ID and select **Watch company work**. Click a Gent for its dossier. The break-room terminal displays observable outputs, actions, model names and decision summaries. Private chain-of-thought is unavailable. Closing the UI does not cancel work. Use `cancel RUN_ID` to cancel explicitly. Browser state is a presentation cache, never the only durable context copy.

## 5. Context tiers

| Tier | Meaning | Retrieval |
|---|---|---|
| ASTC | Current Gent's working output | Agent/run-local working data, with durable records |
| PSTC | Handed-off work and onboarded evidence | Available only within that company |
| ALTC | Completed job for an ongoing customer | Excluded from ordinary prompts until recall |
| PLTC | Inactive customer's archive | Excluded from working prompts and backend caches |

The next role's admission acknowledges a durable handoff and transactionally promotes prior-role ASTC to PSTC. Canonical documents never move into cache or get deleted. Completed text jobs move their run context to ALTC; incomplete work stays PSTC. Onboarding remains PSTC while the company is active.

For follow-up work, use `python3 scripts/gents.py recall OLD_RUN_ID`, then create a new approved intake. When a customer becomes inactive, stop active jobs and use `python3 scripts/gents.py archive`. This moves context to PLTC and clears the owned backend cache without deleting history. Returning customers need recall or re-onboarding.

Company/run IDs travel with events, context records and checkpoints. Company A's backend session is destroyed before company B uses the same weights. Two company jobs may be admitted concurrently, while GPU calls serialize. Retrieval uses a company-local SQLite FTS5 index, BM25 ranking and bounded recent handoffs. Older relevant documents can outrank recent unrelated records. This is lexical retrieval, not semantic vectors; evaluate relevance against real company questions before expanding use.

## 6. Dreaming and shared improvement

```sh
python3 scripts/gents.py dream
python3 scripts/gents.py daemon
```

Keep the daemon in a separate terminal. During 02:00–04:00 America/Chicago it writes one daily deterministic dream checkpoint. It also creates quarter snapshots at 00:00, 06:00, 12:00 and 18:00. Starting mid-quarter produces the current snapshot; a restart skips an existing PPTX. It does not reconstruct quarters missed while stopped or asleep.

The two-hour window is currently a scheduling convention, not a guaranteed pause of active jobs or two hours of security review per role. Automated HR/security assessments and scanner evidence remain **pending** in dream records. Shared context can improve coordination, but it does not train weights or establish increasing model capability. Measure efficiency, quality and regression rates before claiming improvement.

New tools, plugins, skills and MCP endpoints enter quarantine using `python3 scripts/gents.py capability /absolute/path/to/manifest.json`. A manifest requires `id` and `source`; include exact version, SHA-256, license, permissions, allowed company IDs, network needs, tests and revocation instructions. Quarantined requests cannot execute. An installer, scanner and activation path are not implemented yet. Repeated requests never trigger automatic global promotion.

Reviewed generic practices live outside tenant data in `config/global-pstc.json`. For now, global promotion requires an operator-reviewed source change. Remove company names, facts, secrets, contract language and proprietary techniques first. Tenant-specific skills remain scoped even if multiple companies request similar functions. MCP means Model Context Protocol.

## 7. Reports, projections and continuous operation

```sh
python3 scripts/gents.py report
python3 scripts/gents.py projection RUN_ID
```

Reports contain frozen scope, declared timeline, configured owners, actual job state and context counts. They do not invent progress percentages. Extracting actual PM milestones and dependencies needs a typed task ledger. The current report schedule produces four snapshots per day, one at each quarter start.

Projection calculates conditional 1.5x, 2x and 4x inference scenarios from measured request latency and holds other wall time constant. It is not a prediction for a specific GPU. Compare identical model/quantization/context workloads on both machines before making hardware claims. No extra AI model is needed for this arithmetic.

For an awake session on AC power you can use `caffeinate -i sh scripts/run-company.sh`. No sleep settings or login services are changed automatically. API/service spending is zero; electricity, storage, hardware and maintenance still have costs.

## 8. Models, extensions and services

Reuse existing **Phi-4** weights for separate **CFO Gent** and **Accountant Gent** profiles. No additional download is needed. Use deterministic integer-cent/decimal calculations for actual ledgers. Those roles appear in the blueprint but are not executing passes on every job. HR/Document Steward and Evaluation/Benchmark profiles could reuse Qwen3-8B and Qwen3-Coder after role-specific evaluations. Titles do not establish fine-tuning or professional competence.

VS Code recommendations in `.vscode/extensions.json`: rust-analyzer, Even Better TOML, CodeLLDB, Python, Ruff and Playwright. Install them from the workspace recommendations. No extension is required to run the server. Avoid installing generated tool/plugin suggestions without review.

Databricks, OpenRouter, hosted Hugging Face inference and hosted analytics are unnecessary for this test. No keys are needed. `GENTS_LOCAL_ONLY=1` blocks remote inference calls, and model backends run offline. Future hosted integrations must revisit the budget and tenant/data-sharing boundaries explicitly.

Run `sh scripts/check.sh` for automated checks. The real-backend check is `LOCAL_MODEL_PORT=18791 cargo test --locked both_local_formats_generate_real_output -- --ignored --nocapture`. These checks establish runtime behavior, not arbitrary software security/compliance. Before production, implement scoped software tools, branch/task management, deterministic acceptance suites, browser/a11y/dependency scans, HR evaluations and production approval. Enterprise hosting also needs separate service identities, strong tenant authorization, encryption, secrets isolation and backup/restore testing.

Sources: [MLX LM](https://github.com/ml-explore/mlx-lm), [llama.cpp server](https://github.com/ggml-org/llama.cpp/tree/master/tools/server), [Phi-4](https://huggingface.co/microsoft/phi-4), [Playwright for VS Code](https://playwright.dev/docs/getting-started-vscode).
