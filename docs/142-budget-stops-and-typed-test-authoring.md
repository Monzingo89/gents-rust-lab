# Budget stops and typed test authoring

This continues the real observer/test-repair experiment in [141](141-observer-repair-and-python-followups.md). The full company goal remains active. No paid API, model download or training is involved.

## Stop spent allowances without another recovery loop

`inference_budget::reserve` now distinguishes verified allowance exhaustion from ordinary preparation or provider errors. The harness persists the typed stop reason and signals the run's shared cancellation token before any provider dispatch for that denied attempt. Its terminal status is `budget_exhausted`, separate from operator cancellation and task acceptance. Queued intents, native receipts and candidate source remain retained. Restart and manual resume cannot renew the frozen allowance. A crash after persisting the stop reason but before the final lifecycle checkpoint is reconciled against the company journal without replaying queued work. Canonical existing terminal outcomes still take precedence over stale run snapshots.

The new `scripts/inference_budget_smoke.py` uses actual HTTP, the actual company databases, the native file tool and disposable run journals. Decisions are simulated, so this verifies lifecycle enforcement rather than model competence. Limits of one, two and three attempts exercise exhaustion at worker selection, action authorization and after a failed native file read. Checks require one denial, no later provider/tool dispatch or recovery retry, unchanged queued intent, terminal company state, conflict on manual resume and no new reservations on restart. A disposable crash-window fixture preserves the stop reason while removing the terminal checkpoint; startup must finish the budget stop without work replay. Failed authorization preparation is also checked against the same stop boundary.

The existing allowance remains opt-in per owner-approved intake. Reservations still include failed preparation and uncertain attempts, without refunds. This is a model-call allowance, not an estimate of useful work, a provider-cost meter, an independent acceptance gate or permission to start another task.

## Separate authoring from action selection

The previous worker repeatedly selected reads and checks instead of making the requested test edits. The existing `candidate_workflow` controller offers a different execution path: a local model authors bounded source text; deterministic stages perform only the approved native write, full readback and quarantined submission. This reuses the existing controller rather than adding another model-controlled tool. Independent acceptance remains the original six-method positive control and frozen seven-regression quality suite. The proposal-stage acceptance string is not substituted for those correctness requirements.

The first new trial keeps the same Qwen3-Coder-30B worker, with one author call and no live coaching. It submitted a 5,032-byte candidate through one native write and one native read. Independent evaluation rejected it: the candidate supplied no reply while expecting a reply count of one, confused the two API routes, and did not implement the required invalid-input checks. Fewer model decisions therefore did **not** establish better accepted-task efficiency.

The static assembly checker initially rejected `import unittest.mock`, although the frozen author prompt explicitly allowed that standard-library module. Its import allowlist was corrected to match that predeclared permission; the six positive methods and seven regression criteria were not changed. All three generated methods still failed independent evaluation afterward. No rejected test was adopted.

A subsequent trial uses the already installed Qwen3.8-27B model with 2,048 bounded reasoning tokens and 4,096 reserved final tokens. It keeps the same task text, source-size ceiling, one-call allowance, typed controller and host quality suite. This is a descriptive experiment on one real task, not a model ranking or evidence of generalized coding ability. Its outcome must be recorded separately before claiming success.

The design matches the separation of decision, execution and communication described in the publisher's public [chapter 4 preview of *AI Agents in Action, Second Edition*](https://www.manning.com/preview/ai-agents-in-action-second-edition/chapter-4). The publisher describes flow, central orchestration and peer collaboration; our controller and acceptance boundaries are project code, not copied book implementations or a claim of full-book access. Existing three-pattern implementations and their limits remain documented in [128](128-independent-reasoning-acceptance.md).

## Recorded authoring outcomes

The reasoning-model attempt submitted an 8,159-byte candidate in 242.67 seconds, using one reply with reported usage of 2,466 input and 4,132 output tokens. Separated reasoning was observed; the backend did not separately report its reasoning-token count. Independent assembly found three errors because the fixtures placed usage outside a string-valued reply. The standalone patch also omitted its required core-module import. It was rejected.

A subsequent one-call repair supplied the actual nested event shape as pre-dispatch mentoring and used a 512-token reasoning allowance. It produced smaller source and fixed that shape, but omitted required blocked-state and unanswered-attention cases, and attempted to test provider costs through `EvidenceStore` rather than `observe_run`. The positive control failed and the frozen mutation suite identified two escaping regressions. It was rejected too. One oversized pre-intake reference was rejected before approval, dispatch or inference; the retry carried concise diagnostic feedback instead. No submitted method logic was edited by the host.

Both the independent assembly and the frozen standalone patch wrapper were checked. All three new source candidates remain unadopted. Smaller prompts, fewer planning decisions and additional reasoning do not by themselves prove better accepted-task efficiency. [Reviewed measurements and limits](evidence/local-budget-and-typed-authoring-2026-11-15.json) retain the failed outcomes separately from the successful budget lifecycle checks.

| Typed author attempt | Recorded wall seconds | Reported input / output tokens | Native write / read receipts | Independent result |
| --- | ---: | ---: | ---: | --- |
| Qwen3-Coder-30B, no reasoning allowance | 18.26 | 2,424 / 1,332 | 1 / 1 | Rejected |
| Qwen3.8-27B, 2,048 reasoning allowance | 242.67 | 2,466 / 4,132 | 1 / 1 | Rejected |
| Qwen3.8-27B, 512 reasoning allowance plus diagnostic mentoring | 95.19 | 2,741 / 1,475 | 1 / 1 | Rejected |

Every trial used one reserved call and reported $0 provider fees. Reported output does not separately identify reasoning tokens. The third prompt and generation profile changed, so this is not a controlled model ranking. All six canonical native tool receipts and submitted source hashes were rechecked against their company databases.

Current verification: 555 Rust workspace tests passed with 27 opt-in tests ignored, strict offline Clippy and build passed, the final HTTP budget lifecycle fixture passed, the existing autonomy/restart smoke passed, 32 broker checks passed, and 16 retained observer checks passed. The ten-page website checks passed after adding a link to this record. Changes remain local and pending; no deployment or weight/model promotion occurred. The three trial runtimes were stopped only after observing terminal candidate-submission state.
