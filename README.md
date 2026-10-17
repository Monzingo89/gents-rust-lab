# Gents Office — Rust harness learning project

<p align="center">
  <img src="src-tauri/app-icon.png" alt="Gents Office owl wearing a tuxedo" width="220">
</p>

Open **START-HERE.html** for the full walkthrough and complete code appendix, or start at **[docs/00-start-here.md](docs/00-start-here.md)** and follow the numbered chapters. All commands run from this folder unless a chapter says otherwise.

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

The default demo now includes a routing judge and a completion judge, with a bounded repair loop. Start with **[chapter 12](docs/12-ai-judges-and-repair-loops.md)** for the full upgrade. The original fixed-team mode is available as `config/fixed.mock.json`.

## Final reliability upgrade

Read **[chapter 13](docs/13-reliability-and-security.md)** for the SQLite journal, crash recovery, owner-scoped history/deletion, idempotency keys, wall-clock deadlines, deterministic acceptance vetoes, provider boundaries and CI. Review **[SECURITY.md](SECURITY.md)** before handling sensitive data.

## What is implemented

| Area | Included behavior | Limit |
|---|---|---|
| Harness | One planner call, validated plan, bounded worker calls, result handoff | Judge selects a subset and model slots each round; sequential execution |
| Agents | 1..8 available Gents; judge selects planner/worker candidates from an approved catalog | Teaching safety cap; extend deliberately for N |
| Providers | Mock and text chat-completion adapters for OpenRouter, HF, Databricks, compatible Azure endpoints | Endpoint/model compatibility must be checked; no universal API compatibility |
| Office | Two pixel sprites per Gent, rooms reflect server events | Animation is a view of events, not evidence of hidden model reasoning |
| Journal | SQLite checkpoints, interrupted-run recovery, owner history/deletion, duplicate-submission keys | One process; durable cloud volume/shared storage still required |
| Acceptance | Frozen judge criteria plus optional deterministic substring vetoes | Text checks do not establish semantic correctness |
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
