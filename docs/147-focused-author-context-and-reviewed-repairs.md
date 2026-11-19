# Focused author context and reviewed repairs

Source-only jobs now have an opt-in focused context. In one measured pair it reduced reported input tokens by 61.657%, while both candidates still failed. A subsequent installed reasoning model repaired the same Rust function. The earlier Python observer-test repair also now passes its complete frozen review after explicit host diagnostic teaching; those reviewed tests are included in pending repository changes. The full company goal remains active.

## Keep the task and authority, remove unrelated context

An owner may set `candidate_workflow.prompt_profile` to `focused`. The default remains `full`, including for older approved intakes. The enum is part of the immutable intake/approval digest. Model output cannot select or change it.

The focused source protocol keeps the worker identity, exact source task, output contract, source-size limit, verified prior source/diagnostics and runtime permission epoch. Assigned procedure identifiers/hashes remain; full procedure bodies, global guidance, unrelated company history and tool descriptions are omitted from this source-only call. The host still controls write, full readback, submission, budgets and current permissions. This adds no execution, promotion or tool authority.

The trusted system prefix selects the context adapter and the same strict source/summary output schema. Task text and uploaded documents cannot select that prefix. Required source/task data is preserved or preparation fails; the adapter does not truncate a mission to fit. The focused envelope has a 32,768-byte limit, followed by the existing actual-tokenizer admission and output reservation. Normal planner/worker protocols retain their previous profiles.

## Same rejected source, different context

Two one-call Qwen3-Coder-30B repairs received the same task, rejected source, diagnostic, generation allowance, grants and frozen Unicode acceptance. Their actual task-input fingerprints matched. The focused profile reduced input, but both calls returned the same rejected source:

| Profile | Reported input / output tokens | Frozen result |
| --- | ---: | --- |
| Full | 2,848 / 240 | Failed; source unchanged |
| Focused | 1,092 / 252 | Failed; source unchanged |

This is an input-size observation on one pair, not proof of better accepted-task efficiency. Timing can also reflect order, caching and sampling. The feature stays opt-in.

The next bounded trial used the already installed Qwen3.8-27B model with 512 reasoning tokens and 2,048 final tokens reserved. It received the same logical task/reference/feedback fingerprint and produced different source that passed the unchanged finite Rust acceptance. It reported 1,139 input and 763 output tokens, taking 54.679 seconds through submission and host evaluation. Reasoning was observed separately, but the backend did not separately report its token count. The model and generation profile changed; this is not a controlled model ranking.

The [Rust candidate](evidence/focused-rust-candidate-2026-11-15.rs) remains a review artifact. It was not installed into production source or promoted as a general library. The acceptance preserves the original reference function, Unicode/offset/limit cases and overflow checks described in [145](145-independent-experiment-results.md).

## Repair the original Python tests

The same focused protocol was then applied to the previously rejected three-method observer-test task, using its exact original task text and Qwen3.8-27B/512-reasoning profile. The original three other methods, observer implementation, six-method positive contract and seven-regression quality suite stayed frozen.

The first focused draft passed the positive run but missed two regressions: blocked terminal status and unanswered attention. Static review also found explicitly requested cases absent. It was rejected. The host supplied those observed failures and missing-case diagnostics before a fresh one-call repair; the host did not supply replacement methods or edit generated logic.

That repair detected all seven regressions, but still omitted the exact punctuation-only valid basenames `-` and `_`. Passing the mutation suite therefore did not complete the task. A final one-call repair received that precise remaining diagnostic. Its only source change replaced `a-b` and `a_b` with the two required literal cases. The complete positive, mutation and named-case audit then passed.

| Observer attempt | Reported input / output tokens | Outcome |
| --- | ---: | --- |
| Focused draft | 1,309 / 1,712 | Positive pass; two escaping regressions |
| Diagnostic repair | 2,818 / 1,932 | Positive/mutation pass; literal cases still missing |
| Final case repair | 3,030 / 1,880 | Complete frozen review passed |

These three calls took 341.278 seconds in aggregate through source submission, excluding later host review. They used explicit pre-dispatch diagnostic mentoring. They do not establish independent autonomous discovery, successful evaluation design or a new training result.

## Apply reviewed tests, preserve the independent gate

The [final three-method patch](evidence/focused-observer-test-patch-2026-11-15.py) was mechanically assembled with the original class. AST comparisons prove the exact generated method bodies were retained, the other three methods were unchanged, and no outside-class logic changed. Static review checked the requested terminal states, both attention paths, cost cases, valid names and symlink behavior. There are no skips, dynamic execution, core replacements or fabricated assertions in those methods.

The reviewed class is now [company_tests.py](../scripts/fixtures/showcase-observer/company_tests.py). The original frozen quality suite is retained as [company_quality.py](../scripts/fixtures/showcase-observer/company_quality.py), changing only its import binding to that class module. Its assertions and seven mutation cases are unchanged. The existing independent sixteen-check fixture remains unchanged too. The observer runner now executes 24 top-level checks, including the company tests and quality gate. Both new fixtures require owner review for future source repairs.

Python snapshot diagnostics are still distinct from native Rust acceptance. Here, the unchanged protected host suite, exact source assembly and manual static/named-case review support this particular adoption. No general arbitrary-Python acceptance guarantee is claimed. The actual observer implementation is unchanged.

All six local calls in this pass reported $0 provider fees, with 12,236 input and 6,779 output tokens in aggregate. Twelve native source write/readback receipts, intake digests, policy epochs, one reservation per call and actual post-compaction task fingerprints were reconciled. [Measurements, source hashes and limits](evidence/local-focused-context-2026-11-15.json) retain the failed cases alongside the scoped successes. Publishing-assistant usage, hardware and electricity are outside those fee figures.

Verification passed: 573 Rust workspace tests with 27 opt-in tests ignored, 122 related Python checks, strict offline Clippy/build, all three experiment HTTP fixtures and ten-page website checks. The native MCP inventory remains sixteen tools. These checks establish the stated scope; they do not complete the broader continual-learning objective.

## Use and remaining scope

[The focused example](../config/experiment-loop.focused-example.json) keeps the baseline in full context and explicitly selects focused context for its predeclared repairs. After preparing the existing Rust example's input/acceptance files and reviewing the exact company/actor, keep the configured local harness running and use:

```sh
python3 scripts/experiment_loop.py --id local-rust-focused-v1 freeze --plan config/experiment-loop.focused-example.json
python3 scripts/experiment_loop.py --id local-rust-focused-v1 watch
```

The final command stays running; root evidence must actually change before its first trial. Continuation, stop and reconciliation rules remain those in [146](146-conditional-experiment-continuation.md).

This adds scoped source-context control and a reviewed test repair. Broad coding reliability, independently designed evaluations, a general research agenda, blind held-out comparisons, qualified model/procedure promotion and rollback, weight/media gains and a 24/7 soak remain unfinished. Changes are local and pending; no deployment, weight update or model download occurred, and no runtime was left running.
