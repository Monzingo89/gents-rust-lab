# Governed memory in actual workflow decisions

## Purpose

Earlier integrated runs proved that ten models could select a memo reference
and read an original field during assurance. They did not prove that an actual
financial workflow decision depended on retrieved memory. The opt-in
`governed_triage_memory` path now retrieves each accepted reconciliation record
from private canonical memory and supplies that original to the triage model.
The model must select the queue from the unchanged routes contract. The packet
contains neither a forwarded top-level status nor an expected queue answer.

This is source-record retrieval, not semantic-summary reasoning. Advisory
summaries remain separate, source-linked and non-authoritative. It directly
extends the owner's requirements for scoped handoffs and external permission
checks without pretending that summary prose is entailed by its sources.

## Interfaces and boundaries

`memory_handoff.rs` defines a domain-independent `HandoffRepository` interface
with `find` and `revalidate`. Its SQLite adapter uses an authenticated company
connection and a runtime-bound recipient. A query identifies the sender,
correlation key/value and normalized hash of an already accepted predecessor
record. Only source identifiers are enumerated before the existing governed
point-read boundary supplies a body. Retrieval is bounded to 128 candidate IDs
and a 14,500-byte original record; unavailable evidence fails closed.

Each binding records run, recipient, sender, correlation, original source ID,
original-body hash, accepted-record hash and policy epoch. Unacknowledged ASTC
stays private. A sender grant is required, source records must originate in the
same run, and old model epochs cannot revive after revoke/regrant. Failed
predecessor attempts do not qualify merely because their replies were staged:
the normalized original must match the accepted predecessor record.

The enterprise adapter acknowledges only permitted predecessors, retrieves the
ten original records transactionally, prepares focused triage packets, and
revalidates sources before checkpoint acceptance, on cache reuse and before
final publication. Banking validation still independently derives the expected
queue from the original task. Model output cannot choose a recipient, source
identity, access policy or new tool.

An adapter-supplied `evidence_policy_revision` is checked against inference
admission, closing the window where a source read under an old grant could
otherwise be sent under a newer admitted epoch. Existing live revocation and
final persistence gates remain in force. This binding can only deny a call;
it never authorizes evidence by itself.

## Native input proof

Local execution receipts now optionally include `task_input_sha256`, computed
after native prompt compaction. The fingerprint covers the actual structured
task, including retrieved source and routes, while excluding only the runtime
envelope. Old receipts deserialize unchanged but cannot supply this new proof.
No raw prompt is added to ordinary journal events.

The campaign preserves the synthetic decision packets in the private run
artifact and checks their hashes against the actual native triage receipts and
durable checkpoint calls. Its separate Python verifier reads original source
bodies from the canonical or archived table, checks hashes, run/sender/policy,
accepted predecessor membership and exact packet shape, and rejects added
status/queue answer fields. It also checks the frozen task binding and actual
model reply correlation. A matching hash is input linkage, not a proof of the
model's internal reasoning.

## Verification

Component tests deny wrong recipients/owners/runs, unacknowledged ASTC, forged
source hashes, changed source bodies, nonaccepted predecessor hashes and old
bindings after revoke/regrant. Counterfactual packet tests change the original
status without changing the routes/schema and observe a changed native task
fingerprint. An incorrect forwarded status is absent from the delivered packet.
Epoch tests reject missing-type/negative/stale supplied versions; fingerprints
retain source fields across envelope transformations.

167 enabled Rust tests and 32 focused Python tests pass; strict all-target
Clippy and the locked harness build pass. Seventeen native tests remain separate
opt-ins. Integrated evaluation `hard-a2a9ec3bb15d`, company
`bank-a2a9ec3bb15d`, run `3c6e2364-5503-49b9-9ff8-becfbdb72613`, was launched
with fresh shuffled-layout seed 1352517750666660642, passed ten cases, ten
memory-driven triage decisions, ten memos, ten actual assurance consumers and
twenty DAG checkpoints. It made 96 fresh native calls with no call failures,
cancellations or checkpoint reuses, in 409.57 seconds. It sampled 18,783,136 KiB
peak backend RSS and reported zero inference-provider charges. Hardware,
electricity and coaching are excluded; RSS is not exact GPU allocation.
The actual artifact SHA-256 was read back independently as
`a48a11962d3a8712434e4ca67a25cef7f915bdb5da8464311dc45d3c3c6456fc`.
Its [short paper](../data/capability-campaigns/hard-a2a9ec3bb15d/hard-stage-paper.md)
and adjacent result/snapshot retain the evidence.

## Counterfactual failure and protocol two

