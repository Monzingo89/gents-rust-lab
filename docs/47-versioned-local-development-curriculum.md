# Versioned local development curriculum

This document records the two-level curriculum milestone. The later outcome-only third level and its unsuccessful planning trials are documented in [48-outcome-only-local-planning.md](48-outcome-only-local-planning.md); consult the current journal for present qualification state.

The local company now has a persistent curriculum journal, model-free evidence admission, a reviewed next-challenge selector, and an inspectable HTML index. This records qualified synthetic fixtures and preserves failure history; it is not model training, general task mastery, autonomous source promotion or a production scheduler.

## Run and inspect

```sh
node scripts/mcp-development-curriculum.mjs --status
node scripts/mcp-development-curriculum.mjs --next
```

`--next` selects one unqualified eligible reviewed fixture, creates a private attempt lease, runs the real local CEO/team workflow and admits the result only after independent verification. It does not loop over already-passed tasks or silently invent new challenge definitions. Once the reviewed catalog is exhausted it stops scheduling, with an explicit message that a reviewed harder challenge is needed; it does not declare the company goal complete.

The ledger is `data/mcp-development-curriculum/curriculum.sqlite3`; the inspectable index is `data/mcp-development-curriculum/index.html`. The HTML is a rebuildable atomic snapshot of ledger state. The SQLite file is mode 0600 in the private data directory. Raw attempt leases are private files excluded from status/index output. No new package, service, network access, credentials or paid inference was introduced.

The reviewed progression graph lives once in `config/mcp-development-curriculum.json`. A static JavaScript catalog binds those IDs to reviewed fixture data; unknown fixture IDs cannot dynamically import code. Fixture version hashes cover requirements, exact operator inputs, source templates, dependency edges, output contracts and independent expected results. Changed fixture content no longer inherits an older fixture's qualification, while the old attempt remains in history.

## Authority and evidence before advancement

A SQLite immediate transaction and unique partial index permit only one active attempt. Prerequisites must have a recorded PASS for their current fixture version. An unrelated/stale lease cannot finish an attempt, terminal records cannot be rewritten, and unverified proofs or nonzero/unknown provider charges cannot qualify a fixture. Reopening the journal preserves the active attempt; it never assumes a missing observer means an issued task stopped.

The journal is trusted-host orchestration, not an MCP capability or a model-callable authority. Its Node admission path reuses the workflow's full model-free verifier rather than trusting `report.status`. Admission checks the actual run, frozen owner requirements, exact source attachments/bindings, scopes, dependencies, expected values, unchanged original input bytes, each output's exact bytes and one write/readback, no repeated native sources, ordered handoff, independent final gates, measured usage and native local execution metadata.

A read-only SQLite transaction additionally reads current company authority: the frozen intake/digest, owner approval, installed policy, canonical native success receipts and the planning-attempt audit up to the frozen planning revision. Every saved native result must hash to its receipt and match an actual current-policy audit row. Planning usage and execution provenance must agree with the canonical audit, including rejected drafts. Policy tool/skill/sender arrays are compared as sets, matching the native governance representation. The verifier rejects missing provenance or failed/uncertain tool attempts for these bounded deterministic challenges.

An arbitrary saved PASS file cannot promote a fixture. A deliberate test attempted to submit the older baseline as the harder case; the verifier rejected its different immutable inputs before journal admission. The existing active challenge remained untouched. Reports are confined to one workspace workflow UUID directory, symlink evidence paths are rejected, and evidence file reads are bounded. These controls do not make local SQLite encrypted or tamper-proof against a privileged same-user host process.

## Reusing historical evidence without spending inference again

The baseline was independently reverified from its existing outputs, model events and native company audit, then admitted with origin `historical`:

```sh
node scripts/mcp-development-curriculum.mjs --adopt data/mcp-workflow-check/6196b359-d44c-4924-ae46-bcefa645c640 --case reconciliation
```

That did not issue new planning, approval, task execution or model inference. Its token counts remain the original run's measured usage. The second level was then selected and executed with origin `executed`; historical adoption is not presented as new company work.

