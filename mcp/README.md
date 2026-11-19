# Local Rust MCP

The shared Rust `task_engine` module now supplies the first cache-first task path to the authenticated harness owner API: exact source lookup/extraction, scoped lexical retrieval, durable results and validated exact answers. It adds no worker grants or native MCP tools yet. [Architecture implementation and outstanding scope](../docs/148-ceo-cache-first-architecture.md) track the broader CEO/RAG/workflow refactor.

Source-only candidate intakes can opt into `candidate_workflow.prompt_profile: "focused"`; omission retains the full legacy context. This immutable owner choice changes prompt context, not worker identity, native permissions, source/summary schema or source promotion. [Measured context and reviewed repairs](../docs/147-focused-author-context-and-reviewed-repairs.md) include retained failures and the separately reviewed company-test adoption.

Candidate intakes may include bounded `candidate_feedback` reference data under a strict native schema. It can relay verified prior source/diagnostics to a predeclared follow-up author; it cannot grant tools, alter acceptance or constitute a receipt. [Conditional experiment evidence](../docs/146-conditional-experiment-continuation.md) includes an actual failed repair and the unchanged-source stop.

The private host component adapter also supports pinned source/acceptance snapshot verification through the same isolated Rust verifier used by `rust_verify`. This adds no MCP tool or promotion authority. [Independent experiment results](../docs/145-independent-experiment-results.md) describe source custody, finite queue integration and the distinction between host observations and native run receipts.

Status: working governed stdio and loopback Streamable HTTP/SSE core; migration and continuous company-learning
integration are still in progress. This is synthetic local research, not a
regulatory compliance certificate or a general-purpose privileged shell.

Build: `cargo build --workspace --offline`. The trusted harness launches
`target/debug/gents-local-mcp` with `--root`, `--company`, `--owner`, `--run`,
`--actor` and `--phase`. These are process-bound values from the authenticated
run; model JSON cannot override them. Same-user malicious OS processes are
outside this trusted-host boundary. Stdio has no listener or credential. Optional SSE uses a private per-child bearer and loopback listener; see `docs/39-mcp-basement-and-local-ceo.md`.

`src/protocol.rs` implements shared MCP JSON-RPC lifecycle; `src/transports/` implements bounded stdio and loopback SSE framing,
initialization, ping, authorized tools/list and tools/call. Logs go to stderr.
`src/client.rs` owns a credential-free, cancellable child with a ten-second
deadline. `Authority` and `Tool` are the integration interfaces.

The existing governance implementation moved to `src/governance.rs`; the
harness re-exports it, so both use the same authorization/ceiling rules and
canonical company `run_governance` transaction, rather than divergent policy
copies. `functions/` currently registers native exact integer arithmetic,
memory_append, memory_search, skill_list, skill_read and bounded workspace files. Shared input contracts
and phase-scoped approved skills also moved here; harness modules are facades.
Native skill reads intersect the actor's assignments with its process-bound
planner/worker phase. Each function has one tool responsibility.
Existing Python tools remain on the legacy path pending individual migration;
they are not falsely advertised by this MCP server as migrated/audited.
Legacy jobs without an installed policy retain their old arithmetic/file/skill paths;
new MCP memory never bypasses policy or falls back to the old broker.

Every native call checks owner-approved frozen job scope and current policy.
SQL transactions fence revocations and memory writes. Receipts contain actor,
phase, epoch, argument/result hashes and zero provider charges, not raw tool
payloads. Raw memory records are append-only; search applies run/sender/epoch
filters before limiting. Regrant does not silently rehydrate old-epoch records.
Source-linked derived notes must reference currently authorized record hashes.
Known PII/card-pattern gates reuse the existing local safety implementation;
they are not complete DLP. SQLite is private local storage, not encryption.
Search is literal keyword retrieval, not semantic/vector RAG. No cloud
Databricks resources are provisioned.

