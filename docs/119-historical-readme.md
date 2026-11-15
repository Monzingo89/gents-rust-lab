# Historical onboarding snapshot

Preserved from the earlier README on 2026-11-13. This includes historical modes and readiness descriptions; it is not the current onboarding contract. Return to [the current README](../README.md) and its linked evidence for current scope. Relative links are adjusted for this archived document location.

# Gents Office — Rust harness learning project

Current local MCP work: the [type-directed selection trial](../docs/114-type-directed-local-selection.md) independently qualified a six-call local workflow with $0 provider/tool fees. Local inference still consumes tokens, hardware and electricity. The [repository-backed factory preview](../docs/115-company-factory-preview.md) is available for a ready CEO plan; the three-pattern execution factory remains in progress. [Distinct local reasoning programs](../docs/116-local-coordination-engine.md) passed narrow native probes; complete task acceptance and factory integration remain open. See the [release review and remaining gates](../docs/113-factory-and-local-release-review.md) for current limits. Original project code is [MIT licensed](../LICENSE); [third-party material retains its own terms](../THIRD_PARTY_NOTICES.md).

For the local multi-company GGUF/Safetensors workflow, start with [the company first-run guide](../docs/15-local-company-first-run.md). It includes CLI intake, context tiers, report scheduling, the break-room log and explicit readiness limits.

To start the local company, keep its terminal open, and launch the task-request UI in your browser:

```sh
npm run company
```

For a phone on the **same trusted Wi-Fi**, use `npm run company:mobile` instead.
It starts or reuses the laptop company and opens a laptop-only setup page with
the phone's HTTPS address and pairing code. All models still run on the laptop;
the phone can request tasks, inspect the office/logs/training/history and answer
questions. See [mobile access setup](../docs/19-mobile-access.md), including the
first-visit local certificate warning and keep-awake requirements.

The [vertical campus](../docs/20-vertical-campus.md) shows six glass floors, dedicated
work stations and numbered recharge pods with visible heads. The sixteen current
Gents have distinct appearances drawn from twenty supplied characters; shared
local models never imply shared faces. The opening view is a glass-windowed
building: select a window band or floor button to enter one enlarged floor in an
elevated angled view. Eight work rooms have distinct luxury interiors; recharge
suites keep matching sleep pods.
Working sprites walk in separate lanes; Preview walking lets you inspect the
animation while idle without starting models or tasks. Return with Building
exterior, or use the keyboard/touch-accessible crew manifest to inspect a Gent.

Enter your request and select **Ask For a Task**. The local admin judge reviews the request and each tool proposal. The company can search/read the web, use an isolated browser, write and test code in its workspace, download models into `./models`, quantize models, run local LoRA training, and use a configured email inbox. Closing the browser does not cancel work. Yellow questions can be answered in the browser; a supervised restart preserves checkpoints.

See [the admin company guide](../docs/18-admin-company-and-operations.md) for email setup, actual capability boundaries, training evaluation and recovery. No paid inference API key or new admin model is required. Private email credentials belong in the ignored `.env.company`, based on [.env.company.example](../.env.company.example).

[Training Room and CEO oversight](../docs/16-training-room-and-ceo.md) adds local role-specific LoRA adapters, held-out comparison, reviewed promotion, deadline observation and company-scoped all-hands briefs.

<p align="center">
  <img src="../src-tauri/app-icon.png" alt="Gents Office owl wearing a tuxedo" width="220">
</p>

Open **START-HERE.html** for the full walkthrough and complete code appendix, or start at **[docs/00-start-here.md](../docs/00-start-here.md)** and follow the numbered chapters. All commands run from this folder unless a chapter says otherwise.

This is a staged educational starter, not a finished App Store product. The Rust HTTP harness and browser office run locally with simulated responses and a persistent SQLite event journal. Live adapters, desktop PKCE sign-in, and Azure deployment material are supplied for account-backed validation. iOS native authentication, distributed execution, retrieval, training, and production deployment hardening remain explicit exercises.

## First run on your Mac

Install Rust, Xcode command line tools, Node.js LTS, and VS Code using chapter 01. Then:

```bash
cd /path/to/gents-rust-lab
cargo run -p gents-harness --locked
```

Open `http://127.0.0.1:8787`, click **Connect**, then **Start task**. Expand activity entries. The mock models simulate the lifecycle; they do not solve the task.

