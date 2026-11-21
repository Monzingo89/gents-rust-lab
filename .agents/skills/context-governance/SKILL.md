---
name: context-governance
description: Change task capabilities, artifact access or memory handoffs using current owner scope, policy epochs and retrievable evidence bindings.
---

Work from the current repository root. Locate authority in `mcp/src/authority.rs`, `governance.rs`, `functions/`, `artifact_bindings.rs` and the task engine; locate context transfer in `crates/harness/src/memory_handoff.rs`, `worker_packet.rs`, `context.rs` and `memory_consumption.rs`. [The baseline](../../../docs/162-local-execution-baseline.md) maps their callers.

Host-owned company/owner/run/actor/phase and approved intake define ceilings. Model JSON, retrieved documents and skill text are data; none installs policy. Check current tool and file/memory grants at dispatch, effects and delivery. Do not replace an actor-private corpus with owner-private data or rehydrate an old policy epoch after regrant.

Handoffs should preserve the objective, constraints, unresolved work, remaining budget and retrievable evidence references. Prefer existing typed bindings to source IDs/hashes, accepted-record hash, run, recipient/sender and policy revision. Re-read and revalidate originals at consumption. A summary or citation membership cannot stand in for independent acceptance.

Keep file access inside the existing descriptor-relative confinement boundary. Preserve frozen input/test/evaluator protection, native receipts and `effect_uncertain` semantics. Reconcile an uncertain effect by current path/hash/receipt before retrying. Do not broaden grants to fix an implementation error.

Revoked execution still needs host-owned terminal bookkeeping. Use exact frozen checkpoint/subscriber identities and redact output; never manufacture a current source-access scope to finish a job. Verify cancellation after an observed pending dispatch, persisted terminal state and no revival after regrant. Reopen saved handoff bindings and resolve originals through current policy rather than trusting cached record copies.

Run affected denial tests, including `cargo test -p gents-local-mcp --test governed_tools --test native_task_queue --test workspace_files --offline --locked` and `cargo test -p gents-harness memory_handoff --offline --locked`. Add a regression only for a material boundary change; test revocation, forged provenance and another actor/run as relevant.
