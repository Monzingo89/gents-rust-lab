# Local agent capability campaign

## Current stage evidence

On 5 November 2026 the owner clarified the final gate as "Synthetic local
research only." The [completion audit](35-goal-completion-audit.md) accepts
the easy/medium/hard research goal in that scope after rechecking retained
artifacts, native receipts and checkpoints without new inference. Production
qualification is excluded, not implied. Later references below to an active
goal or pending scope are historical checkpoints superseded by this decision.

Latest integrated test `hard-b47513bdc4bf` is PASS: ten cases, ten distinct
models, ten memory-driven triage decisions and ten assurance consumers, with
96 fresh replies, no call failures and zero provider charges. The prior
`hard-31531af8e7e2` FAIL remains retained: risk twice normalized `bank-policy`
to `bank_policy`. Field-independent literal vocabulary passed three native
checks before this fresh full run. [Failure, repair and current result](32-memory-driven-workflow.md).
The stage table records finite workflow evidence, not blanket model or
enterprise qualification.

The finite local prototype stages have passing evidence. These are synthetic
workflow results, not frontier equivalence or regulated-production approval.

| Stage | Verified result | Methods paper |
| --- | --- | --- |
| Easy | One Qwen3-8B agent, governed file write/read, independently verified READY artifact; eight actual replies | [Easy paper](../data/companies/local-smoke-ae6f526ea1/runs/de7cf8f1-d168-48ab-a6d1-1e23595e8444/easy-stage-paper.md) |
| Medium | Two planner/worker pairs, 10/10 unseen cases, 12 local replies and 11,609 ms actual inference overlap; evaluated procedure tuning, not weight training | [Medium paper](../data/capability-campaigns/medium-b2ea9bc741e8/medium-stage-paper.md) |
| Hard | Ten distinct pinned models, 10/10 synthetic cases, ten memory-driven decisions, ten protected memos/consumers, 96 fresh replies, 384 checks of the model-authored release program | [Latest hard paper](../data/capability-campaigns/hard-b47513bdc4bf/hard-stage-paper.md) |

All three artifacts' saved hashes were read back and matched on 5 November
2026. The latest hard run had no failed/cancelled calls, no reused checkpoints
and USD 0 inference-provider charges. Its risk-policy decomposition applies
the papers' narrow responsibilities and independent authority instead of
relying on repeated raw judgments. [Methods and limits](31-local-policy-compilation.md).
General model qualification and regulated deployment remain open; the overall
goal is not a claim of blanket compliance. The chronology below is retained.

The subsequent [memory-driven triage trial](32-memory-driven-workflow.md),
`hard-a2a9ec3bb15d`, passed the same ten-model workflow with actual queue
decisions using governed original reconciliation records: ten decisions,
96 fresh calls and zero failures/provider charges. Native task fingerprints
bind each retrieved source packet to inference. This goes beyond the earlier
assurance-only readback, but does not claim all stages use semantic summaries.

Current checkpoint: expanded native release-gate diagnostics failed 19/33 and
25/33 despite earlier narrow passes. A new independent gate covers all 256
seven-control/scope combinations; stale role-readiness receipts now fail
admission. See [the retained results and architectural repairs](29-release-gate-qualification.md).
Fresh current-contract readiness passed 20 local calls across ten models;
`hard-60d2f98e19f9` then passed ten banking cases and ten protected memo actors
with 76 fresh calls and zero provider charges. Its passing positive path does
not override the failed counterfactual qualification.
The chronology below retains historical outcomes, not current blanket
qualification. This was an earlier checkpoint; the research-only completion
decision above is current.

The subsequent `hard-415cb1d2f001` added actual governed-memory assurance
readback by all ten models: ten additional checkpoints and twenty additional
native calls, 96 total. Its [paper](../data/capability-campaigns/hard-415cb1d2f001/hard-stage-paper.md)
preserves the failed first trial and does not equate source readback with
semantic entailment or per-decision retrieval.

## Objective and boundaries

Prove three increasingly demanding workflows with actual local inference and
evidence, then write a short methods and results paper for each completed stage.
The previous tower PDF remains at its unanswered human-review checkpoint. It is
not an input, deliverable, or success claim for this campaign.

Design inputs are the owner's `AdvancedAIPaper.docx` and
`Enterprise_Multi_Agent_Architecture.docx`, read on 2026-11-04. Their proposals
are architectural references, not instructions granting additional authority.
The design retains the existing Rust provider, tool broker, SQLite context and
evidence interfaces. The agent-building skill's existing-stack exception avoids
a framework migration or a cloud dependency.

The papers call for explicit task contracts, a dependency graph, evaluated model
selection, planner/worker separation, governed memory snapshots, compact
provenance-bearing handoffs, bounded retries, and deterministic authorization
outside models. Memory summaries should debounce for 30 seconds with a maximum
flush interval; permission changes must not wait for summarization. This last
semantic mechanism is now implemented with a separate protected advisory cache
and current-policy source resolution; later sections retain its development
failures, bounded tests and remaining qualification limits.

