# Conditional trials from independent results

This records the initial continuation milestone and failed repair. [147](147-focused-author-context-and-reviewed-repairs.md) subsequently records focused context and later reviewed repairs; the earlier outcomes below remain unchanged.

The company can now continue a frozen source experiment from an independently checked result, without a human editing an input file between trials. The first real campaign exercised that path but did **not** repair the function. Its repeated source stopped the next model call. The full company objective remains active.

## Owner control and actual result signals

An experiment may declare `after: {"experiment":"baseline","outcome":"rejected"}` or an `accepted` condition. The predecessor must appear earlier in the frozen plan. Conditional trials preserve its exact source task, submission criteria and Rust acceptance hash. Actors, tools, output paths and call allowances remain fixed before generation. Models cannot create branches, renew allowances or change acceptance.

Completed results now have durable checksums. Before continuing, the controller verifies the predecessor's result checksum, current source/approval/policy, canonical write/readback receipts and pinned evaluator executables. A runtime failure without independent acceptance is not treated as an independently rejected implementation. A changed predecessor or evaluator stops continuation before another claim is spent.

The next job receives the verified source and a diagnostic excerpt of at most 1,200 UTF-8 bytes as `candidate_feedback`. This reference is frozen into its approved intake and retained with its own checksum before dispatch. A strict native type checks the shape, source hash, sizes, prior run and outcome/stage fields. The author sees it under `prior_trial_evidence`, explicitly as untrusted data that cannot change task, criteria or authority. It is not a receipt or permission grant. Sharing this selected reference with a predeclared recipient does not grant general access to the predecessor's workspace or memories.

An independent result becomes the next trial's evidence signal; no manual file change is needed. A condition mismatch or skipped predecessor produces a retained skip record without a fake run or reserved call. If a repair repeats a source hash already present in its ancestry, later continuation stops. This rule avoids another call when no new revision was produced; it is not a claim that every different revision improves correctness.

The existing uncertain-effect rules remain: an unconfirmed preparation, dispatch or evaluation is reconciled through its original claim and run ID. Restart does not replay mutations or renew budgets. New bookkeeping tables are created once when opening older ledgers; existing frozen plans and allowances are preserved. Unchanged root evidence still dispatches no inference.

## Real local campaign: failed repair, verified stop

The owner froze a three-call campaign for the same `bounded_chunk_end` contract and unchanged finite Unicode acceptance used in [145](145-independent-experiment-results.md). It allowed a baseline and two rejection-dependent repairs using the installed Qwen3-Coder-30B model. All source was authored by the local model and written/read back by native tools. There were no seeded source intents, host source edits or host messages between calls. The external input stayed unchanged after the initial signal.

| Observation | Recorded value |
| --- | --- |
| Campaign loop wall time | 25.445 seconds |
| Actual calls / frozen maximum | 2 / 3 |
| Reported input / output tokens | 5,104 / 521 |
| Native write/readback receipts | 4 |
| Provider fees | $0 |
| Independent acceptance | Both submissions failed |
| Third call | Skipped because source was unchanged |

Both candidates incorrectly return `None` for an empty string at offset zero with positive limit, where the contract requires `Some(0)`. The follow-up returned byte-for-byte identical source despite receiving the actual failure diagnostic. Its summary claimed correctness; the controller ignored that claim. The [rejected source](evidence/conditional-rust-rejected-2026-11-15.rs) is retained unchanged for review and was not adopted.

The host reconstructed both author inputs, including the follow-up's exact source and diagnostic payload, and matched them to the backend's recorded post-compaction task fingerprints. This establishes that feedback reached the actual input; it does not establish that the model used it effectively. Native receipts, intake digests, policy epochs, outcome/feedback checksums and one reservation per actual call were reconciled. [Evidence and limits](evidence/local-conditional-experiments-2026-11-15.json) retain the failed result separately from the successful continuation/stop behavior.

The automatic rejection-to-follow-up mechanism worked. Correctness did not improve. Saving the unused third call is an observed budget outcome, not evidence of better accepted-task efficiency or frontier capability.

## Verification and use

The HTTP continuation fixture uses actual native writes/readbacks and isolated Rust execution. Its seeded baseline fails, the predeclared follow-up passes, and the unused repair is skipped while the external input remains unchanged. Source intents in that fixture are simulated; the campaign above separately uses actual generation. Unit checks cover result/feedback corruption, missing or changed custody, condition mismatches, preserved criteria, repeated sources, skipped branches, multi-step feedback binding and lost follow-up responses. The native parser rejects feedback that tries to add tool fields or breaks source identity/size limits.

Verification passed: 569 Rust workspace tests with 27 opt-in tests ignored, 114 related Python checks, strict offline Clippy/build, all three experiment HTTP fixtures and ten-page website checks. The native MCP inventory remains sixteen tools.

[The repair example](../config/experiment-loop.repair-example.json) reuses the Rust example's task, test hash and input paths, with two predeclared rejection continuations and a three-call maximum. After preparing its baseline and acceptance files using [145's instructions](145-independent-experiment-results.md), review the exact actor/company and run:

```sh
python3 scripts/experiment_loop.py --id local-rust-repair-v1 freeze --plan config/experiment-loop.repair-example.json
python3 scripts/experiment_loop.py --id local-rust-repair-v1 watch
```

Keep the separately configured local harness running. The last command stays running until the finite plan is exhausted or reconciliation is needed. Its first trial still requires actual changed external evidence. Subsequent matching conditional trials use verified results directly. `status` with the same ID shows real runs, skipped trials and reserved calls. A skipped allowance was never claimed; spent claims are not refunded.

This is a bounded result-driven continuation mechanism. Independent suite design, a broader adaptive research agenda, blind held-out comparisons, qualified promotion/rollback, weight-training gains, other-language/media evaluation and a 24/7 soak remain unfinished. No candidate was adopted or deployed, and no runtime was left running.
