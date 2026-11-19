# Real observer repair and bounded Python followups

This extends the review-only Python workflow in [140](140-bounded-python-review-submission.md). The target is the retained company-authored observer, rather than another synthetic string function. None of the diagnostic results independently authorize source promotion, prove general correctness, or finish the broader company-learning goal.

## Native execution changes

An owner-frozen `python_check_contracts` entry can include `frozen_inputs`, a map from declared source path to SHA-256. Those dependencies must match before diagnostics execute and are protected from native writes and media publication, like the frozen suite. Mutable candidate files still require actual native write and complete current-source readback receipts for review submission. A contract containing only immutable sources cannot use automatic candidate submission.

An optional `scratch_bytes` integer, at most 8 MiB, enables separate temporary scratch storage for tests of real filesystem behavior. Source copies remain read-only; host files, networking and child processes remain denied. File mode changes are denied so a scratch hard link cannot make a source copy writable. Byte usage and a 256-entry limit are sampled during and after execution, with bounded descriptor-relative scanning that does not follow symlinks. This is **not an instantaneous disk quota**: transient, deleted and open files are limitations. Per-file output, CPU, elapsed time, resident memory and snapshot-integrity checks remain in force. With no scratch allowance, tests cannot write temporary files.

For explicitly approved `python_review_candidate` jobs, a confirmed native source write now queues the necessary readback pages and frozen diagnostic check. The queue never replays the write. It replaces future unexecuted worker steps so another candidate rewrite cannot bypass observing the result. Every queued tool still passes current policy and ordinary independent action review. Read pages use UTF-8 boundaries. Matching Python source reads now use current source/program identity, so an earlier read of an old version does not consume the readback allowance for a changed candidate. Generic shell/read replay rules remain separate.

## Retained observer and trial boundaries

The original assembled module SHA-256 is `23cd49a71143262cfc1a239b0d3f5a1559fa3e535c9c39055a22404f009c2ca9`. Its original 12 independent checks passed again before repair. A further runtime check found that `candidate_submitted` and `blocked` were incorrectly treated as active states. The repair contract requires only `running` and `queued` to continue polling; every other actual state must be preserved immediately, including when attention is unanswered or an observation deadline has expired. A terminal observation does not establish acceptance of the task it observes.

The first repair trial exhausted its 24 reserved model attempts and was cancelled. It did not establish a qualified candidate. Two further priority regressions were frozen before the fresh trial, producing a 16-method suite. The first trial and its post-generation rejection evidence remain separate from the new pre-generation suite. No paid API, download or training is involved. Local inference uses tokens and compute even when recorded provider fees are zero.

Trial outcomes, independent source assembly, the company's own test quality, and adoption into the task runner require separate evidence. Until that evidence is recorded, the existing observer and candidate drafts remain pending.

## Repaired function result

The fresh trial finished in **260.13 seconds**, with 21 recorded local-model replies and 21 reservations out of 24. Usage records report 145,184 input tokens, 5,276 output tokens and 6,585 cached input tokens; provider fees were $0. These are reported usage fields, not a measurement of electricity or all campaign work.

The worker initially wrote the original condition back three times. One live internal host brief explained that the predicate must recognize the complement of the active-state set. The worker then authored the changed function. This is a **host-coached repair**, not evidence of unassisted reliable autonomy. The same local engine served the worker, action reviewer and final QA; those role labels do not imply independent models.

The assembled module is SHA-256 `bacbf530a2507816fcefc93835d3f84b1bb1f01f581a540dc6ea77d83f050219`. Host assembly replaced only the exact function text; AST comparison verified unchanged function arguments and unchanged non-function code, with no host function-logic edits. All 16 frozen checks passed on the assembly, and both original six-test suites passed again in the offline sandbox. A separate host static review found the intended state predicate change and no added dynamic execution. The output directory remains a trusted owner-created directory; this helper is not a generic untrusted-path filesystem confinement boundary.

At this checkpoint, the runtime reported the review task completed. That runtime status is separate from these independent tests and does not imply source publication, general correctness, model training, or completion of the broader company goal. The repaired module and the company's test-repair trial remain local pending work.

## Own-test trial exposed context loss

The first attempt to repair the company's six existing observer tests was cancelled without a written repair. It repeated source reads and did not qualify any tests. The native read had returned a bounded source page, but the worker checkpoint encoded long tool results as a 3,500-character JSON string prefix. This silently removed part of the source page from the worker's evidence. Giving permission to read a file did not ensure the worker received its complete requested page.

The worker packet now preserves bounded successful native file pages as structured data, including exact text, offsets, continuation marker, digest and receipt. Other large payloads retain their bounded journal/excerpt path. The model-context compiler still fails closed when required evidence cannot fit; this change does not truncate the mission or increase the model context window. Source-bound rejections are also included as identity-specific feedback, separate from generic path-wide blocking: a rejection of one source version must not prevent checking repaired bytes or a different read page. These changes have focused tests and a fresh offline build and strict lint check. The subsequent test-repair trial uses the same frozen native wrapper, independent mutation suite and immutable repaired core.

