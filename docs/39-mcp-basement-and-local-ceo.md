# MCP basement, transports and local CEO planning

This is a bounded local research implementation. It does not establish that the
company can handle arbitrary tasks, migrate every legacy tool, or autonomously
promote source. Existing uncommitted project work is preserved.

## Run locally

Build with `cargo build --workspace --locked --offline`. For the existing company
launcher, set `GENTS_MCP_TRANSPORT=sse npm run company`; keep that terminal running.
Use `GENTS_MCP_TRANSPORT=stdio npm run company` to choose the original transport.
The default is stdio. The launcher builds both Rust binaries offline, preserves
an explicitly supplied transport across local integration-file reloads, and
reports a mode mismatch if an already-running office needs restarting.
This switch changes the native MCP client, not legacy Python
broker tools. No model download, paid provider or remote endpoint is introduced.
The dedicated bounded evaluation is `node scripts/mcp-curriculum.mjs`; it owns
ports 8796/8797 and refuses an occupied port. Its default transport is SSE. If the office already owns the curriculum journal, use
`node scripts/mcp-curriculum.mjs --existing`; this validates the local-only native
configuration and uses the running office at 8788 without stopping its listener.
Use `--case 04-outcome-only-routing` to run just the later routing challenge.

The office now has B1 beneath the lobby. Select **B1 MCP / SERVER BASEMENT** to
inspect the Rust registry, source modules, input schemas, allowed implementation
phases, read-only declarations and external-effect flags. The HTTP catalog
`GET /api/mcp/catalog` is public implementation metadata, not an agent's grant
list. MCP `tools/list` remains actor/phase/current-policy scoped. No tool payload,
credential or private run is returned by the implementation catalog.

## Architecture and transport boundary

`mcp/src/protocol.rs` owns JSON-RPC lifecycle; `transports/stdio.rs` owns bounded
newline framing; `transports/http.rs` owns loopback Streamable HTTP with one SSE
response per POST. `Registry`, `Tool`, `Authority` and `Workspace` are shared
across both transports. Safety policy now lives in `mcp/src/safety.rs`; the
harness re-exports it. MCP source has no path import back into harness Rust code.
Compile-time company contracts/approved skill assets remain monorepo resources.

SSE here uses the current [Streamable HTTP transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports),
not the deprecated 2024 pair of SSE/message endpoints. The one `/mcp` endpoint
accepts JSON-RPC POSTs, returns `202` for notifications, SSE for responses, and
`405` for independent GET streams/DELETE. There is no event replay, independent
server notification stream, remote service discovery or pooled multi-session
server. A fresh process has exactly one trusted company/owner/run/actor/phase.

The harness creates a private random bearer token, clears the child's environment,
passes only that token, and reads a non-secret ephemeral loopback endpoint from
stderr. URLs are parsed and restricted to `http://127.0.0.1:PORT/mcp`; proxies and
redirects are disabled. Every request checks the exact Host, bearer, optional
protocol version, and initialized-session ID. All browser Origins and forwarded
host headers are rejected; security-header duplicates are rejected. Request and
response sizes, child lifetime and cancellation are bounded. Same-user malicious
OS processes remain outside the trusted-host boundary. Do not expose this local
adapter through a proxy, LAN binding or public tunnel.

For manual launch, the original six identity flags remain required. Append
`--transport sse --bind 127.0.0.1:0` and supply a private random `GENTS_MCP_TOKEN`
through the process environment. Stdio needs neither token nor listener. CLI
stdout is still protocol-only in stdio mode. Tool effects and canonical audit
receipts retain the same policy/approval checks under either mode. A lost write
response is uncertain; inspect canonical receipts and workspace before retrying.

## CEO conversation and authority

The CEO floor includes a local planning form. The company must already be
connected and registered. Backend routes are:

- `POST /api/companies/{company}/planning/{session_uuid}` with `message` and
  `expected_revision` starts/continues a conversation.
- `GET` at that route reads the owner-scoped retained session.
- `GET /api/companies/{company}/planning` lists up to 50 owner-scoped session
  summaries. The CEO floor can load retained questions and proposals.
