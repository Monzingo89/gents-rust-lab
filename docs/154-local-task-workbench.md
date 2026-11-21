# Local task workbench

The existing CEO floor now includes **Answers from local evidence**, mounted by `web/app.js` from `web/assets/task-workbench.js`. It uses the same authenticated API client, company selection and local service as other office controls. It adds no separate task store or model process.

Connect the company, select floor **10 / CEO**, and enter a question with an optional date/period. **Get local answer** calls the cache-first `execute` operation. **Analyze only** calls `destructure_task`; it does not authorize worker effects. With optional evaluated intake configured, analysis can itself be asynchronous. The host retains the exact original ask; the UI preserves it while polling the submitted task.

Running requests expose their subscriber task ID, poll `status`, and offer **Cancel my request**. The existing backend handles cancellation isolation for coalesced subscribers. **Check task** can resume a saved company-private UUID. **Clear view** stops browser polling and clears displayed results; it does not cancel backend work. There is no browser storage of private task text/results.

The panel shows actual execution path, cache provenance, measured calls/tokens/memory, answer, blockers, essential questions, assumptions, source identity/revision/quotes/byte coordinates/validity, acceptance checks, and expandable plan/analysis. Missing measurements say `unknown`; zero remains zero. Generated drafts retain the backend's unverified state. Source-citation membership does not become factual/build acceptance in the UI.

All model/source/user strings are inserted through `textContent`, including answers and JSON details. Company, connection, token and logout transitions reset the panel and invalidate pending response generations. Stale responses and queued poll callbacks cannot populate the new view. Clearing or switching views makes no automatic cancellation request. Existing company ownership/authentication and source permissions remain server-enforced.

Run from the repository root:

```sh
cargo build --workspace --locked --offline
python3 scripts/task_workflows_smoke.py --browser
python3 scripts/task_frontdoor_smoke.py --browser
```

These commands launch and stop isolated local harnesses on random loopback ports and use the installed Chrome through Playwright. Workflow generation is explicitly `provider=mock`, so this verifies UI/API/SQLite lifecycle and executor protocol, not model quality. There are no page-route mocks, downloads, provider charges or user-company mutations. The workflow test also runs all nine actual finite executor templates, duplicate subscription/cancellation and source invalidation checks.

Browser assertions cover deterministic source extraction, eligible exact repeat with zero generation, inert HTML-like source text, analysis-only output, asynchronous cancellation/status, a completed unverified draft with source evidence and independent checks `not_run`, unknown metrics, saved task status, company reset and 390-pixel layout without horizontal overflow. The existing CEO browser test checks that exact source/cache answers still bypass model planning and that additional acceptance criteria survive the handoff and prevent unconstrained answer reuse. Screenshots are retained under ignored `data/task-architecture-verification/`.

The workbench does not implement new workspace writes, builds, Cosmos queries, production model qualification or native MCP asynchronous inference. Those boundaries remain in the completion audit. The separate existing-weight MLX protocol trial is documented in [`153-pinned-mlx-task-execution.md`](153-pinned-mlx-task-execution.md).