A second test-repair attempt confirmed a further issue: even structured native pages were removable as optional `recent_evidence` during prompt compaction. It was cancelled without a repair. Current approved Python source pages are now retained separately as required `source_pages`, using current actor, native source digest and policy revision; repeated pages are deduplicated and older source bytes excluded. The packet retains at most 16 KiB in complete bounded pages, without pretending those pages constitute an entire larger file. Required material that cannot fit still fails closed. This preservation rule has its own compaction regression test.

## Task-runner adoption and live check

The independently reviewed module is copied unchanged to `scripts/showcase_observer.py`. The task runner now uses it after a single dispatch, writing snapshots and attention only when their canonical bytes change. Its new `--observe-run` mode reads an existing handle and never enters intake, approval or dispatch. The normal final report retains its run ID; runtime state and observation outcome are separate fields.

A live CLI invocation observed a previously cancelled trial with unanswered attention. It returned `cancelled` immediately. Two further observations used four actual read requests total and a controlled clock for deterministic report bytes: all three persisted files retained their hashes and modification times. The observer dispatched zero tasks and zero inference calls. The reported 18 historical model replies belong to the observed task, not to observation. This proves bounded read-only observation and avoided rewrites of unchanged snapshots, not a general task-speed or token-efficiency gain.

Run this against an existing local harness, substituting the original handle:

```sh
python3 scripts/local-showcase-task.py --server http://127.0.0.1:8788 --observe-run <existing-run-id>
```

The 16 independent tests are retained in `scripts/fixtures/showcase-observer/acceptance.py` and run through `scripts/test_showcase_observer.py`. The source and tests are protected broker inputs. [Public evidence](evidence/local-observer-repair-2026-11-15.json) contains reviewed measurements and hashes, without the private company journals. The company's own test quality remains a separate pending experiment.

## Test-repair result and authorization follow-up

The third own-test trial finally authored a proposed full test module. It did **not** execute the write: its authorization packet exceeded the context byte limit, and the frozen 24 reservations were exhausted during preparation/retries. The run was cancelled with the exact proposed intent retained. No write was replayed. A separate offline host review of those exact proposed bytes found three failing methods; the frozen mutation-quality suite also failed its positive-control check. These own tests are rejected, and none is adopted as qualified coverage.

The source-page fix is therefore an evidenced context-preservation repair, not a demonstrated improvement in this model's test-authoring reliability. The runtime's source observer adoption relies on the 16 separately frozen independent host checks, not these rejected company tests.

Authorization packets now omit bodies of earlier receipt-linked native reads, while retaining their journal reference, original record/result hashes, source hash, offsets and policy revision as explicitly historical metadata. The complete exact **next** request remains unchanged for authorization. The projection does not claim current file state or substitute a modified payload for a canonical receipt. Focused checks cover valid projection, forged linkage rejection and unchanged next-write arguments. This addresses the newly exposed packet-size cause; a further company trial is still needed to measure its effect.

Current verification: 554 Rust workspace tests passed, 27 opt-in tests ignored; strict offline Clippy and the workspace build passed. Python snapshot checks (8), broker checks (32), retained observer checks (16), and the ten-page local site checks passed. Website source and a local preview build are updated; publication/deployment and browser-rendered behavior are not claimed.

A follow-up trial narrows the editable surface to the three failing test methods. Their exact generated method text will be mechanically assembled into the original class, retaining the three working methods unchanged. The same frozen seven-regression host quality suite still applies. This is a decomposition of the real rejected test repair, not a smaller replacement contract or an accepted result.

The narrower trial was also rejected. It recorded 19 model replies and consumed its 20 reserved attempts, with $0 recorded provider fees and one live mentoring brief. It reached two confirmed native writes of the three-method patch, demonstrating authorization of this smaller request under the new packet implementation. This does not prove that the earlier full-module proposal now fits. The reviewer denied a queued **read**, incorrectly reasoning about an earlier **write**; the deny remained in force. A later worker-proposed write with different bytes received its own authorization. The final written source hash `1476c2e9b0286f7dd04bcf7e716b50461fbd450fd54c8f96ba25af68182d8959` was independently assembled and checked after cancellation, so an earlier intermediate candidate was not mistaken for the final result.

The final assembly retained two wrong assertions and attempted `.get` on a callable API fixture, producing two failures and one error. Its positive-control/mutation-quality suite failed. These company-authored tests remain unqualified and unadopted. No native passing diagnostic, complete readback, accepted review or reliable autonomous test-authoring result is claimed. Trial runtimes were stopped after cancellation to release their local model resources; no existing unrelated run was reset.

The next research step should separate source authoring from action selection and evaluate a different installed local planning/authoring combination against these retained failures. This is an untested experimental direction, not a claimed capability gain. The broader goal remains active, including bounded long-running research, durable learning evaluation, communication/orchestration and media/weight experiments; this observer work does not stand in for those requirements.
