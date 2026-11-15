# Native Rust submission control

The author-first weighted-scheduling experiment completed and was independently admitted as PASS, with all eight frozen tests unchanged: attempt `fafb9911-5be2-4528-a852-23edd25b532f`, run `d40d710d-9ad2-4e80-a90e-a10414e8175c`. Its final source was emitted by the local 27B worker. Twenty-eight completed cold records across three actors passed archive integrity, with zero hot records. There were no operator task answers, post-planning questions, missing/unresolved calls, cutoffs, resumes or frontier task implementation.

The experiment did not improve observed efficiency:

| Same frozen weighted-scheduling fixture | Coder-first recovery profile | Author-first experiment |
| --- | ---: | ---: |
| Completed local replies | 24 | 32 |
| Input tokens | 138,610 | 182,587 |
| Output tokens | 12,850 | 22,669 |
| Observed elapsed ms | 1,003,193 | 1,757,082 |
| Planning attempts | 4 | 4 |
| Worker model calls started | 8 | 9 |
| Executed source versions | 2 | 4 |
| Admin-denied source writes | 2 | 4 |
| Native verification calls | 3 | 4 |
| Provider/tool fees | $0 | $0 |

These were descriptive paired observations, not a controlled causal model ranking. Both ran on the old retained native bundle `b229dfe416f9ddc5ed6947455d9e0864c09412c2061d700c8b3e3851d0387fd5`; the framework changes below were not applied to that issued trial. The public office keeps the previously verified recovery profile. Evidence is `weighted-slots-reasoning-author-qualified-measurements.json`, `weighted-slots-author-profile-comparison.json` and the full UUID transcript. Hardware, power and Codex development credits are excluded from provider fees.

## Compile a review request after native gates

The older settlement run needed three additional author-model calls after the first native PASS for its final source. The two weighted-scheduling runs each needed one. These are measured event-window observations in `rust-submission-baseline-observations.json`, not estimated token savings.

`crates/harness/src/rust_submission.rs` now compiles a bounded `finish` proposal from caller-validated owner intake and freshly computed canonical gates. It requires the correct CEO stage/actor, worker phase, approved contract identity, current policy epoch/grants, matching owner-frozen source/test paths and hashes, current canonical acceptance PASS and full readback. Every generic native and artifact gate must also pass. A source-only contract may rely on typed Rust checks without a redundant generic call list; those typed checks still prove actual native acceptance/readback. Missing, stale, mismatched or revoked inputs yield no proposal.

`native_submission::Submission` is the shared typed boundary for graph and Rust review requests. The autonomy controller selects one applicable compiler, preserving existing graph action compilation. A pending action/review is not overwritten. Single-author and ordered CEO Rust stages can request review after verification without a new author-model generation. The host emits `frozen_contract_submission` with origin `host_compiled_frozen_rust_submission`, exact source/test identities, `model_generated_summary=false`, `authority_granted=false` and `independent_qa_required=true`.

This is a proposal, not completion or authorization. Normal admin review, final native/currentness checks and independent QA still evaluate every frozen criterion. No code is authored or repaired, no operation executes from the compiler, no permission expands, no veto is overridden and no source is promoted. Other task requirements may still require model work or QA revision.

The durable submission fingerprint binds contract, actor/stage, policy epoch, exact source/tests, generic artifact facts and any native artifact identity. Repeated receipts or model prose cannot create another automatic request for unchanged work. A changed verified identity can be reviewed anew. The existing checkpoint map is reused, avoiding a second competing automatic-submission loop.

## Observe the completion path honestly

The model-free `sourceReviewPath` projection binds the public event window to the final source, frozen tests, actor and receipt epoch. It checks native argument/result digests and reports the first matching native PASS, later author call-start events and matching compiled submission events. Malformed/missing event windows remain unknown (`null`), rather than becoming false zero-inference claims. These are host-event observations, not permission, inference attestation, QA acceptance or source promotion.

Coding reports and new curriculum admissions retain this `source_review_path`. Older terminal reports and immutable admissions remain unchanged. Full model-free re-verification of all three baseline runs passed after adding the projection, preserving their original report hashes.

## Validation and issued retest

The complete pipeline passed: 417 default Rust tests, 20 opt-in exclusions, formatting, strict Clippy, build, native stdio/SSE and HTTP/Python/Node checks. Focused native tests exercise actual write/readback/failed acceptance, changed bytes and PASS, then prove that extra successful receipts do not change the submission identity. They use an unrelated protocol function, not a benchmark implementation; no model competence follows from those fixture tests. Tests also cover stale/unknown epochs, wrong actor/stage, frozen hash mismatch, missing readback, unverified checks and revoked grants.

The full log is `rust-submission-full-check.log`. The subsequent event-observer-only addition passed all 69 MCP-related Node tests, negative event-window cases and complete model-free baseline verification; see `rust-submission-node-check.log`. These additive bookkeeping changes do not modify native execution or criteria.

A fresh unchanged settlement retest is issued under the previously verified recovery profile (`23b4f92d345691164eebb68202c55732f07c3d228234367542187a063ae04e28`):

- attempt `1e6ab953-c4cc-46e7-b859-a3159e8c0fc1`;
- run `6b4cfc00-1ae2-4718-897c-028be1b6e20d`;
- native bundle `fe1108515b4cbd1fa727d9c0af4356b7b74e09e0cff0fe879be725c10de6977e`;
- manifest SHA-256 `96e23aeba5ce05ee758164bdd7ff19594dfa61eebfcbb577f10bc560981607f7`;
- owned harness PID `80258`, exec session `97625`, log `settlement-native-submission-retest.log`.

This was the issued state when this document was written. The retest subsequently failed unassisted qualification, was explicitly cancelled following a native no-progress diagnosis request, and closed its lease as fail. It never reached source acceptance or the finish compiler. The next unchanged range-union trial independently passed and observed one host-compiled finish with zero post-PASS author generations. See [the terminal evidence and component continuation](91-owned-component-abi-and-native-submission-evidence.md); the original failed report/history remain intact. No elapsed observation timeout permitted cancellation/replay, and no old source, expected output or frontier implementation was supplied.

Callable component reuse, broader model acquisition/training automation, additional hardware isolation, multimodal semantics and real industry assurance remain incomplete. Neither this proposal compiler nor finite synthetic fixture success establishes universal capability, absence of vulnerabilities or regulatory compliance. The full goal stays active.
