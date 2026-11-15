# Guided factory handoff

The local CEO page now prepares the normal frozen owner review from the saved CEO conversation and its canonical architecture assessment. The browser supplies the ready revision, retained assessment ID, data scope and any explicitly edited artifact checks. It cannot select architecture, replace agent staffing or supply model hashes to this endpoint.

1. Discuss a small task with the local CEO until the plan is ready.
2. Ask the CEO and separate local judge to review the architecture. The output judge infers requested text/image/audio/video from the user brief; no media-mode selector is needed. Missing engines or missing assigned tools require clarification.
3. Choose the data scope. Public information and synthetic fixtures can use the baseline path. Private or regulated work currently requires additional control assessment and is blocked here; choosing a label does not certify compliance.
4. Prepare the frozen review. Inspect exact task text, criteria, model-bound factory contract, communication ceilings, tools and artifact checks.
5. Approve the exact digest and start locally using the existing authenticated workflow.

`POST /api/companies/{company}/planning/{id}/prepare-guided` is read-only preparation. It does not run inference, create jobs, approve work or install permissions. The subsequent ordinary intake operation creates a draft. Owner consent, approval, installed governance and native runtime checks are separate mandatory steps.

## Implemented execution boundaries

| Plan | Factory selection | Current outcome |
| --- | --- | --- |
| Reasoning with ordered contributions | Flow | Candidate, then frozen independent local review on new guided intakes |
| Reasoning with hub delegation | Orchestration | Candidate, then frozen independent local review on new guided intakes |
| Reasoning with peer critique/revision | Collaboration | Candidate, then frozen independent local review on new guided intakes |
| Native tool work with ordered stages | Flow | Existing independent QA and frozen native acceptance gates |
| Native tool work with hub/peer coordination | No installed executor | Blocked; no silent substitution |

The UI explicitly identifies reasoning candidates and the separately pinned final reviewer on new guided intakes. See the [acceptance implementation, real writing task and retained failures](128-independent-reasoning-acceptance.md). Older intakes without a review contract remain candidates. Peer agreement and all-agent progress are not proof that the task is done. This handoff is not a general autonomy benchmark or an industry compliance assessment.

Factory objective validation now preserves newlines and indentation. The complete retained user conversation is also included as reasoning input. Internal planning-repair messages do not replace the user's task. Worker configurations must match an unambiguous owner-listed local model pin; the native flow also verifies actual model bytes and recomposes the installed blueprint before execution. Changed canonical planning, architecture, policy, staffing or repository identities require fresh review. No paid fallback or runtime download is introduced.

For local image generation and generic text-to-speech, see [media installation and limits](../mcp/media/README.md). The [cookbook](121-owned-mcp-cookbook.md) covers all 15 native tools. Tools remain actor/phase scoped even when the output judge correctly recognizes the prompt.

## Verification on 2026-11-13

- Workspace Rust tests: 527 passed, 25 opt-in tests ignored; Clippy passed with warnings denied. The new preparation integration test uses explicitly simulated weights/replies, exercises all three reasoning topologies and ordered native preparation, and checks ordinary intake, approval and governance installation without executing a task.
- Read-only current HTTP probe consumed an existing canonical real local architecture/output-judge assessment over a transparently seeded CEO fixture. Exact task preservation, stale revision, unknown-field, regulated-data and cross-origin rejection passed. No new inference, job or permission changes.
- Browser check exercises the actual office DOM with explicitly simulated planning/review/preparation/intake responses. It verifies missing-review/data-scope gates, the exact request, frozen review rendering and approval invalidation on an edit. No real task, approval or model/tool execution occurs.
- Existing real PNG/WAV artifacts were rehashed against their native receipts. The ICO was parsed and contains 16, 24, 32, 48, 64, 128 and 256 pixel variants. These checks prove artifact structure/identity, not semantic media quality.

Ignored local evidence is under `data/mcp-basement-verification/guided-factory-*`. The broader owner trial, structured-output reliability, wider native task qualification, full release audit and main-branch publication remain outstanding. A subsequent real completed writing task and labeled replay are documented in the acceptance guide.
