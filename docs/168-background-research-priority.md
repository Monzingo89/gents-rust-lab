# Optional research yields to asynchronous tasks

November 21, 2026. The office efficiency observer previously considered running legacy jobs and training when deciding whether to yield. It missed the asynchronous grounded and registered workflow executors. A five-second monitor sample alone also cannot safely coordinate cancellation reset with a newly admitted task.

## Changed behavior

Each newly launched asynchronous job now obtains an `office_activity::ForegroundWork` guard before spawning its worker future. The guard cancels optional research immediately and stays alive through execution, owned-session cleanup and terminal checkpoint handling. Compatible subscribers share one job and one guard. Normal completion, cancellation, invalidation, future abort and unwinding release the guard through Rust ownership.

Research admission and foreground admission share a small in-memory mutex. A racing research start either observes foreground work and declines, or starts first and then receives cancellation. An active job cannot have that cancellation cleared by the next monitor tick. Shutdown closes research admission for the service instance before stopping the existing local pool. The monitor reports its yielding phase while foreground work exists.

This gate carries a count, a shutdown flag and cancellation state. The existing runtime pool still enforces model capacity/residency. Existing task policy, grants, reservations, deadlines and validation remain unchanged. There is no second scheduler, task registry, service or crate. The observer still requires explicit `GENTS_OFFICE_ACTIVITY=1`, persists an attempt before inference, keeps its three-hour interval/unchanged-evidence gate, and caps output at 192 tokens with its existing two-minute timeout. Releasing foreground work does not automatically retry research or refund any budget.

The trusted-host office telemetry endpoint adds `foreground.async_task_jobs` with its own current sample timestamp. It describes active in-process job lifetimes; it does not count subscribers or prove model inference. The count includes no task bodies or actor identities, and does not grant access to another task's memory.

## Verification scope

Three new Rust tests exercise cancellation/restart exclusion across two foreground lifetimes, racing admissions, shutdown closure and a real Tokio future abort releasing its guard. They exercise admission state without invoking a model. Existing notebook interval, corruption and restart-budget tests remain unchanged.

The existing HTTP smoke gained a separate `--grounded` mode using the actual older adapter, mock provider and private SQLite store. It proves that two subscribers share one job/guard, cancellation of one does not stop its peer, completion releases priority, and source correction invalidates output and releases priority. It also checks the retained grounded receipt accounting: one fixture reply, zero generative calls, $0 provider fees and unknown tokens. The regular workflow and native stdio/SSE cases check the same shared-job lifetime rule; native revocation also waits for priority release before regrant.

```sh
cargo test -p gents-harness office_activity --offline --locked
python3 scripts/task_workflows_smoke.py --grounded
python3 scripts/local_evaluate.py
```

The fixed local baseline now runs nine commands, including both executor families. Exact results, source/binary hashes and the host inventory probe are in [the evidence record](evidence/background-research-priority-2026-11-21.json). These checks establish lifecycle/control behavior with fixtures. They do not measure actual backend interruption latency, real-model task quality, RSS/context or hardware/electricity/development cost. No new model, paid runtime call, download or production service restart occurred. The broader platform-dependent `scripts/check.sh` and a new visual browser session were not run for this backend-only slice.

Final verification passed **711 Rust tests**, with 33 opt-in tests ignored, strict offline all-target Clippy and all nine local baseline checks in **138.33 seconds**. The broker source guard also protects the observer admission code and these evaluation entrypoints against company-authored patches; its existing denial suite and evaluator failure/timeout tests passed. No evaluator criteria or runtime grants were relaxed.

Docker and Ollama CLIs were present during a read-only host probe, but the inspected local Docker daemon and default loopback Ollama endpoint were unreachable. No service was started. The actual Open WebUI endpoint, shared model ownership and external accounting remain unknown. Broader executor parity, representative real-model tasks and external integration remain on [the bounded backlog](162-local-execution-baseline.md).
