# Governed file renames and binary fingerprints

The native `files` tool now supports renaming a regular file within its existing run workspace. This addresses the rename operation requested by the company goal. It does not grant access to other runs or the repository root, authorize source publication, or qualify arbitrary generated code. Existing pending work remains pending.

## Agent contract

A worker needs `files:rename` and `files:read` in its current owner-controlled grant, or an intentionally broad `files` grant. `files:write` alone does not authorize rename. The job must be approved and active. Planner catalogs omit rename, and execution rejects planner mutations. Read authority is needed because the source hash is a precondition; it does not imply permission to move a frozen input.

First obtain the full file hash without decoding or returning its bytes:

```json
{"op":"read","path":"draft.wav","metadata_only":true}
```

This is a complete bounded fingerprint, not a page: omit `offset_bytes` and `max_bytes`. The result includes `path`, `bytes`, `total_bytes`, `sha256` and `metadata_only:true`, with no `text`. It works for text and opaque binary files, including empty files. It does not count as full text readback for source acceptance. Ordinary UTF-8 page reads keep their existing behavior.

Use the actual returned hash in the rename request; the placeholder below is not an executable hash:

```json
{"op":"rename","path":"draft.wav","destination":"final.wav","expected_sha256":"<actual-source-sha256>"}
```

The source must be a single-link regular file of at most 16 MiB. Both parents must already exist. The destination must be absent, including case/normalization aliases, directories and dangling symlinks. Successful rename preserves the file's bytes, inode and mode. The result reports the source path, destination `path`, byte count, hash, `atomic_rename:true` and `replaced_existing:false`. It is accompanied by the normal actor/phase/policy/argument/result hash receipt. Directory moves, symlink/hard-link sources, hidden/traversal paths, oversized files and existing destinations are rejected. Both source and destination are checked against frozen owner inputs and aliases. The legacy Python file adapter rejects rename and metadata-only reads explicitly.

## Native boundary and uncertainty

The workspace adapter opens directory components relative to confined descriptors without following symlinks. It fingerprints a retained regular-file descriptor, checks the source identity again, and uses an exclusive filesystem rename. Destination nonexistence is also checked before the call, but the exclusive flag remains the atomic protection against a competing creator. After the call, it syncs the parents and verifies the destination inode, mode and bytes. Cooperative MCP mutations are serialized by the existing immediate policy transaction; privileged noncooperating host changes are outside that lock and are not claimed to have compare-and-swap semantics.

macOS uses `renameatx_np` with `RENAME_EXCL`; Apple documents [exclusive-renaming filesystem support](https://developer.apple.com/documentation/foundation/urlresourcevalues/volumesupportsexclusiverenaming). Linux uses the `renameat2` syscall with `RENAME_NOREPLACE`, whose [manual](https://man7.org/linux/man-pages/man2/rename.2.html) documents the no-overwrite flag and filesystem support requirements. Unsupported kernels/filesystems fail closed. There is no copy/delete or overwrite fallback. The implementation was executed on the present macOS host; Linux execution is not claimed by this record.

Filesystem effects and SQLite audit persistence are separate. An error or lost response must not be treated as proof that the source stayed put. The MCP error and result guidance require inspection of **both** paths, their hashes and the canonical audit. In the harness, an unconfirmed dispatched rename keeps its original write-ahead intent and full journal, clears future unexecuted steps, and stops with `needs_reconciliation`. It does not schedule recovery retries, claim completion or automatically resume that move on restart. The run remains available for authorized metadata reads; its terminal state does not renew authority for another mutation. Reconciliation and any new task remain separate from the failed original receipt.

## Verification scope

The native tests exercise text, opaque binary and empty files; unchanged inode/mode/hash; stale hashes and missing parents; worker/read/operation grants; stale policy epochs and terminal jobs; protected inputs and case aliases; symlink/hard-link/FIFO/directory/size/path denial; competing MCP writers and external destination creators; metadata-only reads without text; and actual stdio/SSE transport calls. A lost-audit test confirms that the move can have happened despite a failed receipt and checks the resulting path/hash without replay.

`scripts/rename_reconciliation_smoke.py` uses the actual HTTP harness, actual native binary and actual file/audit operations in a disposable workspace. Only the initial decision queue and model judgments are simulated. It injects receipt failure after an actual binary rename, then verifies the retained pending intent, terminal company state, no subsequent tool retry, unchanged files on restart and rejection of ordinary resume. The fixture initially omitted its native executable and stopped before performing the move; this setup error was corrected, and the final fixture requires the actual move and uncertainty audit. It is infrastructure evidence, not model competence.

The budget lifecycle fixture was also corrected to copy the native executable. Its post-tool budget case now requires a native rejection audit rather than allowing a missing-binary launch error to stand in for the intended file-read failure. Earlier budget results still establish the stop behavior; the rerun strengthens the native-path coverage.

No model inference, download or paid provider is needed for these file operations or lifecycle fixtures. Hashing, filesystem I/O and local test compute still consume machine resources. No production deployment, company-wide coding reliability or completion of the broader goal is established here.

Final recorded checks: 565 Rust workspace tests passed, with 27 opt-in tests ignored; all nine native rename/fingerprint tests passed; strict offline Clippy and build passed. The HTTP uncertainty/crash fixture, corrected native-path budget fixture, 32 broker checks and ten-page site checks passed. The cookbook was regenerated from the built 16-tool implementation catalog. [Source, binary and verification hashes](evidence/local-file-renames-2026-11-15.json) are recorded separately from older historical experiments. Website changes are a local source/build update, not a deployed release.
