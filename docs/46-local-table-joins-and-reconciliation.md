# Local joins, branching inputs and reconciliation

The company can now combine two native source tables through `table_join`, then feed its actual rows into another tool. This adds a branching-input workflow rather than another linear aggregate example. It remains bounded table processing, not arbitrary-task competence or autonomous source promotion.

## Independent native tool

`mcp/src/functions/table_join.rs` implements a pure inner/left join with no filesystem access, subprocess, network, new dependencies or model inference. Both input arrays use the existing flat scalar record contract (64 rows, 16 fields per row, bounded strings/integers). The caller supplies two key names, `kind` and 1..16 explicit `{side,field,as}` output projections. Output aliases must be unique valid field names. Every key and selected field must exist on every corresponding source row, including rows that do not match. Missing right records in a left join produce null right projections.

Keys use exact JSON scalar equality. Numeric, string and boolean keys are distinct; null matches null. This is an explicit local JSON-table contract, not SQL null semantics. Duplicate matches multiply; duplicate source records remain distinct. Output order is original left order, then original matching right order. A left join retains each unmatched left record once. `matched_left_rows` counts source left records with a match, not expanded result rows; `unmatched_left_rows` counts those without one.

The result has `rows`, `left_rows`, `right_rows`, `matched_left_rows`, `unmatched_left_rows` and `exact:true`. More than 64 output records or 16384 canonical input/output bytes fails without a successful partial result or receipt. Iteration is bounded to 4096 key comparisons. The declared result schema and tool metadata come from the independent MCP registry and are exposed in B1, with the same implementation and authority for stdio and loopback SSE. A new tool grant is required; old actors gain no join permission implicitly.

## Real company challenge

```sh
cargo build --workspace --offline --locked
node scripts/mcp-workflow-check.mjs
```

`mcp-workflow-check.mjs` is a workflow runner separated from the fixture in `scripts/fixtures/mcp-reconciliation.mjs`. Future challenge requirements, source attachments, dependency edges, approved artifact contracts and independent expected outputs can be supplied as fixtures without adding conditional modes to the earlier team script. Current selection is a static import of this reviewed fixture, not model-selected executable code.

The fake user asks the CEO to reconcile synthetic sales and catalog files. Both original file contents are operator fixtures pinned by SHA-256 before planning. The CEO must choose two distinct actors and propose four read-only source nodes and six artifacts. The analyst owns two `table_read_json` imports, one `table_join` and one `table_aggregate`; the separate writer owns the file writes and full readbacks. Join inputs come from the two actual parser `/rows` projections. The aggregate consumes only the actual join `/rows` and selects rows whose active flag is JSON boolean true.

The fixture contains duplicate catalog matches, a missing catalog record, negative sales units, and numeric, boolean and string key distinctions. The writer creates `sales_rows.json`, `catalog_rows.json`, `joined.json`, `groups.json`, `total.txt` and `unmatched.txt`. Intermediate projections are deliverables because the current contract engine requires producer artifact projections for input references; private intermediates remain future work.

All expected outputs stay in the experiment verifier. They are never included in the planning packet or supplied as model-generated tool arguments. The runner reviews the CEO's compiled bindings, exact attachments, dependency edges, scopes, encodings and paths before approving the exact digest and installing the frozen policy. It checks every source call's actual arguments/result/actor exactly once; both original input hashes remain unchanged; each output is written/read once; independent final native gates and ordered handoff must pass. It retains every planning attempt's reported local usage and execution provenance, including rejected drafts.

## Observed planning failures and review repair

The first experiment retained in `data/mcp-workflow-check/f16f6200-2fec-40c4-b863-c895a5c3a351/` encountered a truncated JSON draft, followed by a structurally valid draft choosing `json` for an integer file where the fake user explicitly requested `integer_text`. Host validation permits both encodings for an integer projection, so the fixture's stricter requirement check rejected that proposal before any job approval or tool execution. This is retained as FAIL, not called successful planning.