- An owner may POST `{"op":"abandon","expected_revision":N}` to fence an
  interrupted generating turn. Its transcript stays retained; inference/tools
  are not replayed, and a late response cannot overwrite the new revision.

The legacy advisory hotline now checks the same default-deny `GENTS_LOCAL_ONLY` boundary as the main provider before looking up any cloud credential or sending a request. Credential presence alone cannot enable paid inference. The connected UI displays the effective local-only state.

The selected configured admin model must be native `local`; there is no paid,
remote or mock fallback on this endpoint. The first turn's constrained output
schema permits only a clarification question and empty staffing/acceptance.
Subsequent responses may ask another question or propose testable criteria and
registered agents/tools. Fixed validators reject invented actors, shell/network
capabilities, additional fields, unsafe persistence and over-budget transcripts.
Company SQLite stores messages, revisions, output, local execution metadata,
latency and measured token usage. It retains rejected-output hashes and failure
states. A model response is a proposal, never an authorization or tool receipt.

Intake with `ceo_planning_id` must reference the same owner's ready conversation.
The host copies and hashes the retained proposal into frozen intake; acceptance
must match. The host separately compiles the selected worker’s `finish` lifecycle control
into intake as `ceo_host_controls`; it leaves the model proposal unchanged.
Subsequent policy ceilings cannot exceed that plan plus this frozen completion
control. A missing completion-control name is not a failed native-tool selection. Owner approval
of the exact intake digest and normal governance installation still precede
execution. The initial execution adapter supported **one selected worker** plus
mandatory independent admin/QA controls; multi-agent proposals initially remained drafts. This limitation is being replaced by the ordered-team implementation in `docs/40-ordered-ceo-teams-and-native-bindings.md`.
For these plans the runner skips unrelated planner calls and selects the frozen
configured worker. The bounded curriculum performs the approval/install steps
under the operator's explicit authorization for synthetic local experiments.
The planning form itself does not automatically execute a draft.

Planning, implementation registries, legacy tools and general autonomous routing
remain different contracts. Skills/memory sender grants are not synthesized by
this first CEO adapter. Plans needing those additional authorities or multiple
workers require the next scheduler/policy compilation increment.

## Measured failures and improvements

The first synthetic CEO trial guessed `Hello, World!` without supplied filename
or content. It was stopped before any policy/tool installation. First-turn
clarification is now enforced by schema, not prompt wording alone.

A subsequent trial wrote and read the right bytes but QA repeatedly demanded
that `finish` already be completed before it would review `finish`. That run
was cancelled with its history intact. QA now receives the actual current
admin-authorized completion control, explicitly marked awaiting review and
without external effects. It must still verify all external actions from actual
receipts. Review packets use retained tool evidence rather than stale earlier
QA/self-assessment prose. This removes the circular finish prerequisite without
inventing a successful action or weakening artifact/policy gates.

`data/mcp-curriculum/ATTEMPT/` retains reports, actual CEO conversations, complete
run event streams and a standalone escaped `transcript.html`. Users in this
curriculum are explicitly scripted fixtures. Progression requires independent
exact-byte checks, actual successful native write/readback receipts, and native
arithmetic receipts when requested. It stops on failure; no model claim alone
advances difficulty. Tasks progress from exact file creation through multiple files and arithmetic
to a verified artifact, then an outcome-only brief that leaves tool selection to
the CEO. No code in a candidate workspace is automatically run.

Local inference **does consume tokens**. These reports separate actual local
input/output tokens from provider charges of zero. Hardware, memory, runtime and
electricity are not free. Polling, transcript rendering and fixed checks do not
invoke models. A human/host improvement using Codex is separate from company
inference. No autonomous model training or general continuous-learning success
is claimed by a passing curriculum.

## Verification boundaries

Focused HTTP tests exercise real owned subprocesses and TCP requests: lifecycle,
stdio/SSE catalog parity, arithmetic receipts, revocation, actor impersonation,
Origin/Host/auth/version/header rejection, frame bounds, cancellation and refusal
of public binding. Planning tests cover unknown actors/tools, question dispatch,
durable in-flight fencing, cross-owner reads, recovery, frozen intake and ceiling
expansion. Tower tests cover the twelfth floor and procedural server-room render;
the eleven existing photographic room assets are unchanged.

