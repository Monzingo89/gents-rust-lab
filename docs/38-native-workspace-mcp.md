# Native workspace MCP: local authoring and focused audit

## What changed

Governed `files` requests now use the Rust stdio MCP server rather than the
Python broker. The `Workspace` interface separates model-facing capability
contracts from platform I/O. The owner can independently grant files:read,
files:list or files:write. MCP tools/list exposes only those operation schemas;
planner-phase schemas and execution both exclude writes. Skill-read schemas
also enumerate only the assigned skills valid for that phase.

The native adapter bounds file sizes, read pages, write bodies and directory
inventories. Each read page includes the hash of the complete observed file,
the byte offset, truncation and next offset. Unicode page boundaries are
preserved. A planner can inspect an authorized artifact without receiving
write authority. Completed jobs can still read authorized evidence but cannot
mutate files. Legacy jobs without installed governance retain the old broker;
this compatibility route is not claimed migrated or governed by MCP.

## Company contribution and actual failures

The local company authored `is_safe_relative_path` and its tests. First trial
`97746e3a-0435-418d-afc7-0615d25759d9` completed the source-proposal task but its
draft did not compile and one test contradicted the 12-component contract.
It was rejected for promotion. Compiler feedback was supplied without writing
the corrected function for the model. Revision
`c2d7a7e0-8223-474d-8ccb-8caf0331b359` repaired the syntax and boundary cases;
its two authored tests and the independent host suites pass. Only attribution
and formatting were changed by the host in the promoted module. Its source and
test fingerprints are in `mcp/audit/workspace-path-promotion.json`.

The generic revision run later stalled in review despite the qualified
component and was cancelled, preserving journals. This is not called successful
autonomous development. The next controller work must use explicit durable
author/readback/host-review stages, rather than letting stale free-form
self-assessments determine whether those stages occurred.

## Filesystem boundary

Lexical checks alone do not confine files. The platform adapter walks directory
components through owned descriptors using O_DIRECTORY, O_CLOEXEC and
O_NOFOLLOW, then opens a single relative filename. It rejects symlinks, special
files and multiple-link regular files, including outside hard-link aliases.
New directories/files use private modes. Writes stage an exclusive random
temporary file, sync it and atomically rename one filename in the same directory.
The only added direct dependency is the already cached libc version in the lock;
no package/model download or paid provider was introduced.

These mechanisms are based on descriptor-relative
[open](https://man7.org/linux/man-pages/man2/open.2.html) and
[rename](https://man7.org/linux/man-pages/man2/rename.2.html) semantics. Tests
executed on this macOS host; the Linux code path has not been runtime-tested.
Other platforms fail closed. The small unsafe surface is confined to descriptor
ownership, directory enumeration and these platform calls, with safety comments.

A file replacement and SQLite audit are not one atomic distributed transaction.
The existing harness owns the durable outer intent. Admitted native write errors
are conservatively recorded as effect_uncertain, not a claim that no effect
occurred. An injected post-write audit failure proves the replacement can exist
while success is refused, and instructs the caller to inspect the path/hash
before retry. If the database itself is unavailable even an uncertainty record
may fail; the caller must retain its intent and error. No automatic mutating
recovery proposal is registered. This is focused source/threat review, not a
third-party penetration test, complete DLP, disk encryption or certification.

## Verification

Strict all-target Clippy, locked offline workspace tests and workspace build
pass. Eight independent workspace tests cover lexical/Unicode boundaries,
private modes and page hashes, operation and phase denial, wrong owner/run,
symlink/hard-link/FIFO/traversal rejection, size and sensitive-pattern limits,
terminal-job writes, live revocation, actual stdio escaping and audit-failure
uncertainty. Existing company and simulated-autonomy HTTP smoke checks pass.

Fresh actual local-model integration run
`a2d55965-9b9b-44c9-b98d-43fd08f703cf` passed in 50.8 seconds with nine native
Qwen3-Coder-30B calls and zero provider charges. It independently verified READY
bytes, native MCP write and readback receipts, and company completion. The
private report is `data/mcp-native-file-check/<run>/report.json`. This proves
that local-model-to-MCP file workflow, not arbitrary coding autonomy, perpetual
learning, all legacy-tool migration or the full user's goal.
