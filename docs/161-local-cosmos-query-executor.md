# Local Cosmos NoSQL query executor

This checkpoint adds an **in-process, read-only local emulator executor**. It
does not register a Cosmos MCP tool, create canonical native task receipts, or
enable the `cosmos_receipts` task validator. That validator remains explicitly
unsupported. The original task-engine brief remains active.

## Implemented boundary

- `mcp/src/cosmos_check.rs`: versioned Serde/JsonSchema owner contracts, explicit
  NoSQL gateway API/database/container, Hash v2 partition path, frozen row
  oracles and finite budgets. Candidate query files contain only named SELECT
  queries and parameters. Neither format accepts credentials, endpoints,
  arbitrary headers, commands, provisioning or permission increases.
  Every case must specify `partition`: an array selects that partition and an
  explicit JSON `null` permits cross-partition execution. Omission is rejected.
- `mcp/src/cosmos_check/runtime.rs`: immutable prepared input identity; explicit
  host IPv4-loopback emulator configuration; collection metadata before/after;
  real REST plan and query requests; bounded continuation handling; ordered or
  multiset row comparison retaining duplicates; response/header privacy checks
  before hashing/reporting; recursive duplicate JSON-key rejection.
- `mcp/tests/cosmos_check.rs`: nine deterministic transport/oracle tests and one
  opt-in real emulator fixture. Tests include wrong/partial/duplicate/reordered
  rows, partition mismatch, changed collection metadata, explicit cross-partition
  dispatch, missing metrics/plan, excessive rows/bytes, malformed/duplicate JSON,
  raw/escaped private values, redirects, HTTP errors, timeout and cancellation.
- `scripts/task_cosmos_emulator_live.py`: inspects an already-running pinned
  emulator, provisions a unique synthetic database, runs the Rust fixture and
  deletes only that database. It changes one synthetic item after the initial
  observation to prove a fresh check sees mutable data with unchanged source.
  It never starts Docker/VMs, installs software or loads a model.
- `scripts/verify_task_cosmos_report.py`: independently checks current source
  hashes, implementation/input identities, row digests, partition scope, status
  distinctions, false authority flags and the explicit lack of real RU.

The public host API is `HostEmulator::new(port, instance)` followed by
`Snapshot::prepare(contract, host, query_file_text)` and
`runtime::evaluate(&snapshot, &AtomicBool)`. Host callers must establish current
authorization before calling it. The actor label in a contract grants nothing.
There is no environment-driven endpoint fallback or cloud implementation.

Limits are 8 cases, 16 parameters/query, 4 KiB SQL/query, 16 KiB query input,
32 KiB contract, 4 pages/case, 64 rows/case, 32 KiB/response and accumulated
rows/case, 64 KiB final report, 100–2,000 ms/request and at most 30 seconds total.
At most 42 HTTP requests execute. There are no automatic retries, proxies,
redirect following, package scripts, subprocesses or database mutations in the
Rust evaluator. Cancellation is checked before/after each request; an in-flight
request is bounded by its timeout rather than instantaneously interrupted.

`page_size_hint` is a service hint; the separate row/byte limits are hard host
bounds. A repeated continuation or exhausted page limit cannot establish a
complete answer. Source identities include raw query bytes, frozen contract,
host binding and implementation bytes. They **do not freeze database contents**.
Every later validation must execute fresh reads. Matching collection metadata
does not imply an atomic snapshot of documents, and this code does not connect
these observations to an answer cache.

## Actual service evidence and limits

The fixture used the official ARM64 image
`mcr.microsoft.com/cosmosdb/linux/azure-cosmos-emulator@sha256:9c04e3dd9d2b5f9e612cd8c387945ccb99a1e685b8abf8f1590f9efca02813a6`,
health version `EN20261019`. The runtime was a separate Colima profile with two
CPUs, 4 GiB RAM and 12 GiB data/root disk limits. The user's stopped default
profile and active Docker context were preserved. No host directory was mounted.
The container used an internal network, no effective published ports,
`no-new-privileges`, 3 GiB memory, two CPUs and 512 PIDs. Explorer, telemetry and
OTLP/console exporters were disabled. Explicit SSH tunnels exposed only host
IPv4 loopback to the two private VM/container ports. The setup is local software
development infrastructure, with no cloud credentials, paid API or model calls.

