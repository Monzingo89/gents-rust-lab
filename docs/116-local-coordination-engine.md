# Local flow, hub delegation and peer revision

`crates/harness/src/coordination/` now supplies distinct bounded reasoning programs behind replaceable worker, access and journal ports. It is an execution component for the larger factory goal, not a declaration that the complete task-selected factory or free release is finished.

The vocabulary follows the publisher's public [chapter 4 preview of AI Agents in Action, Second Edition](https://www.manning.com/preview/ai-agents-in-action-second-edition/chapter-4). Our implementation is original project code. This reference does not imply that the full book was available, that its code was copied, or that the author reviewed this harness.

| Program | Actual turn behavior | Completion boundary |
| --- | --- | --- |
| Flow | Each frozen role contributes once, in order, receiving authorized earlier messages. | Final role produces an unaccepted candidate. |
| Hub delegation | The first role selects a distinct worker and bounded assignment, receives its contribution, then chooses further delegation or synthesis. Workers may be revisited within the owner-frozen budget. | Synthesis produces an unaccepted candidate; the hub cannot approve it. |
| Peer review | An author drafts, other peers send direct critiques, and the author revises. One or two review rounds are frozen by the owner. There is no model acting as central dispatcher. | Critique is advisory; revision produces an unaccepted candidate. |

Logical topology is distinct from physical GPU concurrency. The owned runtime pool may serialize all calls on this computer. Roles use separate actor/policy session identities even when they share the same model weights; this does not establish independent model families or unbiased agreement.

## Module boundaries

- `program.rs`: typed frozen roles, task input, definition of done, budgets, deterministic turn validation, routing schemas and transitive message lineage.
- `mod.rs`: orchestration over `WorkerPort`, `AccessRepository` and `JournalRepository`; checks exact dispatched input, current policy, native completion/usage and the bounded message protocol.
- `native.rs`: owned local GGUF inference only. It checks the explicit weight pin before each new call and uses existing hardware fitting/capacity management. No model download, paid provider, tool broker or implicit company-history read is available.
- `journal.rs`: authenticated company SQLite checkpointing, exclusive controller ownership, source hashes, native receipts, audit metadata and atomic per-agent memory staging.
- `admission.rs`: binds the program to the retained ready CEO staffing and the owner-frozen text intake. It compiles reviewable communication ceilings and rejects tool/file execution contracts in this reasoning path.
- `company.rs`: existing company-runtime integration and observable calls/candidates. The adapter returns `candidate_submitted`, never `completed` from an agent's own opinion.

Existing native MCP execution remains in the independently governed tool path. This new engine currently performs reasoning turns only. A task needing native code, files or other tools cannot be switched into this engine by retaining those contracts: admission rejects them. A repository-bound reasoning preparation API is now described in [the preparation guide](118-factory-reasoning-preparation.md). Adding a generic tool effect port, independent acceptance and the complete factory conversation/UI binding remains required.

## Communication governance

Each turn names exact earlier source ordinals. The host computes all originating actors, including transitive ancestors. Before dispatch and receipt commit, the current recipient grant must authorize every other source actor. A hub relay does not declassify a worker's content or erase its origin. An owner can remove a communication grant; a dispatch needing that lineage then stops rather than silently reacquiring access.

The compiled direct message graph for hub delegation is a star. Its maximum memory ceilings include possible transitive worker lineage, because later hub instructions may depend on earlier worker results. Those are separate facts: a star-shaped message transport is not proof that workers cannot indirectly receive another worker's information. The owner reviews explicit ceilings and may attenuate them.

The engine subscribes to policy changes before checking admission. A changed policy drops queued/in-flight work, cancels the exact actor/session and prevents output persistence under a stale epoch. It does not automatically replay inference after a revision change. Model messages cannot add actors, grants, tools, budgets or completion criteria.

## Durable evidence and recovery

The journal binds the exact program to an approved running job and the installed policy epoch. It commits `pending` before dispatch. A valid receipt must identify native local inference, a completed non-cutoff reply, exact dispatched task-input hash and complete zero-provider-charge usage. Only then are the message and that actor's short-term memory record committed together.

Completed messages are digest-checked and replay-validated against the original program. Reopening a completed checkpoint reuses its original records without creating replacement model replies. An unresolved or rejected turn stops automatic replay. A received but rejected reply retains its content hash, native receipt and usage without admitting its raw content into a handoff or memory record. A call with no observed reply may still have unknown usage; it is never counted as zero tokens.

Canonical tables are `coordination_programs` and `coordination_turns` in the company's private `brain.sqlite3`; audit kinds include `coordination_turn_started`, `coordination_turn_completed` and `coordination_turn_rejected`. The existing per-agent archive lifecycle applies to a submitted candidate. This is neither completed-task long-term promotion nor model training.

## Current API integration

The ordinary owner-authenticated company intake accepts an optional `coordination_contract` only alongside a retained ready CEO conversation and `deliverable_type:text`. Its role order, identities and assignments must match that proposal, its definition of done must match the frozen acceptance, and assigned tools/native contracts must be empty. Model hashes are explicitly owner-frozen; actual workers are resolved from the installed company configuration and checked against those hashes. The new path requires the normal separate intake approval and governance installation.