The native local-model SSE write/readback check completed with nine Qwen3-Coder-30B
model calls, actual READY bytes and zero provider charges in run
`faf8ad00-07ef-42ca-940d-17af45b1c37c` (60.8 seconds). Its report is retained under
`data/mcp-native-file-check/`. The three-step curriculum passed in attempt `1a511b04-27a9-40d0-93e5-244a5544ff49`
(304.1 seconds, 40 local model calls, 119603 measured input tokens and 8118 output
tokens, zero provider charges). The escaped conversation and every public run
event are in its `transcript.html`. These timings are observed, not targets.
Failed/cancelled attempts are retained separately and not relabelled successful.

A target-filtered offline Cargo metadata inspection found license expressions
for all 155 third-party packages on aarch64-apple-darwin, using permissive license
alternatives. This is metadata inspection, not a vulnerability audit or legal
certification. Unfiltered cross-platform metadata could not complete offline
because an unused platform dependency was uncached; nothing was downloaded.
Native safety/function audits remain focused local reviews. Model redistribution
and provenance/license qualification remain governed by the existing inventory;
this change uses an already installed model and does not redistribute weights.


## Harder source-authoring trial

SSE candidate trial `f5ff2e04-7bf3-4d03-90c7-ed2ec918e9a1` passed normal
write/readback and an injected lost-audit-receipt recovery. Each source required
one native model call; recovery did not regenerate or repeat the uncertain
write. This establishes submission/reconciliation, not source correctness.
Both initial source candidates compiled after host review of their pure safe
function bodies but failed the independent UTF-8 acceptance fixture. An initial
revision also failed; merely repeating test examples did not correct its
vacuous character check. Those candidates were rejected for promotion.

Explicit host mentoring supplied the no-progress invariant to the final local
author. Candidate `206c6dd6-15e7-4e6d-9232-63279a141b31` then passed the fixed
Unicode/offset/overflow acceptance suite. This is a **guided repair**, not
independent autonomous discovery. Its review report is
`data/mcp-basement-verification/guided-candidate-review.json`. A redundant guard
remains, and the candidate is still quarantined and unpromoted. No model-authored
code was inserted into the runtime. The generic author needs a better structured
feedback/repair method before its self-improvement can be called reliable.


## Outcome-only routing follow-up

The first outcome-only proposal selected arithmetic and file operations correctly
but omitted `finish`. That exposed a host experiment/compiler issue: completion
is a host lifecycle control. It is now recorded separately in frozen intake
rather than silently rewriting the CEO's proposal. The next attempt exposed a
schema/runtime mismatch: calculation IDs such as `sum_calculation` were consistent
with the advertised schema but the native implementation rejected underscores.
The correlation-label validator now accepts underscores; both native and shared
schemas publish the exact ASCII pattern, with field-specific rejection feedback.
Invalid paths, controls, Unicode labels, excessive lengths and duplicate IDs
remain rejected. No arithmetic range, type or permission boundary was widened.
The stalled read-only arithmetic attempts were cancelled and retained before
fresh verification. This is contract repair driven by actual local failures,
not a model being credited for steps it could not execute.


Outcome-only rerun `9c30fe8c-1bf4-4d89-a147-50dd7df274f2` passed in 90.9 seconds
with 14 native local calls, 42397 input tokens, 1852 output tokens and zero
provider charges. Native request arguments independently matched [125,-38,17]
and subtract=4; ledger.txt contained exactly 100 with native write/readback.
The complete conversation/events are in
`data/mcp-curriculum/e2e5486c-867d-4996-8b22-070f4a938dad/transcript.html`.
Together with the earlier three stages, four curriculum tasks passed.
Final offline Rust tests passed (216 tests; 17 explicit opt-in tests not run),
strict Clippy passed, UI tests/build passed, and the broader check script had
a passing checkpoint. Final source hashes and evidence summary are in
`data/mcp-basement-verification/`. The local office remains available at 8788.
