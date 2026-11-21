# Task-local workspace evidence and project declarations

The registered owner workflow can now call `workspace_read` and `workspace_list`, alongside `search_knowledge`. These reuse the existing descriptor-relative native workspace adapter. They do not execute package scripts, MSBuild projects, database queries or arbitrary commands. Source capture, model generation and runtime validation remain separate operations.

`mcp/src/task_engine/read_tools.rs` defines the finite argument schemas. The trusted policy intersects each profile's tool vocabulary; model output cannot add a root, tool, endpoint or executable. Native JSON decoding is narrowed to that node's actual allowed tools, and Rust validates again before dispatch. The default remains knowledge search only. Existing native MCP `files` grants and approved side-effect workflows are unchanged.

## Configuration

Inside a complete trusted harness `task_workflows` configuration:

```json
{
  "workspace_reads":true,
  "workspace_root":"data/companies/{company}/runs/<existing-run-uuid>/workspace"
}
```

Replace `<existing-run-uuid>` with the approved run whose existing workspace should be read. The host substitutes the authenticated company identifier; the model never supplies this root. When no root is configured, the default is `data/companies/{company}/task-workspace`. The read workflow does not create directories or populate files. An absent workspace fails clearly.

A deliberate shared project subdirectory such as `projects/sample` is also supported. That is a host decision to expose that project's files to authorized company owners using the workflow configuration. Company-private roots use the `{company}` placeholder. Configuration rejects the app root, absolute/traversal paths, reserved config/model/build directories, and a literal path into another company's private store. Descriptor confinement independently rejects symlinks, hard links, hidden names and special files. The existing privacy classifier applies to the complete file before any page is returned; code/manifests receive no content exemption.

An optional per-model `max_input_bytes` (1024–65536) sets the serialized input ceiling when a measured workflow needs a different host byte allowance. The native runtime still independently enforces its token context and generation controls. This field is not permission to truncate input or evidence. When omitted, the prior conservative byte allowance is preserved. New disabled/absent settings are omitted from serialization to retain prior configuration fingerprints.

The owner API continues to use `POST /api/companies/<company>/tasks`. For example:

```json
{
  "op":"execute_workflow",
  "input":{
    "original_ask":"Read frontend/package.json and report the declared Angular range; do not claim installed versions or a successful build."
  },
  "hints":{
    "domain":"angular",
    "template":"tool_loop/v1",
    "evidence_required":true
  }
}
```

The permitted node tool arguments are:

```json
{"name":"workspace_read","arguments":{"path":"frontend/package.json","offset_bytes":0,"limit_bytes":1024}}
{"name":"workspace_list","arguments":{"path":"frontend"}}
```

An empty listing path denotes the configured project root. Reads are 1–8192 byte UTF-8 pages and fingerprint the complete bounded file. Listings retain the existing bounded scan/filter, serialize at most 4096 bytes and explicitly mark truncation. Neither a partial page nor a truncated/filtered listing establishes absence outside its coverage.

## Evidence, reuse and invalidation

`workspace_evidence.rs` persists captures in the existing company SQLite database, separate from the shared knowledge-source/index tables. Captures bind tenant/subject, task, complete scope/configuration, root identity, relative path, full-file or listing SHA-256, byte coordinates and TTL. Directory evidence refers to the exact filtered listing. File evidence refers to original file bytes. Identical rereads retain the original timestamp; popularity does not extend TTL.

Negative reuse eligibility expires after one second, independently of the up-to-one-hour job/history retention. The additive `retained_until` migration preserves existing job bodies/digests and prior expiry values; it launches no work. A retry receives a new task/job while the original failure remains inspectable. This read-only task engine's intake/model/tool/plan audit events follow its bounded job retention and are removed when their jobs expire or are invalidated. Cleanup names only these new event kinds; native action, approval and uncertain-effect records remain untouched.

There are at most 32 captures per task and 512 per tenant/subject, with bounded rows and up to one hour of retention. New captures remove expired rows. Cache inspection reports this component; invalidation removes terminal captures and keeps active-task proof. A missing/tampered record is not replaced with model prose.