For the native desktop window, keep the harness running; in a second terminal:

```bash
npm ci
npm run tauri dev
```

## AI judges upgrade

The default demo now includes a CTO routing role and a Compliance Security & QA Engineer, with a bounded repair loop. Start with **[chapter 12](../docs/12-ai-judges-and-repair-loops.md)** for the full upgrade. The original fixed-team mode is available as `config/fixed.mock.json`.

## Local planner-worker proof of concept

The local POC onboards every Gent with company-specific expense and release documentation, retrieves relevant chunks into each role's prompt, builds and independently verifies a generated Rust expense CLI, then runs a local dream/consolidation pass into a private quick cache. It uses the three checked-in-place GGUF paths from `config/local-poc.json`. Model weights and dream caches remain ignored by Git. No API key or cloud inference is used.

Prerequisites:

- macOS with the existing Rust toolchain, project dependencies, and `/usr/bin/sandbox-exec`.
- A Metal-enabled `llama-server` from llama.cpp available on `PATH`, or its absolute executable path in `LLAMA_SERVER`.
- The three nonempty GGUF files at the exact paths declared in `config/local-poc.json`.
- Port `8791` available on loopback, or another loopback model port set through `LOCAL_MODEL_PORT`.

`llama-server` must advertise `--model`, `--host`, `--port`, `--ctx-size`, `--parallel`, `--n-gpu-layers`, and `--reasoning`. The harness checks these flags, starts only its own child process, binds it to `127.0.0.1`, disables free-form reasoning so the schema-constrained calls spend their output budget on the required JSON, enables Metal offload, uses an 8,192-token context and one inference slot, and keeps at most one model loaded. It stops and awaits the owned server before switching models. With 36 GB unified memory, this deliberately favors sequential execution over keeping the 16–19 GB planner and worker files resident together.

Start the POC from the repository root:

```bash
AUTH_MODE=local CONFIG=config/local-poc.json cargo run -p gents-harness --locked
```

If `llama-server` is not on `PATH`:

```bash
LLAMA_SERVER=/absolute/path/to/llama-server AUTH_MODE=local CONFIG=config/local-poc.json cargo run -p gents-harness --locked
```

Open `http://127.0.0.1:8787`, select **Connect**, then **Start task**. POC mode locks the UI to this task:

> Build a Rust CLI that reads a JSON array of expenses and prints the total amount per category, sorted alphabetically.

The routing, plans, file edits, tool requests, repair decisions, and final assurance response come from local model calls. Compilation, tests, and black-box examples are real allowlisted offline Cargo processes. The tool API confines file access to the per-run source allowlist; generated processes receive a cleared environment and run inside a macOS sandbox that denies networking and writes to the harness source/configuration paths. The Compliance Security & QA Engineer cannot override a failed deterministic check. Up to two repair cycles are allowed.

Select **Cancel** to stop a run. Cancellation stops an active allowlisted tool or the application-owned `llama-server`; it never searches for or kills unrelated processes.

Artifacts are written to `data/poc-runs/<run-id>/expense-cli/`. Durable run events are in `data/runs.sqlite3`, model-loader output is in `data/llama-server.log`, and the UI can export the recorded event JSON. The tool API and process sandbox protect the harness source from generated-code writes.

The onboarding source files are under `docs/company/`. They define the fictional Gents Office expense contract and engineering release standard used by this test. Onboarding is required before routing or planning. After active work, each Gent's local planner model receives only observable events, deterministic evidence, retrieved document chunks, and its prior cache summaries from a rolling 24-hour window for a bounded dream pass. Valid summaries are atomically written to `data/rag-quick-cache/<gent>.json`; invalid dream output falls back to a deterministic evidence summary. Cache records contain no private chain-of-thought and retain at most 20 runs per Gent.

Troubleshooting:

