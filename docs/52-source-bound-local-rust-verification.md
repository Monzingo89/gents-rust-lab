# Source-bound local Rust verification

This milestone extends the local MCP curriculum from data transformations to a company-authored Rust function. It does not promote generated source into the application, train weights, or establish general software or regulated-industry competence.

## Boundaries and architecture

`mcp/src/rust_verification/` separates frozen-check contracts and receipt-bound completion from syntax admission, process isolation, and verification request identity. `mcp/src/functions/rust_verify.rs` is the registry adapter. The existing stdio/SSE host adapter invokes the same registry and policy checks; actor identity, owner approval, frozen test path/hash and policy epoch never come from tool arguments. The caller supplies only `source_path`.

The owner freezes `required_rust_checks` before execution. Test paths cannot be candidate paths and are protected from agent writes. Candidate and frozen acceptance bytes are read through the descriptor-relative confined workspace adapter. Completion requires a native PASS for current source/test bytes and current policy plus a matching canonical completed receipt. A successful transport/tool invocation returning `passed:false` does not satisfy completion. Source edits or policy changes invalidate earlier acceptance.

The syntax adapter accepts a deliberately narrow safe-function dialect. Candidate imports, modules, attributes, macros, unsafe, async, extern and non-pure standard-library paths are rejected. Frozen acceptance allows only test attributes and recursively inspected assertion macros. This dialect is a restriction on capability, not a proof that all Rust code is safe.

The current execution adapter requires macOS Seatbelt (`sandbox-exec`) and installed Rust/Command Line Tools. It uses separate deny-default compiler and runtime profiles, private scratch storage, no network, no home data, no external writes and no runtime child processes. Candidate library and owner tests compile separately, without Cargo, shell, downloads or dependencies. Processes have CPU/wall/output/file limits and observed RSS bounds; polling RSS is not a hard memory quota. Unsupported platforms fail closed. A portable Linux/Windows isolation adapter remains necessary; generic cross-platform secure execution is not claimed.

The trusted host forwards only the absolute compiler path into the MCP child's cleared environment. It does not forward HOME, provider credentials or other ambient configuration. Rust verification has its own bounded client deadline; other tools retain their existing shorter deadline.

## Planning and retry identity

Owner-selected `planning_mode:implementation` keeps the CEO focused on choosing actors, authoring/checking tools and acceptance, without inventing a precomputed read-only data graph. Data planning retains its existing graph contract. The native tool requires an owner-frozen check for every selected author before intake can proceed.

Verification requests are observations of source bytes, frozen-test bytes, actor, policy epoch and compiled verifier implementation. The harness stores this identity in proposals/results and caps identical attempts. New source or a repaired verifier can be checked without deleting history or relaxing write replay protection. Old blanket rejections do not permanently forbid this stateful check. The identity is not authorization: governance and independent admin review still precede execution.

## Reviewed trial and failure history

The fifth reviewed fixture is `rust-range-union`: a pure inclusive-u32 interval union function. The CEO chooses minimum staffing and available tools. The frontier assistant supplies infrastructure and independently frozen acceptance, never the candidate implementation. Eight tests include edge assertions, input immutability, maximum-u32 cases and 9,261 small-domain interval triples checked with a finite-domain oracle. These tests are visible to the author and are not a held-out benchmark.

The first runner attempt failed before inference because it parsed the plain health response as JSON. Seven real CEO planning replies then stalled on a data-graph requirement; the same planning session reached a ready eighth revision after the implementation-mode repair. One mistaken recovery command used an unknown placeholder session and was rejected before inference; its failed report was retained.

Run `83822cb6-9268-4f77-acbd-83e6c652548d` then exposed the cleared-HOME compiler lookup defect. The run was cancelled for that diagnosed defect, repaired and transport-tested, and resumed with its original approval and checkpoint. An explicit operator answer documented the infrastructure repair without providing code or test answers. Duplicate protection then blocked the retried verification, so a second diagnosed maintenance stop introduced state-bound verification identity. No checkpoint, candidate source, test bytes, grants or definition of done was edited to force success. These interventions must remain visible in any capability metric; this is not an uninterrupted autonomous trial.

The source-bound eight-test native PASS was observed after the second repair. Final independent workflow verification and curriculum admission then passed, using report `data/mcp-workflow-check/562c038b-42ca-4f9f-b1e5-c1314ef7864d/report.json`. The model authored the delivered source; its SHA-256 is `0c761c49a1303fe82e30396d024ae4f932c15d3de9c6559f119ecf3a12d26dbd`. All seven pre-maintenance tool journals retained their exact hashes. The report records 46 completed local replies including eight planning attempts, 230,816 input / 9,351 output tokens, four interrupted/failed run calls resolved by retained history, two resumes, one operator answer and zero unresolved run calls. Provider charges were USD 0; no source was promoted.

A fresh post-repair retest (`170eba88-a510-4d95-bf0a-3b830c5943fd`) failed in planning: the CEO repeatedly assigned two agents to a single function, then repeated an already answered question. A same-session recovery (`e6ca72a3-2ed4-44e2-a7f3-1c741785cc68`) selected one author but omitted the readback grant and reached the session's revision ceiling before execution. Both failures remain retained; neither is a qualified fresh success. The implementation-mode instruction now explains that independent authorization/final QA are already provided by the host and that write/readback need separate operation grants. This changes planning guidance, not fixture acceptance, source code, grants or benchmark labels.

