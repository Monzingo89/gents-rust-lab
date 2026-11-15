# Owner-selected native workflow authorization

Model review remains the default. An owner can select `--approved-workflow` for a new curriculum data workflow:

```sh
node scripts/mcp-development-curriculum.mjs --next --approved-workflow --runtime-config config/company.mcp-development-wide-context.json
```

The CEO still plans with the synthetic user and chooses tools, actors, source contracts and artifacts. The fixture reviewer freezes the same acceptance criteria. The owner option changes only the authorization mode in the installed grants and ceilings; it supplies no answers, task implementation or model training. It cannot change the mode of an issued run through the runner. Source-authoring fixtures are excluded.

`mcp/src/contract_execution.rs` enforces exact operations inside the native transaction after ordinary policy authorization. A source must match the frozen actor, tool and resolved arguments, and be either a declared deterministic computation or a pinned read-only file input without external effects. An output must match its declared writer, path, canonical body and actual current-epoch source receipt. Readbacks are complete reads of declared outputs. Replacement requires the current hash and a recognized prior canonical write in the same epoch. Unknown preexisting targets, arbitrary text reads, changed arguments, unpinned inputs, stale epochs and inactive jobs are rejected. Existing descriptor-relative confinement, alias checks, write/read permissions, audit receipts and uncertain-effect handling still apply.

Native receipts and the canonical authority audit record `contract_enforced: true`. This attests enforcement, not task correctness. `crates/harness/src/contract_authorization.rs` prepares an authorization observation; execution repeats the checks. The harness records `authorization_decision` with `model_called: false`, rather than inventing a model-admin reply. Submission requires current passing native and artifact gates and proceeds to independent local QA. It never accepts its own task. Delegation retains the recipient's owner-selected mode and cannot enable or remove the constraint. Owner policy updates remain separately authenticated and revisioned.

The guard serializes cooperating MCP writers. It is not an atomic filesystem transaction with SQLite, a sandbox against a privileged same-user process, or a claim of vulnerability freedom. It supports the current macOS/Linux descriptor adapter; Windows remains fail-closed pending an adapter. Tool registration still requires trusted owner-managed code. The option does not authorize downloads, training, network access, source promotion or industry-regulated use.

Other host repairs remove invented `STEM_...` template artifacts when exact frozen artifact bindings exist, retain PDF/evidence gates, fingerprint actual native submission evidence, and promote ledger labels only after linked complete native readbacks. A readback proves observed bytes, not correctness. Existing failures remain in history and the unchanged benchmark rejects failed or replayed operations.

Regression coverage includes actual native stdio and authenticated loopback SSE, policy revocation and delegation, altered inputs and targets, source provenance, and both authorization modes through private HTTP. HTTP planning/QA mocks are infrastructure tests, not local company competence. Actual curriculum trials are reported separately.

Validation: `data/mcp-basement-verification/approved-workflow-complete-check.log` records strict Clippy, 395 passed default Rust tests (20 opt-in probes excluded), the full HTTP/Python/Node pipeline and UI production build. The subsequent audio-routing fixture extension has separate native preflight and fixture/plan-review checks; it does not change native runtime code. The daily offline scan was not due and retained `REVIEW_REQUIRED`; neither this pipeline nor that scan certifies vulnerability freedom.
