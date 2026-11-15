# Ordered CEO teams and verified native data flow

This continuation extends the requested local company rather than treating the
single-worker prototype as the final goal. It remains local research, not proof
of arbitrary task autonomy, certified security or autonomous source promotion.

## Team contract and execution

The local CEO may propose one to four distinct configured actors in execution
order. Every new assignment includes its task, native tool names/operations and
one to four concise stage acceptance criteria. The root definition of done is
preserved. The final actor is reviewed against both the original definition and
its own stage criteria. Old one-worker frozen jobs remain supported.

The host records explicit ordered handoff edges and per-actor finish controls in
frozen intake. Each stage may receive the accepted output of all earlier stages;
these are approved prefix edges, not a parallel DAG or unrestricted memory bus.
Policy installation rejects extra tools, skills and unrelated/future senders.
The operator approves the actual intake digest before any company execution.
A ready conversation can be revised with a new user message and expected
revision. Existing jobs retain their original frozen proposal. New callers can
pin `ceo_planning_revision` and `ceo_plan` to reject stale planning snapshots.

`ceo_team.rs` compiles and checks this contract. The existing action loop remains
responsible for every admin decision, native dispatch, privacy check, write-ahead
intent and retained tool journal. An exclusive controller lease prevents two
controllers advancing the same multi-agent run. A non-final finish submits the
current stage to independent QA, then atomically replaces the controller's local
checkpoint with an accepted handoff and next stage index. It does not declare
the company goal complete. After interruption, completed stages are reused and
ambiguous pending effects retain the existing inspect-before-replay rule.

Successful native calls and required receipts stay actor-scoped. A stage gate
checks its own frozen native requirements; final acceptance requires all actors'
requirements. Read-only repetition detection is stage-scoped so an independent
reviewer can read the same artifact. Existing mutation history is retained.
Prior model summaries are labeled unverified claims; bounded actual native
result projections are supplied separately from full retained review packets.
The controller checks current sender permissions and policy epochs before
handing source data to a worker. The native process can receive a trusted
`--policy-revision` fence; Registry checks it inside the immediate execution
transaction, including when the tool grant itself has not changed. The same
fence is used by stdio and loopback SSE.

## Why an independent artifact contract was necessary

The first real two-worker experiment completed the analyst stage and actual
writer file operations, but recursive finish payloads repeatedly exceeded the
local response budget. That run was cancelled, with its history retained.
`finish` now has a bounded typed contract: a brief deliverable and evidence
reference, not copied handoffs, review objects or delivery-gap metadata.

A later experiment demonstrated a more serious failure. The native arithmetic
result for [31,-8,19] with subtract=2 was correctly `{sum:42,difference:40}`.
The CEO guessed 42 in a stage criterion. The analyst summary, downstream writer
and semantic QA repeated that guess, and the company incorrectly accepted a
file containing 42. The independent exact-byte experiment rejected it. This
false positive remains visible in the rejected report; it is not presented as
a successful task or autonomous learning result. Source counts/hashes alone do
not prove a model used the correct result field.

The next trial's gate correctly blocked completion but the model repeatedly
changed a correlation label instead of using the exact approved source
arguments. The controller now compiles such literal native source and data-copy
calls from the owner-approved binding. This is deterministic host compilation,
not model-authored arguments. `frozen_contract_action` events state that
provenance. The normal policy/admin/intent path still executes every action.
Already-attempted mutating requests are not automatically replayed.

## Native source-to-artifact bindings

An optional owner-approved intake contract can specify:

```json
{
  "artifact_bindings": [{
    "path": "total.txt",
    "source_actor": "Engineer Gent",
    "writer": "PM Gent",
    "source_tool": "integer_arithmetic",
    "arguments": {"calculations": [{"id":"approved-total","values":[31,-8,19],"subtract":2}]},
    "pointer": "/calculations/0/difference",
    "encoding": "integer_text"
  }]
}
```

This declares source inputs and a projection, not a guessed answer. The source
and writer must be in the approved ordered team with matching capabilities.
The final bytes are derived from the actual successful native result.
`integer_text` accepts only an actual JSON integer; `utf8_text` and canonical
`json` encodings are also available. Paths are confined to the run workspace,
and bindings are bounded to sixteen artifacts of at most 8192 bytes each.

