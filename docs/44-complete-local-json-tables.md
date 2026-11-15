# Complete local JSON tables and next-action review

The company can now derive an aggregate from an actual operator-provided run-workspace JSON file, rather than requiring the operator to paste typed records into the planning attachment. This is a bounded new capability, not arbitrary local task competence.

## Native capability

`table_read_json({"path":"inventory.json"})` lives entirely in the independent Rust MCP crate (`mcp/src/functions/table_read.rs`). It uses the existing descriptor-relative workspace adapter, current run identity, tool-specific grant, policy transaction, result-schema validation, hashed canonical audit and native binding capture. It grants neither general `files:read` nor write access. The catalog and B1 basement discover it from the registry; both stdio and loopback SSE use the same implementation.

The result contains `rows`, `source_path`, `source_sha256` and `source_bytes`. The hash covers the complete original UTF-8 bytes, including formatting and trailing whitespace. The row contract is the shared `table_aggregate` input contract: at most 64 flat records, 16 scalar fields per record, ASCII field names of 1..40 letters/digits/underscores, strings at most 128 characters and integers within +/-1e12. Row order, duplicate records, nulls and boolean/string distinctions are preserved. Empty tables are valid.

The adapter requires the entire file to fit within 8192 bytes. It rejects a partial read even when the first page alone contains valid JSON. It accepts only one JSON array of records, rejects trailing documents, duplicate decoded field names (including escaped aliases), floats, nested values and invalid fields, and checks the decoded rows for the existing sensitive-data patterns before schema diagnostics or result retention. JSON content is never evaluated as code. Paths, symlinks, hard links and special files remain subject to the existing workspace boundary.

No new dependencies, executable plugins, subprocess tools, network access or paid providers were added. The existing serde JSON decoder is used with a unique-key row visitor. Raw source-dependent decoder messages are replaced with bounded structural errors.

## CEO plan and task

Run the real local experiment with:

```sh
cargo build --workspace --locked --offline
node scripts/mcp-team-check.mjs --normal-only --file-table
```

The experiment starts its own loopback listener on 8796 with local inference on 8797 and refuses an occupied listener. The fake user is explicitly scripted. The CEO chooses two distinct workers and proposes the native contracts. Immutable planning inputs contain the source path and an empty aggregate rows slot, not the computed answers. The script reviews the compiled proposal before approving its exact digest and installing the frozen policy. The source fixture is explicitly operator-seeded, not attributed to an agent.

The analyst receives only `table_read_json` and `table_aggregate`. Its second native call obtains `rows` from the first call's actual `/rows` projection through receipt-bound `argument_sources`. A separate writer receives only the approved file operations and must produce/read back five artifacts: `input_rows.json`, `groups.json`, `total.txt`, `matched.txt` and `excluded.txt`. The independent verifier checks source hash/length/path and unchanged input bytes, actual parser-to-aggregate data flow, native actors, exact output bytes, one write per path, no source replay, handoff order and final canonical admin evidence.

## Actual failure and architectural repair

The first file-table experiment (run `fc8175de-7e21-4152-8c8f-3009b161e1d9`, report directory `data/mcp-team-check/3ae27c41-5eed-4f42-8199-234f03178192`) completed the parser and aggregate exactly once. The admin then repeatedly denied the first approved write because the five artifacts had not yet been written and read back. This was a circular review failure, not missing native source data. After twelve recovery attempts the run requested attention. No file deliverables were manually supplied.

The repair introduces `ceo_team::next_action_evidence`. Intermediate admin requests receive successful prior native receipts and the selected artifact's exact source/hash observations, with an explicit distinction between authorizing the next action and accepting completion. Future write/readback completion failures are not presented as prerequisites to executing those operations. `finish` still receives every full native/artifact check, and all controller, native authority and independent QA gates remain unchanged. The repair does not normalize a deny verdict into allow or grant new capabilities.

The owned listener was gracefully restarted at the saved accepted-analyst checkpoint (`stage=1`, `step=3`, `pending=null`) to load this tested repair. `maintenance.json` records the reason; this was not a timeout-triggered restart. The same run UUID was resumed with an explicitly recorded scripted maintenance answer. The original FAIL report remains unchanged; separate same-run recovery evidence records PASS, five correct artifacts, no repeated source calls and no repeated artifact writes. Recovery measured 33 completed local replies, 134907 input and 4739 output tokens, two unfinished calls without reported usage, zero worker-model calls and $0 provider charges. This cost includes the failed review loop; it is not a zero-token claim.

## Verification and limits

`mcp/tests/table_read_json.rs` exercises exact file hashes, real native input composition, changed-source invalidation after re-observation, duplicate/escaped duplicate fields, input boundaries, invalid types, raw and escaped sensitive fixtures, scope denial, traversal/symlink/hard-link rejection, projection types and actual stdio execution. The controller regression verifies that intermediate packets do not demand their own results while finish retains all missing checks.

The fixture verifier is deliberately independent and task-specific. General task correctness, continuous source-file monitoring, approval of file contents by hash, arbitrary JSON/CSV/document formats, large-file streaming and private intermediate projections remain future work. The new capability reads the current confined file at execution time; an approved path alone does not freeze its bytes. The shared workspace reader may internally read up to its existing 16 MiB safety ceiling before this adapter rejects files over 8192 bytes. Existing sensitive-pattern checks are not complete DLP, and local SQLite is not encryption. No autonomous code promotion or model-weight learning is claimed.

## Fresh run after the repair

A new task run `d32c8c4f-7b02-4e72-9c0e-567cc984f14c` passed in `data/mcp-team-check/b01a994c-d873-4365-9f57-0afca7952328/`. Its self-contained `transcript.html` shows the scripted fake user, actual local CEO messages, reviews, tool calls and QA. Three planning attempts included one rejected writer-ownership draft; host validation rejected it and the CEO corrected it before approval. Execution had no admin denials and no attention questions. Exactly one parser and one aggregate ran, the original 709-byte input stayed unchanged, and all five outputs passed native receipt/readback and final independent checks. Outputs include total `25`, matched `5`, excluded `3`, and the exact three warehouse groups. These expected values came from the verifier, not the planning prompt.

Measured cost: 20 completed local replies, 78346 input and 3369 output tokens, zero unfinished calls and zero worker-model calls; $0 paid-provider charges. One rejected planning attempt retained measured usage and a rejected-output hash but lacked native execution metadata in the historical audit. That omission is preserved as unknown; it was not backfilled. A forward observability fix now retains local execution metadata, model and latency even when a completed local reply is rejected by validation, without storing its raw rejected text. The read API and regression test cover new failed-attempt metadata and legacy missing metadata. This does not strengthen old evidence retroactively.

The original report's `analyst_native_calls` counted only the primary aggregate call; `native_parser_calls` separately recorded the import. A supplemental independent `data/mcp-basement-verification/file-table-summary.json` records the actual two-call total and source hashes; the future runner counter now totals both. Original reports remain unchanged.

Final verification: 255 Rust tests pass (17 opt-in tests remain ignored), strict Clippy, the complete `scripts/check.sh` smoke/Python/Node suite and Vite build pass. The last planning-audit adjustment was rechecked with the Rust suite and strict Clippy. The refreshed live office confirms local-only SSE and eight catalog functions; `data/mcp-basement-verification/file-table-basement.png` shows the new function with its source and result-schema disclosure. The broader company goal remains active.

The subsequent [approved source-byte milestone](45-approved-source-byte-contracts.md) adds optional hash-pinned imports and rejects the smaller complete-file ceiling before reading bodies; the earlier resource-limit and path-only observations above describe this milestone's original implementation and evidence.
