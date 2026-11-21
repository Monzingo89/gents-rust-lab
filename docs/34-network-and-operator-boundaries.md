# Public network admission and canonical operator input

## Outcome

Two independent boundaries were repaired. Public HTTP transports now share a
versioned address policy instead of diverging Python and browser predicates.
Accepted operator answers now survive stale worker checkpoint writes. The full
local `bash scripts/check.sh` passed after a retained failed run exposed the
second issue. No paid inference, real banking effects or external publication
occurred. The previous ten-model financial result was not rerun or relabeled.

## Public HTTP policy

The previous text adapter already validated DNS answers and pinned its socket.
The browser helper also pinned addresses, but omitted 6to4, benchmarking and
new documentation ranges. Reproducing its full original BlockList showed that
mapped loopback was already rejected through Node's IPv4 mapping; a reduced
IPv6-only diagnostic was not evidence to the contrary. Python classified the
well-known NAT64 prefix as global, consistent with the registry; the new
public-native profile deliberately rejects translation mechanisms rather than
treating that library result as sufficient admission.

`config/public-network-policy.json` is now the reviewed source for separate
Python and Node address-admission modules. The conservative profile admits
native IPv6 global-unicast space with special-purpose exclusions, rejects
mapped/translated addresses and known private/metadata ranges, and can deny
some globally reachable special services. Every DNS answer must qualify;
mixed or oversized sets fail closed. HTTPS, port 443, original TLS identity
and validated-address pinning remain required. Browser HTTP transport is a
separate module, strips credential/Host headers case-insensitively and retains
its response bound. Text requests with nonempty caller-supplied headers cannot
follow redirects, regardless of header casing. Every unauthenticated redirect
still passes the destination policy on its next connection.

The policy, transports, browser helper, fixtures and security tests are protected
from ordinary autonomous source repair. Private integrations must use separately
reviewed, authenticated adapters, not exceptions invented by a web skill.
The policy is an engineering profile informed by the
[IANA IPv4](https://www.iana.org/assignments/iana-ipv4-special-registry/),
[IANA IPv6](https://www.iana.org/assignments/iana-ipv6-special-registry/) and
[OWASP SSRF guidance](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html).
It is not a claim that all admitted addresses are currently routable or that
the entire host/browser has an OS-enforced egress firewall.

Seven Node and five Python security tests pass. Forty-seven shared address
vectors produce matching decisions in both runtimes. Mocked transports prove
mixed-DNS rejection before socket creation, numeric pinning with the original
TLS name, per-hop rejection, case-independent credential handling and response
limits. No private service was contacted; TLS certificate validation itself
was not replaced in the production code. CI and the local check script now
include these offline regressions. Remote CI has not been run or published.

## Operator answer race

The first broader check failed `autonomy_smoke.py` after cancel/resume. A new
deterministic regression reproduced the underlying lost update: a worker loads
an unanswered checkpoint, the operator answers, and the worker saves its old
snapshot. The previous implementation returned null instead of `Owner Project`.
The initial failure is retained in this record; increasing the timeout or
resetting a task was not the repair.

`operator_answers.rs` provides the concrete `FileAnswers` storage adapter.
Authenticated API identity is passed by the runtime, not model JSON.
Receipts bind company, run, question identity/text hash, owner, answer and time.
They are synced and atomically published without replacing a concurrent winner,
inside a 0700 directory as 0600 files. Loads merge the canonical receipt into
an unanswered snapshot. Workers no longer overwrite owner input. Existing
inline answered checkpoints remain readable. Cleared questions do not acquire
old answers; new question text produces a different binding.

The regression now passes. Additional checks reject duplicate publication,
changed scope/content and receipt substitution, and prove one winner for
concurrent replies. The real HTTP mock-model smoke then passed question
survival, restart, cancellation, authenticated resume, stale-answer rejection,
sandbox file/shell output and non-replay of an uncertain email intent. These
are actual lifecycle/tool checks with simulated model decisions, not new native
model capability evidence. No uncertain external action was automatically sent.

## Verification and rollout

The complete local check script passed: 169 enabled Rust tests, strict workspace
Clippy, locked build, HTTP lifecycle/recovery/company/approval smoke tests,
Python suites, Node suites, web syntax and Vite build. Seventeen native model
tests remain opt-in. A separate focused run passed 37 Python tests. The smoke
fixture now copies only the new network module and public policy alongside
the broker, keeping its disposable root isolated from user data.

Only the verified idle office listener was restarted, using its unchanged
`office-view.sqlite3`, offline inference and disabled scheduler. Chrome was
refreshed: Office Open, sixteen Gents, eleven floors and walking activity remain.
The existing company-status POST succeeds, run history remains empty in the
office view, and an untrusted Origin still receives 403. No task, checkpoint or
company journal was deleted. The sole remaining harness is the office view;
no model server or evaluation process remains running.

Receipts are immutable through this repository, not retention-locked against a
hostile same-user OS process. Application encryption, real service identities,
full browser/network isolation, external-effect guarantees and an explicitly
scoped compliance review remain unproven. These repairs must not be presented
as regulatory certification or frontier equivalence.