`mcp/src/artifact_bindings.rs` stores only selected source projections and file
receipt metadata in private versioned SQLite tables. Inserts share the native
call's transaction with its canonical successful audit receipt. Receipt bodies
remain payload-free. Hashes bind original arguments, result, projection value,
policy epoch and runtime actor; verification also requires the canonical audit
entry. An audit failure rolls the projection back, and altered projection data
fails integrity checks. This is private storage, not disk encryption.

Artifact acceptance requires the current source epoch, authorized source sender,
matching source arguments, the selected value, writer-owned native write and
complete readback receipts for the same bytes, and an actual current confined
filesystem read. An externally corrected file cannot replace missing writer
receipts. A model's summary, QA verdict, intermediate sum or forged JSON cannot
substitute for these checks. Missing/wrong artifacts expose a bounded actual
native expected value for repair. Large source payloads remain in their retained
workspace/journal; a receipt hash does not pretend to contain omitted content.

`next_call` proposes only an already frozen source request or a source-derived
write/readback to the bound path. It does not execute tools, add permissions,
install code or create a model-facing privileged tool. Once the bound native facts are verified, the host may submit one bounded stage
finish for independent review without a worker-model call. The submission is
logged as host-compiled, not model-authored. QA still checks every original and
stage criterion; an unsuccessful submission falls back to local reasoning.
Qualitative work beyond the binding still belongs to the local workers and
independent review. CEO planning, admin review and QA still use measured local
model tokens. This is not a claim that the entire company uses zero tokens.

## Verification and limits

Focused tests cover ordered handoff ceilings, missing/stale source epochs,
unchanged-grant stale writes, real SSE child epoch fences, independent reads,
projection integrity, audit rollback, actor access, traversal, exact argument
matching, boolean/type rejection paths, source-derived call proposals and the
requirement for both native writer operations. The existing suite continues to
exercise transport, tenant, filesystem and interrupted-effect controls.

`scripts/mcp-team-check.mjs` launches an isolated native local company at
8796/8797, refuses occupied ports, saves each CEO conversation and every public
run event, and produces an escaped `transcript.html`. It tests two independently
scoped workers, then cancellation at an accepted stage boundary and resumption
of the same run. It independently checks exact bytes, source arguments,
actor-owned receipts, order and no repeated analyst execution. Call/token counts
cover measured completed replies; interrupted calls may have unreported local
tokens. Provider charges remain zero by enforced local configuration. Polling
and fixed verification do not call models.

The broader goal remains unfinished: explicit sparse dependency graphs, reliable
source authoring/review/promotion, remaining legacy tool migration, protected
memory adapters and a durable evaluation/promotion/rollback curriculum controller
still require implementation and evidence. Ordered-team tests are not evidence
that the company can safely handle any task or improve its own weights.


## Verified continuation outcomes

The normal and interrupted trials passed in attempt
`54f475a1-f8a6-401b-b3cc-cfa7dcdb22e7`:

- Run `29ba984d-27b3-4960-ac3d-2195e0520ad9` produced exactly 40 from
  [31,-8,19] minus 2, with one analyst call and writer-owned write/readback.
- Run `09f1e9ab-b8e2-4874-8efe-daf4f54ab3f3` produced exactly 21 from
  [16,-5,13] minus 3. The same run was cancelled after the accepted analyst
  stage and resumed; it retained exactly one analyst native execution.

Both had zero worker-model calls. Together they used 20 completed local calls
for CEO planning/admin/QA, with 48123 measured input tokens, 3193 output tokens
and zero provider charges in 88.4 seconds. The interrupted trial also had one
cancelled/unfinished guard call whose local tokens may be unreported. The
conversations, all public events and explicit compilation provenance are in
`data/mcp-team-check/54f475a1-f8a6-401b-b3cc-cfa7dcdb22e7/transcript.html`.

A fresh-value verification in attempt `f66aca15-4e2d-4f56-baac-53a005b42414`
passed with exactly 55 from [73,-29,18] minus 7, again with one analyst call,
writer-owned exact readback and zero worker-model calls. It used 10 completed
local guard/planning/QA calls, 24145 measured input tokens, 1623 output tokens,
zero provider charges and 45.2 seconds. This additionally exercised the bounded
QA check-count decoder and post-review artifact revalidation. Timings are
observations, not quality gates or promises.

