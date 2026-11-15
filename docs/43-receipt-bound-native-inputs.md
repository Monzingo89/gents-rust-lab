# Receipt-bound native input composition

The company can now use an earlier native result as input to a later read-only
source call without a model copying or reconstructing the data. This extends
the existing local CEO contract, transport-independent MCP registry, scoped
receipts and exact artifact gates. It does not create a privileged model tool,
install code, grant new source capabilities or claim arbitrary-task autonomy.

## Explicit derived slots and dependency graph

Exact inputs remain immutable. An owner can declare up to eight top-level
`derived_fields` on an attached input. Each must have an explicit empty
schema-valid placeholder. The CEO must fill exactly those fields using native
references; it cannot replace a nonempty literal or add another derived field.
For example, a second table input may declare:

```json
{
  "id": "regroup-input",
  "tool": "table_aggregate",
  "arguments": {"rows":[],"filter":null,"group_by":"rows","value_field":"total"},
  "derived_fields": ["rows"]
}
```

The local CEO selects the producer source and its approved projection:

```json
{
  "id": "regroup",
  "input": "regroup-input",
  "argument_sources": [{"field":"rows","source":"warehouses","pointer":"/groups"}]
}
```

A producer artifact check must select that same source/projection. The pure host
compiler resolves it to a native binding reference such as
`{"field":"rows","path":"warehouses.json"}` and sorts bindings in dependency
order. The path is a stable label for the approved projection, not permission
to read an intermediate workspace file. Missing producers, cycles, duplicate
fields, literal overrides and future actor-stage dependencies are rejected.
Sources and artifacts retain their existing eight/sixteen limits. References
currently target complete top-level fields and selected approved artifact
projections; arbitrary nested substitutions or private intermediate views
without an artifact contract remain future work.

## Native authority and evidence

`mcp/src/bound_sources.rs` resolves values only from private selected projections
with matching successful canonical worker receipts in this run and current
policy epoch. It recursively verifies exact producer arguments, source actor,
tool, pointer, result/value hashes and current sender/recipient permissions.
Resolution is memoized within a bounded check. It cannot use model summaries,
workspace artifact contents or caller-supplied receipt JSON as evidence.

The destination input schema and size limit are rechecked after replacement.
A derived consumer must remain read-only after resolution. Missing dependencies
produce an unmet check and no compiled dependent call. Actual dispatch still
passes the independent admin decision, current policy and intent journal.
Native binding capture resolves dependencies again inside the registry's
execution transaction before it can record the consumer projection. Thus an
unrelated earlier tool success cannot manufacture a satisfied dependency.

Completion rechecks the chain and exact writer-owned file write/readback
receipts. When a newly observed producer value changes within the same policy
epoch, older consumer evidence no longer matches the resolved argument hash.
Fresh consumer capture now updates that concrete hash on its projection row;
a regression test covers this with file-read text feeding a memory query.
This is observation-based provenance, not continuous monitoring of source files
or cryptographic protection against a privileged owner rewriting the database.

The operator's actor order and the tool-input graph are separate. One staffing
stage owns all source nodes assigned to that actor. The decoder bounds staffing
by the configured roster and constrains input references to owner-declared
slots. If exactly one staffing assignment owns a source capability, the host
can bind an omitted source actor to it. A writer can likewise be inferred only
when ownership is unique and ordered. Ambiguity still requires an explicit
choice; source capabilities and write permissions are never inferred.

## Required readback and planning provenance

Real CEO attempts exposed repeated staffing mistakes, missing derived-field
references and omission of `files:read` despite a required artifact readback.
Those rejected attempts remain in their original FAIL reports and planning
audit. The compiler now completes only required read-only verification when the
selected artifact writer already has `files:write`. It adds no write authority
or source capability. The original model reply is preserved in the transcript;
compiler-added readback controls are recorded separately. The resulting draft,
actual actor tools and intake digest still require explicit owner approval.