The reusable runner now feeds review failures back to the same saved planning session as explicit scripted `planning_repair` messages. It preserves the source attachments, user goal and scopes, and does not insert expected answers or handwritten bindings. Planning recovery accepts `--planning-session ID --existing-store PATH`; this recovers planning and does not restart an issued task. The original session `1466042e-c186-46f0-ba00-616a16591c3f` was revised rather than discarded. Additional review caught the second integer file's encoding too. Those rejections remain visible in the conversation and `operator_reviews` before approval.

The observer continues polling a specific issued UUID through transient HTTP errors; it has no time-window cancellation. If a fatal observation error occurs, it checks the authoritative same run or generating planning session before cleanup. Live or uncertain issued work retains its owned listener. Terminal failure, absent run, or completed work can stop the owned listener. This is a test fixture runner, not a general production scheduler or automatic curriculum promotion service.

## Verification boundaries

Native tests exercise scalar type distinctions, nulls, duplicate expansion, stable order, inner/left/empty behavior, selected fields on unmatched rows, aliases, invalid/nested/floating inputs, absent authority, projection types, row/byte overflow without partial receipts, receipt-bound two-input composition and actual stdio. The composition test explicitly calls the join prematurely with matching literals; without both native input receipts, that early result cannot populate the bound join projection. A later genuine two-source execution and aggregate pass the complete native artifact/readback gates.

The same-user trusted host, existing sensitive-pattern limitations and local database privacy boundaries remain. Native source/schema checks do not prove that an arbitrary reconciliation policy or user-supplied dataset is correct. The overall goal stays active.

## Execution failures that changed the architecture

Run `ec837952-db36-4fc1-a80d-7d526324bdf0` initially completed all four native sources but could not hand off: the legacy artifact gate treated “SHA-256” in a local source criterion as a demand for an external URL and a 40-character Git revision. This invented requirement was absent from the local task. Model workers repeatedly proposed finish, sometimes claiming downstream files existed; the native artifact checks prevented those claims from satisfying the missing-artifact gate. Those public replies remain in the transcript and are not execution evidence.

The repair distinguishes explicit citation/source-URL/immutable-revision requirements from local hashes. The existing legacy instruction-kit/parts-evidence profile remains, as do PDF, inventory and score gates. A regression exercises local hash/evidence acceptance without invented remote prerequisites and retains explicit URL/revision rejection until an actual evidence artifact exists. Independent native bindings and QA still verify source integrity and stage criteria.

The failed completion branch also previously advanced the step counter without entering `repair`, bypassing the existing no-progress recovery budget. It now routes through the same recovery accounting and attention threshold as other failures. A real HTTP smoke with simulated model decisions repeatedly submits finish while a required report is absent; it must pause with a recovery question rather than spin indefinitely or create a false report.

After a controlled checkpoint cancellation/reload for that tested repair, the same approved UUID was resumed without replanning, reapproval, policy installation or manual output files. The resume observer verifies the original owner fields and the complete frozen intake/digest against current company authority, including server-added host controls. An initial observer comparison wrongly expected the server's augmented intake to equal the unaugmented draft; it failed safely before resuming and is retained in `data/mcp-workflow-check/735a14d8-7448-4202-a1eb-5b521cc483fb/`.

The corrected resume in `data/mcp-workflow-check/47d3bab0-d635-4b1b-b1ad-5b0c0b381b30/` reached company status completed, but the independent verifier reported FAIL: the analyst had repeated the pure aggregate once during recovery. Completion status alone therefore did not pass the experiment's no-repeat requirement. All original FAIL reports are preserved; no recovered benchmark success is claimed for that run.

## Suppressing redundant pure source calls

Native tools now declare whether their result is a deterministic function of validated arguments without I/O. Only integer arithmetic, aggregation and join declare this property; file reads, memory searches and other observations do not. B1 shows this distinction alongside read-only status. A read-only operation is not automatically a pure computation.

