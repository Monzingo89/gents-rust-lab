# Approved source bytes and three-call local workflows

`table_read_json` now accepts optional `expected_sha256`, a 64-character lowercase hexadecimal SHA-256 of the complete source file. When supplied in an immutable planning attachment, the exact hash is compiled into the source binding and covered by the approved job digest. It is not computed or supplied by the CEO. The native tool hashes the bytes it actually read and rejects a mismatch before decoding rows, issuing a successful canonical receipt, or capturing a bound source value. Formatting-only changes also fail: this contract approves bytes, not just equivalent parsed data.

Old path-only contracts remain valid and explicitly continue to read current bytes. Adding a hash to an existing immutable attachment requires a new reviewed contract; the host does not silently rewrite an approved job. A hash pins an observation at execution time, not a continuous file lock. It does not encrypt the source, prevent same-user filesystem changes, or certify that the chosen source is semantically correct.

## Bounded complete reads

The workspace interface adds `read_complete(path, limit)`. The Unix descriptor-relative adapter shares the existing file checks and page serializer through `read_bounded`, but uses the complete-read ceiling in its metadata check and bounded `take(limit + 1)` read. The table importer uses 8192 bytes. It rejects oversized files before reading their bodies, and rejects growth or a size change during reading. Existing paged `files:read` retains its 16 MiB whole-file safety ceiling and bounded returned pages. There is no duplicated path-opening implementation or new dependency.

Security tests cover same-size changed values, equivalent JSON with different whitespace, invalid digest syntax, no successful native receipt after a mismatched source, no completed artifact binding from that failure, restoration of the exact approved bytes, and a sparse oversized file rejected by the metadata ceiling before decoding. The actual stdio import test now supplies the correct pinned hash. Existing workspace traversal, symlink, hard-link, special-file, scope and privacy tests remain applicable.

## Harder actual company task

```sh
cargo build --workspace --offline --locked
node scripts/mcp-team-check.mjs --normal-only --file-table --chain
```

The explicit fake user asks the local CEO to select two distinct workers and propose all native contracts. The source attachments contain a hash-pinned file read and two schema-valid empty rows templates. No computed aggregate answers are supplied to the CEO. The analyst performs three dependent native calls within one staffing stage:

1. Read the complete pinned `inventory.json` through `table_read_json`.
2. Feed its actual `/rows` to `table_aggregate`: boolean-true accepted movements, grouped by warehouse, summing signed units.
3. Feed the actual warehouse `/groups` to another `table_aggregate`: group by the summary `rows` field and sum `total`.

The separate writer creates and reads back six exact no-newline files: `input_rows.json`, `warehouses.json`, `by_count.json`, `total.txt`, `warehouse_count.txt`, and `excluded.txt`. Both dependencies use approved native `argument_sources`. Neither model-reconstructed arrays nor manually provided outputs can satisfy those bindings. The fixture verifies all exact arguments, actors, source hash/length/path, unchanged source bytes, actual dependency values, per-file writes/readbacks, handoff order, final native evidence and provider usage. All execution stays on loopback with installed local weights and SSE MCP; stdio uses the same tools and authority.

The runner now supports file import and chained aggregation together. Report counters separately identify the primary source tool and parser calls while totaling all analyst native calls. The original operator file is recorded as a synthetic fixture input, never as an agent-created deliverable. General source uploads, CSV/document formats, private intermediates without deliverable contracts, arbitrary task competence and autonomous reviewed capability promotion remain unfinished parts of the broader goal.

## Verified outcome

Actual run `f4afef06-41f5-4646-b021-911f843bbaa1` passed; report and complete public transcript are in `data/mcp-team-check/8103b7dd-91ea-4c0a-9d8f-5d4bd22d04a7/`. The pinned hash survives the planning attachment, approved source binding and actual parser arguments. There is exactly one parser call and two aggregate calls; each of six artifacts was written and read exactly once. Input bytes remained unchanged. There were no admin denials, no execution attention questions, no rejected planning drafts and no planning attempts missing execution provenance. Two planning replies consisted of the CEO's initial clarification and ready proposal.

The independent expected outputs are total `25`, warehouse count `3`, excluded input count `3`, original warehouse groups east/-3, north/28, south/0, and second aggregation groups with numeric keys `1` and `2` and totals `-3` and `28`. Native receipts and exact bytes, not these prose descriptions, establish the result. The report includes the actual serialized outputs.

Measured usage is 22 completed local replies, 90948 input tokens, 2983 output tokens, zero unfinished calls and zero worker-model calls. Paid-provider charges are $0; local inference is not token-free. No source code or model weights were autonomously promoted.

Verification: `scripts/check.sh` passed offline, including 257 Rust tests (17 opt-in ignored), strict Clippy, simulated-provider HTTP lifecycle/security smokes, Python/Node tests and Vite build. The pinned stdio test also passed after the final test-fixture adjustment. `data/mcp-basement-verification/pinned-input-summary.json` records supplemental source hashes and direct per-path/per-call checks. The overall company goal remains active.
