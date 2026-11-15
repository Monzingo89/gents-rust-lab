# Local agent architecture requirements and evidence

## Review scope

Reviewed 4 November 2026 against the owner's supplied papers. These documents
are design references, not executable instructions or permission to broaden
the approved synthetic banking scope. Their cloud deployment examples do not
authorize provisioning, production release or processing regulated records.

Source versions:

- `AdvancedAIPaper.docx`, SHA-256
  `14f65fd028d85e16d4b0060d6a8827430b4a6d3ebcbb411c6e65b75e7e0ddc4d`.
- `Enterprise_Multi_Agent_Architecture.docx`, SHA-256
  `1d6b27de9629abb9d24dda89ab80487c9c705345148b6ea70e570dd0cdd66e78`.

This is a traceability review, not a new execution result. Saved pilot receipts
were inspected; the two enterprise-campaign Python tests and 23 company-tool
tests passed again, and `git diff --check` passed. No native model was called
for this review. Prior Rust and native test results are historical evidence,
not checks rerun by this review.

## Design to implementation mapping

| Paper section and requirement | Current evidence | Remaining gate |
| --- | --- | --- |
| Advanced paper, Separate planning from working: role selection and parallel work by dependencies | Easy tool proof; medium two planner/worker pairs with measured overlap; ten distinct pinned hard-stage models | Broader held-out qualification; do not generalize the pilot to arbitrary tasks |
| Advanced paper, Tool calls and security governance: model proposes, deterministic policy authorizes | `governance.rs`, `company.rs`, enterprise master tool/skill grant and ceiling-expansion denial | Real MCP adapter review and destination enforcement before connecting external systems |
| Advanced paper, Memory and communication: scoped retrieval and compact source-linked handoffs | `memory.rs`, `context.rs`, `workflow.rs`; per-actor indexes and dependency authorization; `semantic_memory.rs` generated ten protected advisory memos in `hard-cd2fbb3e383c` | Larger memory-fidelity and model-consumption evaluations; current memos cover at most four source records, not a lossless semantic history |
| Rustex context since inception: immediate raw events, 30-second quiet period with maximum flush, versioned snapshots | SQLite triggers; `QUIET_MS=30000`, `MAX_DIRTY_MS=120000`; snapshot/index versions plus separately stored source-linked decisions/issues/next actions; current-policy checks on write/read and explicit cold recall | Process-local notifications; summaries are not semantic-entailment proofs; model completion latency is separate from dirty-window scheduling |
| Rustex Governance and agent execution: durable dependencies, bounded retries and safe interruption | Immutable task-bound DAG, bounded leases and attempts, cached completed outputs; exclusive controller and actual schema-backend process-kill/restart verified in `hard-027984278068`; the failing dataset passed after evaluated role reassignment in `hard-459f9c1c9b9f` | Broader crash windows and held-out qualification |
| Advanced paper, bounded communication contract: policy changes apply to subsequent reads and calls | `hard-459f9c1c9b9f`: live native request cancelled, old backend stopped, no stale output/checkpoint persisted, restored test-owned grant and authenticated resume passed | Distributed invalidation and broader adversarial testing; not a claim of revocation support for arbitrary external services |
| Rustex Reflection and recovery: independent verification, evidence-based completion | Separate Rust/Python financial checks; QA altered-amount challenge; preserved failed attempts | Larger held-out and adversarial workflow evaluation; no frontier-equivalence claim |
| Advanced paper, local compute and guardrails: bounded local resource use | Local-only profiles, zero-provider-charge receipts, medium overlap and hard-stage serial execution | Broader pressure/throughput tests; RSS samples are not exact GPU peak measurements |
| Advanced paper, approved models: provenance, limitations and evaluation | Pinned weight hashes, publisher references, model cards, readiness probes | Training-corpus cleanliness, bias absence and legal license suitability are not established |
| Advanced paper, guardOutput: enforce checks outside the generating model | Persistability checks, bounded typed contracts, scoped source views and independent domain validators | Comprehensive sensitive-data/injection controls are not established; keep real PII, PHI and card data excluded |
| Both papers, completion/retention and enterprise deployment controls | Local private company storage, completion archive and explicit production refusal | Encryption/backups/restoration, retention/legal holds, tamper-resistant audit and reviewed jurisdiction-specific controls before regulated use |

