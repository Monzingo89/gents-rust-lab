# Factory architecture and local release review

Review date: 2026-11-13. This is the initial repo-wide architecture/release pass, not an exhaustive vulnerability audit or an assurance report. The expanded goal remains active. Do not deploy before the owner completes a real local task and explicitly authorizes a later release. Existing dirty work and historical evidence must be preserved.

## Verified design reference

The publisher's public chapter preview identifies flow/assembly-line, orchestrator/hub-and-spoke and collaboration/peer-to-peer patterns. It emphasizes typed transfers, scoped tools and observable guardrails. The proposed factory mapping below is our implementation design, not copied book code or a claim to have read the unavailable full book. Source: [Micheal Lanham, AI Agents in Action, Second Edition, chapter 4](https://www.manning.com/preview/ai-agents-in-action-second-edition/chapter-4).

## Findings requiring release work

| Priority | Finding and evidence | Required change |
| --- | --- | --- |
| High | No task-selected architecture factory is present. `ceo_team.rs` parses 1..4 ordered stages and authorizes ordered handoffs. `workflow.rs` is a separate durable read-only DAG primitive. These are useful pieces, not three interchangeable runtime architectures. | Define typed factory ports/repositories, implement distinct flow, central delegation and governed peer collaboration strategies, integrate selection after confirmed definition of done, and verify each actual execution path. Never relabel one sequential executor as three implemented patterns. |
| High | The Rust verifier and component invocation sandbox explicitly fail outside macOS in `mcp/src/rust_verification/sandbox.rs`. Descriptor-relative workspace support covers macOS/Linux but not arbitrary platforms. | Publish an accurate support matrix, add separately tested Linux/Windows isolation adapters before enabling those code tools there, and preserve fail-closed behavior. GPU branding alone does not prove platform support. |
| High | README describes multiple historical modes and readiness snapshots, with an old six-floor description and mixed demo/local/cloud material. It does not present a single current local onboarding path or complete MCP cookbook. | Rewrite the entrypoint around purpose, real recorded demo, verified capabilities and limits, one safe local startup path, installation/model integrity, definition-of-done conversation, factory behavior and tool recipes. Keep legacy adapters in clearly separate advanced docs. |
| High | No project LICENSE was found. Ignoring model files does not grant redistribution rights to software or assets. | Owner selected MIT on 2026-11-13. LICENSE and THIRD_PARTY_NOTICES.md are added; retain model/asset provenance and complete the release asset review. |
| High | A public introduction site, FAQ, privacy/terms pages and defensible readiness statement are absent. The office UI is a local execution surface. | Build a separate local-previewable static introduction site; never publish the loopback-auth execution daemon. Draft pages from actual collection/retention/network behavior. Deployment remains blocked by the owner's local trial and later explicit authorization. |
| High | Only Apple M4 Max timings are currently measured for the qualified six-call workload. Other GPU time/cost claims lack evidence. | Calculator distinguishes observed device/workload results from user-supplied throughput scenarios and unknown hardware. Quote zero provider/tool fees for fully local execution, excluding hardware, electricity and development. Never promise arbitrary tasks will finish or invent GPU benchmark rates. |
| Medium | `main.rs` is about 2046 lines, `company.rs` 2513 and `ceo_planning.rs` 1959. Wiring, persistence and planning concerns are concentrated. Existing provider traits, MCP crate, pure validators and memory modules offer useful seams. | Extract cohesive repositories/factories incrementally behind existing contracts; preserve native receipts, identity, cancellation, source pins, policy epochs and owner approval. Avoid an untested whole-repo rewrite. |
| Medium | `.github/workflows/ci.yml` runs Ubuntu Rust tests and a subset of Python/Node checks; `scripts/check.sh` has much wider local coverage. Current actual model proof is local, not CI/platform proof. | Add release-oriented matrix checks and explicitly gated native inference/sandbox probes. Report which checks actually ran. |
| Medium | `SECURITY.md` mentions credential-enabled OpenAI question escalation without the current default-disabled operator boundary. `provider.rs` and `escalation.rs` deny cloud use unless `GENTS_LOCAL_ONLY=0`; `run-company.sh` forces local-only/offline. | Update docs to describe current defaults accurately. Keep any explicit cloud override outside the free local product path; model output must never change it. |
| Medium | No fresh marketing GIF exists for the latest actual workflow. Preview walking and simulated tests are available but are different evidence. | Capture a real completed local run with synthetic inputs and visible receipts, remove private data, and label simulation/actual execution correctly. Do not fabricate an execution animation as proof. |

## Boundaries already confirmed in this pass

- `.gitignore` excludes `models/`, `*.gguf`, `*.safetensors`, `data/`, SQLite files and local secrets. `git ls-files` found no tracked model weights or SQLite files; only example environment templates were tracked by that focused query. Full publication secret/asset review remains required before any push.
- Local provider calls reject remote inference by default, independent of credential presence; the local launcher explicitly enforces local-only/offline configuration.
- The owned MCP crate and stdio/SSE transport boundary already exist. Tool capability selection is not permission installation. Current owner policy, source custody, typed ABI, descriptor-relative I/O and independent task checks remain required.
- Actual task qualification is narrow, source-bound evidence. The latest successful six-call baseline used local CEO/QA inference and host-compiled native execution with zero worker generations and zero provider/tool fees. Local inference still consumed tokens.
- Per-agent completed archive integrity is independently checked in owned local SQLite. This is not model training, memory truth or guaranteed recall quality.

## Factory contracts to implement

