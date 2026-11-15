# Qualified component review packages

The local company repaired and completed the unchanged `rust-weighted-slots` fixture: attempt `d8d8e89e-7eb3-4b09-af53-e23b1f8e0374`, run `dd5f7883-10ff-4638-a662-4abc22d7a10a`. Final QA, independent curriculum admission and completed archive integrity all passed. All eight frozen acceptance tests passed for source SHA-256 `8a3ec5c2ca9ed65da60be122e0a1f740f2b739e3f98f97dbe8a4a4b7bc96e8a7`, suite SHA-256 `5986e39064b4d0a922b34bb963a877f1e84b111a574da9ff48a5db32ae755f51`.

This required actual repair: two denied source writes, two executed candidate-compilation failures for the first written version, then a genuinely changed local-model write/readback and native acceptance PASS. The exact accepted write appears in a 27B local-reasoner worker reply. There were 24 local replies, 138,610 input tokens, 12,850 output tokens and 1,003,193 ms; zero failed/unresolved model calls, cutoffs, resumes, operator answers or post-planning questions; $0 provider/tool fees. Those fees exclude hardware, power and Codex development credits. Twenty completed cold records across three actors passed independent archive count/digest/contract/epoch checks, with zero hot records. No frontier implementation, task answer, source promotion or weight training was supplied.

The profile remains SHA-256 `23b4f92d345691164eebb68202c55732f07c3d228234367542187a063ae04e28`; native bundle remains `b229dfe416f9ddc5ed6947455d9e0864c09412c2061d700c8b3e3851d0387fd5`. Its reviewer-assigned hard synthetic health-operations label does not establish clinical, privacy, regulatory or general scheduling assurance. Evidence uses the `weighted-slots-reasoning-recovery-` prefix under `data/mcp-basement-verification/`, with the full UUID transcript retained. Its original launch predates portable host observations, so that historical field remains unknown.

## Export and inspect

```sh
node scripts/mcp-qualified-component.mjs \
  --report data/mcp-workflow-check/d8d8e89e-7eb3-4b09-af53-e23b1f8e0374

node scripts/mcp-qualified-component.mjs \
  --verify f995cd27fa8782a9ee59fb84286e997ddfb01f39fd3a4284d0d40f769fdcf14c
```

The first command independently rechecks the real source workflow before exporting a review artifact. The second checks stored package bytes only. Neither calls a model, executes candidate code, installs a tool, starts a benchmark, changes governance or grants promotion. The CLI rejects activation flags; there is no activation operation.

The actual weighted-scheduling package is `data/owned-rust-components/f995cd27fa8782a9ee59fb84286e997ddfb01f39fd3a4284d0d40f769fdcf14c/`. It contains exactly:

- `source.rs`: the accepted local-model source, copied without edits;
- `acceptance.rs`: the unchanged owner-frozen test bytes;
- `qualification.json`: newly derived source-bound proof, original evidence-file hashes, completed archive metadata and original launch host observations when available;
- `manifest.json`: content-addressed file hashes and the parent company/run/actor, frozen contract, policy epoch, fixture, source/tests, profiles and native bundle identities.

`scripts/mcp-component-store.mjs` owns bounded reads and sealed data publication. `scripts/mcp-qualified-component.mjs` owns qualification/export orchestration and reuses the existing coding verifier, native bundle probe and read-only archive verifier. No task algorithm or reference implementation was added to either adapter. A repeated real export produced the same component ID. A real failed source report was rejected before publishing another package. The original task reports and immutable curriculum admissions were not rewritten.

## Boundaries

Every package is `review_ready_fixture_only`, with `execution_authorized=false` and `promotion_authorized=false`. It is a snapshot of limited fixture evidence, not ongoing certification or tool authority. No server or model currently consumes it as an executable registry. The native catalog remains at 12 functions. A separate execution adapter and fresh owner-scoped approval are still required for callable reuse; package metadata or model claims cannot substitute for them. This is the review/custody stage toward reuse, not a completed autonomous promotion feature.

Evidence reads are bounded and checked through file descriptors before/after reading. Paths reject traversal, URL escaping, symlink roots/parents/leaves, hard-linked files, unsafe ownership and group/other write access. Publication uses private temporary directories, exclusive non-following file creation, file sync, read-only files, a read-only directory, rename and verified readback. Existing packages are verified rather than overwritten or silently resealed. Corruption, extra entries and a recomputed manifest attempting to grant execution are rejected. These checks require a POSIX ownership adapter; Windows ACL support remains missing. They do not protect against a privileged process with this same UID, supply encryption, prove dependency/compiler safety or grant industry compliance.

## Directory-symlink regression

The path checks exposed the same trailing-slash pattern in the existing native runtime bundle builder. On this host, `lstat` of a directory path ending in `/` followed a final directory symlink, allowing an alias to appear ordinary. A disposable fake-binary regression reproduced the missing rejection without executing any fixture binary. `runtime-bundle-directory-symlink-red.log` records that pre-fix failure.

The builder now checks the directory path without the trailing slash for both source directories and cached bundle directories. Negative tests cover a `target/debug` alias and a cached directory alias to matching sealed bytes. All 66 MCP-related Node tests initially passed after this change; all 67 passed after the reply-outcome correction in docs/89. These include six new component-store/export tests. Actual ordinary-path bundle creation/probing still returned the unchanged bundle ID. No native executable or running trial was replaced. Evidence is `qualified-component-all-mcp-node-check.log` and `qualified-component-runtime-validation.json`; package export/integrity results use the `weighted-slots-component-` prefix. The previous full Rust/HTTP/Python/build validation remains the evidence for unchanged native code; it was not rerun for these Node-only changes.

## Continuing local evaluation

The same-profile `rust-minimum-settlement` retest is attempt `9812d7b3-2cf2-4de6-b4a3-a64ad51c4f42`, run `6a57c953-6db2-49a5-bcff-5a81c5e89a2e`, owned harness PID `40438`, exec session `60384`. Its reviewer-assigned extremely-hard label remains synthetic scope. Preserve that exact live UUID/lease until terminal; no elapsed observation time permits replay. Revised source reached native acceptance PASS after an earlier acceptance failure; a later worker generation cutoff is retained. Final QA and independent admission are still required before claiming task qualification. No operator task answer or frontier implementation has been supplied.

That trial subsequently completed and was independently admitted. Its terminal proof contains **two** cutoff failures with complete retained usage; the earlier live observation above was not the full terminal count. The exact qualified source and archive, corrected reply outcomes, and next role-profile experiment are documented in `89-native-reply-outcomes-and-author-profile.md`. Both the original report and immutable admission remain unchanged.

Its report now includes actual observer launch metadata from the portable adapter: darwin/arm64, Apple M4 Max, 14 logical CPUs and 38,654,705,664 OS-reported memory bytes. Those are neither a GPU allowance nor inference attestation. The public office continues with its existing run store and sealed native bundle. Broader reuse execution, model acquisition/training automation, Windows isolation, multimodal semantics and real regulated deployments remain incomplete; the full goal remains active.