Microsoft documents vNext as a subset of the NoSQL gateway API with
[authentication and RU accounting unsupported](https://learn.microsoft.com/en-us/azure/cosmos-db/emulator-linux#feature-support).
The service actually returned `x-ms-request-charge: 1`, timing/query metrics and
a logical query-plan response. Reports preserve those headers/plan as
**emulator observations**. `real_ru` stays null and `real_ru_state` is
`unavailable_emulator`. Requiring real RU returns `unsupported` even when the
functional query oracle passes. No charge estimate, production query efficiency,
physical index usage, plan accuracy or production RU claim is made. Some plan
and metric fields are placeholders. A plan-shaped response establishes presence,
not correctness of its contents.

The live trial separately checked partition-header-only selection of alpha and
beta rows and an explicitly authorized cross-partition query. A page-size hint
of one returned two rows in the observed alpha query; the report records that
the hint was not honored. Actual multi-page handling is covered by deterministic
transport fixtures, not claimed as proved by this one live query.

Loopback HTTP is deliberately unauthenticated. The host instance label binds
configuration, not a cryptographic identity for the listener or image. Docker
inspection in the fixture supplies separate provenance; the Rust HTTP response
cannot attest that image or rule out another local process replacing a listener.
Broader API/partition types, real account authentication, production RU, native
grant/currentness wiring and cross-domain Angular/.NET/Cosmos checks remain open.

The next integration must use an explicit worker grant and owner-approved
actor/run contract, execute HTTP between short authorization transactions,
recheck policy/source/cancellation before committing canonical observations,
charge actual requests/tool work to the shared task budget and preserve the
point-in-time nature of database evidence. Required Cosmos checks must bypass
answer/completed-task cache substitution. A stored query-source digest or an
unchanged collection metadata response must never stand in for fresh data reads.

## Commands

Ordinary CI uses no emulator:

```sh
cargo test --offline -p gents-local-mcp --test cosmos_check
bash scripts/check.sh
```

The following is explicit local development setup, never a task/tool action.
Create the isolated profile without changing the active Docker context or
mounting the home directory. Its installed image remains in this private VM's
cache after shutdown:

```sh
colima start gents-cosmos-sdk --activate=false --template=false --cpus 2 --memory 4 --disk 12 --root-disk 12 --mount none --ssh-agent=false --ssh-config=false --binfmt=false --vz-rosetta=false --runtime docker --vm-type vz
mkdir -p /Users/rmonzingo/Library/Caches/gents-sdk/cosmos-docker-config
cosmos_host=unix:///Users/rmonzingo/.colima/gents-cosmos-sdk/docker.sock
DOCKER_CONFIG=/Users/rmonzingo/Library/Caches/gents-sdk/cosmos-docker-config docker --host "$cosmos_host" pull mcr.microsoft.com/cosmosdb/linux/azure-cosmos-emulator@sha256:9c04e3dd9d2b5f9e612cd8c387945ccb99a1e685b8abf8f1590f9efca02813a6
docker --host "$cosmos_host" network inspect gents-cosmos-isolated >/dev/null 2>&1 || docker --host "$cosmos_host" network create --internal gents-cosmos-isolated
docker --host "$cosmos_host" run --pull=never --detach --name gents-cosmos-contract --network gents-cosmos-isolated --memory 3g --cpus 2 --pids-limit 512 --security-opt no-new-privileges=true -e ENABLE_TELEMETRY=false -e ENABLE_EXPLORER=false -e ENABLE_OTLP_EXPORTER=false -e ENABLE_CONSOLE_EXPORTER=false mcr.microsoft.com/cosmosdb/linux/azure-cosmos-emulator@sha256:9c04e3dd9d2b5f9e612cd8c387945ccb99a1e685b8abf8f1590f9efca02813a6
cosmos_guest_ip=$(docker --host "$cosmos_host" inspect gents-cosmos-contract --format '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}')
ssh -F /Users/rmonzingo/.colima/_lima/colima-gents-cosmos-sdk/ssh.config -o ControlPath=none -o ControlMaster=no -o ExitOnForwardFailure=yes -o ForwardAgent=no -o ServerAliveInterval=10 -o ServerAliveCountMax=2 -N -L "127.0.0.1:48481:$cosmos_guest_ip:8081" -L "127.0.0.1:48480:$cosmos_guest_ip:8080" lima-colima-gents-cosmos-sdk
```

Keep that terminal running. The SSH command fails if either local port is busy;
choose another pair and pass the same pair to the fixture. Do not replace another
process's listener. In a second terminal, from the repository root:

```sh
python3 scripts/task_cosmos_emulator_live.py --port 48481 --health-port 48480 --docker-socket /Users/rmonzingo/.colima/gents-cosmos-sdk/docker.sock --report data/task-cosmos-live/2026-11-19/standalone-release.json
python3 scripts/verify_task_cosmos_report.py data/task-cosmos-live/2026-11-19/standalone-release.json --output data/task-cosmos-live/2026-11-19/verification.json
```

Afterward stop the SSH terminal with Ctrl-C. Remove the owned test container and
stop only the dedicated profile:

```sh
docker --host unix:///Users/rmonzingo/.colima/gents-cosmos-sdk/docker.sock stop gents-cosmos-contract
docker --host unix:///Users/rmonzingo/.colima/gents-cosmos-sdk/docker.sock rm gents-cosmos-contract
colima stop gents-cosmos-sdk
```

The report source hashes identify the tested version. Rerunning after edits
requires a fresh live report; the independent verifier rejects source drift.
See `evidence/161-cosmos-query-executor-2026-11-19.json` for checkpoint results,
retained failures, source/log/report hashes and remaining scope.
