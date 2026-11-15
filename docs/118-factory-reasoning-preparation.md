# Factory preparation and typed reasoning evidence

This is the next integration step after [the factory preview](115-company-factory-preview.md) and [the distinct coordination programs](116-local-coordination-engine.md). The owner can now prepare a repository-bound reasoning program from a retained ready CEO plan and freeze it through the ordinary intake/approval/governance path. The complete task-selected factory milestone remains open: CEO architecture selection, general native tool effects, independent final acceptance and a simple owner UI still need integration and verification.

## What the preparation endpoint does

`POST /api/companies/{company}/planning/{id}/prepare` is owner scoped. It reads the current ready planning revision, composes through the existing agent/tool/control repositories and a reasoning architecture repository, then returns:

- The actual selected candidate executor, configured actors and worker configuration identities.
- Communication ceilings for the selected topology, with transitive lineage preserved.
- A version 2 coordination program with explicit input, local weight pins and bounded delegation/review budgets.
- An `intake_patch` containing the program, retained planning identity/revision and a typed `factory_binding`.
- Explicit `candidate_only:true`, `owner_review_required:true` and `execution_authorized:false` flags.

Preparation is read-only. It does not create a job, sign an intake, install policy, start a model, download anything or accept a task. Models cannot call this endpoint to install authority. The general tool-enabled architecture menu still reports its separate executor gaps; a reasoning candidate executor cannot masquerade as a native tool executor.

## Request and local lifecycle

Supply `expected_revision`, `specification`, `input`, `model_sha256`, `max_delegations` and `peer_rounds`. The specification uses the existing preview schema: version, objective, input/output descriptions, exact definition of done, exclusions, coordination, data classification, control profiles and required capabilities. Coordination is `ordered`, `adaptive_delegation` or `peer_review`. Model pins map each exact CEO-assigned actor to its reviewed local GGUF SHA-256. The host does not invent a hash, infer a model license or treat a hash as provenance approval.

Use this sequence with the same private local harness and owner identity:

1. Finish the CEO conversation and inspect its ready proposal and exact definition of done.
2. Prepare the matching specification and actual input. Review the complete blueprint, model identities, program and communication ceilings.
3. Merge the returned `intake_patch` into an ordinary intake; supply the owner's timeline and company/provider signer fields. Freeze the intake through the existing company operation.
4. Inspect the complete frozen job and separately approve its exact digest. Install only the owner-selected policy, within the compiled ceilings.
5. Start that approved task through the existing run endpoint. The company adapter recomposes against the current repositories before inference; altered selected worker configurations or blueprint snapshots fail closed.
6. Inspect actual native receipts and the resulting candidate. Independent final acceptance remains necessary. A model's candidate or another peer's opinion cannot mark the task complete.

A change to the frozen program input, role assignment, model pin or budget changes its program hash. A change to the specification, proposal, selected repository records or candidate architecture changes the relevant blueprint identities. The ordinary approved intake digest binds all these values together. The preparation blueprint hash uses canonical compact JSON with sorted object keys; it is separate from the versioned coordination program's typed serialization hash.

The boundary deliberately refuses to remove required tools from a task. All CEO-assigned tools and requested capabilities must be empty for this reasoning path. Native file/tool/artifact execution contracts, missing roles/pins, unknown fields, stale revisions, unresolved control profiles and confidential/regulated classifications are rejected or remain unresolved. Such tasks require the existing governed native path or future full factory integration. This restriction does not fulfill the broader media, enterprise or arbitrary-task goal.

## Versioned evidence references

Version 1 programs retain their historical protocol, input hashes and checkpoints. Their narrative `evidence` strings remain unverified model prose. Existing frozen runs are not reinterpreted under a new decoder.

Version 2 contributions retain `deliverable`, `evidence` and `unknowns`, but `evidence` may contain only exact host-supplied source IDs:

- `input:<sha256>` identifies the frozen explicit input.
- `message:<ordinal>:<sha256>` identifies an earlier message actually supplied to this turn.

