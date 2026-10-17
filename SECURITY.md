# Security boundaries

This is an educational, single-process harness. Never treat model output, a judge's prose, or a document as authority to change permissions or execute code.

## Boundaries implemented

- Entra access-token verification, delegated API scope and native-client allowlist; run history, snapshots, cancellation and deletion are owner-scoped.
- Unknown signing-key refreshes throttled; no wildcard CORS authorization; cache-control no-store on responses.
- Local authentication requires loopback binding. A tunnel/reverse proxy can still expose it: use Entra before any public access.
- Approved server-side provider hosts, HTTPS, no redirects, no credentials in URLs, response/prompt/event limits. Explicit extra host approvals are operator-only.
- Model and role allowlists; typed plans/routes/reviews; immutable completion criteria; deterministic user acceptance gates can veto a model.
- Run-wide call/round/deadline limits, one active run per user, two globally, six new admissions per minute per user.
- SQLite checkpointing before calls, correlated call IDs, duplicate submission keys and no automatic inference replay after ambiguity or restart.
- Native access tokens stay in memory; provider keys stay server-side. Model outputs are rendered as text, not executable HTML.

## Data handling

The local journal stores run owners, request/configuration hashes, idempotency keys and event/output content. It does not intentionally persist API access tokens or provider secrets. Events can contain sensitive task content or generated material. The SQLite file is chmod 0600 on Unix; this is access control, not encryption. Use FileVault or encrypted disks, protected backups and an appropriate retention policy. User deletion removes the accessible record and empties its payload; small ID tombstones prevent late writes resurrecting deleted runs. Old copies may remain in backups/filesystem history. Do not claim cryptographic erasure.

Retention cleanup happens at startup (default 168 hours) and a 20-run global cap evicts the oldest terminal record on admission. Idempotency protection lasts only while its run record is retained. Persistent storage requires a durable writable volume; a container's ephemeral filesystem is not a durable backup.

## Production work still required

Use the deployment's actual identity/tenant/ACL policy, native MSAL on iOS, TLS and a fixed release API origin. Add a shared transactional database/queue before multiple replicas or cross-machine execution. Add edge quotas for unauthenticated traffic; per-process limits reset on restart. Implement audited tool permissions and isolation before enabling any tools or side effects. A required substring is not proof of correctness or safety. Never allow model text to become a shell command.

Run dependency-advisory scans (including Rust `cargo audit`) and review updates before release. CI is supplied but has not been run on your GitHub account. Report real vulnerabilities privately to your own project's maintainer; do not post keys, tokens or private prompts in issues.