Function code is not dynamically trusted because a model wrote it. New
implementations stay in isolated run workspaces until source review, fixed
security checks and regression tests pass. Registry promotion must remain a
trusted-host action. No paid provider, arbitrary subprocess, secret reader,
policy installer, download or shell tool is registered in the native server.
`audit/native-functions.json` records the focused source-review boundary and
denial tests, not an independent certification. Frozen intake can require
`required_mcp_calls: [{actor,tool}]`; completion then requires actual canonical
successful current-epoch receipts, not model claims. Additional document/chunk adapters,
planner orchestration, tier lifecycle and real continual local eval/tuning remain required
before the full user goal is complete.

`python_check` is a native, owner-scoped **diagnostic** tool for repaired Python
source. An owner freezes `python_check_contracts` with `id`, `actor`, explicit
`source_paths`, top-level `tests_path`, `tests_sha256`, and `interpreter_sha256`.
The launching host configures an absolute `GENTS_PYTHON` pointing directly to the
installed interpreter (for python.org framework installs, its
`Resources/Python.app/Contents/MacOS/Python` executable). The MCP client forwards
only that path across its cleared environment. No interpreter is downloaded.
The worker needs separate `python_check` and `files:read` grants. It supplies
`contract_id`; the harness binds current source/policy/implementation identity
before ordinary admin review, and native execution rechecks it before copying
inputs. Frozen test files cannot be overwritten with native file/media tools.
Unchanged identities retain the harness retry cap; generic shell replay rules
are unchanged. Full original journals remain intact.

The helper is embedded in the compiled MCP binary. Candidate code enters as
JSON data, executes in a separate read-only macOS sandbox and returns bounded
diagnostics through the existing native privacy gate and hashed receipt. A
Python program can spoof zero exit: `reported_pass` never establishes independent
acceptance, source promotion or company completion. No automatic acceptance gate
was added. Runtime libraries are not fully content-addressed and no results are
cached. See [experiment and limits](../docs/138-python-snapshot-verification-experiment.md).
Workers receive current-source, receipt-verified diagnostic observations during
planning; stale source results and later uncertain attempts are suppressed.
These are factual context, not cached execution or acceptance. The first actual
company trial regressed a passing repair and was stopped unqualified; see the
[negative result and follow-up](../docs/139-native-python-diagnostics-and-rejected-repair.md).

Fresh review-stage jobs may also approve `python_review_submission` with
`version:1`, the exact actor and diagnostic `contract_id`, together with
`deliverable_type:"python_review_candidate"` and a frozen `max_model_calls`.
Current-source native write and complete paged-read custody plus a receipted
`reported_pass` allow the host to propose `finish` once per source/contract
fingerprint. Ordinary admin and final QA still run; rejection creates no right
to repeat an unchanged automatic submission. This is explicit review-stage
submission, never automatic correctness acceptance or source promotion.
The attempt ceiling is run-wide, durable and includes failed preparation and
uncertain attempts. It grants no tools and cannot be reset by model switching.
See the [verified bounded trial and prior failures](../docs/140-bounded-python-review-submission.md).

Frozen Python contracts can pin immutable source dependencies and optionally
allow bounded, separate temporary scratch I/O. Confirmed candidate writes can
queue native readback and diagnostics under the existing grants. Current source
pages remain structured required worker material; source-bound rejection
feedback never becomes a path-wide ban on checking repaired bytes. See the
[observer trial and limits](../docs/141-observer-repair-and-python-followups.md).

Inspect implementation metadata without starting a company or running models:

```sh
./target/debug/gents-local-mcp --implementation-catalog
```

This command grants no access and performs no task tool calls. Regenerate the
reviewed cookbook from this catalog after reviewing new recipes and schemas.

`Workspace` is a separate interface, with a macOS/Linux descriptor-relative
adapter. `files:read`, `files:list` and `files:write` are independently grantable;
the stdio catalog narrows its operation schema accordingly. A planner can read
but cannot write, even with a broad file grant. Read pages are at most 8192 UTF-8
bytes from a regular single-link file up to 16 MiB; writes are at most 32768
bytes and also subject to the total JSON-RPC frame limit. Known sensitive-pattern
checks apply to the complete source, not just a selected page. Directory scans
and returned inventories are bounded. Windows fails closed; native macOS is
tested, while Linux is a supported implementation path not yet run here.

