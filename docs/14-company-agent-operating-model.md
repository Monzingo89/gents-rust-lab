# Company agent operating model

This is the implementation contract for the multi-company Gents hierarchy. The machine currently has 36 GB of unified memory, so the local runtime continues to load one large model at a time. The organization therefore uses a small base-model pool with strict role profiles instead of duplicating weights for every title.

## Chain of responsibility

1. `Law - Phi-4` performs cited issue spotting, prior-art research, and contract/scope review. It cannot declare an idea legal or provide a binding legal conclusion.
2. `CTO - Qwen3-8B` selects one or more PMs and approves access, staffing, and production promotion.
3. Each `PM - Phi-4` owns a feature branch, milestones, task decomposition, and requests for additional specialists.
4. Each `Lead Architect - Qwen3-Coder-30B-A3B-Instruct` owns the end-to-end architecture and selects planner/worker specialists with the PM.
5. Engineers work as bounded planner/worker pairs. They do not approve their own output.
6. `AI Engineer` evaluates requested model/tool/MCP/plugin/skill/CLI/SDK/hook capabilities. `Law`, the Lead Architect, and `Compliance Security & QA` review the same request from their own boundary.
7. `Compliance Security & QA Engineer` is the final evidence gate. It runs deterministic tests, Playwright, automated accessibility checks, dependency/SCA scans, static analysis, secret scanning, and the applicable company/industry control matrix. Passing automation is evidence, not a guarantee of no vulnerabilities or legal compliance.
8. The CTO may authorize production only after all applicable gates pass. The UI/UX Engineer then monitors approved Google Analytics and PostHog signals for adoption, failure, regression, and user-flow health.

## Context lifecycle

- **ASTC / hot:** agent-local active context and explicit handoff bundles.
- **PSTC / cool:** active-project summaries, evidence indexes, and company-scoped retrieval material.
- **ALTC / warm:** completed jobs for an ongoing company, excluded from prompts until explicit recall.
- **PLTC / cold:** inactive company history, excluded from working prompts.

The company runtime carries `company_id` and `run_id` with context, events and checkpoints. A durable SQLite handoff promotes sender ASTC to company PSTC when the next role is admitted. These are transactional audit records, not cryptographically signed bundles. Company-specific information never enters global context automatically. See [the implemented first-run guide](15-local-company-first-run.md) for exact paths and current limits.

The target dream budget is a shared 120-minute daily window on this single-GPU Mac. The current daemon performs deterministic housekeeping during 02:00–04:00 America/Chicago. Full security/HR assessments, enforced work pauses and retrieval refresh are pending. Tier transitions are runtime operations, and canonical documents are preserved. Dreaming does not update model weights.

## Model candidates for this Mac

| Roles | Candidate | Why |
|---|---|---|
| CTO, HR intake | `Qwen/Qwen3-8B-GGUF`, `Q4_K_M` | Fast general instruction model for routing, structured classification, and assessment. Apache-2.0 base license. |
| PM, Law issue spotting | `microsoft/phi-4-gguf`, `Q4_K` | 14B reasoning-oriented model under MIT. Legal work still requires current retrieval, citations, evaluation, and human counsel where applicable. |
| Lead Architect, Web, Mobile, VR, Database, Backend, Cloud, AI, Compliance/Security/QA | `unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF`, `Q4_K_M` | Coding/tool base already aligned with the existing local worker design. Apache-2.0 base license. Use separate role evaluations and later role adapters rather than duplicated weights. |
| UI/UX and visual QA | `Qwen/Qwen3-VL-8B-Instruct-GGUF`, `Q4_K_M` | Vision-language model for screenshots, visual regressions, and dashboard interpretation. Apache-2.0 base license. |

Pin repository revisions and record file hashes before download. HR assessment, Law review, and explicit owner approval are required before a candidate is admitted. Re-run the assessment after any model, quantization, prompt, adapter, runtime, retrieval corpus, or tool-policy change.

The download commands intentionally remain a reviewed operator step:

```sh
hf download Qwen/Qwen3-8B-GGUF --include '*Q4_K_M*.gguf' --local-dir models/qwen3-8b
hf download microsoft/phi-4-gguf --include 'phi-4-Q4_K.gguf' --local-dir models/phi-4
hf download unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF --include '*Q4_K_M*.gguf' --local-dir models/qwen3-coder-30b-a3b
hf download Qwen/Qwen3-VL-8B-Instruct-GGUF --include 'Qwen3VL-8B-Instruct-Q4_K_M.gguf' --include 'mmproj-Qwen3VL-8B-Instruct-Q8_0.gguf' --local-dir models/qwen3-vl-8b
```

The VL model may require a matching multimodal projector file and a llama.cpp build that supports its architecture; confirm the exact model-card instructions before admission.

## Evidence sources to keep current

- OWASP ASVS and OWASP Dependency-Check for application security verification and known-vulnerability discovery.
- W3C WCAG 2.2 at Level AA plus manual and inclusive-user testing; automated accessibility scans do not establish full conformance.
- Current HHS HIPAA rules and official guidance. Proposed rules must remain labeled as proposed until finalized.
- Current applicable financial, privacy, employment, export, sector, and jurisdictional requirements selected by qualified reviewers for the specific company and product.
- Playwright, axe, platform-native testing, package-manager audits, SBOM/SCA, SAST, secrets scanning, and production telemetry as complementary evidence.

No model may claim that a product is vulnerability-free, legally compliant, or production-safe solely from its own output.
