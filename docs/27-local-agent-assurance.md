# Local agent assurance boundaries

## Decision

The local synthetic banking pilot passes its measured functional and scoped
governance tests. It is not approved for real banking, healthcare or cardholder
data, and is not a certified enterprise deployment. Keep the explicit
synthetic-only profile and production refusal enabled.

On 5 November 2026 the owner explicitly selected "Synthetic local research
only." The [completion audit](35-goal-completion-audit.md) therefore accepts
the local research goal, with the exclusions and measured limits in this
report unchanged. This is not acceptance of a real banking deployment.

The easy and medium methods papers are linked from
[the campaign journal](24-local-agent-campaign.md). The latest accepted hard-stage paper
is `data/capability-campaigns/hard-b47513bdc4bf/hard-stage-paper.md`. It records
ten cases, ten distinct models, 96 fresh local calls, ten source-linked memos
and USD 0 inference provider charges. The goal's broader model/compliance
qualification must not be inferred from those finite pilot results.

The prior attempted run `hard-31531af8e7e2` failed the final risk readback
despite passing the actual triage decisions. The model twice changed a hyphen
to an underscore; both replies were rejected and attempts were not reset.
Field-independent literal-vocabulary decoding now passes a three-call native
diagnostic; fresh integrated trial `hard-b47513bdc4bf` then passed 96 new calls
and all ten memory consumers, including exact `bank-policy` readback. See
[the retained failure and methods](32-memory-driven-workflow.md). Older accepted
receipts do not override this current qualification gap.

## Measured controls

| Boundary | Evidence | Important limit |
| --- | --- | --- |
| Master delegation | Actual 4B grant/revoke probe, owner ceiling enforcement and stale-revision denial | Model proposals cannot install a policy outside the owner's ceiling |
| Tool and skill access | Runtime identity authorization, operation scoping, unassigned skill and ceiling-expansion denials | General MCP and external-service adapters still require their own review |
| Memory scope | Owner/tenant/agent denials, per-source current-policy checks, source hashes and explicit archive recall | Shared task/evidence is not a secret compartment; same-user host access is trusted |
| Live revocation | `hard-459f9c1c9b9f` cancelled a native call and stopped its backend before any stale output/checkpoint committed | One integrated local scenario, not distributed revocation |
| Restart and bounded replay | `hard-027984278068` process-group kill/restart recovered the schema checkpoint and preserved predecessor hashes | Recovery passed; that whole task later failed QA. Only read-only local effects are replayable |
| Clock and controller fencing | Monotonic workflow deadlines, kernel controller epochs; summary recipient locks and non-serializable ownership guards | Component tests do not prove every sleep, filesystem or crash window |
| Memory-consumer injection | The original mixed-context 4B fixture failed; the source-only worker repair passed five real local calls including source/advisory attacks and revocation | One bounded native scenario, not a general injection-resistance claim |
| Integrated memory consumption | All ten models selected a memo reference and read an authorized original-source field in `hard-415cb1d2f001`; twenty native calls and ten additional DAG checkpoints | Assurance readback, not proof every financial decision uses summaries or that summary prose is entailed by sources |
| Actual memory-driven decisions | `hard-a2a9ec3bb15d` passed all ten triage decisions from governed original reconciliation records, bound to accepted predecessors, current grants and native task fingerprints | One downstream stage, not every financial decision or semantic-summary entailment; trusted-host receipts are not tamper-resistant attestation |
| Financial integrity | Signed integer arithmetic tool, independent Rust/Python reference checks, altered-amount QA challenge and eight native QA diagnostics | Synthetic reconciliation only, not advice, settlement or a real account decision |
| Release gate | Independent admission covers all 256 seven-control/scope combinations; source-derived decoder capacity supports all blockers | Expanded native model trials failed at 19/33 then 25/33, including a false allow; outputs remain rejected and the model is not broadly qualified |
| Opt-in policy compilation | Two local model-authored normal/suspended programs passed 768 program checks; the integrated model-authored program passed 384 independent checks and actual-input execution | Two model replies are not 768 model judgments; bounded DSL correctness does not establish general model qualification or production permission |
| Data handling | Known email, US identifier and Luhn card-pattern gates plus private file permissions | Pattern heuristics are incomplete DLP; no reliable general PHI/PII classifier is claimed |
| Local API request boundary | Untrusted Host, Origin, opaque-origin POST and cross-site metadata requests return 403 before routing; native/same-origin company status and approved preflight pass; refreshed Chrome building and navigation work | Trusted-host local mode, not hostile-OS-client authentication, general XSS/CSRF assurance or production identity isolation; [evidence](33-local-api-boundary.md) |
| Public HTTP destination policy | Shared versioned Python/Node policy, 47 concordant address cases, validated-address pinning, redirect checks and credential stripping; tests fail before private socket creation | Conservative public-native HTTP profile, not a host/browser firewall or authorization for private integrations; [methods](34-network-and-operator-boundaries.md) |
| Canonical operator answers | Deterministic stale-checkpoint regression repaired; private immutable-through-repository receipts and actual cancel/restart/resume smoke pass | Authenticated runtime writes, not model-created authority; trusted OS filesystem, not retention-locked storage |
| Provenance | Ten distinct pinned publisher/base identities, actual weight hashes, reviewed card/license snapshots and role readiness receipts | Not a training-corpus audit, bias guarantee or legal license opinion |

## Dependency scan and repairs

Installed the standard `cargo-audit` 0.22.2 scanner. The initial scan of 185
locked dependencies used RustSec database commit
`965bf9ad083b58e6340d65829d85217b18be9290`, containing 1,403 advisories and last
updated 3 November 2026. It found two certificate-trust advisories:

- `RUSTSEC-2026-0382`: `rustls-webpki` 0.103.15.
- `RUSTSEC-2026-0381`: `webpki-roots` 1.0.9.

Targeted lockfile updates changed only those package versions/checksums to
0.103.16 and 1.0.10. The next `cargo audit --json --no-fetch` found zero
vulnerabilities and no warnings against the cached database. All 154 enabled
Rust tests passed afterward; fourteen native opt-ins are separate. The fresh
ten-model evaluation had already launched using the preceding binary and
loopback HTTP; its receipt is not misrepresented as a TLS regression test.

This is a known-advisory scan, not proof that dependencies are vulnerability
free. No model license or operating-system security certification follows.

## Production admission remains denied

Before regulated data or real enterprise effects, an accountable operator must
define the intended jurisdiction, data classes, deployment and relevant
standards. Missing controls include reviewed identity/service isolation,
application/backup encryption and key management, recovery drills, protected
retention/legal holds/deletion, tamper-resistant audit export, network egress
controls, external-tool idempotency, data minimization, qualified privacy/legal
review, broader model qualification and ongoing incident ownership. Local auth
is a trusted single-user development mode, not a multi-user security boundary.

The application must not manufacture a compliance verdict merely because a
model is named security, risk or CEO. Future security changes require measured
negative tests and independent approval, not unrestricted self-editing of
policy code. Current source-patch protection keeps governance and persistence
boundaries outside ordinary autonomous edits.

The expanded release diagnostic, stale-readiness rejection and repair-path
alias hardening are documented in
[the release-gate methods and results](29-release-gate-qualification.md).
Earlier narrow model passes must not override that broader failed evidence.

The current [completion audit](35-goal-completion-audit.md) records the owner's
research-only scope and accepts the three experimental stages. The earlier
unanswered scope question is resolved; no blanket compliance is asserted.
