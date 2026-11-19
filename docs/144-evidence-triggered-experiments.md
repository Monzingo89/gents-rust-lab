# Evidence-triggered bounded experiments

This records the admission/observation milestone. [145](145-independent-experiment-results.md) subsequently adds optional independently executed Rust acceptance; the results and remaining scope below describe the earlier snapshot.

The local company can now admit a finite, owner-frozen queue of experiments when explicitly selected evidence changes. The new controller was implemented by the publishing assistant. The company exercised it with real local source-authoring calls; the generated source remains rejected. The full continual-learning objective is active.

## What executes

`scripts/experiment_loop.py` freezes exact tasks, criteria, actors, tool grants, per-trial call allowances, total allowance, a public harness configuration snapshot and baseline input hashes. Inputs must be bounded regular files under `data/experiment-inputs/`. Their bytes supply a change signal, not executable instructions or additional authority. Input bodies are not automatically inserted into model prompts. No research-profile defaults or permissions are inherited silently.

The first poll after freezing does nothing until evidence changes. Each distinct content fingerprint can claim at most one of the predefined trials. Returning to the baseline or to an already consumed fingerprint remains idle. A changed mtime alone is not new evidence. Idle polls make no HTTP requests, model calls or ledger writes. The current implementation hashes local files every five seconds while watching; it is polling, not an OS filesystem notification service.

A private SQLite ledger, through the owned SQLite bridge, stores the complete frozen snapshot and a claim before any external mutation. The claim reserves the entire trial allowance without refunds. The existing native intake, approval, governance and model-call budget enforce the actual run. Each prepared job receives only its exact frozen grants. The controller yields when it observes another approved/running company job; this check does not lock out unrelated external clients racing to start work.

An exclusive process lock spans each poll, including preparation and dispatch. A second poller cannot treat the first poller's in-progress preparation as a lost operation. The ledger retains the original run ID across crashes, failures and lost responses. Subsequent polls only observe that run; they never replay intake, approval, policy installation, dispatch or resume. Missing runs require owner reconciliation. A failed preparation still consumes its claim. Exhausting the finite queue stops watching; a new reviewed plan requires a new loop ID.

Runtime results, model reply counts and zero-fee checks reuse the independently checked observer. `completed`, `incomplete`, `blocked`, `failed`, `cancelled`, `candidate_submitted` and `budget_exhausted` are recorded distinctly. Interrupted, uncertain and timed-out runs remain outstanding. Unknown states and invalid costs fail closed. Every result explicitly says `independently_accepted:false` and `promotion_authorized:false`. A runtime outcome alone never selects a winner, changes weights, adopts code or expands the plan.

## Verification and actual trial

Twenty-one controller checks cover idle caching, restart, lost-response reconciliation, crashes before intake, finite reservations, unchanged fingerprints, configuration drift, concurrent polling, attention, unexpected states, invalid costs, input links/FIFOs/size/traversal and actual HTTP redirect rejection. The actual HTTP smoke fixture uses a private running harness and real company databases. Twenty unchanged polls make zero HTTP requests. Two one-call jobs consume exactly two native reservations, including one dispatch with an intentionally lost response. Reopening the controller observes the original run without another POST. Decisions in that lifecycle fixture are simulated; it proves admission/durability, not model competence.

A separate **real** local trial uses the installed Qwen3-Coder-30B model and the existing typed candidate workflow. The hypothesis is to split the previously rejected three-method observer test repair into three one-method source-authoring calls. The original six-method positive contract and seven-regression quality suite were frozen unchanged. Each predefined trial made one call, one native write and one complete readback. All six receipts, three source hashes, intake/approval digests and native reservations were reconciled against the company database. The finite queue stopped after three claims.

| Observation | Recorded value |
| --- | --- |
| Overall three-trial wall time | 44.508 seconds |
| Local replies / reserved calls | 3 / 3 |
| Reported input / output tokens | 7,512 / 2,021 |
| Provider fees | $0 |
| Independent result | All three drafts rejected at static review |
| New positive/mutation execution | Not run after static rejection |

All three drafts import nonexistent module `m` rather than using the supplied module alias. The attention draft also replaces the core observer with a mock, violating the frozen task; other required cases are omitted. The host did not fix, execute or adopt these drafts. Consequently decomposition has **not** established better correctness or accepted-task efficiency. It used more reported tokens than the earlier single-call Qwen3-Coder trial. These are descriptive trials with different prompt structure, not a controlled model ranking. Generation latency and overall orchestration time are recorded separately in the [evidence](evidence/local-experiment-loop-2026-11-15.json).

The real trial retains the exact controller source it used. Afterward, the final controller added explicit handling of existing `incomplete` and `timed_out` runtime states. The final unit checks and actual HTTP fixture were rerun; the historical real-trial source hash is preserved separately from the final source hash.

## Running a reviewed queue

Keep the owner-configured local company harness running in its terminal. This controller requires local authentication, company tools and local-only mode. It never starts a model server, downloads weights or switches to a cloud provider itself; admitted runs use the harness's configured backend. Native tool and provider checks still apply.

Use [the example plan](../config/experiment-loop.example.json) as a starting point. Review its existing company name, exact configured actor and task before freezing. Create the selected input file with real baseline evidence; the example expects `data/experiment-inputs/local-research/evidence.json`. Register the company through the normal owner interface if it does not already exist. Then, from the repository root:

```sh
python3 scripts/experiment_loop.py --id local-research-v1 freeze --plan config/experiment-loop.example.json
python3 scripts/experiment_loop.py --id local-research-v1 status
python3 scripts/experiment_loop.py --id local-research-v1 watch
```

The last command stays running and waits for changed evidence. Update the selected input only with actual new evidence. Ctrl-C stops the watcher; an already dispatched company run continues under its existing budget and permissions. Use `poll` or `status` with the same ID to inspect it later. Do not create another ID merely to bypass an unresolved effect or exhausted allowance.

The public configuration snapshot identifies exposed model/provider names and capabilities; it is not a full model-weight or server-binary attestation. The real trial separately retains its local runtime/configuration hashes. State directories are private and owner-controlled; these Python path checks do not replace native descriptor-relative confinement against a hostile host process. The controller supports a narrow local tool allowlist and optional source-only candidate workflow, not arbitrary shell execution or every existing harness contract.

This is durable, evidence-triggered experiment admission and runtime observation. Automatic independent evaluation, adaptive hypothesis selection, held-out comparisons, qualified procedure/model promotion, rollback and a 24/7 soak are still unfinished. No service was left running, no candidate was adopted and no site was deployed.
