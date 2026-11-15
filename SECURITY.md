# Security boundaries

This is an educational, single-process harness. Never treat model output, a judge's prose, or a document as authority to change permissions or execute code.

## Boundaries implemented

- Entra access-token verification, delegated API scope and native-client allowlist; run history, snapshots, cancellation and deletion are owner-scoped.
- Unknown signing-key refreshes throttled; no wildcard CORS authorization; cache-control no-store on responses.
- Local authentication requires loopback binding. A tunnel/reverse proxy can still expose it. Keep the execution daemon private; a public introduction site is a separate artifact. Authentication alone does not make the daemon ready for public operation.
- Approved server-side provider hosts, HTTPS, no redirects, no credentials in URLs, response/prompt/event limits. Explicit extra host approvals are operator-only.
- Model and role allowlists; typed plans/routes/reviews; immutable completion criteria; deterministic user acceptance gates can veto a model.
- Deterministic admission and tool-result classification rejects apparent direct email addresses, US SSNs, payment-card PANs, and identified health data before journal/context/event persistence. Policy discussions containing only terms such as PII, PHI, HIPAA, or PCI remain allowed.
- The observability API exposes correlated model-call state, context-handoff metadata, issues, and bounded feedback loops. Context bodies and private chain-of-thought are never duplicated into observability events.
- Run-wide call/round/deadline limits, one active run per user, two globally, six new admissions per minute per user.
- SQLite checkpointing before calls, correlated call IDs, duplicate submission keys and no automatic inference replay after ambiguity or restart.
- Native access tokens stay in memory; provider keys stay server-side. Model outputs are rendered as text, not executable HTML.
- Cloud inference and question escalation are disabled by default, even when credentials exist. The local launcher enforces `GENTS_LOCAL_ONLY=1`. The legacy OpenAI escalation adapter requires an explicit operator `GENTS_LOCAL_ONLY=0` override before reading its server-side key; it is outside the free local product path. Model output cannot select that override or authorize an action.
- The owned MCP crate separates stdio/SSE transport from native tools. Current actor/phase policy, confined workspace I/O, source/API identities, native receipts and independent acceptance remain required. The Rust code sandbox currently fails closed outside macOS; changing GPU hardware does not create platform isolation support.
- Factory blueprints are owner-scoped composition previews with current planning-revision and definition-of-done checks. They do not install policy, authorize execution or certify compliance. Unsupported architectures, unknown tools and missing control profiles remain explicit gaps.
- Runtime-created skills are declarative compositions of existing catalog tools. They cannot add an executable, permission, provider, or policy; each composed step is separately reviewed and executed.
- The local-code-repair skill can read and patch ordinary allowlisted repository files after Admin Judge review. Patches require the previously observed SHA-256, create a run-private backup, run fixed checks, roll back on failure, and use supervised checkpoint-preserving restart. Authentication, provider, secret-loading, sensitive-data enforcement, and global-policy files are excluded and require human review.

## Data handling

The local journal stores run owners, request/configuration hashes, idempotency keys and event/output content. It does not intentionally persist API access tokens or provider secrets. Events can contain sensitive task content or generated material. The SQLite file is chmod 0600 on Unix; this is access control, not encryption. Use FileVault or encrypted disks, protected backups and an appropriate retention policy. User deletion removes the accessible record and empties its payload; small ID tombstones prevent late writes resurrecting deleted runs. Old copies may remain in backups/filesystem history. Do not claim cryptographic erasure.

Retention cleanup happens at startup (default 168 hours) and a 20-run global cap evicts the oldest terminal record on admission. Idempotency protection lasts only while its run record is retained. Persistent storage requires a durable writable volume; a container's ephemeral filesystem is not a durable backup.

## Production work still required

The controls above are defense-in-depth and compliance-readiness evidence; they are not a HIPAA, PCI DSS, SOC 2, or legal compliance certification. This build deliberately avoids raw payment-card scope and fails closed on detected direct PII/PHI. Enterprise handling still requires a documented data inventory, lawful basis/BAAs where applicable, validated de-identification or tokenization, encryption and key management, workforce/access procedures, incident response, vendor assessments, retention/backup controls, independent testing, and qualified legal/compliance review.

Use the deployment's actual identity/tenant/ACL policy, native MSAL on iOS, TLS and a fixed release API origin. Add a shared transactional database/queue before multiple replicas or cross-machine execution. Add edge quotas for unauthenticated traffic; per-process limits reset on restart. Review installed native tool permissions and platform isolation before expanding tools or side effects. A required substring is not proof of correctness or safety. Never allow model text to become a shell command.

Run dependency-advisory scans (including Rust `cargo audit`) and review updates before release. See the GitHub Actions checks for the exact source revision. Report suspected vulnerabilities using [GitHub private vulnerability reporting](https://github.com/Monzingo89/gents-rust-lab/security/advisories/new), or email robertjmonzingo@gmail.com if that route is unavailable. Do not post keys, tokens or private prompts in public issues.
