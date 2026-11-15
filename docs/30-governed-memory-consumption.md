# Governed memory consumption inside the local workflow

## Purpose and boundary

Saving ten memos does not establish that ten models use them. The optional
consumption audit adds actual planner selection and worker original-source
readback for each configured model before the banking report can be accepted.
It is workflow assurance, not evidence that each earlier financial decision
was made from a semantic summary or that summary prose is entailed by sources.

Enable the isolated synthetic profile with:

```sh
python3 scripts/enterprise_campaign.py --challenge-layout --semantic-memory --consume-memory
```

Normal runs and old configurations are unchanged. In configuration the new
setting is `semantic_memory.verify_consumption=true`; it requires the enterprise
profile. No provider credentials, cloud storage, model downloads or general MCP
transport are added.

## Interfaces and control flow

`memory_consumption.rs` isolates the read-only consumption service from domain
banking logic and the provider implementation. It uses the existing summary
repository and authorized source resolver rather than a second context store.
The workflow supplies the configured runtime actor; model JSON supplies only a
source alias, never an owner, recipient, database ID or permission grant.

1. The configured planner sees a bounded advisory memo and selects the alias
   cited by its first decision. Selection is independently checked.
2. The resolver rechecks the current memo fingerprint, recipient, policy epoch,
   original source hash and archive/access rules. It records metadata-only audit
   evidence through the existing transactional company operation.
3. The configured worker gets only the original source and a field name, with
   no advisory prose or expected field value. It reads back `case_id` from a
   financial source, or the requirements-only authority label from the master's
   fallback. The verifier compares the actual reply with original source JSON.
4. The source is resolved again before commit. Each consumer has an immutable
   DAG node depending on its own financial stage, two bounded attempts, native
   receipts and a policy-bound checkpoint. Cached results undergo fresh source
   and recipient checks; they cannot override revocation or a changed memo.

The existing ten financial nodes remain required. Ten additional audit nodes
must complete when enabled. The final report includes source hashes, memo
fingerprints, policy epochs and readback results. Provider events retain the
two actual calls per model and can be correlated with checkpoint receipts.

`memory_select` and `memory_extract` use isolated context profiles without
historical model output or general guidance. Their replies pass the data gate,
but are not staged as canonical model memory and do not acknowledge/promote
handoffs. This prevents derived audits from becoming self-supporting evidence.

## Verification and outstanding limits

159 enabled Rust tests pass, including source-only extraction, forged values,
extra output fields, wrong owner/recipient, stale memo fingerprints, changed
source hashes, revoked permissions and regrant without old-epoch revival.
Fourteen native opt-ins remain separate. Strict Clippy and build pass; the
focused Python suites pass 30 tests. The campaign verifier additionally checks
original-source values, all twenty checkpoint hashes, actual phase/model
identities, native receipts and absence of staged derived-audit replies.

The new service is protected from ordinary autonomous source repair. It does
not grant new tools or broaden any recipient's memory scope. Broader risk-model
qualification still fails as documented in the release-gate report. Real
regulated data, production actions, distributed isolation, encryption/key
management and regulatory certification remain outside the approved pilot.
The overall architecture goal remains active.

## Retained initial failure and source-contract repair

`hard-c1dc47fcf253` completed the ten financial stages and ten memos, then four
actual model consumers. Its coding model returned the quoted word `rules`
instead of the first decision's source alias on both bounded attempts. The run
remains FAIL with 86 completed native calls; no retry count was reset and no
bad alias was resolved or accepted.

The repaired source view gives every decision an explicit position and a
`source_alias` field, separate from advisory text and quotes. The decoder may
emit any alias from the memo's entire authorized source index, not just the
task's correct alias. Changing the first decision while retaining the source
index produces an identical decoder schema in the counterfactual unit test.
Thus field naming/vocabulary are repaired without supplying the selection or
source-field answer. Original-source extraction and independent checks remain.

The review also found that private company directories were mode 0700 but new
SQLite files could inherit mode 0644. The connection adapter now creates or
tightens the database to 0600 before SQLite creates WAL sidecars. A filesystem
unit test verifies the database mode. This is local file protection, not
application encryption or a boundary against a same-user host process.

## Fresh integrated result

`hard-415cb1d2f001` passed all ten banking cases and ten consumer checkpoints:
96 fresh local calls, including twenty consumer calls and ten summaries, with
no failed/cancelled calls or reused checkpoints. All ten distinct models
selected a memo reference and read an original-source field. The external
verifier correlated their actual native replies with model/phase events and
checked all twenty DAG checkpoint hashes. Elapsed time was 414.66 seconds;
sampled peak backend RSS was 18,416,928 KiB, not exact GPU memory. Provider
charges were USD 0, excluding coaching, hardware and electricity.

Artifact SHA-256:
`545e665d10e194b71f06f6083103a5141dddf5ae64906d432739a5cd7d43c083`.
The [short paper and adjacent receipts](../data/capability-campaigns/hard-415cb1d2f001/hard-stage-paper.md)
retain the initial failure and separate assurance readback from financial
decision-making, semantic fidelity and enterprise qualification.