The adapter opens every directory relative to an owned descriptor with
`O_NOFOLLOW`, rejects hard links/special files, creates private files and replaces
one filename atomically. The local company's lexical helper is one filter,
not filesystem authority. Failed admitted external writes are conservatively
audited as effect_uncertain; a filesystem replacement and SQLite audit are not
a single distributed transaction. An injected audit failure test confirms that
the caller is instructed to inspect the path/hash and no success is claimed.
Do not blindly retry a mutating call after losing its receipt. See
[`docs/38-native-workspace-mcp.md`](../docs/38-native-workspace-mcp.md) for the
local-model provenance, focused audit and integration evidence.

Protocol references:
- [stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
- [tool lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)
- [security guidance](https://modelcontextprotocol.io/docs/2025-11-25/tutorials/security/security_best_practices)

Owner-frozen source-to-artifact bindings now persist selected native projections
separately from payload-free receipts and require canonical source plus writer
write/readback evidence and current file bytes. Optional process-bound
`--policy-revision` rejects stale source use inside execution transactions. See
`docs/40-ordered-ceo-teams-and-native-bindings.md` for the data-flow boundary,
observed semantic-QA false positive, focused tests and ongoing team experiments.

`table_aggregate` performs bounded typed-equality filtering, grouping and exact
integer totals over up to 64 flat scalar records. Inputs/outputs are capped at
16384 UTF-8 bytes; each record has at most 16 fields. No expressions, coercion,
deduplication, external paths, subprocesses or network are supported. All
selected fields must exist on every row, including excluded rows, and sum
values must be actual integers. Groups use canonical JSON key order and retain
zero totals. Boolean true, numeric 1 and string "true" remain distinct.

Native functions may declare `resultSchema` in the implementation catalog. This
schema describes the result value inside the native envelope, not the entire
MCP response. Declared schemas are checked before successful audit/binding
capture. Artifact pointers and encodings are checked by the MCP registry from
the tool-owned schema; the harness no longer duplicates arithmetic result-field
validation. Tools without declared schemas retain actual-result validation at
execution and are explicitly shown as such. A schema is neither execution
proof nor a grant. Scalar/array pointers still require a value in the actual
successful result; static bounds cannot prove a filter will produce a row.

A frozen artifact binding may include `argument_sources:[{field,path}]` for
read-only composition. Each field must already be an empty placeholder in the
literal input; each path labels an earlier approved projection in the same run.
The native resolver verifies canonical current-epoch receipts and access, then
replaces that field with the selected native JSON value and validates the final
arguments. It never reads the labeled workspace artifact to obtain source data.
Absent, stale or unauthorized dependencies cannot produce a satisfied binding.
See docs/43-receipt-bound-native-inputs.md for the CEO form and verified limits.

`table_read_json` reads a complete confined JSON table (<=8192 source bytes) with duplicate-key/type/whole-document checks and exact source hash/length. Its `/rows` can feed `table_aggregate` through approved native argument sources. It has its own read-only grant and does not imply general filesystem access. See [the file-table experiment and review repair](../docs/44-complete-local-json-tables.md).

For source-byte approval, supply `table_read_json.expected_sha256` in the immutable input contract. A changed file imports nothing; whole-file reads now reject the 8192-byte ceiling before reading oversized bodies. See [approved source bytes and the three-call workflow](../docs/45-approved-source-byte-contracts.md).

`table_join` provides bounded typed inner/left joins with explicit projections, stable row order and fail-closed output limits. Its two `/rows` inputs can come from independent native receipts. Pure computations declare `deterministic` separately from `read_only`; ordered-company scheduling can detect an already-observed exact source without fabricating another receipt. See [local joins and the reconciliation experiment](../docs/46-local-table-joins-and-reconciliation.md).


`files` also supports worker-only exclusive rename with `files:rename` plus
`files:read`, a mandatory source hash and an absent destination. Text and binary
files up to 16 MiB are supported; frozen inputs cannot be moved or replaced.
Use `read` with `metadata_only:true` (without paging) to inspect opaque file hashes.
This does not establish text readback. An unconfirmed harness rename retains its
intent and stops for reconciliation. See [contract and tests](../docs/143-governed-file-renames.md).
