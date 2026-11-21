# Shared, instance-bound probe lifecycle

November 21, 2026. The RAG and Rust real-model probes duplicated port allocation, environment setup, `/api/config` polling and child shutdown. Both accepted a successful response without proving that their newly launched process owned the listener. A regression using two actual Rust harnesses reproduced acceptance of the incumbent on an occupied port. No inference was needed to expose it.

Both probes now use `scripts/local_trial.py`. This small factory constructs the existing harness with separate ephemeral HTTP/model ports, a whitelist environment, local authentication/local-only inference, disabled office/scheduler work and offline download settings. It supplies a fresh `GENTS_STARTUP_ID` and requires that exact instance plus local/company/tool flags before returning an API handle. A wrong listener is rejected before company registration or task dispatch; it is not stopped. Readiness is only control-plane readiness, not model residency, native bundle verification or tool/task acceptance.

The factory owns a process group. Normal exit, startup failure, exceptions and SIGTERM clean up that group; a two-second grace is followed by forced termination and leader reap when needed. It checks descendants even after the leader exits. SIGTERM unwinds the Python context so a detached harness does not escape the evaluator's outer five-second cleanup grace. `lifecycle.json` records the instance, control-plane outcome, exit/forced-shutdown state and exception type without task payloads or credentials.

Readiness has both short HTTP timeouts and a POSIX wall-clock alarm, so a stalled response cannot keep the readiness loop alive. The factory is intentionally for these main-thread POSIX probe entrypoints. It restores signal handlers/timers and refuses to replace an active process timer. Normal `LoopbackAPI` calls retain their existing 30-second default; the new optional timeout argument lets readiness use its remaining budget. RAG now shares that existing no-proxy/no-redirect, bounded-response transport too.

## Evidence and verification

The initial readiness regression reported six failed assertions, including the actual occupied-port case. That failed output is retained; its shell wrapper printed the log afterward, so a standalone red-run exit code was not retained. The fixed tests pass without changing the assertions that exposed the bug.

Ten lifecycle tests cover foreign instance and unsafe policy rejection, process exit racing a successful response, connection/deadline failure, a probe that ignores its HTTP timeout, actual Rust startup/occupied-port preservation, parent SIGTERM cleanup, an ignored-TERM descendant and permission-error reconciliation. The Rust fixture's private run store contains no runs or events. Source comparison also confirms that all ten original probe acceptance assertions are unchanged (six RAG, four Rust). Model settings, tasks, grants, call limits and acceptance suites remain intact; the two probes retain their different checkpoint/persistence behavior.

A later repeated check failed with macOS `EPERM` while testing an exiting group's liveness. The failed log is retained. [Apple's syscall documentation](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kill.2.html) distinguishes permission failure from an absent process/group. The helper therefore reconciles that result using read-only process-group membership/state; it never treats `EPERM` alone as absence. An active group that cannot be signalled, a failed observation or malformed metadata still fails cleanup, with unknown forced-shutdown state recorded. The exact kernel cause of the observed transient remains unproven. The final ten-case suite and ten additional actual occupied-port/startup/shutdown repetitions passed; they do not establish arbitrary-load reliability.

The shared helper, its tests, both probe/evaluator entrypoints and the native runtime test directory are protected from unreviewed agent source patches. The existing broker denial suite passed all 32 tests with the added paths and case-alias coverage. Owner review remains available; model output cannot edit these host checks.

```sh
python3 -m unittest discover -s scripts -p test_local_trial.py
python3 scripts/local_evaluate.py
```

The expanded **eleven-command** baseline passed in **146.710 seconds**, including real Rust control-plane startup and the existing HTTP/SQLite, native stdio/SSE, cache, cancellation and screen fixtures. That run included the seven lifecycle cases present when it started. The later deadline/permission-reconciliation changes were checked separately with all ten final lifecycle tests passing. The experiment controller's 33 tests, independent evaluator's 21 tests and evaluator failure/timeout tests also passed. The new lifecycle test is wired into `scripts/check.sh` and CI; the full broad repository script and remote CI were not rerun for this Python-only slice.

This slice made **zero generative/paid calls and incurred $0 provider fees**. It does not establish new model task competence. Peak/RSS/context/cache/cost measurements not collected by these lifecycle checks remain unknown. Earlier real-model task results retain their recorded binaries/source identities; they were not rerun or relabelled after this refactor. [Versioned evidence](evidence/owned-probe-lifecycle-2026-11-21.json) records logs, source hashes, assertion comparisons and fresh verification.

No production company service or private configuration was restarted or changed. The existing launcher factory in [163](163-company-lifecycle.md) retains its broader sealed-bundle/native-transport readiness contract; this Python factory is limited to the two isolated probes. Open WebUI location and broader held-out routing qualification remain open, as do executor-retirement and lifetime-accounting backlog items.
