# Local API authority and browser origin boundary

## Finding and change

Before this change, the local API returned HTTP 200 for `/api/config` with an
untrusted `Host` or `Origin` header. Existing CORS response policy did not reject
those requests at routing. No exploit against a third-party site or live task
was attempted. Read-only requests established the baseline on an isolated
evaluation backend.

`local_ingress.rs` now owns one responsibility: request admission for local
authentication. It validates the listener's loopback authority, explicit
canonical origins, browser fetch metadata and a Referer fallback before routing.
Unknown authorities, mismatched absolute request targets, duplicate authorities,
unapproved/opaque origins and unapproved cross-site API requests are rejected.
`X-Forwarded-*` is not trusted. Same-origin browser use, explicitly configured
development/Tauri origins, CORS preflight and native local clients remain
supported. Wildcards, credentials and paths are not accepted as configured
origins. Configuration is checked before persisted work can launch.

The module has no model or storage dependency. Its middleware runs outside
the CORS layer so an untrusted preflight cannot bypass it. Entra bearer
authentication remains unchanged; this new policy applies only to local mode.
The ingress and existing authentication modules are excluded from ordinary
autonomous source repair.

This follows the separation between CORS and server-side request checks in
the [OWASP CSRF prevention guidance](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html).
It is defense in depth, not a new regulated-data authorization or a claim that
all browser/CSRF attack variants are covered.

## Verification

167 enabled Rust tests passed, including negative authority/origin cases and
an actual loopback HTTP middleware test. Its handler counter proves that five
rejected requests never reached the handler; native and approved-origin
requests and preflight passed. Seventeen native model tests remain opt-in.
Strict all-target Clippy, the locked harness build and 32 focused Python tests
passed. No dependency was added.

The office listener had zero runs in its separate `office-view.sqlite3` journal.
Only its verified PID was stopped to apply the change. The first startup met
the departing process's journal lock and exited safely; after confirming that
the old owner and listener were gone, startup succeeded with the same store.
No journal, company, checkpoint or task was deleted. The scheduler remains
disabled and inference remains offline/local-only.

Live checks on the hardened `127.0.0.1:8788` listener:

| Request | Result |
| --- | --- |
| Native GET config and run history | 200, local mode and zero office-view runs |
| Same-origin company status POST | 200, existing company data returned |
| Untrusted Host on config GET | 403 |
| Untrusted Origin on config GET | 403 |
| Opaque Origin on a nonexistent-run cancellation POST | 403, no task target |
| Cross-site fetch metadata without Origin on config GET | 403 |
| Approved localhost development-origin preflight | 200 |

Chrome was refreshed after restart. It rendered Office Open with sixteen Gents
and eleven floors. Operations selection and return to the exterior worked,
and the walking view remained enabled. The client flow is config/company
status from the local API into roster/company rendering; the tour is a
visualization, not proof that real company tasks are currently executing.
No console/network-inspector capture is claimed. HTTP responses, native UI
state and the actual company status response supply separate boundary evidence.

## Remaining security scope

Local authentication still trusts same-user native/OS clients. Header-free
native requests are intentionally supported, and same-origin XSS is not solved
by this boundary. This does not provide service identity isolation, encryption,
tamper-resistant audit, distributed authorization or production compliance.
The synthetic-only profile and independent policy/data gates remain required.
