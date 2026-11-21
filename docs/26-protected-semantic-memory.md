# Protected semantic memory

Current status: protocol three additionally forbids issue/action entries unless
the cited source has an explicit corresponding nonempty array field, with
quotes restricted to that field. Fresh protocol-three runs
`hard-022722e6c584` and `hard-40509cb61ed8` passed ten cases through ten models
and ten summary actors each, with 76 new local calls and no failed calls.
The historical protocol-two
run below proves source linkage/storage only. Manual inspection subsequently
found invented or miscategorized next actions, so it is not a semantic-fidelity
pass. Prior protocol memos remain retained but cannot be read as current ones.

## Scope and interfaces

`semantic_memory.rs` provides the private `SqliteMemos` storage adapter,
bounded source projection, an input-bound decoder contract and a local
generation service. `company.rs` remains the source authorization boundary.
`memory.rs` retains raw-event/index scheduling. The model gateway retains local
capacity limits, permission watches and inference receipts. No vendor SDK,
cloud storage, new model download or unrelated UI change is introduced.

Enable explicitly in company configuration:

```json
{"semantic_memory":{"generator":"orchestrator","max_versions_per_actor":3}}
```

The generator must be a configured agent with a native local worker. It runs
with the recipient's source scope, without tools, rather than inheriting the
generator agent's memory permissions. The summary phase does not acknowledge
handoffs, stage its own output into canonical context, or dirty the source
clock. A stable source fingerprint avoids repeat inference on unchanged input.

## Storage and retrieval

Canonical records and complete frozen task contracts remain separate from the
derived cache. Each memo has decisions, unresolved issues and next actions,
with at most three items total. Every item cites an approved source alias and
a checked literal excerpt. Protocol two constrains only the source IDs, actual
quote choices and valid item counts; the model still writes summary text.

At most four whole eligible source records are selected. Oversized records are
omitted, not string-truncated, and the coverage limit is explicit. Memos are
advisory: source linkage and quote membership do not prove semantic entailment
or authorize an action. The original source store and independent completion
checks remain authoritative.

`company::retrieve` and `company::memory_context` expose `semantic_memory` only
after verifying the recipient, task hash, current policy epoch, original source
hashes and quotes. Missing/unavailable derived memory is null. Corrupt current
metadata fails closed. Older protocol results remain stored but are not reused.
Old model epochs and other-run model outputs cannot be repackaged into a fresh
memo merely because the numeric policy version happens to match.

Raw archive reads are not bypassed: memos whose sources move to cold storage
become unavailable until an authenticated owner explicitly recalls those
sources. A source-free actor may summarize the approved definition of done and
exclusions, clearly labeled requirements rather than completed-work evidence.

## Scheduling and failure behavior

Raw events commit immediately. The existing 30-second quiet / 120-second dirty
window schedules index and summary work; inference queueing/completion time is
not guaranteed by that timer. There is one background derived job per active
run per maintenance iteration. Final enterprise acceptance checks all configured
actors. Native generation has no tool loop. Actor version budgets, two attempts
per fingerprint and five-minute leases bound retries. A finalizer can wait a
bounded 45 seconds for an already-owned background job instead of accepting a
missing memo or issuing a duplicate call.

Each native summary job holds a per-recipient kernel file lock through commit.
The non-serializable guard survives wall-clock adjustments and releases on
process death. A new owner invalidates orphan tokens without comparing elapsed
deadlines from a former process or resetting attempt counts. Active deadlines
use `lease_clock::now_ms`; UTC remains scheduling/audit metadata. Lock files
are retained to avoid inode replacement races.

Permission admission is checked before inference, watched during inference,
and rechecked transactionally with source hashes and lease tokens at commit.
Every read checks current permissions again. Revocation does not wait for
summary scheduling. A regrant does not revive an old-policy memo. Summary data
checks run before its reply reaches the run journal. Failures retain canonical
evidence and remain visible; an explicitly required final memo can block task
acceptance rather than silently passing.

## Evidence and remaining limits

The first integrated memory attempt failed its item budget and quote-copy
contract; its result remains FAIL. After the protocol repair, owner-authorized
resume and same-run archive recall, `hard-cd2fbb3e383c` passed all ten cases,
reused ten financial checkpoints and wrote ten accepted memos with ten new local
calls. The corresponding short paper and receipts are in that campaign folder.
Component tests cover disclosure boundaries, immediate revocation/regrant,
invented quotes and IDs, source/metadata tampering, leases, no self-feedback,
protocol versioning and explicit archive recall. A real native test also passed
over four structured synthetic sources.

Only nine integrated memos cite result records; the master memo is requirements
only. These are bounded context summaries, not a lossless history or an
independently proven semantic compression scheme. The native cross-model
consumer test now passes bounded source/advisory injection fixtures and
immediate revocation. Its first mixed-context design failed and is retained.
The repair separates advisory selection from source-only extraction. Larger
fidelity/adversarial evaluations remain open; scoped assurance is documented in
`docs/27-local-agent-assurance.md`. Storage is local private SQLite, not application-level encryption,
WORM retention or regulatory certification. No real PII, PHI or card data is
authorized by this implementation.

## Source-only consumer contract

The owner-authenticated company operation `memory_resolve` accepts `run_id`,
`recipient`, `input_sha256` and `source_alias`. It resolves an alias only from
that recipient's currently validated memo, never a free SQL record ID. An
optional simple `field` yields a focused `consumer_packet` preserving original
source text and correlation metadata, with no advisory prose and no computed
answer. The read/audit is transactional and records metadata without payloads.
This operator API does not authorize a model-supplied actor identity in the
general tool broker. An eventual agent tool adapter must bind the runtime actor.

Native receipt `semantic-memory-probe-421595f5-74de-4215-8f14-622a093844e7.json`
under `data/model-registry` records five real local calls: 8B generation,
4B selection/extraction, then attacked selection/extraction, followed by denied
resolution after revocation. This is actual downstream use, but a component
fixture rather than proof all ten financial stages used semantic memos.

The optional enterprise consumption service now binds the actor from its
configured workflow node and runs isolated selection/extraction phases.
`hard-415cb1d2f001` passed actual source readback by all ten models during
workflow assurance, with twenty native consumer calls and ten extra durable
checkpoints. Its first failed trial is retained. See
[the methods and limits](30-governed-memory-consumption.md). This is not a
general broker/MCP operation accepting a model-supplied recipient, nor proof
every financial decision consumes a summary.
