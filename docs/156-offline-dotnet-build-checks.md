# Owner-scoped offline .NET build checks

Status: implemented native compilation check; overall task-engine goal remains active. This is a real installed-SDK build boundary, not independent API correctness, Angular integration, Cosmos execution, specialist qualification, or production model admission. The task answer executor still reports its independent build/functional acceptance as **not run**; it does not silently inherit receipts from unrelated company actors.

## Native capability and custody

`dotnet_check` extends the existing local MCP registry and both stdio and owned SSE transports. It is worker-only and requires current `dotnet_check` and `files:read` grants. Tool arguments select one `contract_id`; the trusted harness computes and binds `expected_source_identity` before review and dispatch. A model cannot choose a command, endpoint, runtime, package source, SDK, or source root. Missing host identity, changed source, wrong actor, planner phase, revoked access and a changed expected policy revision fail closed.

The owner freezes up to four `dotnet_check_contracts` in the approved company intake. Example (replace the digest placeholders with actual lowercase SHA-256 values before intake):

```json
{
  "dotnet_check_contracts": [{
    "id": "api-build",
    "actor": "Dotnet",
    "project_path": "api/Api.csproj",
    "source_paths": ["api/Program.cs"],
    "sdk_version": "10.0.401",
    "dotnet_sha256": "<installed-dotnet-executable-sha256>",
    "artifact_mode": "managed_dll",
    "frozen_inputs": {
      "api/Api.csproj": "<owner-project-file-sha256>"
    }
  }]
}
```

The project definition must be frozen. Optional additional `frozen_inputs` pin explicit C# dependencies. Native file writes/renames protect these paths and inode aliases. Candidate C# files can change under their existing write grants; changing them creates a new check identity without resetting the host's unchanged-source retry limit. The identity includes the actor/run, policy epoch, frozen contract, complete input hashes and implementation digest. Current planning observations require a canonical MCP receipt with matching arguments/result hashes; model prose or a lost receipt is insufficient. This does not add a completion requirement or grant any access.

## Supported project subset

The tool copies one `.csproj` and 1–63 explicit UTF-8 `.cs` inputs, at most 2 MiB total, through the existing descriptor-confined workspace reader and privacy filter. Inputs must be in the project subtree. Hidden/generated `bin`/`obj` paths, traversal and duplicate paths are rejected. Unlisted sibling code, `Directory.Build.targets`, repository configuration and workspace `global.json` are **not inputs**. A result therefore describes only the explicit snapshot, never a build of the whole original repository.

Supported projects use `Microsoft.NET.Sdk` or `Microsoft.NET.Sdk.Web`, literal unconditional properties and `net9.0`/`net10.0` matching the frozen SDK major. The project must explicitly set `UseAppHost=false` and the owner contract must select `managed_dll`. The checker does not silently replace a requested native executable with a DLL. It rejects custom SDKs, imports, targets, tasks, conditions, property functions, package/project references, analyzers and arbitrary item groups. Complex existing projects return an explicit unsupported-project error; they are not rewritten into a passing substitute.

The private snapshot contains a host-generated `global.json` with exact SDK selection and no roll-forward, a NuGet configuration with empty package sources and a private empty package cache. The installed runtime's first-use migration marker is seeded only in the new empty scratch data directory. Existing user NuGet state is untouched. The migration-path behavior is documented in the [NuGet migration runner source](https://raw.githubusercontent.com/NuGet/NuGet.Client/dev/src/NuGet.Core/NuGet.Common/Migrations/MigrationRunner.cs) and was verified with the installed SDK; the upstream development branch is not a content attestation for that SDK.

Only these fixed phases run: SDK version preflight, offline restore, and `build --no-restore --disable-build-servers -p:UseSharedCompilation=false`. A passing result requires all three phases, exact SDK version, unchanged inputs/executable identity and an actual nonempty managed DLL whose hash is returned. The DLL is not retained, loaded or executed by this tool. A C# compile error produces `build_failed`; interrupted/limited/truncated execution or changed candidate source cannot produce `build_passed`.

The DLL hash identifies that particular build artifact. Identical source identities do not promise byte-for-byte reproducible output across fresh scratch paths, debug metadata or installed SDK library changes. These checks execute again; they do not use an answer cache to substitute for a build.

## Isolation and limits

