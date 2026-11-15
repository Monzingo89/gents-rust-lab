# Local table aggregation and native result contracts

The company now has a native `table_aggregate` capability alongside arithmetic,
files, memory and skills. This is a bounded data operation, not arbitrary code
execution or general data-science autonomy. It is implemented in the independent
Rust MCP crate without new dependencies, credentials, downloads or paid APIs.

## Exact behavior and limits

Inputs contain `rows`, `filter`, `group_by` and `value_field`. Rows are flat
objects of strings, bounded integers, booleans or null. A null filter includes
all rows; `{field,equals}` uses exact typed equality. Boolean true, integer 1
and string "true" do not match one another. The group field preserves its scalar
type, and the value field must be an actual integer on every row. All selected
fields must exist on all rows, including excluded rows, so filtering cannot hide
malformed source records. Duplicate rows count separately; no deduplication,
normalization or imputation occurs.

The function returns `input_rows`, `matched_rows`, `excluded_rows`, `groups`
(each with `key`, `rows`, `total`), grand `total` and `exact:true`. Groups use the
lexical order of serialized canonical JSON keys. This is deterministic and keeps
typed keys distinct; it is not numeric sorting. Zero-total groups remain, and an
empty input or no matches yields an empty groups array and zero total. Integer
addition is checked. No expressions, filesystem, subprocess or network is used.

Inputs and outputs are capped at 16384 UTF-8 bytes. There are at most 64 rows,
16 fields per row, 128 characters per string, and integers within
[-1000000000000,1000000000000]. Field names contain 1..40 ASCII letters, digits
or underscores. Existing planning-attachment and artifact-projection budgets
also apply; they can be smaller than the tool's direct input/output limits.
Larger datasets, joins, decimal arithmetic, deduplication and pagination require
separate reviewed adapters rather than silent truncation or type coercion.

## Decoupling result contracts

Native tools can now declare a `result_schema`. The implementation catalog
exposes it as `resultSchema`, describing the native value inside its receipt
wrapper. It is not advertised as the schema of the complete MCP response.
Arithmetic and table aggregation declare schemas; tools without one explicitly
show that absence. The MCP registry validates declared results before successful
binding capture and canonical receipt commitment.

The MCP registry also checks artifact JSON pointers and encodings against these
declarations. It validates object fields, canonical array indices, declared
array bounds, JSON pointer escapes and selected scalar types. Integer output
requires `integer_text`, string output `utf8_text`, while JSON can preserve a
structured value. The harness no longer duplicates arithmetic output-field
validation. Native read-only call classification is also queried from the
registry instead of requiring another hardcoded harness dispatch entry.

These are shape checks, not evidence that a call ran. A declared maximum cannot
prove that filtering produced a particular array row. Actual successful native
results, exact approved inputs, canonical receipts, policy epoch, writer-owned
write/readback and current confined file bytes remain required by the existing
artifact binding checks. No schema installs grants or bypasses admin/QA.

The shared input validator gained schema-valued object properties, object-field
counts and explicit boolean/null checks to support flat records. This remains a
controlled supported JSON-schema subset, not a general external schema engine.

## Real local company trial

Run `59f96227-57be-4970-9d83-f05cee43ef8a` passed using two independently scoped
actors, real native local CEO planning and four CEO-proposed bindings. The
analyst had only the table capability; the writer had only file read/write and
host finish control. The immutable eight-row fixture contained signed inventory
adjustments and misleading string/numeric acceptance markers. The model never
received precomputed expected answers or handwritten artifact bindings.

One native aggregation selected five records and excluded three. Independent
fixture checks and actor-owned native receipts verified:

- `groups.json`: east / one row / -3; north / two rows / 28; south / two rows / 0,
  stored as exact compact canonical JSON;
- `total.txt`: exactly `25`;
- `matched.txt`: exactly `5`;
- `excluded.txt`: exactly `3`.

All files had no newline and were read back through native file calls. The
accepted analyst handoff preceded the writer. Each destination had one write,
the analyst executed once, and the final admin packet retained every required
source/artifact fact. Two planning attempts passed without rejection. Sixteen
completed local CEO/admin/QA calls used 61991 input and 2298 output tokens.
There were zero worker-model calls, zero unfinished calls and zero provider
charges. This is not a claim that the company used zero local model tokens.

The full scripted fake-user conversation, actual local CEO responses and every
public execution event are in:

`data/mcp-team-check/1a6e20ab-1dee-4a00-bf87-4f3dfa2eb1e4/transcript.html`

Repeat this bounded experiment with:

```sh
node scripts/mcp-team-check.mjs --normal-only --table
```

It preflights local-only native providers and actual SSE transport, uses an owned
isolated listener/store and independently checks the original source arguments,
projections, native actors, exact bytes, one source call and no repeated writes.

## Verification and remaining work

Four native tests cover strict typed equality, grouping order/type identity,
zero totals, duplicates, empty input, missing/excluded-row fields, float/boolean/
string rejection as integer values, nested data, row/field bounds, actor denial
and declared projection types. The existing native arithmetic, filesystem,
policy-epoch, stdio/SSE, binding-integrity and audit tests continue to pass.
The final offline full check passed 242 Rust tests with 17 opt-in tests ignored,
strict Clippy, HTTP/mock smokes, Python/Node checks and production Vite build.
The final schema-disclosure UI change additionally passed syntax/build checks.

The owned idle office listener was refreshed only after confirming no active
jobs and waiting for its old journal lease to close. Its API now reports seven
functions and local-only SSE. Native Chrome verified the new basement entry,
source/effect boundaries and separate input/result schema disclosures after
refresh. Evidence: `data/mcp-basement-verification/table-basement.png` and
`table-full-check.log`. No user job was started or stopped during that UI check.

The broad goal remains active: richer local data/file adapters, qualified source
authoring, protected memory and durable curriculum promotion/rollback still need
implementation. This trial establishes a particular bounded data workflow, not
arbitrary-task competence, certified security or autonomous weight improvement.
