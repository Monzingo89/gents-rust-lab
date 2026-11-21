# Frozen native .NET HTTP checks

`dotnet_api_check` compiles an explicitly captured, package-free Web SDK project, runs that copied application, and sends owner-frozen HTTP cases to it. The host checks actual status codes and JSON. Compilation, application behavior and independent acceptance remain separate results. This implements a finite .NET/API validator; Angular/Cosmos integration and independent specialist qualification remain open in the [full brief audit](151-task-engine-completion-audit.md).

## Authority and contract

API execution requires all three current actor grants: `dotnet_check`, `dotnet_api_check`, and `files:read`. A compilation grant alone cannot execute the application. The approved parent intake must contain a matching actor contract in `dotnet_check_contracts`, including the project hash, exact installed SDK version, executable SHA256 and explicit C# source paths. The existing strict project parser rejects packages, custom targets/imports, analyzers, project references and executable MSBuild properties. API mode additionally requires `Microsoft.NET.Sdk.Web` and `managed_dll`; no dependency download or unsandboxed fallback exists.

Add this `api` object to the frozen build contract described in [156](156-offline-dotnet-build-checks.md):

```json
{
  "schema_version": 1,
  "listen_mode": "inherited_loopback_socket",
  "startup_timeout_ms": 3000,
  "request_timeout_ms": 1000,
  "runtime_timeout_ms": 8000,
  "cases": [
    {
      "id": "health",
      "method": "GET",
      "path": "/health",
      "expected_status": 200,
      "expected_json": {"ok": true}
    },
    {
      "id": "sum",
      "method": "POST",
      "path": "/sum",
      "request_json": {"left": 7, "right": 5},
      "expected_status": 200,
      "expected_json": {"sum": 12}
    }
  ]
}
```

Only 1–8 uniquely named GET/POST/PUT/DELETE cases are accepted. Paths are bounded relative targets. Caller-selected endpoints, executable arguments and network permissions are absent from this contract. Each case is at most 8 KiB and the total serialized case data at most 32 KiB. Privacy checks apply before persistence. Expected responses remain in the host; they are not copied into the application environment or scratch files.

The candidate must accept the host's inherited listening socket, for example:

```csharp
var builder = WebApplication.CreateSlimBuilder(args);
builder.WebHost.ConfigureKestrel(options => options.ListenHandle(
    ulong.Parse(Environment.GetEnvironmentVariable("GENTS_API_LISTEN_FD")!)));
var app = builder.Build();
app.MapGet("/health", () => new { ok = true });
app.MapPost("/sum", (Operands p) => new { sum = p.left + p.right });
app.Run();
record Operands(int left, int right);
```

The installed framework's [Kestrel API](https://raw.githubusercontent.com/dotnet/aspnetcore/v10.0.0/src/Servers/Kestrel/Core/src/KestrelServerOptions.cs) supplies `ListenHandle`. The descriptor environment variable is the host interface; the candidate does not choose a port.

## Runtime and host oracle

The host binds and retains an IPv4 loopback listener on an ephemeral port, then passes it as descriptor 200. Keeping that listener alive avoids a release/rebind race. The macOS runtime profile denies every new bind, outbound connection, filesystem write except `/dev/null`, and fork. It permits only the exact installed dotnet executable, copied scratch reads, runtime/host libraries and required public system libraries. Runtime SDK-directory reads and global filesystem metadata access are absent. Input/source and artifact hashes are checked around execution.

The environment is cleared and rebuilt without credentials or user configuration. Process inspection is explicitly self-only, and process-argument sysctls are explicitly denied. The live boundary fixture checks a numeric `kern.procargs2` buffer read against its actual test parent without logging the buffer. A deny-default profile with only a sysctl allowlist did not stop that read on this host; the explicit process inspection restriction is required by the passing fixture. Required hardware queries include `hw.pagesize_compat`, consistent with the installed Apple sandbox profiles. These are measured boundaries on this macOS host, not a claim of isolation on other platforms.

The host disables proxy use and redirects and sends each case once. A failed POST is not retried. Requests require the frozen status, `application/json` media type and exact structural JSON equality. Duplicate decoded keys are rejected through the same strict JSON decoder used by project metadata. Responses over 8 KiB, privacy failures, invalid JSON, incorrect media/status/data, exits and timeouts cannot pass. The HTTP oracle retains only response size, digests, status and generic diagnostics; it does not log response bodies or transport-error strings. Bounded SDK/application stdout and stderr remain diagnostics under the existing privacy gate.

The first request's timeout includes startup and is at most five seconds; subsequent requests are at most two seconds each. The complete runtime contract is at most fifteen seconds and must cover its request budget. A process-group watchdog, bounded output drains, sampled 2 GiB RSS limit and scratch limits remain active. Ending checks kills the application group. SDK phase limits, immutable input rules and short transactions remain those described in [157](157-executable-task-validators.md). Compiler output and runtime output are diagnostics, never an HTTP success oracle.

An initial feasibility probe that allowed loopback binding also allowed a wildcard bind on this host. That approach was not adopted. The retained inherited-socket fixture verifies denial of wildcard binds, another live loopback listener, host-file reads/writes, scratch writes, same-dotnet subprocesses and cross-process argument reads.

