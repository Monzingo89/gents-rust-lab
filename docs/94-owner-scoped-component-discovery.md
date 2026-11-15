# Owner-scoped component discovery

The basement can expose stored local Rust interfaces to an owner and capture a small selected subset in a CEO conversation. Discovery does not execute a library, stage its source, approve a job or promote a tool. This adds a planning interface to the earlier finite reuse qualification in [93-real-local-component-reuse.md](93-real-local-component-reuse.md); it is not a new task-capability benchmark or model-training result.

## Separate responsibilities

- `scripts/mcp-component-store.mjs` lists a bounded number of owner-controlled review package names. Existing immutable package verification still checks all sealed bytes, names, hashes, ownership and alias restrictions.
- `scripts/mcp-owned-catalog.mjs` checks the historical fixture snapshot and asks the owned Rust parser to describe the exact source. It emits function schemas, ABI encoding, source/API/package identities and inspector hash. It never selects an invocation operation. Source bodies, acceptance bodies and historical conversations are excluded.
- `crates/harness/src/owned_catalog.rs` is the owner disclosure adapter. `GET /api/companies/{company}/components` authenticates the caller, checks destination ownership, and includes only packages whose origin company currently belongs to that same identity. Ownership checks use read-only SQLite with no schema creation. Foreign IDs, rejection IDs, counts and the private global hash are excluded. The owner view gets its own digest.
- CEO planning accepts only `owned_component_ids`, not a client-supplied catalog. On revision zero the host obtains a fresh owner view, selects one to four unique IDs, and retains at most 12,288 bytes of metadata. Following turns reuse that exact snapshot; changing the selection requires a new conversation. Audit records retain its digest. Unsupported fields and unavailable IDs fail before inference.
- `web/assets/owned-discovery.js` owns the browser presentation/selection lifecycle. B1 displays expandable interface metadata. The CEO panel offers optional interface selection. Connection, company, token and new-conversation changes clear client selections; a generation counter discards stale catalog responses. User strings enter the DOM through `textContent`.

The private inventory is bounded to 32 package IDs, 32 KiB per item and 192 KiB aggregate interface metadata. The HTTP subprocess has a bounded stream and deadline, no inherited credentials, null stdin and suppressed stderr. Its only configured environment is the host executable search path. The Rust description helper runs from a private byte snapshot with its hash reported. No new actor MCP capability is registered; the native tool catalog remains thirteen functions and existing stdio/SSE authority remains unchanged.

The selected snapshot is historical conversation evidence. It does not become a live ownership grant when an origin company changes owner. Actual invocation must still establish current owner/run/actor policy and frozen source/API/function/input identities through the existing native adapter. A ready plan proposing `owned_rust_call` from discovery alone is rejected until owner-prepared invocation inputs are present. Source staging and binding preparation remain separate from this discovery feature.

## Verification scope

`mcp-owned-catalog.test.mjs` uses explicitly simulated historical metadata. One source has a nonterminating function body: description succeeds because the function is never called. Tests reject modified sealed sources and package aliases and exclude private source/test/conversation markers. These tests establish discovery behavior, not genuine source qualification.

Rust tests exercise foreign-owner filtering, missing authority, selected-ID validation, immutable conversation retention, rejection of injected catalog fields and refusal of discovery-only ready invocation plans. A regression also covers macOS `/var` alias normalization before SQLite's no-follow open.

`owned_discovery_smoke.py` launches a private HTTP harness and isolated synthetic packages. A real owner-filtered endpoint hides the foreign package; foreign company and unknown destinations fail. A selected snapshot survives a deliberately unavailable local planning-policy checkpoint without dispatching inference. Replacing IDs fails without advancing the revision; the run store stays empty. No models, candidate functions or paid services are called by this smoke check.

Logs are under `data/mcp-basement-verification/owned-discovery-*`. Browser and real local CEO observations, when completed, are recorded separately from synthetic protocol tests. The broad goal remains active: automatic owner binding/source staging from the browser, richer component composition, harder task qualifications, model acquisition/training automation, additional hardware isolation and industry assurance remain open. No universal security or regulatory compliance claim is made.

## Actual office and local CEO observation

The full `scripts/check.sh` finished with exit 0: 436 default Rust tests passed, 20 opt-in tests were excluded, formatting and strict Clippy passed, and HTTP/Python/Node/native transport checks plus the production browser build passed. The old idle office supervisor exited 0 after confirming its exact open `data/mcp-curriculum.sqlite3`, six unchanged terminal run IDs and no generating public planning sessions. The office was refreshed to sealed bundle `a77b005ac2ce5900f20c85284b881e842321a1acae5ecc03093e2164ea1c3795`, manifest SHA-256 `afd95c9b6ebd3b1c9ac9b0101605e80e9d466d71924e3fecbca9a0aa456a50c9`, under the unchanged reasoning-repair profile. The new supervisor exec session is `12464`; the live local-only SSE catalog still has thirteen native tools.

Chrome displayed both actual stored components and expanded `select_slots` schemas. The scripted user selected that one component and submitted one form request. Conversation `f5c7ac52-67f3-457c-8386-68a0c3a81d54`, revision 1, retained catalog SHA-256 `65ac557296af3947f6ccdff5457b2b982994e826dd6fdc34dfcb8e931948e349`. Qwen3-Coder-30B returned a valid clarification asking for specific input parameters and successful-completion criteria. It created no staffing, native contracts, policy or task execution. The actual local reply used 7,823 input and 92 output tokens, with 11,005 ms recorded model latency and $0 provider/tool fees. Startup/wait time, electricity and Codex development credits are separate; local inference is not zero token use.

Browser verification then confirmed that the selected checkbox was disabled, a new conversation cleared both choices, and reopening the saved conversation restored its exact selection and messages. No second model request was sent. The public run history remained the same six terminal runs. This demonstrates metadata custody and a first-turn clarification, not semantic understanding of every schema or a newly qualified task.

- [Full actual conversation and observed public steps](../data/mcp-basement-verification/owned-discovery-conversation.html)
- [Verification record](../data/mcp-basement-verification/owned-discovery-verification.json)
- [B1 screenshot](../data/mcp-basement-verification/owned-discovery-basement.jpg)
- [Saved CEO conversation screenshot](../data/mcp-basement-verification/owned-discovery-ceo.jpg)

The daily offline scanner reported `NOT_DUE` with latest status `REVIEW_REQUIRED`. No new downloads or training occurred. The next useful architecture step is a separately governed owner binding/source-staging adapter: select a stored function, preserve exact source custody, freeze requested arguments and source/API identities against the CEO-chosen actor, then review/admit execution. Discovery metadata alone must never implement that authority transition.