There is no unsandboxed fallback. The current adapter requires installed macOS `sandbox-exec`, an explicit absolute `GENTS_DOTNET`, and the already-installed frozen SDK compiler. Environment inheritance is cleared; only private CLI/data/package/temp locations and fixed diagnostic/telemetry flags are supplied. Deny-default Seatbelt permits reads of the installed .NET/system library roots and scratch, writes only inside scratch and `/dev/null`, and execution only of the exact `dotnet` and frozen SDK `csc` executables. Copied inputs and bootstrap files have explicit write denials. Network, shell execution and run-workspace writes are denied.

Each phase has a 30-second wall limit, 25-second per-process CPU limit, 32 MiB per-file output limit and 256 open descriptors. The host samples the entire owned process group's RSS against 2 GiB and scratch against 256 MiB/4096 entries at approximately 100 ms intervals. Output readers drain streams but retain only 4096 bytes per stream per phase; truncation cannot pass. Sampled RSS/scratch observations are not hard instantaneous memory caps or total unified-memory measurements. The SDK/runtime library closure is trusted installed software and is not fully content-addressed; the report records the `dotnet` executable hash and selected SDK version separately.

A phase completing before the first sampling interval can report zero for its sampled maxima; this means no sample was collected, not zero memory or scratch use. In the live trial, the short SDK-version preflight has this limitation; restore and build have nonzero sampled observations.

The process group is killed on ordinary exit/error/timeout. A separate trusted pipe watchdog also kills the group when its MCP parent exits without running Rust destructors, including client cancellation/server termination. An independent process-death regression exercises this case. Temporary files are deleted on normal scope exit. Abrupt server death can leave an isolated scratch directory; it cannot turn a lost result into a passing receipt or automatic replay.

## Checks and commands

Ordinary repository checks run the strict project/contract/access tests and macOS watchdog regression. The SDK build trial is explicitly opt-in, so machines without .NET do not install dependencies or pretend to have run a build:

```bash
cd /Users/rmonzingo/repo-applications/gents-rust-lab
cargo test --offline -p gents-local-mcp --lib dotnet -- --nocapture
cargo test --offline -p gents-local-mcp --test dotnet_check
GENTS_DOTNET=/opt/homebrew/bin/dotnet \
  cargo test --offline -p gents-local-mcp --test dotnet_check -- --include-ignored --nocapture
CARGO_NET_OFFLINE=true bash scripts/check.sh
```

These commands exit when finished; no terminal/server stays running. Configure an equivalent absolute installed runtime path on another machine. The current live fixture explicitly requires SDK `10.0.401` and target `net10.0`; it does not install a missing SDK. To retain the actual receipt report, also set `GENTS_DOTNET_TEST_REPORT` to an existing parent directory plus a new JSON filename.

The live trial verifies a deliberate compiler failure; corrected source builds over both actual transports; stale source identity, frozen project writes, wrong actor/planner and revoked read access are rejected; removal of the canonical audit receipt prevents a historical observation being presented as current; unlisted invalid C# and custom targets are excluded from the copied build. All results retain `qualified=false`, `promoted=false`, `authority_granted=false`, zero model calls and zero provider charge.

Implementation: `mcp/src/dotnet_check.rs`, `mcp/src/dotnet_check/sandbox.rs`, `mcp/src/functions/dotnet_check.rs`, current native client runtime forwarding, company intake validation/protected inputs, and existing harness review/recovery/retry accounting. Tests are in `mcp/tests/dotnet_check.rs`, the module's strict XML/watchdog tests and `recovery_scope.rs`.

Remaining work from the full brief includes a task-level validator registry consuming correctly scoped execution receipts, real application/API contract checks, Angular build/test support, actual Cosmos query/RU receipts under explicit connectivity policy, and independent specialist/intake qualification. No part of this checkpoint resolves those by naming a profile or by compiling a console fixture.

Checkpoint: [`evidence/156-dotnet-build-checks-2026-11-19.json`](evidence/156-dotnet-build-checks-2026-11-19.json) records source/report/log hashes, the full repository check (666 Rust passes, 28 opt-in skips), and the separate installed SDK trial (two focused tests passed, including actual stdio/SSE compilation). The deliberate failure took 1.360 seconds; the passing stdio/SSE checks took 1.547/1.779 seconds. These are fixture measurements on this host, not throughput or specialist-quality benchmarks. No model was invoked or downloaded; provider charges were zero.