Rejected attempts remain separate. The first payload-loop trial, the false
semantic acceptance of 42, and the missing-correlation-label/QA-coverage trials
are not counted as successful tasks. The current fixed gate prevents the
observed source/byte mismatch from being accepted from model agreement alone.

Ready-plan edits now append audit start/completion records and clear stale error
indicators while preserving failures in the private audit. Intake can pin the
planning revision and proposal; already approved jobs retain their own snapshot.
The CEO trace includes sanitized accepted-stage metadata, without copying raw
handoff payloads into the metadata-only observability view. Candidate authoring
and imperative team execution cannot be mixed in one intake until their stage
adapters are explicitly integrated.


Final validation: the locked offline workspace passed 228 Rust tests; 17
explicit opt-in tests (including paid-provider probes) were not run. Strict
Clippy, the complete `scripts/check.sh` suite, its HTTP/CLI recovery and mock
smokes, UI tests and Vite build passed. Real native runs above are separate
evidence from simulated smoke decisions. The refreshed office at 8788 reports
local-only inference and `local_sse_mcp`; no active office job was stopped to
refresh the binary. Source publication/deployment was not requested.

## Office planning to execution continuation

The local tool-enabled office's task button now opens the CEO conversation.
A ready conversation exposes a separate execution review. Preparation freezes
intake through the owner-scoped company API, pins the saved session revision,
checks all submitted owner inputs survived unchanged, and shows the returned
job digest, actors, tools, authorized handoffs and completion criteria. Native
host controls and receipt requirements added by the server remain inspectable
under the exact frozen scope disclosure. Preparation does not approve or start.

An explicit scope checkbox enables the execution action. Approval uses the
returned digest, governance installation uses expected revision zero, and start
uses a stable idempotency key. Partial progress is tracked within the page for
safe retries. Company/connection/sign-in changes are blocked during execution
requests, and changes of planning context discard pending browser reviews. A
page reload loses the pending browser review; it does not erase frozen drafts,
approvals, audit entries or running jobs. Dedicated pending-draft recovery UI
remains future work. Existing non-native compatibility paths remain available.

`web/assets/ceo-execution.js` is shared by the browser and the real local
experiment. It prepares review data only; authenticated server validation and
policy enforcement remain authoritative. Owner-entered native artifact bindings
are optional advanced JSON. The UI explains that without such a contract,
semantic model QA alone does not establish exact source-to-file correctness.
That limitation is superseded by docs/41-ceo-proposed-native-contracts.md: typed CEO proposals and immutable input references now have real local execution evidence. Free-text literal extraction still requires operator review.

Verification for this continuation:

- Four Node tests cover role separation, actor substitution, snapshot isolation,
  frozen-scope tampering, key reordering and oversized requirements.
- JavaScript syntax and the production Vite build passed.
- A real local CEO and two-agent run through the shared review preparation passed:
  `70c3d48a-5899-4678-8348-37d47da1d155`, report directory
  `data/mcp-team-check/8a25dac7-887e-4344-9a8c-44204e01c34b/`.
  The analyst calculated [73,-29,18] minus 7, and the independent writer saved
  and read back exactly `55`. One analyst native call, ordered accepted handoff,
  bound write/readback, zero worker-model calls, ten completed CEO/admin/QA model
  calls, 21929 measured input tokens, 1662 output tokens and zero provider cost.
  The conversation, exact execution review and every public run event are saved
  alongside the escaped transcript. No interrupted/unreported calls occurred.
- Native Chrome checks on the isolated curriculum company loaded a retained
  ready conversation, prepared an actual draft, inspected its digest/permissions,
  confirmed the start action was disabled, checked that consent enabled it, and
  cleared consent again. Browser execution itself was not clicked. The separate
  shared-code real run above verifies approval/install/start through the public
  APIs. Two test-only draft jobs remain auditable and unapproved; no user job
  was started or stopped during the browser check.

## Multi-artifact curriculum and evidence-window repair

The ordered-team experiment now supports `--multi-artifact`. Its analyst executes
one exact native request containing three calculation rows, including negative
results, cancellation at the input bound and zero. The writer produces three
integer text files plus a canonical compact JSON ledger preserving the complete
native calculations array. Four frozen bindings select three different integer
projections and the full array from that one native result. Independent BigInt
calculations and exact-byte checks stay in the experiment verifier; answers are
not inserted into the fake-user conversation, CEO plan or worker input.