## Task integration and receipts

The built-in `dotnet` profile now requests `dotnet_api` and `dotnet_build`, alongside custody and output-shape checks. Current host policy and admission-bound source identities determine whether each can execute. Missing API cases or grants remain explicitly `not_run`. Model output cannot create a runtime binding or grant. API validation is a host phase, separate from model-callable read tools.

One native API invocation records both its compilation and HTTP facets. A three-stage pipeline therefore compiles once per API contract. Its later build check reuses that same task's canonical compilation facet after current authorization/source checks. This reuse is keyed by contract ID; a separate compile-only contract still runs its own SDK build. Each actual invocation reserves the scheduler's conservative 2 GiB estimate and charges one shared tool attempt. The compile-facet projection does not reserve or charge a second SDK invocation. If the API invocation loses or fails to commit its canonical receipt, its compilation facet stays `not_run`; that same contract is not automatically recompiled by the later build validator. A live negative fixture constructs a synthetic test-card diagnostic at runtime and proves the unchanged privacy gate blocks its result without another SDK build.

An application can compile successfully and fail its API case. The resulting task is blocked with separate passed-build and failed-API checks. A zero exit or `{"passed":true}` printed to stdout cannot pass an HTTP check. Passing finite cases does not establish general API correctness, factual answer quality, independent acceptance, expert qualification or promotion, and does not enter the verified answer cache.

The API report records host observation time in Unix seconds; exact millisecond timing remains in the canonical audit. This avoids card-shaped numeric timestamps in diagnostics while retaining the unchanged privacy gate for candidate data and output.

Native results include `compile_passed` and an optional `api` report with per-case status, observed/expected JSON digests, contract digest and runtime observations. Compact task receipts optionally include `api_contract_sha256`. Both facets point to the same canonical native result and artifact. Source identity includes execution mode, full frozen contract, explicit input hashes, actor/run/policy and implementation digest. Old implementation-bound claims intentionally invalidate. The complete MCP frame is size-checked before the observation and canonical audit commit together.

Status and saved observations recheck current sources, SDK/grants and canonical audits. Source edits or missing receipts clear answer claims. Another actor cannot retrieve the task. Build-only observations remain separate from `latest_current_api`; the latter exposes only a current canonical API result. None of these reads reruns code.

## Reproducible checks

```bash
cd /Users/rmonzingo/repo-applications/gents-rust-lab
cargo build --offline --workspace
cargo test --offline -p gents-local-mcp --lib dotnet_check::api
GENTS_DOTNET=/opt/homebrew/bin/dotnet \
  cargo test --offline -p gents-local-mcp --test dotnet_api_check -- --include-ignored --nocapture
GENTS_DOTNET=/opt/homebrew/bin/dotnet python3 scripts/task_validation_smoke.py --api-live
GENTS_DOTNET=/opt/homebrew/bin/dotnet python3 scripts/task_validation_smoke.py --api-live --sse
GENTS_DOTNET=/opt/homebrew/bin/dotnet python3 scripts/task_validation_smoke.py --mixed-contract-live
CARGO_NET_OFFLINE=true bash scripts/check.sh
```

Commands finish without leaving a server terminal running. Live fixtures require already-installed SDK `10.0.401`; missing prerequisites fail without downloading. `--report <new-json-path>` stores task reports in an existing parent directory. `GENTS_API_TEST_REPORT=<new-json-path>` retains component receipts. Ordinary repository checks run the bounded HTTP oracle and contract tests without installing .NET or executing candidate applications; the installed-SDK component test is opt-in.

The task fixtures use mock worker replies and real native stdio/SSE, owner HTTP admission, SQLite, installed SDK and application HTTP. They test source/audit invalidation, actor privacy, a failed API despite a passing compile, shared budgets, no duplicate compilation and a changed in-flight source preventing dispatch. The mixed-contract fixture independently checks that compile-only contracts are retained. Component fixtures also test application exit/spoofed output, an unresponsive endpoint, and the runtime boundaries above. No model quality or specialist admission follows from these tests. Provider cost is zero; no cloud, paid service or model download is used.

The first full repository check exposed a mock-operator regression after optional transcript compaction: the mock repeated a completed write because it ignored the retained successful-operation index. The mock now consumes that host index, and a regression test verifies progress survives compaction while failed observations cannot satisfy it. Production completion gates, permissions and compaction rules are unchanged. The failed run is retained with the checkpoint evidence.

Checkpoint results and source/report hashes are recorded in [evidence/158-dotnet-api-checks-2026-11-19.json](evidence/158-dotnet-api-checks-2026-11-19.json). The overall goal remains active and incomplete.

The final repository check passed with 677 Rust tests and 29 opt-in skips, plus the Python/Node/protocol/web/site checks. Separate installed-SDK component and stdio/SSE task trials passed on the tested Mac. The independent report verifies thirteen canonical task SDK results, twenty-two task receipt facets, three rejected source-bound invocations and five component result hashes. Test workers were mocked; compilation, application HTTP, source/audit invalidation and runtime boundary checks were real. Final portability annotations were separately format/Clippy/build checked, and the current implementation digests were exercised again by the live SDK fixtures. No Linux target was installed or cross-compiled.
