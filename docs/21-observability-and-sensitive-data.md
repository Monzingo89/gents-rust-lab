# 21 — Observability floor and sensitive-data assurance

Floor 07 is the harness control room. It visualizes correlated local-model calls, metadata-only context handoffs, failures, and the repair/recovery events that follow them. The source of truth is `GET /api/runs/<run_id>/observability`; the floor never exposes private reasoning or copies retrieved context bodies.

## Feedback loop

1. A call, tool action, handoff, or deterministic gate emits a structured event.
2. Failures and policy blocks become issues in the observability projection.
3. Existing bounded recovery chooses a different installed local model or requests a corrected proposal. After four invalid proposals, the Executive Gent must choose a materially different catalog action from retained verified evidence; that exact action is queued for independent Admin Judge review rather than becoming a yellow operator question.
4. The Admin Judge and deterministic acceptance gates remain authoritative. Observability cannot grant permissions or weaken policy.
5. Destructive actions, unknown side-effect status, and missing external authorization still stop for an operator.

Inference/load failures follow the same unattended policy: the checkpoint is retained, distinct installed models are rotated, and the Executive Gent schedules a bounded retry. Resource discovery exposes configuration presence and allowlist counts, never raw addresses or secret values. A model failure, invalid JSON proposal, repeated tool request, inaccessible public page, or exhausted replanning budget is therefore not by itself a reason to interrupt the operator.

This is automatic diagnosis and bounded repair, not unrestricted self-modifying code. Agents may repair ordinary allowlisted repository files through hash-guarded replacement, private backup, fixed regression checks, automatic rollback, and supervised restart. Authentication, provider, secret-loading, sensitive-data, and global-policy files remain human-reviewed. Tool installation and permission expansion are not automatic.

When local deterministic resolution cannot answer a low-risk yellow question, the harness can send only that pre-screened question to OpenAI through the server-side `RUST_OPENAI_API_KEY`. The structured response either supplies advisory guidance for the local agents or requires the operator. The local agent then re-proposes its work and the local Admin Judge reviews it again. Destructive, financial, authorization, identity-specific, and sensitive-data questions never auto-resolve. Each question gets one cloud attempt, with `store: false` and a bounded response.

Agents can also create run-private declarative skills by composing approved single-responsibility tool IDs. These procedures are reusable by planner or worker roles according to their manifest, but they add no code-execution path or authority. Each actual step remains visible on this floor and independently judged.

## Sensitive data

The default policy is scope avoidance. Task intake and exact tool requests are scanned before persistence/execution. Tool results are scanned before journal, context, or event persistence. Apparent direct email addresses, US SSNs, valid payment-card PANs, and identified health data are blocked or discarded. Use approved tokens or de-identified test data.

The UI reports `controls_enforced_not_certified`. Passing these checks is not proof of HIPAA or PCI DSS compliance. An enterprise deployment needs documented scope, encryption/key management, identity and least privilege, audit retention, incident response, vendor and BAA review where applicable, secure backups/deletion, independent security testing, and qualified compliance/legal sign-off.