A confirmed task specification must include input sources, requested outputs, testable definition of done, exclusions, effect classes, communication needs, data classification and local hardware limits. Missing facts trigger CEO questions rather than guessed requirements.

`ArchitectureRepository` exposes versioned strategy descriptors and real executor availability. `AgentRepository` supplies owner-approved role/model templates. `ToolRepository` supplies owned tool descriptors and exact source/API identities. `ComplianceProfileRepository` supplies versioned control requirements and gaps, never a compliance verdict. Repositories are read-only sources of owner-controlled configuration, not model-generated authority.

`CompanyFactory` composes a proposal through those ports. Its complete target is the simplest available pattern that meets the confirmed task: ordered flow for predetermined stages; orchestration for central adaptive delegation; collaboration for bounded peer proposal/critique with explicit communication grants. Local GPU work may serialize even when the logical topology has independent agents. Unsupported patterns/tools/controls return an explicit missing capability rather than falling back silently. Tool creation follows a separate quarantined author/test/review/admission path, with no automatic grant or global promotion.

The output is a reviewable execution blueprint bound to specification/repository versions. It records architecture, actors, local model pins, minimum tools, communication edges, required controls/gaps, budgets and definition of done. Final owner approval and native governance remain separate. Changing a factory implementation must not rewrite approved work or bypass any enforcement.

Acceptance requires meaningful factory tests, API/CLI/UI integration, policy-negative checks and a real local-model task per pattern with independent artifacts/receipts. The factory is not done until those paths are implemented and verified. The owner must be told when that milestone is actually reached.

## Local website and documentation deliverables

Prepare a clear local-preview introduction page, actual demo GIF, measured capabilities, quick start, MCP cookbook, architecture guide, hardware/time scenario calculator, FAQ, privacy, draft terms and security/readiness pages. Link the actual book reference without implying affiliation or certification. Structured metadata, semantic HTML, accessible navigation, crawlable content, robots/sitemap and optional configured canonical origin should describe real content; no hidden keywords, fabricated testimonials, fake benchmarks or ranking guarantees. Leave production URLs and hosting undecided until deployment is authorized.

SOC reports concern controls assessed by qualified CPAs; adding pages does not produce a SOC assurance report. The site should present readiness evidence and gaps without claiming attestation. Source: [AICPA SOC suite overview](https://www.aicpa-cima.com/resources/landing/system-and-organization-controls-soc-suite-of-services). Operational access reviews, incident response, encryption/backup/retention processes, change management, vendor/data inventory and assessment scope still need owner evidence and independent review. Privacy/terms drafts require an operator identity and legal review before release.

## Delivery order

1. Finish the current typed-decoder compatibility issue and verify the unchanged local workflow.
2. Complete the subsystem review and implement/integrate the factory contracts and three real strategies, preserving all existing guards.
3. Make the definition-of-done conversation and missing-tool path usable for an owner trial; verify a real user-like local task and provide exact startup steps.
4. Prepare the introduction site, real demo, cookbook, calculator and policy/readiness drafts locally; verify accessibility, links, metadata and builds.
5. Let the owner try a local task. No deployment or public execution exposure before that trial and separate later authorization. Broader goal completion remains unproven.

License decision: the owner explicitly chose MIT. The project license now exists; model weights and third-party media/dependencies retain their own terms. This does not authorize deployment.

Progress: the unchanged typed-selection trial passed independent verification; see [the measured result](114-type-directed-local-selection.md). The first repository-backed composition boundary and owner-scoped preview API now exist; see [the factory preview](115-company-factory-preview.md). Distinct reasoning-only flow, hub and peer candidate programs now exist and passed narrow native probes; see [coordination scope and evidence](116-local-coordination-engine.md). Full blueprint-to-execution binding, tool effects and independent acceptance for those programs remain explicit gaps. No preview grants execution. `autonomy.rs` is 4861 lines in this snapshot and remains a high-priority extraction seam. This update does not close the initial review findings or the three-pattern milestone.

The security documentation now reflects default-disabled cloud escalation, the private daemon boundary, native MCP gates and preview-only factory semantics. The daily offline model scan check returned `NOT_DUE`; its latest report still has eight `unreviewed_weight` findings among 18 inspected artifacts. These are unresolved admission/provenance review findings, not proof of known exploitable vulnerabilities or a clean security assessment. No weight was downloaded, deleted or silently approved.

The memory subsystem exposes concrete `SqliteSnapshots`, `SqliteHandoffs` and `SqliteContextArchive` operations. Its current owned local SQLite archive requires an enclosing transaction and independently checked actor/policy/contract digests; governed handoff reads revalidate sender, recipient, run, source record and policy revision. Reuse these storage boundaries for future strategies; add another backend only when required by an actual caller. This inspection does not prove semantic recall quality, model fine-tuning or complete control coverage.

The owner queued a separate publication/tools/operations goal after this one is complete. Its sequence and acceptance boundaries are recorded in [the follow-on roadmap](117-owner-follow-on-roadmap.md). Recording it does not complete this goal or authorize premature deployment.

Further progress: [factory reasoning preparation](118-factory-reasoning-preparation.md) now binds reviewable programs through ordinary intake/governance, with native version 2 probes and source-reference checks. [The introduction site](120-local-introduction-site.md) is built and browser-checked locally; README onboarding is rewritten and [all 13 native tools have recipes and exact schemas](121-owned-mcp-cookbook.md). CEO architecture choice, full native effect/acceptance integration, actual demonstration GIF, broader benchmarks, release reviews, finalized operator policies and the owner local trial remain open. None of these increments closes the full goal or publishes source.