- **Missing model:** startup fails with the exact configured path. Confirm the file exists and has a nonzero size; a directory alone is not treated as a complete download.
- **Runtime missing or incompatible:** install/select a compatible llama.cpp `llama-server`, then set `LLAMA_SERVER`. The harness reports a missing required flag instead of guessing alternate arguments.
- **Model load or memory failure:** inspect `data/llama-server.log`. Close memory-heavy applications or choose a compatible smaller configured GGUF; the harness does not deliberately use swap and never substitutes a mock.
- **Port conflict:** set an unused `LOCAL_MODEL_PORT`; the server remains loopback-only.
- **Invalid model response:** schema-constrained JSON is requested, then Rust validates it. One bounded correction attempt is made; persistent reasoning-prefixed, fenced, truncated, or schema-invalid output fails visibly.
- **Generated project failure:** open the run in Saved runs and inspect actual exit codes, stdout, stderr, judge feedback, and the retained workspace. A failed required check ends blocked after the repair limit.

## Final reliability upgrade

Read **[chapter 13](../docs/13-reliability-and-security.md)** for the SQLite journal, crash recovery, owner-scoped history/deletion, idempotency keys, wall-clock deadlines, deterministic acceptance vetoes, provider boundaries and CI. Review **[SECURITY.md](../SECURITY.md)** before handling sensitive data.

## What is implemented

| Area | Included behavior | Limit |
| --- | --- | --- |
| Harness | Existing text workflow plus a typed local coding POC with onboarding, retrieval, router, plans, tools, evidence, judge, repair and dream consolidation | Sequential execution; POC task is intentionally fixed |
| Agents | 1..8 available Gents; the local POC requires documented onboarding and per-Gent dream-cache consolidation | Teaching safety cap; extend deliberately for N |
| Providers | Mock, owned local llama-server, and text chat-completion adapters for OpenRouter, HF, Databricks, compatible Azure endpoints | Endpoint/model compatibility must be checked; no universal API compatibility |
| Office | Two pixel sprites per Gent, rooms reflect server events | Animation is a view of events, not evidence of hidden model reasoning |
| Journal | SQLite checkpoints, interrupted-run recovery, owner history/deletion, duplicate-submission keys | One process; durable cloud volume/shared storage still required |
| Acceptance | Frozen judge criteria, optional text vetoes, and POC compile/test/black-box evidence | The local POC checks only its fixed expense-CLI contract |
| Metrics | Provider input/output/cache/reasoning counts, cost when returned, latency | Missing usage is unknown; no fabricated estimates |
| Identity | Single-tenant RS256 JWT validation, scope/client checks, owner access | Native macOS PKCE code supplied; no token refresh; iOS bridge unfinished |
| Cloud | Bicep foundation, container, deployment walkthrough | Accounts/resources not created for you |
| Apple | Tauri desktop/mobile scaffold, publishing walkthrough | Build/sign/review on your Mac; iOS release blocked until auth completed |

## Source map

- `crates/harness/src/judges.rs`: **routing, acceptance checks and repair loops**.
- `crates/harness/src/main.rs`: the **harness** — run state, lifecycle, limits, scheduling, events, authorization gates.
- `crates/harness/src/store.rs`: single-process **persistent event journal and recovery**.
- `crates/harness/src/acceptance.rs`: **deterministic user acceptance rules**.
- `crates/harness/src/provider.rs`: the **model port/adapters** — vendor requests and normalized usage.
- `crates/harness/src/local_runtime.rs`: the owned, single-model **llama-server lifecycle**.
- `crates/harness/src/poc.rs`: the isolated **expense-CLI tool workflow and deterministic verification**.
- `crates/harness/src/auth.rs`: the **API authorization boundary**.
- `config/mock.json`: two **agent definitions**, each containing planner/worker model slots.
- `web/`: **office visualization**, task entry, logs, usage. HTML/CSS/JavaScript render the GUI; orchestration is Rust.
- `src-tauri/`: **native shell**, desktop sign-in and app packaging.
- `infra/`: **Azure foundation and backend container**.

Never place provider keys in the web files or native bundle. The desktop/mobile app calls your Rust API, which calls providers.

## Learning milestones

1. Read the architecture and run the mock office.
2. Trace one task through the Rust code.
3. Swap one model slot to a live provider.
4. Connect OpenRouter, Hugging Face and Databricks independently.
5. Add Entra sign-in and reject unauthorized API requests.
6. Deploy the authenticated Rust API to Azure.
7. Add durable state, RAG, tool permissions and telemetry.
8. Build and sign macOS, then finish the iOS integration and publish through TestFlight.

Verification details: `docs/11-validation.md`. Reference links: `docs/10-sources.md`.