Its contract fields are `version:1`, `coordination`, `objective`, `definition_of_done`, `input`, `roles`, `max_delegations` and `peer_rounds`. Each role has `actor`, `assignment` and `model_sha256`. The current bounds are 1..4 roles, 1..4 delegated worker turns and 1..2 peer rounds; hub and peer programs require at least two distinct roles. The native worker adapter supports local GGUF models only. A weight hash is an integrity pin, not publisher provenance or a vulnerability-free attestation.

This low-level intake path does not yet turn a preview blueprint into an approved execution contract, choose coordination automatically after CEO questions, supply a complete owner UI, or independently accept arbitrary task results. The architecture repository therefore has not promoted these candidate-only programs into its general factory executors. The complete three-pattern factory milestone remains open.

## Verification scope

Unit/protocol tests cover distinct dispatch sequences, bounded routing, transitive relay denial, immediate policy cancellation, complete-checkpoint reuse, unresolved-call refusal after actual SQLite restart, exclusive controller fencing, per-actor staging, corruption, rejected-reply usage retention and epoch revocation at commit. Their model receipts are explicitly simulated.

`scripts/coordination_smoke.py` checks the real private HTTP admission path for each pattern with seeded CEO plans and deliberately absent models. It verifies scope/tool-ceiling rejection and terminal preflight failure without the generic automatic retry loop. This is infrastructure evidence, not local task qualification.

An opt-in native probe exercises all three programs against the same small synthetic text task using the installed owner-pinned model. It retains frozen contracts, native messages, receipts, exact-output checks and failures in ignored local evidence. It seeds the owner/CEO plan and does not qualify the CEO planning conversation, native tools, regulated use or the complete factory. See the recorded outcome before making any success claim:

```sh
cargo test -p gents-harness coordination::native_probe::actual_local_three_pattern_candidates \
  --locked -- --ignored --exact --nocapture --test-threads=1
```

The first native development probe passed the flow exact-output check, then rejected an invalid hub route. Its original artifacts remain under `data/factory-coordination-probes/66f91f2e-ad69-4311-959f-464be78924f1/`. That first implementation lost usage metadata for the final rejected reply before checkpointing; the missing usage remains unknown. It was not silently repaired or counted as zero. The decoder now expresses disjoint valid action shapes from the full frozen worker roster, and rejection auditing preserves future observed usage. The fresh unchanged-task result is recorded below; the original failed probe remains separate evidence.

### Fresh native result, independently checked

The frozen retest `8181b6d9-a987-4509-9c60-7126325761d1` passed its exact synthetic output oracle in all three programs using the same installed Qwen3-Coder-30B weights. These are candidate-program probes with seeded owner/CEO plans, not a real CEO planning conversation or complete factory acceptance.

| Program | Actual dispatch sequence | Input / output tokens | Program elapsed |
| --- | --- | --- | --- |
| Flow | Hub contributes → Worker contributes → Critic contributes | 2,049 / 312 | 105.883 s |
| Hub | Hub routes → Worker contributes → Hub routes → Critic contributes → Hub routes → Hub synthesizes | 4,757 / 383 | 203.598 s |
| Peer | Hub drafts → Worker critiques → Critic critiques → Hub revises | 3,087 / 419 | 140.006 s |

Names are actor identities, not model families. In peer mode the actor named Hub is the author, not a model dispatcher. Program elapsed includes weight integrity checks and runtime/model setup, so it is not inference throughput or a prediction for another device.

Thirteen actual native replies used **9,893 input + 1,114 output = 11,007 local tokens** with **$0 provider/tool fees**. Earlier failed-probe usage is separate and partially unknown; this is not an all-development token total. Hardware, electricity and frontier development are excluded. No new weights, dependencies or paid services were introduced.

The separate read-only verifier checks canonical approval/intake/program digests, the frozen test binary and actual model bytes, all packet hashes, native receipts, actual source lineage/grants, dispatch topology, exact final outputs, audit counts and per-agent archive checkpoint digests. All 13 records are candidate-submitted archives; none declares runtime task acceptance. The owned private model listener was closed. Local evidence:

- `data/factory-coordination-probes/8181b6d9-a987-4509-9c60-7126325761d1/`
- `data/mcp-basement-verification/coordination-compatible-native-proof.json`
- `data/mcp-basement-verification/verify-coordination-native.py`

**Observed limitation:** the first flow reply claimed Worker/Critic contributions despite having no predecessor sources. Those claims are unverified model prose, not canonical execution events. The final exact-output oracle passed, but that does not validate its narrative evidence. New version 2 programs now enforce exact host-supplied source references; see [typed evidence and preparation](118-factory-reasoning-preparation.md). This historical version 1 result retains its original protocol and unverified prose. Existing `model_output` memory remains advisory; this probe does not establish hallucination-free agents.

The full `scripts/check.sh` run passed with **512 Rust tests, 21 explicitly ignored tests and 152 Node tests**, plus its Python/HTTP checks and web build. The opt-in actual native test passed separately. Unit/HTTP fixture success is not a native task benchmark or a vulnerability audit.

The site, real demo, MCP cookbook, hardware calculator, complete release review, owner local trial and later deployment authorization remain separate requirements. Nothing here certifies SOC, banking or health compliance.

The later version 2 probe archives its exact test binary; see [the preparation result](118-factory-reasoning-preparation.md). The earlier version 1 verifier recorded a successful live binary-hash check, but that target binary was not separately archived before later Cargo rebuilds. Its saved proof/log is historical evidence, not a claim that its old binary path still reproduces that hash.