Planning repair feedback now has `message_kind:planning_repair`. It is retained
for the CEO and transcript, but excluded from execution requirements and owner
numeric facts. This fixes a discovered failure where rejected-draft errors were
concatenated into the task and caused needless actor-ownership questions.
Admission also receives the actual approved frozen CEO plan/bindings. Legacy
conversation entries without this marker retain their original meaning; history
is not retroactively rewritten.

## Verified outcomes and observer recovery

The original composition attempts failed validation before tools ran. A later
valid run, `2946d10b-d254-4566-b7fc-74cfc290377c`, reached an accepted analyst
handoff after both native transformations. Its observer then incorrectly treated
a fixed observation window as failure and cancelled progressing work. That FAIL
report is retained. The observer cutoff was removed; transient polling failures
now re-poll a confirmed live handle. A separate recovery observer resumed the
same saved UUID and approved checkpoint rather than starting another task.

Because that legacy intake included diagnostic repair text as requirements,
two explicitly logged fake-user clarifications copied its already-approved
staffing/scope facts. They introduced no new grant or calculated domain answer.
The same run then passed independent native-result/argument and exact-byte
checks without replaying either source or any destination write. Its recovered
trace/report are in:

`data/mcp-team-check/18269842-cb2a-494b-810f-fcead4fea9a7/same-run-resume-transcript.html`

That recovery counted 28 measured local replies, including audited rejected
planning attempts, with 120057 input and 8053 output tokens. One unfinished call
may have unreported local usage. All measured provider charges were zero.
Original reports are unchanged; recovery is recorded separately.

The final fresh trial, `029f5b66-795e-490e-b636-a8a0ccd6f39c`, passed with two
planning attempts, no rejected planning drafts and no post-planning questions.
The CEO proposed two native source nodes and four artifacts. One analyst first
summarized accepted inventory by warehouse, then fed its actual native groups
array into a second aggregation by movement-count bucket. A distinct writer
saved and read back:

- `warehouses.json`: east / 1 row / -3; north / 2 rows / 28; south / 2 rows / 0;
- `by_count.json`: count-key 1 / 1 warehouse / -3; count-key 2 / 2 warehouses / 28;
- `total.txt`: exactly `25`;
- `warehouse_count.txt`: exactly `3`.

JSON used exact compact canonical bytes; all files had no newline. The second
call's actual arguments matched the first call's actual native projection.
There were exactly two source calls, one write per destination, no worker-model
calls and no replay. Seventeen completed local CEO/admin/QA replies used 73200
input and 2700 output tokens, with no unknown calls and zero provider charges.
This is not zero total local model token use. The full fake-user/CEO and public
agent trace is:

`data/mcp-team-check/cf319c0e-6d45-4519-b49e-ea9180580317/transcript.html`

Repeat the bounded case with `node scripts/mcp-team-check.mjs --normal-only --chain`.
`scripts/mcp-team-resume.mjs REPORT_DIRECTORY ORIGINAL_RUN_STORE` observes and
reverifies a retained table-composition fixture without new planning/approval.
Its oracle is limited to the declared fixture tools, not a general verifier.

## Verification and limits

Final offline checks passed 250 Rust tests, with 17 opt-in tests ignored,
strict Clippy, HTTP/mock smokes, Python/Node checks and the production build.
Native tests cover receipt-before-dependency attempts, exact data copying,
completion/readback, policy epoch changes, corruption, unauthorized recipients,
cycles, literal/duplicate overrides, mutating consumers and same-epoch input
changes. Compiler tests cover owner slots, topological ordering, unique actor
inference, read-only readback closure, first-turn decoder bounds and diagnostic
scope separation. Final observer/UI syntax and build checks also passed.

The broad goal remains active. Nested/file adapters, private intermediate
views, stronger semantic qualification, protected memory, source promotion and
a durable evaluation/promotion/rollback curriculum still require work. These
results do not establish perfect planning, general arbitrary-task competence or
autonomous weight training.