Evidence: `data/mcp-basement-verification/rust-verifier-*-tests.log`, `rust-verifier-full-check.log`, `rust-verifier-retry-clippy.log`, both maintenance snapshots, and the complete per-attempt transcripts under `data/mcp-workflow-check/`. The curriculum admits only independently verified reports; unsuccessful attempts remain retained.

## Source-grounded practice

The publisher's public description of [Micheal Lanham, AI Agents in Action, Second Edition](https://www.manning.com/books/ai-agents-in-action-second-edition) covers orchestration/collaboration patterns, memory, evaluation/feedback and metacognition harnesses. This implementation applies explicit orchestration, scoped tools, independent checks and observable recovery. This is a mapping to the public description, not a claim to have read the full book or copied its implementation.

The [MCP transport specification](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2025-03-26/basic/transports.mdx) distinguishes stdio from Streamable HTTP and its older HTTP+SSE predecessor. The current local SSE adapter is a pinned compatibility transport, not a claim of conformance to every newer transport revision. Loopback binding, origin rejection and process-owned credentials remain transport boundaries; changing transports must not change tool authority.

## Remaining user goal

Realistic B1 scenery, daily local model-security checks, hardware-aware model acquisition/training adapters, more task families and modality/industry capability measurements remain separate work. Existing governed per-agent memory is not evidence of universal regulatory compliance. Provider/tool charges can be zero while local tokens, electricity, storage and hardware are still consumed. No vulnerability-free or compliance certification claim is made.


## Follow-up planning evidence

The next fresh retest (`7869dd82-27a5-4134-bc3a-9c2c6c34e378`) selected one author but omitted `files:read` in every ready draft. Retained-session recovery (`8a7b9e16-6a18-4c02-aacc-a93cc1772224`) tested describing implementation work before selecting tools; it still split readback to a second actor and reached the revision ceiling. Decoder order alone did not resolve the failure. These reports are retained as failures, with no issued task run.

A separate pure draft compiler in `implementation_planning.rs` now extends the existing native-data readback mechanism to a selected Rust author that already chose `files:write` and `rust_verify`. It adds only `files:read`, records the addition explicitly in `host_compiled_readbacks`, and produces a draft for normal owner review/approval. It adds no author, writer, verifier, execution call or runtime authority. Native choices remain available, and multiple distinct tasks can still use different actors. Raw CEO selections and reviewed compiled tools are reported separately. The independent verifier checks that the compiled proposal differs from the raw CEO proposal only by this visible required readback operation. Final capability evidence must still come from actual fresh execution; compilation is not proof of autonomous success.


## Native readback completion repair

Fresh run `6feb40a5-6443-44d4-88d6-10fc1ca84f95`, report `25c838f2-b883-4829-8d22-9097abf46f73`, reached a ready plan in two local replies and wrote/compiled a passing candidate. Its runtime marked completion despite never executing the required source readback. Independent evidence admission rejected it, so the curriculum retained FAIL. The automated completed state and failed independent report both remain unchanged; this is a caught false positive, not a successful fresh trial.

Native completion now requires a full current-byte readback alongside current-byte acceptance. `rust_source_readbacks` captures only actual worker reads of a frozen Rust author's source at offset zero with no truncation, atomically with the canonical file receipt. The final gate checks run, actor, path, hash, policy epoch and matching canonical argument/result hashes. Candidate edits, partial reads, a missing canonical receipt or a changed epoch cannot inherit a readback. The model cannot create this record with prose. This strengthens the host's enforcement of the existing fixture requirement; no acceptance tests or candidate code were changed.

Source reads also use dependency-aware bounded repetition accounting, including pagination arguments. Reading the same state repeatedly remains capped, while editing the source permits its new bytes to be read. Ordinary writes retain their existing replay protection. The earlier recovered PASS was independently reverified after these changes from its existing actual full readback and acceptance receipts, without inference. Legacy stored completion views are historical evidence, not a current-build certificate.

Regression evidence is in `rust-source-readback-gate-tests.log` (five native tests including actual stdio/SSE transport execution and the missing/partial/deleted-receipt checks), `rust-readback-retry-tests.log`, and `rust-implementation-planning-tests.log`. A new full repository check and fresh end-to-end retest are recorded separately; neither is inferred from these unit tests.


## Fresh admitted run after native gating

Run `6f136bc2-12f1-4372-87e2-f0f1aa53c8ea` passed independent verification and curriculum admission with report `data/mcp-workflow-check/a836e926-c733-4110-930f-eae249ec0ec2/report.json`. The local model authored new source with SHA-256 `fd72fb56364ee15d332604cb44d101558abeb992771edf74d8711b2815c47798`. The owner acceptance hash stayed unchanged. The final native gate included both current-source acceptance and an actual full-source readback.

Measured completed replies: 22, including four planning replies; 112,836 input / 4,159 output tokens. One model call failed without a completed usage reply and was resolved by bounded in-run recovery; its unknown token usage is not included in those completed-reply totals. There were no manual operator answers, maintenance resumes or unresolved calls. The scripted planning review rejected one redundant staffing draft. Execution also retained four blocked duplicate-read proposals and one governance denial before finishing. Two real verification calls and two full readbacks were made. This is successful recovery under host controls, not perfect efficiency or an uncoached planning benchmark.

The full repository check passed with 302 Rust tests (17 opt-in ignored), strict Clippy, smoke/Python/Node checks and the Vite build. The office was restarted only after checking it had no active jobs, using the same local journal/config/ports and the normal launcher with its attached daily scanner. The current browser/API catalog has ten native functions, local-only inference and SSE; the room is decorative and does not itself execute a task.
