# Company factory composition preview

The first factory boundary is implemented in `crates/harness/src/company_factory/`. It composes the current ready CEO proposal through four replaceable interfaces: `ArchitectureRepository`, `AgentRepository`, `ToolRepository` and `ComplianceProfileRepository`. The native adapter reads installed owner configuration and the owned MCP registry. It performs no downloads, inference, tool execution, policy installation or job creation.

This is a composition preview. The three-pattern factory milestone remains unfinished: ordered flow has an existing runtime; adaptive orchestration and peer collaboration explicitly report `executor_not_implemented`. No sequential executor is relabeled as those patterns. Blueprint previews are not accepted as execution authority. Existing frozen intake, owner approval, governance and independent acceptance still control execution.

## What the preview records

- The requested coordination pattern, actual executor availability and retained CEO assignments.
- Configured local native workers, identified by configuration hashes. Preview does not hash model weights, load models or prove installation, capacity, GPU fit or competence.
- The union of assigned and explicitly requested capabilities, with descriptor hashes, ownership metadata, platform support and local/fee scope. A missing tool remains a development gap; nothing is downloaded or automatically admitted.
- Required baseline controls, additional owner-selected control profiles and missing evidence. These are requirements, not passed assessments or statutory coverage. Unknown profiles and confidential/regulated data scope produce explicit gaps.
- Separate hashes of the specification, ready proposal and selected repository contents, plus the complete preview digest. A digest does not approve anything or protect against a privileged same-user process.

For ordered flow, the preview includes proposed host handoff controls. Other patterns have no invented communication graph. Native policy still decides each actual actor/phase communication edge at execution time. Per-agent memory grants and archive controls remain independent.

## Local API recipe

First discuss the task in the CEO room until its saved conversation is `ready`. Read the actual current revision and definition of done from that session. Then submit an explicit specification to:

```text
POST /api/companies/COMPANY_ID/planning/PLANNING_UUID/blueprint
Content-Type: application/json
```

The local daemon's normal authentication and origin restrictions apply. The default request-body limit is 16 KiB. Replace the example requirements and revision with your actual ready proposal; do not treat this synthetic example as an executed task:

```json
{
  "expected_revision": 2,
  "specification": {
    "version": 1,
    "objective": "Read a synthetic local fixture",
    "inputs": ["fixture.txt"],
    "outputs": ["A receipt-bound read result"],
    "definition_of_done": ["Native read matches the frozen input"],
    "exclusions": ["Network and paid services"],
    "coordination": "ordered",
    "data_classification": "synthetic",
    "control_profiles": ["local_baseline"],
    "required_capabilities": []
  }
}
```

`coordination` accepts `ordered`, `adaptive_delegation` or `peer_review`. Data classification accepts `synthetic`, `public`, `confidential` or `regulated`. Use synthetic/public inputs for the initial local trial. A classification label is not a sensitive-data detector or legal determination.

A valid composition returns `ready_for_owner_review`; missing capabilities return `missing_capability` with structured gap codes. Both responses contain `preview_only:true` and `execution_authorized:false`. Stale revisions, changed definition of done, unknown request fields or an inaccessible tenant/session are rejected. The specification is supplied for preview; it is not silently merged into or saved as the CEO conversation. Resume the conversation when its actual requirements need to change.

`local_baseline` is always included. It requires owner-approved frozen intake, current actor/phase policy, confined workspace I/O, native source/action receipts, independent acceptance and per-agent memory governance. Domain-specific profiles are not yet supplied. The factory must not manufacture a banking, health or SOC assurance verdict.

## Platform and effect semantics

The native `external_effects` flag means effects outside a SQLite transaction, including local file writes. The factory exposes that fact as `nontransactional_effects`; `local_only` separately expresses the reviewed execution scope. The adapter admits only the current reviewed local tool names. New registry tools require an explicit adapter review before being considered local by this preview.

Rust verification/component invocation currently requires the macOS sandbox. Workspace file operations currently have macOS/Linux support. Platform metadata is not a compiler/tool installation check or successful platform test. Other operating systems retain explicit gaps rather than an unsandboxed fallback.

## Verification and remaining work

Focused tests exercise real repository substitution, stale revisions, changed acceptance, ambiguous architectures, digest drift, missing tools, external custody/fees, local worker restrictions, control gaps and tenant isolation. `scripts/company_factory_smoke.py` exercises the actual HTTP endpoint against a private seeded synthetic plan, verifies cross-origin/foreign-owner rejection, and checks that jobs, planning, audit and governance records remain unchanged with no model listener. It is a protocol test, not local model task qualification.

The full local `scripts/check.sh` passed on 2026-11-13, including 501 Rust tests (20 ignored), the private HTTP check, linting, existing recovery/native transport checks and the web build. The eight new focused factory tests passed. The HTTP check is included in GitHub CI configuration; GitHub execution has not been verified here. Local evidence is retained under ignored `data/mcp-basement-verification/company-factory-*` files.

The idle office at `http://127.0.0.1:8788/?autoconnect=1` was refreshed locally after the checks, preserving its six terminal runs and journal inode. Runtime bundle `6f80906acb7b45944c0f2ca4971ece580f6e9722774419164b5ebcb568c6cbfb`, native executable hashes, local-only configuration and SSE transport were checked through API/process/store evidence. This is not visual-browser proof or a new local-model task qualification of the factory; the actual typed-selection task remains bound to the prior bundle recorded in chapter 114. No publication or deployment occurred.

Still required before the three-pattern milestone: implement distinct central delegation and governed peer strategies, move task/coordination clarification into the CEO flow, select compatible agent roles per strategy, support the quarantined missing-tool development path, bind an approved blueprint to runtime repositories/checkpoints, add owner-facing UI/CLI review, and independently qualify a real local-model task through each strategy. After that, the owner must try a task locally before any later deployment authorization. The separate public introduction site, real demo GIF, calculator and release material remain pending.

Follow-on work: a [bounded local coordination engine](116-local-coordination-engine.md) now implements distinct reasoning candidate programs and a guarded low-level company intake path. These candidate-only programs have not been promoted into the preview repository's general executors: blueprint binding, native tool integration and independent task acceptance are still pending. The original preview snapshot above must not be interpreted as the current absence of all coordination code, nor as completion of the three-pattern factory milestone.