`artifact_bindings::observed_pure_call` verifies that an exact owner-bound source computation already has current-epoch canonical evidence, resolves its input dependencies, rechecks the actor's current permission, and authenticates the selected source projection. Different arguments, actors, epochs, corrupt projections and unbound work cannot produce the hint. The hint contains hashes and identity, not a fabricated new result or execution receipt. For a duplicate pure proposal in an ordered CEO job, the controller records no execution, enters normal bounded repair, and asks the model to use existing evidence or submit the correct current stage. It neither suppresses mutable observations nor converts an admin deny into allow. Direct MCP callers still receive normal tool semantics; this is a controller scheduling guard.

The native regression verifies the unobserved/observed distinction, exact argument matching, current grant/epoch requirements, corrupt evidence rejection, and that file/memory observations are excluded. A fresh company challenge is required to verify the complete repaired path rather than relabeling the previous failure.

## Fresh repaired workflow: independently verified PASS

The fresh task run `f31b2c4b-8b27-4210-b1ea-cf12da311680` completed with exactly four native source calls, three dependency edges, six exact artifacts, unchanged pinned operator inputs, one native write/read per output, accepted ordered handoff and complete final admin/native evidence. No worker-model calls, admin denials, post-planning questions or unfinished calls occurred. The CEO made three planning attempts, including one draft rejected by the scripted operator for the unmatched-count encoding; there were no host-schema-rejected attempts in this session. The review correction and original CEO replies are retained in the public conversation.

Its initial observer report in `data/mcp-workflow-check/1f450ecf-73c4-476b-9e98-1c50ec4471fa/` recorded FAIL after completion because the new verifier referenced `requiredTools` outside the review helper's scope. This was an observer defect, not a missing native source. The helper now returns that value explicitly. The completed UUID was reverified through `--resume-report`; the observer saw status completed and made no resume POST, new planning, approval, task start, inference call or output write. All independent checks passed in `data/mcp-workflow-check/6196b359-d44c-4924-ae46-bcefa645c640/`. The original error report is preserved, and the new report records `before_resume_status:completed` rather than pretending the task was rerun.

The successful self-contained `transcript.html` in that directory includes every actual public model/tool event and the scripted fake-user conversation. Expected final bytes include total `25`, unmatched left count `1`, eight joined records and four category groups: boolean/9, hardware/11, numeric/7, returns/-2. The fake user never supplied these expected answers to the CEO or workers. The independent literal fixture and native receipt/data-flow checks establish them.

Measured successful-session usage: 24 completed local replies including all three planning replies, 107509 input tokens, 4046 output tokens, no unreported unfinished calls and $0 paid-provider charges. This is not token-free local inference, and these numbers do not hide or incorporate the earlier, separately retained failed experiments. Those earlier attempts consumed additional local inference while exposing the repaired controller/verifier problems. There is no claim of autonomous training or arbitrary-task competence.

The refreshed native office shows nine functions, the join source/schema/effects, and deterministic status separately from read-only. Screenshot: `data/mcp-basement-verification/join-basement.png`. Native tests and strict Clippy pass; the new real-HTTP simulated-provider smoke verifies bounded repeated missing-artifact completion attempts. Final complete check logs and supplemental file hashes are recorded with the milestone evidence.

Final check: 264 Rust tests passed (17 opt-in ignored), strict Clippy, the complete HTTP/mock/Python/Node suite and Vite build passed in `data/mcp-basement-verification/join-all-final-check.log`. A final pure-source hardening also requires every matching bound projection to be present before suppressing a duplicate; a missing projection still needs an actual native execution. The focused native composition suite verifies loss/recovery of one matching projection, and strict Clippy/build passed after that adjustment. No extra company task or inference replay was needed for this negative-case hardening. `scripts/check.sh` now syntax-checks the new workflow runner and fixture as part of future checks.