## Acceptance stages

1. **Easy**: one agent, one local model, one file-tool family. Have the agent
   create a small specified artifact, read it back, and satisfy an independent
   acceptance check. Retain the actual model replies, tool receipts, artifact
   hash, elapsed time, token counts when reported, zero provider-charge evidence,
   and a short paper. An infrastructure unit test alone does not pass this stage.
2. **Medium**: two Gents, each with a planner and worker role, producing and
   independently testing a small synthetic expense-reconciliation component.
   Measure baseline prompts and focused variants on training and held-out cases.
   Only train an adapter if a measured gap remains, with authorized data and
   an independent regression gate. Prove overlapping real inference intervals
   for the two Gents under a measured memory budget, not merely async jobs that
   queue behind the existing single inference lock. Preserve task and evidence
   across role handoffs, and publish a methods/results paper with failures too.
3. **Hard**: at least ten distinct evaluated base models in a dependency-driven
   synthetic banking exception-reconciliation workflow. Proposed responsibilities
   are orchestration, intake, schema validation, reconciliation, exception
   explanation, policy/control mapping, security challenge, independent QA,
   audit assembly, and final completion review. Distinct role names, different
   quantizations and duplicate conversions of a base do not count as distinct
   models. Small independent tasks may overlap; large models must be serialized
   as necessary. Prove denial of forbidden calls and cross-tenant/agent memory
   reads, revocation, bounded retries, restart recovery, audit correlation,
   artifact integrity, and a complete evidence report and methods paper.

Stage three uses generated account tokens and synthetic amounts only. No real
customer identifiers, credentials, card data, health records, financial advice,
money movements, external email or production release. A responsive review UI
is optional and cannot substitute for workflow controls. No blanket assertion
of legal compliance, absence of bias or equivalence to frontier models is an
acceptance criterion; these require scoped evaluations and qualified review.

## First implemented governance increment

An authenticated owner installs a versioned policy through the existing company
API using `op=governance_install`, a run UUID, `expected_revision`, and `policy`:

```json
{
  "revision": 1,
  "master": "Engineer Gent",
  "ceilings": {
    "Engineer Gent": {
      "tools": ["files:write", "files:read", "finish"],
      "skills": [],
      "memory_from": []
    }
  },
  "grants": {
    "Engineer Gent": {
      "tools": ["files:write", "files:read", "finish"],
      "skills": [],
      "memory_from": []
    }
  }
}
```

Initial installation requires revision one and expected revision zero. Further
owner updates increment once and compare the stored revision. An agent with the
runtime master identity and `access_delegate` capability can replace a grant
only within the target's installed ceiling. Empty grants revoke capabilities,
skills and shared-memory sources. Model-supplied actor identities are not used.
Every tool action re-reads the policy, including after a slow model review.
Decoder narrowing and filtered skill/capability manifests help small models
focus, but never replace deterministic execution authorization.

Memory authorization filters database queries before returning bodies. Explicit
sender grants control promotion and retrieval even after memory becomes PSTC;
revocation is therefore not dependent on moving or deleting records. Ranked
search and fallback browsing apply the same filter. Own run-local ASTC remains
available to its agent. Archived/other-run context still requires the separate
owner recall mechanism. Run-wide task contracts and verified tool evidence
remain shared within the approved project; these are not compartmentalized
secrets. Do not put a secret in the shared task or tool journal.

Policies are opt-in per run; old company runs retain legacy behavior. Governed
campaign calls reject remote providers. Storage remains private SQLite under
the existing company owner boundary, not application-level encryption or a
retention-locked audit service. Same-user host processes are outside this
application authorization boundary. Production use needs separate OS/service
identities, encryption/key management and a reviewed deployment threat model.

## Remaining architecture work

- Capacity-aware inference lanes with cancellation, isolated caches, exclusive
  training admission, and memory-pressure admission checks. An opt-in two-lane
  adapter is now implemented for GGUF files no larger than 3,000,000,000 bytes,
  with a conservative host-memory startup budget. Large models, training and
  vision helper operations retain exclusive admission. This is not a hard OS
  memory limit or live pressure governor; medium acceptance still requires
  correct held-out outputs as well as measured inference overlap.
- The persisted read-only DAG store is now integrated with the medium adapter
  (see below). A ten-model enterprise controller, end-to-end process-crash
  recovery and handling of uncertain external effects remain to be verified.
- Semantic summaries and broader per-group workflow retrieval. Versioned,
  permission-aware reference indexes and bounded flush scheduling are now
  implemented and tested, but are not semantic model-written summaries.
- A reviewed global/domain skill registry behind an interface; future MCP
  transport must retain runtime identity and resource-level authorization.
  Initial security, finance, HR and IT procedures are now versioned in the
  shared catalog, explicitly assignable to planner or worker phases. Department
  procedures are excluded from unassigned old jobs. MCP transport is pending.
