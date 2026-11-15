# Native reply outcomes and the author profile experiment

The unchanged `rust-minimum-settlement` trial completed and was independently admitted as PASS: attempt `9812d7b3-2cf2-4de6-b4a3-a64ad51c4f42`, run `6a57c953-6db2-49a5-bcff-5a81c5e89a2e`. The first written source failed acceptance; the actual changed 27B local-model source passed all eight frozen tests. There were no operator answers, post-planning questions, resumes, missing reply receipts or unresolved calls. Two native model calls failed at their generation limits. Both cutoff receipts and their usage remain part of the evidence; this was not a clean first-attempt run.

Accepted source SHA-256 is `72860e886ae61b78d54dec9ce063fc4b611143b617c0dc625c9d8206a2f455a9`; the frozen suite SHA-256 is `22670cee8c52ed10f6e49682a7b582ec7da53584b602b584f174f77b8ed99f86`. There were two executed source writes and one admin-denied source write. The same recovery profile and native bundle from docs/88 were used, with actual launch host metadata retained. Observed elapsed time was 1,044,737 ms; measured input/output usage was 144,005 / 13,936 tokens and $0 provider/tool fees. Hardware, power and Codex development credits are excluded. Independent archive proof and the full fake-user/CEO/execution transcript are retained. No frontier implementation, source promotion, model download, weight training or fintech assurance follows from this result.

## Correcting reply-count semantics

The terminal run exposed a metrics bug: the field named `completed_local_replies` counted every retained reply record, including native generation-cutoff receipts. Usage on a cutoff receipt is measured usage, but the generation did not finish normally. Worker completion counts had the same problem.

The shared `nativeReplyOutcomes` projection now separates `local_reply_records`, normally completed `stop` generations, `length` cutoff receipts and unclassified outcomes. Unknown outcomes are explicit, not invented completions. Both coding and artifact-graph verifiers retain input/output usage for all measured records, including cutoffs. The graph verifier also reuses the same strict safe-integer/zero-fee usage reducer as source verification. Worker reply-record and completed-reply counters are distinct; failed cutoff events are not completed workers.

The corrected settlement proof contains **21 recorded replies, 19 normally completed generations, two cutoffs and zero unclassified outcomes**. All measured input/output totals remain unchanged. Its original terminal report and immutable curriculum proof were not rewritten; `settlement-reasoning-recovery-reply-outcome-reverification.json` and `settlement-reasoning-recovery-qualified-measurements.json` hold the new derivation. Newly admitted proofs retain the outcome split. The rendered curriculum labels older proof fields as historically reported records rather than retroactively claiming completed generations.

All 67 MCP-related Node tests passed, including known cutoff usage, unknown outcome and worker-completion cases. Fresh complete model-free verification also passed on the actual settlement, weighted scheduling and a native artifact-graph run; no extra model inference was used. The latter two retained unchanged token totals and all-normal reply outcomes. A five-source-fixture snapshot then rechecked every current source case through the complete verifier.

| Unchanged source fixture | Recorded replies | Normally completed | Cutoffs | Local input / output tokens |
| --- | ---: | ---: | ---: | ---: |
| Range union | 15 | 15 | 0 | 72,727 / 5,285 |
| Cent allocation | 13 | 13 | 0 | 62,308 / 5,385 |
| DAG batches | 12 | 12 | 0 | 54,134 / 5,421 |
| Weighted scheduling | 24 | 24 | 0 | 138,610 / 12,850 |
| Minimum settlement | 21 | 19 | 2 | 144,005 / 13,936 |

These are five descriptive fixture qualifications under one retained profile/native bundle, not new fixture versions, controlled model rankings, IQ scores, general task mastery or weight learning. The total is 85 reply records, 83 normal completions, two cutoffs, 471,784 input / 42,877 output tokens, zero operator answers/post-planning questions and $0 provider fees. `reasoning-recovery-source-capability-snapshot.json` contains full current proofs and original report hashes. All five accepted sources have exact local 27B worker write witnesses.

## Review custody and next experiment

The settlement review package is `data/owned-rust-components/12212c0f7d2fa0247840b2bbaed1e61381b7419f29d866e77f2e18f3edaf31e0/manifest.json`. It contains the unchanged local source, frozen suite and corrected independent proof. Execution/promotion remain false. Package identity covers qualification metadata as well as source/tests; new proof metadata can produce a new package version without mutating an older snapshot. This is still review custody; a governed execution adapter for callable reuse remains unfinished.

Across this series the coder-first author path repeatedly recovered through the installed 27B reasoner, which emitted every accepted source. A separate `config/company.mcp-development-reasoning-author.json` now assigns that same bounded model to the worker slots from the start. Its SHA-256 is `7de6da2abfdb74af14894cae138bc51e48595962e5925397b2da3601fe071e46`. The existing CEO planning candidate policy, coder admin/QA, task criteria, actor roster, permissions and native bundle remain the same. Worker reasoning/final budgets are 2,048 / 2,048 with 16k context, matching the already-tested recovery slots. It is an explicit owner-profile experiment, not automatic learned routing or evidence that the new profile is better.

The unchanged weighted-scheduling comparison is attempt `fafb9911-5be2-4528-a852-23edd25b532f`, planning session `bc3cd788-cfe4-4932-8b14-6a0ab39ac1c8`, owned harness PID `55656`, exec session `76471`, tracked in `weighted-slots-reasoning-author-retest.log`. Keep that exact session/run/lease until authoritative terminal state. No prior source, review package, expected output or frontier task solution is supplied to its actors. Full qualification and measured costs, including failures, must precede a comparison or public profile change. A single paired fixture trial would still be descriptive rather than a controlled model ranking.

The public office retains the previously verified recovery profile, original run store and native bundle. Daily offline scanning remains `NOT_DUE` with latest `REVIEW_REQUIRED`. Broad callable reuse, model acquisition/training automation, additional hardware isolation, multimodal semantics and real regulated-industry assurance remain incomplete. The full goal stays active.