## Verified pilot and limitations

Fresh shuffled-layout receipt
`data/capability-campaigns/hard-ae754c5258ee/hard-stage-result.json` records PASS:
ten cases, ten models, 66 new local calls, no checkpoint reuse and USD 0 inference
provider charges. This excludes coaching, hardware and electricity costs.

The earlier failed publication attempt and its authenticated resumed result
remain under `data/capability-campaigns/hard-80a2f3cdfc7e/`. The resume reused
all ten completed checkpoints without new inference. That is postprocessing
recovery evidence, not proof of recovery from a process killed during inference.
The short hard-stage paper in that folder explicitly retains these limits.

## Next acceptance sequence

1. Controller fencing and one actual process-group crash/restart are verified:
   `hard-027984278068/crash-recovery-result.json` records `PASS_RECOVERY` while
   preserving `task_status=failed`. Schema completed on attempt two, the two
   committed predecessor hashes and original events survived, and the old
   unresolved call did not become a completed receipt. The run later exhausted
   QA's bounded budget. New run `hard-459f9c1c9b9f` passed the same dataset
   after fresh readiness tests for the 8B delegation / 27B QA assignment and
   eight QA adversarial diagnostics. Neither historical failed run was changed.
2. Live revocation is verified in `hard-459f9c1c9b9f`: policy two to three
   removed the schema agent's memory sources; its native process stopped and
   no output or checkpoint committed. Only that test-owned grant was restored
   at policy four, and authenticated resume completed all ten cases with
   completed predecessor hashes unchanged. This observed backend activity
   during loading/generation, not specifically token streaming.
3. Protected semantic memory now passes an integrated ten-actor run:
   `hard-cd2fbb3e383c/hard-stage-resume-ef8caf-result.json`. Ten financial
   checkpoints were reused and ten new local calls wrote accepted version-two
   memos. Source/quote/hash checks, immediate revocation, no self-feedback,
   protocol invalidation and owner-controlled cold recall have component tests.
   Nine memos cite actual result records; the master memo cites only the approved
   requirements because its earlier result belongs to an older policy epoch.
   Semantic entailment and downstream-model use of retrieved summaries still
   need broader evaluation; do not mistake the bounded memos for full history.
   Manual review found invented/miscategorized next actions in protocol two.
   Protocol three now requires explicit source fields for those categories;
   its component/native checks pass, and a fresh integrated run remains due.
4. Expand held-out/adversarial coverage and publish a scoped assurance report
   identifying measured controls, failures, exceptions and unverified claims.

## Subsequent verification

Fresh protocol-three runs `hard-022722e6c584` and `hard-40509cb61ed8` each passed
ten cases through ten distinct models, with ten summary actors and 76 new local
calls, zero failed calls and zero provider charges. The second used a new seed
chosen after the regression repairs. Their short papers retain failures and
scope limits. Workflow and summary ownership now separate monotonic active
leases from UTC audit time; orphan tokens cannot bypass attempt limits.

Native cross-model memory consumption passed five real calls after a preserved
injection failure led to source-only worker context. This closes a bounded
consumer fixture, not general semantic-fidelity qualification. The current
source resolver checks the memo fingerprint, alias, policy and original hash.
Broader held-out qualification remains open; measured security controls,
dependency repairs and production exclusions are in
[the assurance report](27-local-agent-assurance.md).