The local decoder receives the complete allowed ID set. The host independently rejects fabricated IDs, changed hashes, future/unreceived messages and duplicate citations before committing output or per-agent memory. A rejected observed reply retains its hash/native usage in the existing non-replayable pending checkpoint. Replay validation and the SQLite journal enforce the same versioned rule.

A valid citation proves source identity and availability, not factual correctness, semantic support, successful tests, tool execution or task acceptance. The deliverable and unknowns are still model output. This change prevents arbitrary narrative claims from entering the structured citation field; it does not establish hallucination-free agents.

## Architecture and verification

`company_factory/preparation.rs` owns the typed preparation request/binding and compiler. `coordination/evidence.rs` owns versioned reference validation and decoder construction. Worker execution, access policy and durable checkpointing remain behind their existing independent ports. No new dependency or provider was added.

Unit checks cover all three actual candidate descriptors, owner isolation, read-only preparation, missing/changed models, input/program tampering, unsupported tools/data profiles, altered repository snapshots, citation forgery and exact checkpoint reuse. The private HTTP smoke now prepares each pattern and uses its patch through real intake, approval, governance and run admission. Deliberately missing models fail terminally without an automatic retry. Its CEO plans are seeded fixtures and do not prove native planning or task completion.

An opt-in native test uses the installed pinned local model with factory-prepared version 2 programs:

```sh
cargo test -p gents-harness coordination::native_probe::actual_local_factory_prepared_typed_citations \
  --locked -- --ignored --exact --nocapture --test-threads=1
```

Its exact synthetic output oracle remains outside runtime input. The test archives its exact binary before inference so later Cargo rebuilds cannot replace that evidence. This is still seeded owner/CEO planning, reasoning-only candidate execution and a narrow private oracle. Check the actual recorded outcome before claiming success; final factory/UI/tool acceptance remains separate.

## Recorded native result

Factory-prepared probe `a119f1e5-d0ce-46d0-829c-d721fe7ca2b1` passed the exact sorted-ID synthetic output check in all three programs. The same installed owner-pinned Qwen3-Coder-30B weights served separate role/session identities. The CEO/owner plan remains an explicitly seeded fixture, not an actual planning-conversation qualification.

| Program | Actual native turns | Input / output tokens | Program elapsed |
| --- | ---: | ---: | ---: |
| Flow | 3 | 2,621 / 473 | 118.158 s |
| Hub delegation | 7 | 7,509 / 1,044 | 250.885 s |
| Peer revision | 4 | 4,068 / 747 | 145.740 s |

Total: **14 replies, 14,198 input + 2,264 output = 16,462 local tokens, $0 provider/tool fees**. Program elapsed includes integrity checks and setup. This is not an efficiency improvement claim; the citation IDs consume context, and the hub selected a different bounded dispatch sequence. Hardware, electricity and frontier engineering remain separate. Earlier probes/failures are separate; unknown usage is never zeroed.

The independent `scripts/verify_coordination_candidate.py` reads canonical records without importing runtime code or calling models. It verifies the archived test binary and actual model bytes, frozen preparation/blueprint/program identities, owner approval, native packet/receipt hashes, typed citation availability, lineage/current grants, dispatch shape, exact final fixture output and per-agent archive digests. The runtime still reports `task_accepted:false`; all 14 memory records are candidate-submitted, not completed-task promotion. Its owned model listener was closed.

```sh
python3 scripts/verify_coordination_candidate.py \
  --probe a119f1e5-d0ce-46d0-829c-d721fe7ca2b1 \
  --output data/mcp-basement-verification/factory-binding-native-proof.json
```

These private development artifacts are not bundled release attestations. Three negative checks on temporary copies rejected unknown usage, a forged citation despite a rewritten checkpoint digest, and altered per-agent memory. The canonical owner store was not changed and no inference was replayed. The qualified curriculum count remains 22; this is a framework retest, not three new general capabilities.
