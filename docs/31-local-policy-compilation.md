# Local model policy compilation

## Architectural change

The expanded risk-model diagnostic remains a failure: 19 of 33 baseline
judgments and 25 of 33 revised judgments were correct, with a false allow in
both trials. Repeating that judgment indefinitely would not meet the papers'
bounded-recovery requirement. The opt-in `compiled_release_policy` method
instead asks the pinned local risk model to implement the owner's English
policy as a small declarative program. It does not replace the independent
authorization service or grant production access.

`release_policy.rs` has one responsibility: interpret and verify this bounded
artifact. Programs contain one to three ordered rules, predicates from a fixed
catalog, and either deny or allow-synthetic-evaluation outcomes. They cannot
contain executable code, tool calls, credentials or new permissions. The model
receives the owner contract and catalogs, not example solutions, expected rule
mappings, or a precomputed verdict. Only the transport correlation identifier
is decoder-bound. The interpreter preserves wrong rules; an independent gate
rejects them rather than repairing their answers.

The enterprise adapter requests and records the actual native model reply,
verifies its program, executes it on the original risk input, and checks the
result independently. The campaign's separate Python verifier replays the
program, checks its correspondence with the native reply, and compares its
actual contribution with the saved artifact. Ordinary autonomous source repair
cannot modify this policy interpreter. Legacy runs retain the raw judgment
path unless explicitly configured otherwise.

## Focused native evidence

`data/model-registry/release-policy-probe-411f88c6-98d6-4891-b349-3f852fcc21c0.json`
contains two actual local Gemma calls with pinned weight verification. One
implements the normal synthetic-only contract; the other implements an
evaluation-suspended contract. Both passed. The normal generated program denies
failed controls, denies non-synthetic scope, and otherwise allows synthetic
evaluation. The suspended generated program denies every input.

Each program was executed over all 128 combinations of seven controls under
synthetic, production and unknown scopes. This is two model replies and 768
deterministic program checks, not 768 independent model judgments. Separate
negative tests prove that a wrong production-allow program remains wrong in
the interpreter and is rejected by the authority gate, and that executable
predicates are rejected. No weights were trained.

The full ten-model workflow passed separately as `hard-055a7b4542dd`, with ten
synthetic cases, ten protected summaries, ten actual source consumers and 96
fresh native calls. There were no failed/cancelled calls or checkpoint reuses.
The program generated in that run passed 384 independently replayed checks.
The run took 414.42 seconds, sampled 19,231,984 KiB peak backend RSS (not exact
GPU allocation), and reported USD 0 inference-provider charges. The report's
independently read-back SHA-256 is
`6942285806e7dbc04dd824de1c7d91a5efa9539d45d6170c761d7ea9097fbcc2`.
The [short paper](../data/capability-campaigns/hard-055a7b4542dd/hard-stage-paper.md)
and adjacent result/snapshot preserve the integrated evidence.

## Relationship to the owner's sources

The two unchanged source papers are `AdvancedAIPaper.docx` and
`Enterprise_Multi_Agent_Architecture.docx`. Their hashes and requirement mapping
are in [the traceability review](25-paper-traceability.md). This change applies
their separation of planning, execution and deterministic governance; evaluated
model selection; narrow responsibilities; bounded repair; and evidence-based
completion. Local inference, scoped memory, source-linked handoffs and durable
checkpoints remain separate components rather than being folded into a single
model prompt. Cloud deployment proposals in the papers are not implemented by
this experiment.

The public publisher contents and chapter-one preview of
[*AI Agents in Action*](https://www.manning.com/preview/ai-agents-in-action/chapter-1)
identify actions, profiles, memory and compression, evaluation, planning and
feedback as topics. Our mapping is an engineering interpretation: the existing
tool interface, phase-specific model profiles, source-indexed memory service,
independent validators and durable dependency graph implement those concerns
without a framework migration. The full book was not supplied or read; chapter
titles are not evidence that its detailed implementations were reproduced.

## Verification and limits

The implementation passed 160 enabled Rust tests, strict all-target Clippy,
the locked harness build, 31 focused Python tests and `git diff --check`.
Fifteen native tests are opt-in and excluded from the 160 count; the new
two-call native probe was run separately. Runtime policies default off and
require an explicit campaign flag:

```sh
python3 scripts/enterprise_campaign.py --challenge-layout --semantic-memory --consume-memory --compiled-risk
```

This finite rule language is suitable for the stated synthetic release policy,
not arbitrary banking judgment or frontier-model equivalence. Correct program
execution does not establish semantic summary fidelity, absence of bias,
general injection resistance or regulatory compliance. Provider-charge
receipts exclude coaching, electricity and equipment. Real regulated data,
external business effects and production release remain prohibited.