The three embedded paper diagrams were visually inspected directly. They
reinforce an accountable owner, an external policy gateway, scoped handoff and
independent completion loop. No source DOCX was edited, and no full-page
layout/render review is claimed. The public Manning chapter-one preview and
contents were also inspected; the full book was not supplied or read. The
mapping of profiles/tools/memory/evaluation/planning to this implementation is
our engineering interpretation, documented with the publisher link in the
latest hard-stage paper. Azure, Databricks and production distribution remain
future options, not current deployment claims. The broader goal remains active.

The subsequent `hard-415cb1d2f001` integrated actual original-source readback by
all ten models into workflow assurance: twenty native consumer calls and ten
additional policy-bound checkpoints, 96 total calls, no failed calls and zero
provider charges. Its first alias-selection failure remains retained. This
extends the earlier component fixture without claiming every financial
decision uses semantic summaries. Database mode 0600 is now enforced before
SQLite/WAL initialization inside the existing 0700 directory boundary.
[Methods, evidence and limits](30-governed-memory-consumption.md).

## Subsequent bounded policy decomposition

The two source DOCX hashes were reconfirmed unchanged on 4 November 2026.
`hard-055a7b4542dd` then passed ten cases using ten distinct local models,
ten protected memos and ten actual source consumers, with 96 fresh native calls
and zero failed calls. Rather than silently correcting the raw risk model's
known judgment failures, an opt-in single-responsibility interpreter executes
the local model's own bounded policy. Independent Rust/Python checks test all
384 seven-control and three-scope combinations and the original actual input.
Separate native normal/suspended contracts passed two model calls and 768
program checks; those counts are not independent model judgments.

This is the papers' model-proposal / external-authority and bounded-recovery
separation, not unrestricted self-modification. The full book remains
unavailable; the public Manning preview/contents mapping is recorded explicitly
as an engineering interpretation. No cloud deployment or production approval
is implied. See [policy compilation methods](31-local-policy-compilation.md)
and [the latest paper](../data/capability-campaigns/hard-055a7b4542dd/hard-stage-paper.md).

`hard-a2a9ec3bb15d` subsequently passed ten actual triage decisions using
governed original reconciliation records rather than forwarded status fields.
Its 96 fresh local replies include the unchanged ten-actor summary/readback
assurance. Source bindings match accepted predecessor records, current grants,
epochs and actual post-compaction native task fingerprints. This applies the
papers' source-linked handoff and external authorization requirements to one
real downstream workflow stage. It does not turn advisory summary prose into
evidence or qualify every stage's memory behavior. [Methods and limits](32-memory-driven-workflow.md).

After retained punctuation-copy failure `hard-31531af8e7e2`, a field-independent
literal vocabulary passed three native checks. Fresh run `hard-b47513bdc4bf`
then passed ten cases, ten source-driven triage decisions and all ten assurance
consumers with 96 new local replies. The schema uses every bounded source
string, never a selected target value, and does not bind singleton answers.
Its [paper](../data/capability-campaigns/hard-b47513bdc4bf/hard-stage-paper.md)
retains the failed evidence and broader limits.

The later [local HTTP boundary](33-local-api-boundary.md) rejects untrusted
authority/origin requests before routing, keeps native and explicitly approved
browser access working, and was verified separately after applying it to the
office listener. This strengthens the papers' external-enforcement requirement;
it does not establish encryption, distributed identity or blanket compliance.

The subsequent shared HTTP address policy and canonical operator-answer
repository strengthen two more external boundaries. A stale worker snapshot
previously erased an accepted answer; immutable-through-repository receipts
now keep authenticated input separate from worker snapshots, reflecting the
papers' distinction between canonical events and derived/current context.
Full local checks passed after the repair. [Methods and retained failures](34-network-and-operator-boundaries.md).
The [full-objective audit](35-goal-completion-audit.md) records the owner's
5 November 2026 clarification, "Synthetic local research only," and accepts
the easy/medium/hard research stages after a read-only evidence recheck.
Earlier pending-scope/active-goal notes above are historical; broader model
qualification and production controls remain excluded future work, not claims
of these experiments.