The first integrated PASS used protocol one's model-visible provenance fields.
It did not survive every fixed-identity diagnostic. Native probe
`memory-triage-probe-d7431145-4df3-4057-9775-a8edf68e3d13.json` passed 4/5:
matched was routed to data quality. A metadata variant using valid synthetic
hashes, `memory-triage-probe-7d10a2b1-573a-4fb8-b826-b4b418435ecd.json`, also
passed 4/5, but duplicate was routed to reconciliation instead. Both remain
failed evidence. These observations do not establish a general causal account
of the model's mistakes or permit ignoring independent queue validation.

Protocol two separates custody metadata from the model's business input. The
model sees the complete original record and route contract; source hashes,
sender identity and accepted-predecessor bindings remain enforced and recorded
by the runtime rather than repeated as opaque strings in its task. The policy
epoch still reaches deterministic inference admission. No route answer,
selected rule or corrected model reply is supplied. Original source facts stay
in the native task fingerprint and are matched to the independently verified
custody record by the external verifier.

The first source-focused probe
`memory-triage-probe-175200a1-d8f5-429e-8ccf-0e5c0879b485.json` passed 5/5.
The expanded native probe
`memory-triage-probe-f6ec4d24-218f-4dcb-bbe8-b1a665887ae5.json` passed ten actual
local calls: five statuses under the same identity, followed by reverse-order
statuses under a fresh random identity. Both identities retain the same
route contract. The second half changes identity/order, not the five-status
domain; it is not independent broad qualification. Native receipts bind every
actual input fingerprint and report zero provider charges.

Config and cached state now require explicit protocol two. Missing, older or
unknown versions fail closed when governed triage is enabled; unrelated legacy
runs remain unchanged. Protocol-one checkpoints cannot be silently resumed as
protocol two. A fresh integrated evaluation `hard-31531af8e7e2`, seed
2324256897681248695, company `bank-31531af8e7e2`, run
`32964319-2eb7-4a3a-8068-bae43eeb7f21`, passed the ten actual triage decisions
but failed its last assurance consumer. Risk copied `bank-policy` as
`bank_policy` twice. Nine consumers completed; both wrong replies were rejected
and the two-attempt budget was retained. Its 98 replies and failed final status
remain in its snapshot/result. No final acceptance artifact was published.
This does not invalidate the earlier positive-path receipt, but demonstrates
why neither a stage pass nor a small native probe is broad qualification.
The command below targets protocol two, not exact replay of protocol one.

The next repair adds a field-independent literal vocabulary to extraction
decoding: all source string values of the allowed size, not the target field's
value or the requested-field name. Arrays and nested records contribute too.
Only vocabularies with 2 through 128 distinct values are constrained; a singleton
is deliberately not converted into an answer. Unit tests change the requested
field while the schema remains identical and test the singleton fallback.
The model must still select the requested value; independent exact-source
validation remains authoritative. Native and fresh integrated verification
are separate, and cannot revive the failed attempt budget.

Native literal-copy probe
`literal-recall-probe-eba80e17-f0c3-465e-946a-76b0f6e844b9.json` passed three
actual pinned risk-model calls. Two request different fields over the same
source vocabulary; the third changes the original hyphenated identity. The
schema includes every bounded source string, not only the expected value, and
the original native replies are retained. This is three literal-copy checks,
not broader semantic-memory qualification.

A fresh full test passed as `hard-b47513bdc4bf`, seed
4553174152809359061, company `bank-b47513bdc4bf`, run
`1c8a0367-0f6a-463e-b87c-581a67db9fe9`. It has a new frozen task and budgets;
the failed run's attempts were not reset. All ten cases, ten actual triage
decisions, ten memos and ten assurance consumers passed with 96 fresh native
replies, no call failures/cancellations and no checkpoint reuse. Risk read back
`bank-policy` exactly. The independent verifier checked all twenty checkpoint
hashes and source/native-input correlations plus 384 release-program cases.
Elapsed time was 414.89 seconds; sampled peak backend RSS was 18,014,080 KiB,
not exact GPU allocation. Provider charges were zero, excluding coaching,
hardware and electricity. Independently read-back artifact SHA-256:
`3ff7025cfac154912f75e95ab69384d15f5fd2a01a9a58b5169331027f2416c7`.
The [short paper](../data/capability-campaigns/hard-b47513bdc4bf/hard-stage-paper.md)
and adjacent result/snapshot retain the accepted evidence. The later local
HTTP boundary was tested independently; it did not retroactively change this
evaluation binary or the failed prior receipts.

```sh
python3 scripts/enterprise_campaign.py --challenge-layout --semantic-memory --consume-memory --compiled-risk --triage-memory
```

## Limits

This path covers one actual business stage across ten cases; it does not claim
that every financial stage consumes semantic summaries or that memory
compression is lossless. It retains synthetic-only operation, independent
completion and production refusal. The repository is protected from ordinary
autonomous repair, not from a hostile process running as the same OS user.
Encryption, distributed identity, immutable audit export, broader adversarial
qualification and accountable regulatory review remain separate requirements.