- Model identity/license/provenance inventory and measured role evaluations.
  Only nine apparent base-model directories were present at the initial
  inventory; none are counted as ten distinct evaluated models yet.
- Scoped security/control mapping, negative tests and independent release review.

## References

- [Owner architecture paper](../Enterprise_Multi_Agent_Architecture.docx)
- [Owner research proposal](../AdvancedAIPaper.docx)
- [Manning publisher overview for AI Agents in Action](https://www.manning.com/books/ai-agents-in-action):
  orchestration, tool use and retrieval/memory topics. Only the public publisher
  material has been consulted, not an unavailable complete book.
- [Official MCP security guidance](https://modelcontextprotocol.io/docs/2025-11-25/tutorials/security/security_best_practices):
  transport security, scope minimization, token passthrough and confused-deputy
  risks. Connecting an MCP server does not grant permission to every tool.

## Initial observation

The first real Qwen3-8B file-task attempt reached the 180-second test deadline
without completion. It tried to read a not-yet-created file twice, then broad
resource/journal recovery replaced progress. This is a failed baseline, not a
completed easy stage. The next trial uses a governed, narrowed tool manifest and
decoder, preserves the actual acceptance check, and permits a bounded 600-second
local run. No tower instructions or subject facts are supplied by the assistant.

## Easy stage verified result

The narrowed second trial still repeatedly read the missing file and stopped
after bounded recovery. Inspection identified a decoder-order hypothesis:
serde_json's sorted schema maps put `arguments` before the action discriminator
and write `content` before `op`. The llama.cpp grammar consumes property order.
`contracts::InferenceWire` now serializes inference schemas in their declared
required-field order, without changing canonical approvals or journal hashes.
The schema's permitted values and independent authorization are unchanged.

The next real trial passed on 2026-11-04 in 46.56 seconds with eight Qwen3-8B
replies, two successful file-tool operations (write then exact READY readback),
independent QA and an external byte-for-byte artifact check. Provider charges
were USD 0.0, excluding hardware and electricity. Artifact SHA-256:
`c2e3ac47f4a325469c1a2d5f117e463ec943c721986d5d9f09ac4540b7d80526`.

Receipts and the completed stage paper are under
`data/companies/local-smoke-ae6f526ea1/runs/de7cf8f1-d168-48ab-a6d1-1e23595e8444/`:
`easy-stage-result.json`, `easy-stage-paper.md`, and numbered tool journals.
This is evidence for the easy task only, not a general model capability claim.
This initial easy-stage result did not establish either later stage.

## Medium stage verified result

The medium stage passed on 2026-11-04 using two Qwen3.5-4B planner/worker
pairs. Batch arithmetic and direct final-status decisions failed earlier
training trials, including attempts with Qwen3-8B. Case-scoped turns and exact
arithmetic improved source extraction, but did not eliminate inconsistent
status decisions. No failed trial was treated as accepted work.

The accepted architecture has local planners author a five-rule declarative
implementation, workers extract observable facts from original input records,
an authorized exact-integer capability produce arithmetic evidence, and a
bounded interpreter execute the model-authored rules. The reference checker
never supplies rules or repairs answers. A unit test explicitly demonstrates
that a wrong model-authored mapping remains wrong for independent rejection.

The selected workflow passed ten training and ten unseen held-out cases.
The held-out run used 12 real local model replies, took 36.36 seconds, showed
11,623 ms maximum overlapping request intervals on distinct owned backend
processes and ports, and sampled 7,624,864 KiB peak combined backend RSS.
Provider charges were USD 0.0, excluding electricity and hardware. RSS is not
an exact measurement of all unified GPU allocations, and HTTP overlap is not
a claim that every GPU kernel executes simultaneously.

Results and the short paper are retained under
`data/capability-campaigns/medium-ab1d154ce548/`.
The held-out run is company `pair-7b8d40262dd8`, run
`6fbb9b16-7375-48a9-aa1b-cf38e79d8951`. Its evidence SHA-256 is
`857ff6863c929ca01ce94561c88ac1f7e33ca0ca17f957f58e3e1d28b9ca674a`.
Training selected the workflow before the held-out trial. Repeated training
iterations are exploratory, not evidence of general frontier-model equivalence.

Enable the pool only in a reviewed local runtime with
`GENTS_LOCAL_SMALL_LANES=2`. Default execution remains serial. Reproduce the
selected trial with `python3 scripts/paired_campaign.py --candidate Qwen3.5-4B
--variant compiled_policy` after building the harness. No adapter was trained
or promoted; capability/prompt/workload tuning was sufficient for these cases.

The ten-distinct-model enterprise workflow, semantic memory summaries,
governed MCP transport, broader adversarial/security controls and its final
paper remain incomplete. Neither completed local stage establishes regulatory
certification or suitability for real banking/health data.

## Hard stage preparation

The offline inventory now streams actual weight SHA-256 hashes and reads bounded
GGUF metadata without loading weights or executing repository code. It excludes
projector files and groups Phi-1's original/MLX conversion as one candidate base.
It reports candidates separately from admitted models; embedded metadata and a
downloaded file do not grant model admission. Planner/worker agents can request
the read-only `models_inventory` capability through their existing policy grants.

Three small native GGUF candidates were downloaded at fixed revisions: Qwen2.5
0.5B Instruct, Qwen2.5 1.5B Instruct and SmolLM2 1.7B Instruct. Source model cards,
license documentation and actual file checksums were checked. Their exact
revisions, hashes and remaining admission limits are in
`config/enterprise-model-candidates.json`. Seven preexisting GGUF candidates plus
these three provide a possible ten-base roster, not a completed ten-model run.

Each new candidate passed two real local structured policy-comprehension probes
with zero provider charges. Receipts are under `data/model-registry/probes/`.
These are narrow compatibility/comprehension checks, not enterprise role tests
or proof that a model enforces security. Code and destination systems remain
responsible for authorization.

Native model configuration now accepts an optional `context_tokens` budget
between 2048 and 16384. It is enforced in llama-server startup and native
tokenizer preflight, reserving output tokens. Changing it discards the owned
inference cache, not durable evidence. The new candidates were probed at 8192;
SmolLM2's documented limit is 8192. Defaults remain unchanged for existing jobs.
The value is operator/registry configured, not guessed from a display name.

Qwen3.5-4B also passed a real master delegation/revocation probe. It authored two
`access_delegate` requests: first assigning an Inspector a read capability,
finance skill and permitted memory sender, then replacing that grant with empty
lists. The deterministic service checked the authenticated master, current
revision and immutable owner ceiling before each transaction. Final revision
three denied the Inspector's former file-read operation. The isolated company
and full actual replies/receipts remain in
`data/model-registry/master-delegation-probe.json`. This proves bounded local
configuration, not a model's right to expand the owner's authorization.

The remaining hard-stage gates are publisher/role admission of the complete
roster, a dependency-driven ten-model workflow,
broader recovery/adversarial tests,
scoped control evidence and the final short paper. No production or real
regulated-data authorization is implied by the preparation work.

## Memory implementation and limits

Canonical context and a dirty-generation marker commit in the same SQLite
transaction. Reference-index versions flush after 30 seconds of quiet or at
most 120 seconds after the first dirty write, subject to normal host scheduling.
The event-driven housekeeping loop calls no model or paid API. Each projection
binds company, run, recipient, policy revision and frozen-contract hash; source
IDs, tiers and hashes are retained without copying advisory model text. Derived
indexes keep 64 versions per recipient; canonical context and audit history are
not removed by this cache retention. Completed runs force a final checkpoint
before canonical records move into the non-indexed private cold archive.

Reads still query live authorized source bodies, so debounce cannot delay access
revocation. Permission changes force a fresh projection and inference-cache
identity. Process-local permission notifications cancel queued/in-flight model
futures; post-call checks and transactional policy comparison reject stale
outputs before persistence. Notifications are not a distributed event bus.

Tests cover debounce/max-flush clocks, durable queue state, tampered indexes,
immediate revocation, stale-output persistence rejection and final pre-archive
capture. This is a bounded relevance/recent source index, not a complete semantic
summary or an information-flow system: previously authorized derived facts are
not transitively tainted and erased. SQLite file privacy is not encryption,
WORM audit retention, regulatory certification or protection from same-user host
processes.

An opt-in native test also passed two real local requests under distinct policy
epochs with different owned backend PIDs and zero provider charges. Receipt:
`data/model-registry/policy-epoch-probe.json`. This proves cache replacement,
not an end-to-end in-flight revocation integration test.

## Post-memory regression evaluation

Historical medium acceptance remains recorded above, but does not imply every
subsequent revision passes. Campaign `medium-94283f064510` passed all ten training
cases, then failed held-out extraction by repeatedly inventing a duplicate ID.
The source verifier rejected it after three attempts. A Qwen2.5-1.5B trial
`medium-05d39c490f59` also failed the boolean-extraction task. Neither failure is
reported as completed work.

Revision-two campaign `medium-40503aabd0f0` also rejected Qwen2.5-1.5B: it
invented source columns for an empty case, and its planner authored incorrect
rules. This candidate is not admitted to either role by these trials.

Compiled-policy workflow revision two separates responsibilities further:
workers copy ordered payment IDs/currencies, generic bounded code calculates
column predicates, the exact integer capability supplies amounts, and local
planner-authored rules determine statuses. Extraction and complete final output
still face independent verification; incorrect copied columns or planner rules
are never replaced with expected answers. This is a deliberately constrained
tool-assisted workflow, not free-form financial reasoning. New campaigns use a
fresh recorded held-out seed generated after training selection, rather than
reusing the fixed cases that informed earlier architecture repairs. The process
remains exploratory, not a blind comparative benchmark.

Revision-two Qwen3.5-4B campaign `medium-e19974eea61d` passed all ten training
cases and ten freshly seeded held-out cases with no rejected observations.
Held-out execution used 12 actual replies, took 30.18 seconds and measured
10,673 ms overlapping request intervals on separate owned backends. Peak sampled
combined backend RSS was 8,248,800 KiB; provider charges were USD 0.0, excluding
hardware/electricity. Paper and complete reports:
`data/capability-campaigns/medium-e19974eea61d/medium-stage-paper.md` and
`medium-stage-result.json`. Held-out company `pair-9746fb702fb3`, run
`435b5570-c258-47db-8b25-ce440c60186d`, evidence SHA-256
`d9edf7035f6176fd4a463ba9ffa2217261f3b87a4b0db9e761aeba7d240db198`.
This is a measured small-task result, not evidence that enterprise admission or
all future runs will pass. Failed approaches and their traces remain intact.

## Durable dependency execution

`workflow::WorkflowRepository` separates scheduling state from model execution.
Its SQLite adapter stores an immutable, task-digest-bound graph with up to 64
nodes, weight fingerprints, runtime actor identities, dependencies, bounded
attempts and expiring leases. Only read-only local computation is supported;
automatic replay of email, transfers or other uncertain external effects is
not implemented or authorized by this store. The existing medium adapter now
uses four nodes, one planner and worker checkpoint per Gent. The weight hashes
bind local artifacts, not publisher admission or ten distinct base identities.

Claim, input access and commit each re-read live governance. Direct and
transitive upstream actor permissions are checked before dependency outputs
are returned, including cache hits. Source-output hashes bind each lease;
expired/replaced leases, changed policies, altered source checkpoints and
stale failures cannot commit over a newer attempt. Failed call traces remain
advisory evidence, not successful DAG outputs. Reopening a store reuses original
completed replies rather than manufacturing new model calls. Every transition
and cache reuse is durably correlated with the run, stage and attempt. Runtime
stage events also enter the existing observability stream.

Six new pure tests cover immutable graphs, cycles, dependency readiness,
identity/weight mismatch, competing claims, reopen/cache reuse, expiry/retry
limits, transitive access, revocation, tampering, tenant/run isolation and
sensitive-output rejection. These use explicitly labeled fixtures; they are
not native inference or a full application process-crash test. Graph-lineage
checks do not track arbitrary information flow in legacy context outside DAGs.
Checkpoint completion is never financial acceptance; independent verification
still checks the actual outputs.

Real campaign `medium-88d4e6d0292c` passed ten training and ten fresh held-out
cases, and an external SQLite reader verified all four checkpoint hashes.
Weight hashing initially added substantial debug-build overhead. Optimizing
only the SHA-256 dependency retained actual streaming checks and brought the
next campaign `medium-b2ea9bc741e8` back to 35.76 seconds training and 36.37
seconds held out. These timings are observational, not a controlled benchmark.
The second campaign again passed all twenty cases with four durable checkpoints
per run, zero rejected observations and zero provider charges. Held-out overlap
was 11,609 ms and sampled combined backend RSS was 8,194,400 KiB. Paper:
`data/capability-campaigns/medium-b2ea9bc741e8/medium-stage-paper.md`.
Evidence SHA-256:
`216dc1729e4229fdbe56314116fddee0fee907c7e0e42f8893ef6e725c4781da`.

A read-only Hugging Face checksum check also matched the existing Qwen3-8B
weight at the publisher's pinned revision. Partial provenance details are in
`config/enterprise-model-candidates.json`; they do not replace license review,
role qualification or the remaining ten-model workflow gates.

## Ten-model artifact and role-readiness preparation

`config/enterprise-model-roster.json` now separates ten pinned publisher model
identities from their proposed roles. `scripts/model_provenance.py` streams
actual local weight hashes and compares exact filenames, sizes and LFS hashes
at immutable remote revisions. Small publisher/quantizer cards and license
documents are retained under revision-specific private directories, without
executing repository code or downloading other weights. Reports retain the
declared license and source links, not legal approval. Older reports are
archived before replacement. Requests do not inherit cloud keys or send an
implicit Hugging Face token. All ten initial artifacts passed; the current
roster is evolving based on measured role failures, not on directory names.

`scripts/role_probe_campaign.py` runs owned native backends serially with no
cloud credentials. Each role receives two counterfactual banking-workflow
readiness cases, with exact external expected outputs excluded from model
inputs and decoder schemas. Original replies, usage, model fingerprints and
actual backend execution receipts are retained. Campaign manifests snapshot the
roster and protocol. These are exploratory readiness checks, not held-out role
qualification or the hard-stage workflow itself.

Initial campaign `4da1bbeb8114403da48879a73e16f061` passed six roles and rejected
four. The smallest models mishandled missing fields, source classification and
exception routing. The audit model omitted an otherwise valid receipt when its
body contained an injection attempt. Reference assembly now gets IDs and full
source hashes rather than unnecessary source bodies; the attacked receipt is
retained, and the original acceptance output remains unchanged. Retest
`64ef5cab2eb84183bbb9041a8fddfb10` passed both audit cases. This is data
minimization, not a claim that arbitrary prompt injection has been solved.

Qwen2.5-7B Instruct and Qwen3-4B replaced failed tiny candidates and passed their
classification/triage checks. Campaign `1bfaeaa88ee34372afb54769f77b5abe` passed
nine of ten roles. Qwen3-1.7B still failed field presence, including a later
inventory-assisted test (`5000bf98f7044d948c3570d3a6a6c6ed`). A code-derived
inventory distinguishes absent keys from empty-valued keys without supplying
the missing-field answer. The 1.7B model is not admitted. The roster now
proposes Qwen3-14B for that role, pending its own provenance/readiness receipts.

The download choices stay within the declared Apache-2.0/MIT evaluation filter;
an inspected Qwen2.5-3B candidate declared a different license family and was
not selected. This filter is not a substitute for scoped license/legal review.
Native schemas, source views, the roster and evaluation scripts are protected
from agent `source_patch`, preventing a model from editing its own acceptance
fixtures or authorization boundary. The ten-model dependency execution,
broader role qualification, restart/revocation integration, security/control
evidence and final hard-stage paper remain unfinished.

Qwen3-14B subsequently passed both inventory-assisted schema cases in campaign
`cc660b4505334c7d9c81c404d8e9cd62`, after its new pinned artifact was checked.
The expected outputs were unchanged. All ten current models now have basic
readiness passes across the retained runs. `role_probe_campaign.py --summarize`
independently rechecks exact role/model manifest matches, original source hashes,
actual reply contents, local execution receipts and zero provider charges. It
indexes the latest matching result, including a failure; it does not fall back
to a more convenient historical pass. The index explicitly keeps enterprise
qualification at zero. These results justify the next synthetic workflow
experiment, not production use or a claim that the hard stage is complete.

## Integrated ten-model banking pilot

`banking.rs` supplies the synthetic-domain contract, source-view builders,
bounded rule interpreter and independent checks. `enterprise.rs` binds the
ten pinned models to owner-governed actors, durable dependencies and actual
local execution. Only the enterprise profile permits ten configured actors;
legacy limits remain unchanged. Models run serially, and the generic tool loop,
remote providers and ambiguous external effects are excluded from this profile.
The local master actually assigns an integer capability and finance procedure;
the owner ceiling and live policy still determine authority.

Initial integration failures are retained: `hard-61a8b69da416` and
`hard-0a53011657fb` failed classification. Bare role protocols expected root
task fields, whereas the harness nested them. A scoped version-three input
adapter now preserves root fields and rejects reserved-metadata collisions.
Its three-case native diagnostic passed; the integrated classifier subsequently
passed all cases. `hard-32363e719fdb` reached audit but failed required ordering.
Canonical source presentation now preserves all IDs, duplicates and source
hashes without repairing the model's answer or weakening the checker.

Run `hard-80a2f3cdfc7e` completed all ten model stages but hit a filename versus
directory validator error during report publication. After a path-construction
repair, the authenticated owner resume route verified the original approval and
frozen contract, restarted the backend and reused all ten completed checkpoints.
The result passed with 66 retained original calls and zero new calls. Ten private
memory indexes persisted; 66 context records moved to the cold archive on
completion. Live negative checks covered cross-owner/tenant reads, ungranted
memory and writes, and ceiling expansion. QA rejected an altered amount.

Fresh shuffled-layout run `hard-ae754c5258ee` then passed all ten cases with all
ten models, 66 new calls, no reused checkpoints and ten actor memory indexes.
External Python verification checked actual result values, checkpoint hashes,
audit memberships/source hashes, QA rejection, delegated tools/skills and zero
provider charges. The short methods/results paper is saved at
`data/capability-campaigns/hard-80a2f3cdfc7e/hard-stage-paper.md`.

These are verified integrated pilot results. Mid-inference process-crash
recovery, a live in-flight revocation check, broader adversarial/role
qualification and the remaining scoped-assurance evidence are still pending.
Semantic model-written memory summaries, distributed notifications, encryption,
WORM audit retention and regulatory certification are not implied by the pilot.

## Actual process-crash recovery and QA regression

An OS file lock now fences each company/run controller independently of its
run-store path. Only the exclusive trusted controller can invalidate orphaned
tokens for an unchanged approved read-only graph. Recovery preserves completed
outputs, enforces the original attempt ceiling, and does not replay external
effects. Startup uses the original approval and preserves company-journal
cancellations even if the UI/run store was stale. Recovery tests cover
exclusive ownership, wrong-owner/run rejection, stale tokens, exhausted budgets,
original-contract checks and terminal-state preservation.

`python3 scripts/enterprise_campaign.py --challenge-layout --crash-stage schema`
created `hard-027984278068`, seed `6749117949563070164`, company
`bank-027984278068`, run `a611f730-a1b6-4d30-bf5f-283276a16bdb`.
The test observed a fresh owned native child with an unresolved schema call,
killed only its own process group, and restarted the same config/run store.
The schema stage completed on attempt two. Master and intake hashes were
unchanged and their checkpoints reused. Original events were preserved and
the interrupted call never became a completed receipt. This proves a crash
during native loading/generation, not specifically during token streaming.
`crash-recovery-result.json` separately records `PASS_RECOVERY`.

The full run remains FAIL: QA repeatedly misjudged a valid empty-payment case
with zero observed cents and negative difference. It stopped after three
attempts, preserving seven completed stages. No acceptance checker or attempt
budget was weakened. A field-aligned source view retains original receipt and
candidate values without supplying equality results, mismatch names or verdicts.
In a three-case native diagnostic the existing QA 8B model passed two cases but
missed a sign-change mismatch. Changing decoder field order did not fix it and
was reverted. The installed master 27B model passed all three targeted cases.
These are exploratory diagnostic results, not completed role requalification.
The authoritative roster has not yet been swapped; the next step is testing
whether the 8B model can take the easier delegation role and the 27B model can
take QA, then requalifying both roles and rerunning the failed dataset.

The overall hard stage remains open. Live in-flight revocation, semantic
summaries and broader assurance tests are still pending. No new weights were
downloaded and all inference in this experiment was local.

## Evaluated role reassignment and live revocation pass

The authoritative evaluation roster now assigns Qwen3-8B to bounded master
delegation and Qwen3.8-27B to independent QA. Existing pinned artifact provenance
is reused only after offline weight/card rehashing; immutable source receipts
are retained and no role qualification is transferred with an artifact.
`model_provenance.py --reuse-verified` implements this distinction. Five unit
tests cover the provenance path, including tampered cards/weights and no network
access during offline reassignment. Both new assignments passed two fresh
counterfactual readiness cases in role campaign `949467df67734e8e84241e0bf5321572`.
The aggregate index again has ten ready, distinct publishers and zero claimed
enterprise-qualified models.

An eight-case QA diagnostic exposed a missed absent key even in the larger
model. The QA view now includes actual candidate/receipt key inventories, along
with the original objects and aligned values. It supplies neither an equality
answer nor a mismatched-field list. Exact-type/zero/sign guidance and the new
view passed all eight diagnostics. The deterministic verifier separately now
distinguishes absent fields from present nulls. Diagnostic tuning is exploratory,
not a blind accuracy estimate.

`hard-459f9c1c9b9f`, company `bank-459f9c1c9b9f`, run
`4780fcd8-b0d6-4b5c-9d53-6469910a63ce`, repeats the exact failed seed and layout
from `hard-027984278068`. It passed all ten cases through ten distinct models.
During an unresolved call with a fresh native schema backend, the test owner
revoked only its memory sources (policy two to three). The permission watch
cancelled that call and stopped the old backend. SQL checks found no schema
output in hot/archive memory and no schema checkpoint. The test checked there
was no concurrent policy edit, restored only its own grant at policy four, then
used the authenticated original-task resume route. Master and intake hashes
were preserved; all subsequent schema output records were bound to policy four.

The full run records 67 started calls, 66 completed replies and one cancelled /
failed attempt, 27,083 reported input and 1,818 output tokens for completed
replies, ten actor memory indexes and USD 0 provider charges. Aborted-call token
usage is not included; electricity, hardware and coaching are also excluded.
Elapsed time was 434.83 seconds, sampled peak backend RSS 19,482,752 KiB (not an
exact GPU peak). Banking artifact SHA-256:
`c427080847fe44bf1a0f6315a887956f899991774dd59f709d6a77154073dc64`.
The short paper is `hard-459f9c1c9b9f/hard-stage-paper.md` under the campaigns
directory. All owned evaluation processes were stopped after verification.

These results close one integrated live-revocation scenario and the observed
QA regression, not the overall goal. Semantic memory summaries, broader held-out
and adversarial coverage, and remaining scoped assurance evidence are open.

## Protected semantic-memory increment

`semantic_memory.rs` now separates source authorization, derived storage,
bounded local generation and retrieval through `SummaryRepository`. Canonical
context is not replaced by summaries. Explicit configuration selects an existing
local generator and per-actor version budget. Summary inference has no tools,
uses recipient-scoped sources, and neither stages itself as canonical model
context nor dirties its source clock. Fingerprints, two attempts and bounded
leases prevent feedback loops or unbounded retries. Both generation admission
and transactional commit check the policy epoch; every retrieval rechecks
sources and permissions immediately. Older model epochs cannot be laundered
into fresh memos. Archive recall remains explicit and owner-authorized.

The first `hard-cd2fbb3e383c` attempt completed the financial stages but failed
summary acceptance, retaining its original FAIL report. Protocol two fixed a
prompt/budget mismatch and escaped-quote copying by constraining actual source
IDs/excerpts and all ten valid one-to-three-item shapes. Summary text remains
model-written. Prototype replies were retained, not repaired into accepted
outputs. Unit tests cover source/quote forgery, metadata/task tampering,
immediate revocation and regrant, version invalidation, leases, no self-feedback,
and explicit cold-source recall. A native four-structured-source test passed.

Authenticated resume with explicit same-run evidence recall passed on company
`bank-cd2fbb3e383c`, run `8be2b255-2b36-4a37-9a06-6d3e7f63cdcb`. It reused ten
financial checkpoints and made ten new local calls for ten accepted memos.
Nine cite actual result records; the master's memo cites approved requirements
only because its proposal came from the preceding policy epoch. New generation
reported 18,195 input / 1,150 output tokens and USD 0 provider charges. Resume
took 233.83 seconds with 6,330,688 KiB sampled peak RSS, not an exact GPU peak.
The preserved journal includes 66 original financial replies, three rejected
prototype-summary replies and ten accepted new summary replies. Source hashes,
native receipts, quote membership, actor coverage and all case results passed
independent Python verification. Banking artifact SHA-256:
`b4dfef99c6c4b26d5d4c424aba512fb8c62d1d57617d96ad04eef0321ee4d4de`.

Implementation details are in `docs/26-protected-semantic-memory.md`; the short
paper is in that campaign folder. These memos cover at most four source records
and three items and are not semantic-entailment proofs or a lossless history.
Broader memory-fidelity / downstream-model consumption tests, held-out security
evaluation and scoped enterprise assurance remain open. No regulatory
certification, encryption or retention-lock claim is added.

Manual inspection then found that protocol-two summaries invented or
miscategorized next actions despite valid citations. Therefore that PASS is
source-linkage/storage evidence only, not a semantic-fidelity pass. Protocol
three now permits issue/action categories only when the cited source supplies
the corresponding explicit nonempty array, and restricts quotes to that field.
Prior protocol results are retained but cannot be retrieved as current memos.
Component tests include both denial of invented actions and acceptance of
explicit source issues/actions. The native four-record test passed again.

A fresh protocol-three integrated run is started, not yet accepted:
`hard-3baea70a697a`, seed `6701287396732063604`, company
`bank-3baea70a697a`, run `a5440ce6-f402-4a91-a7a3-14bcc945eb8a`.
Its original task/criteria explicitly require source-grounded memory categories.
The evaluator records its live handle and will verify the terminal result;
do not restart it merely because observation has timed out. As of launch,
147 enabled Rust tests and strict Clippy pass; 13 native opt-ins remain separate.

## Protocol three and downstream retrieval verified

The `hard-3baea70a697a` run failed a UTC lease after a measured wall/elapsed
time discontinuity. Its authenticated resume later exhausted risk retries
because the model changed a transport ID. Neither failure was erased. Workflow
leases now use monotonic time under a kernel-owned controller epoch. Only
input-derived correlation IDs are decoder-bound; business verdicts remain
model-generated and independently checked. A failed-QA counterfactual exposed
a second risk-model error; an explicit view of each actual boolean repaired
that diagnostic without supplying the release answer.

Regression `hard-022722e6c584` passed ten cases, ten models and ten protected
protocol-three summary actors, with 76 new local calls and no failures. The
fresh seed in `hard-40509cb61ed8` also passed the same counts without retries,
cancellations or checkpoint reuse. It took 349.13 seconds with 19,644,928 KiB
sampled peak backend RSS and USD 0 provider charges, excluding coaching,
equipment and electricity. Its adjacent short paper and independent result
file document method, artifact hash and scope.

Summary jobs now use per-recipient kernel ownership and elapsed deadlines;
orphan recovery preserves attempts. A separate five-call native fixture proved
8B summary generation and 4B selection/extraction, including source/advisory
injection and immediate revocation. The first mixed-context consumer failed;
the repair removes advisory prose from worker extraction packets. The
transactional owner API resolves only a current memo alias, not arbitrary IDs.
This does not assert that all financial stages consumed semantic summaries.

The post-repair suite passes 154 enabled Rust tests and strict Clippy; fourteen
native opt-ins remain separate. Focused Python suites passed 10 campaign,
23 company-tool and 11 model inventory/provenance tests. Two locked TLS
dependencies were patched after actual RustSec findings; the rescan found zero
known vulnerabilities and no warnings against that database. See
[scoped assurance](27-local-agent-assurance.md), not a compliance certificate.
Broader model qualification and regulated-production controls remain open.