The historical baseline qualification refers to verified fixture content, not certification of the current binary. A recorded PASS is verification at admission time, not continuous monitoring of later filesystem or policy changes. Runtime implementation changes do not automatically become evidence of learning or current-build regression success.

## Harder second level

`reconciliation-edges` uses the same four-call graph with more adversarial data: ten sales rows, nine catalog rows, null-key matching, identical duplicate catalog records, string-versus-boolean flags, a missing key and signed cancellations that must retain zero-total groups. Both original files are SHA-256 pinned before CEO planning. No expected answers are sent to the CEO or workers.

Actual run `db8adccd-7550-46c0-a478-6446df96271f` passed. Its report and complete fake-user/CEO conversation and execution transcript are in `data/mcp-workflow-check/7be8bc0d-afb5-47e3-bfe4-09f2eba6c9e8/`. The native pipeline imported both files, joined their actual rows and aggregated the actual join result. All six files were written/read once; there were no repeated source calls or worker-model calls.

Independent expected results include thirteen joined rows, nine boolean-true selected rows, four excluded rows, total `5` and one unmatched left row. Groups retain cancelled/zero totals: boolean=3, cancelled=0, nullable=6, numeric=-4, zero=0. Native counts preserve multiplicity: the zero group has four rows, and cancelled has two. The exact arrays and native hashes, not this prose, establish the result.

Measured new execution: 24 completed local replies, 112382 input tokens and 4053 output tokens, with $0 paid-provider charges. Local inference consumes tokens; electricity and hardware are excluded from provider charges. The ledger distinguishes this new run from the historical baseline's 24 previously measured replies.

## Failure and recovery boundaries

Live or uncertain workflow observations retain the attempt and its owned listener. After inspecting authoritative run state, recover the same UUID with the workflow runner's `--resume-report`/`--existing-store` path; a successfully reverified report can be reconciled into the active curriculum attempt:

```sh
node scripts/mcp-development-curriculum.mjs --reconcile data/mcp-workflow-check/REPORT_UUID
```

This command admits evidence; it does not resume or start a model job itself. There is no automatic lease stealing or timeout restart. A failed challenge remains unqualified and its report/reason remains in the attempt history. An unresolved observer/runner failure may require explicit inspected reconciliation. Editing a fixture while an attempt is active requires inspecting its original frozen version rather than silently relabeling it as a new fixture.

The older `mcp-curriculum.mjs` remains for historical starter cases. Its five-minute window is now telemetry only: it continues observing the same live UUID, retries transient observations, recognizes a missing run, and retains live/unknown work during cleanup instead of cancelling it because observation expired. Generating CEO planning is also retained on uncertain cleanup. The new journal-backed entry point is the default for reviewed progression.

## Verification

The journal tests exercise prerequisite gates, fixture-version changes, terminal immutability, wrong leases, unverified/stale/unknown-cost proofs, durable single-active-attempt concurrency, private file modes and canonical owner/digest/draft rejection. CLI tests ensure malformed or conflicting options fail before starting a listener/task. The full workflow verifier rechecked both historical and new runs against actual canonical native receipts and planning audits after extraction. The new scripts and tests are included in `scripts/check.sh`.

The catalog currently has two reviewed levels. More capabilities, more domains, new challenge families, current-build qualification, source promotion/rollback and broader autonomous task competence remain future work. The overarching goal remains active.

Final verification passed: 264 Rust tests (17 opt-in ignored), strict Clippy, the complete smoke/Python/Node suite and Vite build. Four journal tests and the CLI boundary test pass; the final terminal-failure assertion and script cleanup refinements were rechecked with focused tests/syntax checks. Full output is in `data/mcp-basement-verification/curriculum-complete-final-check.log`; current source hashes and proof links are in `data/mcp-basement-verification/curriculum-summary.json`. Both current fixture versions are qualified, the ledger contains one historical and one newly executed attempt, and no active attempt remains. The live office is still explicitly local-only with SSE MCP and nine native functions.
