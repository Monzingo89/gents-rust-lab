# Bounded company startup and shutdown

The launcher previously waited for `/api/config` without observing cancellation inside its startup loop. It signalled only shell/wrapper PIDs, so build or service descendants could outlive a failed startup. An unsolicited exit could also leave a non-cancellable restart timer running for up to a minute.

The existing entrypoints now share `scripts/company-lifecycle.mjs`. `startOwnedProcess` is a small construction boundary for an owned process group, its actual exit result and idempotent bounded cleanup. It waits for the spawn/error event before addressing the group, requests graceful termination, then forces termination after the grace limit if descendants remain. It never signals an existing service discovered through HTTP. The macOS keep-awake utility is a leaf child and uses direct child signalling.

`scripts/start-company.mjs` retains environment reload, doctor/onboarding, local-only shell launch, optional email listener, exit-75 checkpoint restart and bounded exponential backoff. Setup commands have a 60-second ceiling; readiness has a five-minute ceiling; child cleanup normally allows five seconds. Cancellation interrupts readiness, setup and backoff. Failed startup/onboarding cleans up owned processes. Cleanup precedes replacement launch, preventing overlapping runtimes left behind by an exited wrapper.

`scripts/run-harness.mjs` uses the same lifecycle boundary for the actual Rust harness and offline scanner. Supervised descendants stay within the supervisor's process group instead of creating nested groups that could escape forced cleanup. The wrapper only selects this behavior when the launching supervisor PID matches its actual parent. Direct wrapper launches own their child groups themselves. These process identifiers are lifecycle metadata, not task authorization.

## Readiness contract

`probeCompany` accepts only a credential-free loopback HTTP origin and does not follow redirects. It checks:

- Local authentication, local-only inference, company mode and company tools in `/api/config`.
- The requested stdio/SSE native transport in `/api/mcp/catalog`.
- A supported, verified sealed native runtime bundle.
- For a newly launched service, the exact generated `GENTS_STARTUP_ID` returned by the Rust configuration endpoint.

The ID distinguishes the launched instance from another listener winning a port race. Environment reload preserves the host-provided startup ID and supervisor PID. Existing compatible services are checked and reused without acquiring process ownership. Incompatible or unsealed services produce an error; the launcher does not stop them or silently replace them.

This readiness check establishes the control plane and native bundle contract. Models still load on demand. It does not establish model fit, task acceptance, tool grants or external integration readiness. `/health` remains simple liveness.

## Verification

```sh
node --test scripts/company-lifecycle.test.mjs scripts/company-env.test.mjs scripts/model-security-service.test.mjs scripts/mcp-runtime-bundle.test.mjs
cargo build --workspace --offline --locked
node scripts/company-lifecycle-smoke.mjs
python3 scripts/local_evaluate.py
```

The lifecycle tests run the actual supervisor implementation against local fixture processes/HTTP. They check immediate cancellation, ignored cancellation in a probe, deadlines, spawn failure, early exit, a stubborn orphan descendant, preservation of an unrelated listener, wrong instance, wrong transport, unsealed bundle, cloud-enabled settings, onboarding failure, restart 75, cancellation during backoff and reuse of an already-running compatible service. Environment tests verify that local files cannot replace the launch IDs. Existing source-authoring gates now also protect the lifecycle helper, launcher and its acceptance fixtures from worker self-editing.

The separate smoke copies the current four Rust executables into a disposable workspace, creates the normal sealed bundle and starts the actual Rust harness with a synthetic configuration. It checks the returned startup ID and actual bundle binding, rejects a wrong instance, observes no runs, requests graceful shutdown and verifies that the listener closes. No production environment files, company stores, model weights, email or inference are used. Its measurements include startup/shutdown latency and one idle harness RSS sample; model/peak/GPU memory and real task completion remain unknown.

The baseline command includes both lifecycle checks alongside its existing task/cache/governance checks. The native transport, call-budget, revocation, cancellation and restart evaluators remain unchanged. Evidence for this slice is retained in [company-lifecycle-2026-11-21.json](evidence/company-lifecycle-2026-11-21.json).

## Current operating commands

```sh
npm run env:check
npm run company
```

Keep the company terminal running and open the printed loopback office address. Ctrl+C requests shutdown of that launcher's owned processes. The launcher refuses incompatible existing listeners; inspect and stop those intentionally in their own terminal. Fixture checks use temporary random ports and do not require shutting down the user's company. No live production-company restart or real-model readiness trial was performed in this slice. The separate mobile/activity launchers retain their own contracts; they were not consolidated without parity evidence.