The first real trial (`a15646ac-7391-4fd3-8e34-0cb5adefb2a6`, report folder
`data/mcp-team-check/0830e208-d928-48ef-877d-5c44985d62bc/`) exposed an actual
scaling defect: all four artifacts and native readbacks were correct, but the
admin's rolling evidence window omitted the original calculation and first file.
The reviewer repeatedly denied stage submission on the false premise that the
calculation or verification had not occurred. That looping test run was
explicitly cancelled to repair the observed cause; it remains a failed trial.

`ceo_team::authorization_evidence` now supplies compact host-derived canonical
call checks, every selected artifact's source/hash/write/read checks and accepted
stage metadata to the admin. This does not copy model deliverables, full result
payloads or entire history. It preserves failed checks, policy revisions and all
sixteen supported artifact rows rather than relying on six recent operations.
These are execution observations, not grants or an automatic allow override.
The admin still reviews each exact action, and finish still submits to separate
independent QA followed by the existing current-policy and exact-file rechecks.
The arithmetic catalog now explicitly documents its output fields and that it
does not echo input values/subtract, after the CEO invented such a criterion.

The repaired real trial (`b449c949-120e-468d-9d27-4d1f0a8abefd`, folder
`data/mcp-team-check/30984fd4-1f49-4b34-a877-5cec3de55cfb/`) passed with:

- `loss.txt`: exactly `-34`;
- `cancellation.txt`: exactly `-20`;
- `zero.txt`: exactly `0`;
- `calculations.json`: all three actual native id/sum/difference rows in their
  original order with canonical sorted keys, no newline.

There was one analyst native call, eight writer file operations, zero worker
model calls and no admin denials. Sixteen completed local CEO/admin/QA model
calls used 58727 input and 2238 output tokens, with zero provider charges and no
unfinished calls. The retained final admin packet independently shows the
upstream arithmetic receipt and all four artifact checks even though the
rolling recent history no longer contains the calculation. Its extracted proof
is `data/mcp-basement-verification/multi-artifact-review-proof.json`.

`--interrupted-only --interrupt-writer` exercises cancellation after at least
two writer-owned readbacks and resumption of that same run, rather than only
interrupting at the stage boundary. The verifier requires one source execution,
exactly one write per destination, actor-owned native readbacks, correct handoff
order and complete canonical evidence in the final admin packet. This is an
explicit controlled local experiment, not a claim of arbitrary crash recovery
or autonomous learning. The script preflights native local-only providers and
the actual SSE harness transport before starting inference.

The interrupted writer experiment completed run
`ad56a7d7-3af3-4139-9f41-b771f08424b6` after cancellation following two native
readbacks and resumption of the same UUID. Every destination had exactly one
native write, the analyst ran once, all four files matched the independent
oracle and the final admin packet retained the full canonical evidence.
The experiment's final verifier then crashed while assuming that an admission
admin event contained a tool request. This was a verifier defect after actual
company completion, not a company failure or an inferred pass. The original
FAIL report and transcript are retained unchanged. The corrected shared
`scripts/mcp-team-verification.mjs` safely distinguishes admission events and
requires complete native evidence; a regression test also rejects missing or
failed checks and a substituted source actor.

Saved events, checkpoints, native journals and the current files were rechecked
without repeating inference. The separate `reverified-report.json` and
`reverified-transcript.html` in
`data/mcp-team-check/3ab7bd85-acd4-4cd4-a23c-21fd3343c9b5/` record PASS, bind the
original report by SHA-256, explain the correction and retain the full scripted
user/local CEO conversation and public execution trace. There were zero worker
model calls, sixteen completed local CEO/admin/QA calls, 58708 measured input
tokens, 2121 output tokens, no unfinished calls in the retained event stream and
zero provider charges. This measured count does not imply cancelled local
requests can never have unreported token use.

Full offline `scripts/check.sh` passed: 229 Rust tests, 17 opt-in tests ignored,
strict Clippy, HTTP/mock smokes, Python/Node checks and the production Vite build.
The verifier regression was added and passed separately after that full run.
Evidence is retained under `data/mcp-basement-verification/multi-artifact-*`.
The owned idle office listener at 8788 was refreshed only after its API reported
no running jobs; its new catalog and local-only SSE configuration were verified.
The overall goal remains active. Automatic typed binding proposals, richer local
capabilities, protected memory adapters and durable curriculum qualification /
promotion / rollback remain unfinished.