The host rechecks captured inputs before subsequent dispatch and final delivery, even inputs a later answer did not cite. Status rechecks the current authorized capture root independently of ordinary corpus dependency roots. A changed tail outside a displayed page, deletion, scope/root change, expiry, forged quote or changed listing invalidates the answer. Ordinary ingested corpus dependency paths remain relative to the original trusted repository root; capture paths are relative to the configured project root.

Identical-request coalescence also checks file captures. A stale completed job is kept as invalidated history and detached from the reusable key; a new request receives a fresh job rather than its stale answer. An affected active job is stopped before further compatible reuse. Compatible unchanged duplicates share the original execution, and additional model/tool calls are zero for the duplicate subscriber. Original plan/task provenance is retained.

Raw native tool results are audited with call IDs, success/failure and measured elapsed time. Prompts carry each evidence passage once plus stable passage indices in tool-result metadata. A successful final source recheck adds `current_workspace_source_bytes/v1: passed`. Independent factual/build acceptance stays `not_run`; generation remains unverified and does not enter the verified answer cache.

## Deterministic project metadata

`project_metadata.rs` attaches typed static observations to complete manifest reads. It makes no observations from partial input. It preserves separate or conflicting declarations and rejects duplicate decoded JSON keys. MSBuild parsing uses the locally cached, pinned Quick XML 0.42.0 API; it bounds XML depth/events, rejects DTD/entity expansion, and never evaluates projects, imports, conditions or expressions.

| Source | Observed data | Unestablished data |
| --- | --- | --- |
| `package.json` | Exact Angular/CLI/TypeScript dependency, devDependency and peerDependency ranges | Installed or published package versions, compatibility, successful Angular compilation |
| `package-lock.json` | Exact package records when present | Current installation state |
| `.csproj` | SDK-family declaration and selected property literals, preserving conditional/expression markers | Installed SDK version, effective evaluated properties, successful .NET compilation |
| Cosmos declaration JSON | Declared API/container/partition-key values, including resource-shaped declarations | Actual cloud schema, executed query, RU charge or query plan |

Malformed/unsupported metadata returns the same typed schema without extracted facts. Unrelated API JSON is not classified as a Cosmos declaration; the separately captured source observation remains available. Runtime checks in the metadata are explicitly `not_run`. This is source inspection, not specialist qualification.

## Verification and bounded live evidence

From the repository root:

```sh
cargo test -p gents-local-mcp --test workspace_tasks --locked --offline
cargo test -p gents-harness workspace_settings_tests --locked --offline
python3 scripts/task_workflows_smoke.py --workspace
python3 scripts/task_workflows_smoke.py --workspace-denied
# Includes these checks and the existing repository suite:
CARGO_NET_OFFLINE=true bash scripts/check.sh
# Optional existing-model trial, no download, cloud or production admission:
python3 scripts/task_workspace_live.py
```

Core filesystem tests cover full-file hash changes outside the page, root/scope/TTL isolation, forged evidence, bounded captures/listings, privacy, link rejection and deletion/invalidation. Metadata tests cover conflicting ranges, conditional/unevaluated MSBuild properties, duplicate decoded keys, malformed XML and unavailable runtime facts. Real HTTP/SQLite tests read three fixture manifests, verify native captures/metadata, preserve the shared corpus, reject denied reads, invalidate changed files, reject same-ask stale reuse and stop on a mid-flight change. Model decisions in those transport tests are marked mock.

A separate native Qwen3-4B trial read a company-private fixture manifest and returned its exact randomized declaration `^20.3.158`: one actual workspace tool call, two native generative calls, 1867 input tokens, 731 output tokens, 14.23 seconds in the coordinator, and 3,930,570,752 bytes maximum observed process residency. The source hash and frozen literal/call-count criteria passed. The fixture range does not assert a published/installed Angular version. Provider fees were zero. Report, criteria and archived private workspace are under ignored `data/task-workspace-live/9e1ea1b6-5723-4ae7-9ee6-b10e7714cb9e/`; the report pins its tested binary and model. Later result-check metadata is covered by the current deterministic/HTTP checks. One tool-use trial does not qualify a specialist or repair the earlier failed intake qualification.

The full goal remains active. Native MCP async execution, broader provider/embedding adapters, real specialist builds/queries and integration acceptance, task UI integration, routing caches and independent model qualification remain on the original completion audit.
