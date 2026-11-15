# Graph-owned planning capabilities

The outcome-only challenge exposed repeated contradictions between the CEO's native graph and its separate staffing tool lists. This change makes explicit graph ownership the source of draft capabilities for fixed-input composition. It preserves the original task, transformation choices, expected results, independent roles and approval boundary.

## Planning protocol

The local decoder now generates status and clarification first, then native contracts, staffing and completion criteria. Stage tools precede descriptive task prose. The existing `InferenceWire` serializer uses schema `required` order only on the inference wire; canonical values, journal encoding and approval hashes retain their existing serialization. Upstream's [schema-to-grammar object rule](https://github.com/ggml-org/llama.cpp/blob/master/common/json-schema-to-grammar.cpp) illustrates why field order affects constrained generation. This is a protocol improvement, not proof that order alone makes planning reliable.

Question and ready shapes are disjoint. A question has a nonempty question, no assignments, no completion criteria and no source/artifact work. A ready plan has an empty question, nonempty bounded staffing and completion criteria. Native runtime validation still checks the complete proposal after decoding.

For rosters of at most four actors, the ready decoder enumerates bounded staffing tuples with distinct actors, retaining every permitted order and one-to-four-stage choice within the roster limit. It does not choose the actors or force the synthetic task's stage count. Larger rosters retain runtime uniqueness validation without a combinatorial decoder expansion. Fixed-input references in graph mode must have an explicit owned source declaration before capability compilation; this prevents a missing input declaration from surfacing later as a misleading missing-grant error.

Fixed-only attached inputs expose `capability_mode: native_graph`. In this mode the CEO must explicitly declare each source's actor, each artifact's writer, and the ordered distinct staffing actors. It supplies empty staffing tool arrays. The pure compiler derives each stage's tools solely from those source calls and artifact writers. A source owner receives its declared registered source tool; an artifact writer requests `files:write` and `files:read`. There is no shell, network, download, memory-write or generic execution grant introduced by compilation.

The compiler rejects a second capability list, unknown or missing owners/writers, duplicate stages, and stages with no declared graph work. Existing compilation still checks source immutability, allowed pure derived tools, empty receipt-bound data inputs, valid projections, cycles, actor ordering and exact native bindings. Unknown or invalid graph calls do not become executable by acquiring a draft tool name.

Every compiled capability set is recorded separately in `host_compiled_capabilities`, retained alongside the actual raw local CEO message and shown in the transcript. Independent review checks that compilation retained the CEO's staffing, stage criteria and transformations, that owners/writers were explicit, and that the compiled tool sets are reviewable. The frozen effective draft goes through the same exact owner approval and policy installation as before. Deriving an unapproved draft capability list installs no runtime authority.

Template-based and other existing planning flows retain their original capability representation. Historical plans remain readable. The new mode does not silently rewrite older proposals or choose agents or transformations for the model.

## Repair and resource bounds

Review now names capabilities unused by the CEO's own graph rather than returning a vague scope mismatch. This diagnostic uses actual declared work, not the evaluator's hidden expected graph or numeric answers. Role-separation feedback restates the user's requirement for analysis followed by a different output actor; it does not select either actor.

User-message content retains its 12000-byte budget. Saved CEO drafts have a separate bounded transcript budget of 96 KiB, with at most eighteen messages and the existing nine-revision ceiling. Previously, retained ready drafts consumed the user-message budget and blocked a remaining repair turn. Old drafts remain excluded from inference facts; increasing retention does not inject their prose into execution requirements.

Native local execution provenance now includes a bounded recognized `finish_reason` when supplied by the local server. Older records omit that optional field unchanged. Clarification output remains capped at 256 tokens. Measured `length` terminations showed that the earlier 1536-token allowance could truncate explicit graph/role plans. Fixed-input composition now allows at most 2048 output tokens; other planning remains at 1024. The cap does not force generation to consume its full allowance. This is a fixed bounded allowance, not an automatic replay or unlimited budget increase. All completed replies, including failures, retain measured usage in the canonical audit.

## Validation boundaries

Tests verify wire ordering with value/hash preservation, disjoint question/ready bounds, retention and owner-message limits, bounded output allowances, explicit ownership, compiled scopes, rejection of redundant grants, and independent reviewer detection of missing ownership, hidden compiled scopes or host-edited staffing. Real trials continue to use the unchanged third-level fixture; failures remain failures until a complete run and its native evidence pass independent admission.

## Measured trials

Three new unique planning conversations completed 27 actual local replies, including their same-session repair attempts. Canonical planning audit rows matched the saved usage and execution provenance: 124952 input tokens, 24421 output tokens, zero paid-provider charges. No task was approved or issued from these conversations. Electricity and hardware are excluded from provider charges.

The latest full transcript snapshots are:

- `data/mcp-workflow-check/234f77c3-f24a-4819-b032-7de574375cd5/transcript.html`: graph-first trial and bounded recovery; remaining redundant capability requests were rejected.
- `data/mcp-workflow-check/c18cb873-60a0-4734-b2a2-d49fcf7b365a/transcript.html`: graph-owned capabilities; combined source/output duties and a truncated repair remained unsuccessful.
- `data/mcp-workflow-check/ccb0e948-f073-4725-8680-729cadebf01f/transcript.html`: distinct staffing and explicit-root diagnostics. A two-stage draft assigned only imports to the first actor while combining transformations and writing under the second, contrary to the requested independent analysis/output roles. A later missing fixed-input declaration was rejected. The conversation exhausted its revision ceiling.

These are failed planning trials, not completed task runs. The second challenge remains qualified from its earlier evidence; the unchanged outcome-only third challenge remains unqualified. The journal retains its terminal failure records rather than rewriting them after diagnostic recovery. The latest repair transcripts above preserve later turns from the same sessions. `data/mcp-basement-verification/graph-owned-summary.json` contains their audit-checked measurements and report paths. Repeated trials do not demonstrate arbitrary-task competence or model training.

Final verification passed: 274 Rust tests, 17 opt-in tests ignored, strict Clippy, all smoke/Python/Node checks and the Vite build. The complete log is `data/mcp-basement-verification/graph-owned-final-all-checks.log`; the summary contains current source hashes. The office was refreshed after confirming no active runs, and its current API confirms local-only mode, SSE MCP and nine native functions. No challenge listener or active curriculum attempt remains. The broader company goal remains active.
