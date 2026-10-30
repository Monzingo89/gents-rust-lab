# Company run recovery

The Burj Khalifa attempt `29180f79-47e8-41b6-aeb6-1c7bcf8507c4` stopped after 2,362 events with `run journal snapshot exceeds 8 MiB limit`. Its 46 executed tools were discovery/resource/journal calls; they did not produce the requested kit. This is a runtime failure, not a verified QA result.

## Corrected failures

- Direct `sh scripts/run-company.sh` startup now uses the same server-side `.env.local` and `.env.company` loader as supervised startup. Launcher-owned port, configuration, journal path, supervision, and local-inference settings take precedence over generic environment-file defaults. Secret values are never printed.
- SQLite schema v2 separates run metadata from ordered event rows. The 8 MiB metadata limit and 2 MiB individual-event limit remain; a long run's aggregate event history can exceed 8 MiB. Existing snapshots migrate transactionally. Stale writes and deletion tombstones retain their protections. Back up before migration; schema v2 must not be opened by an older binary.
- Startup reconciles a newer file checkpoint only if its identity and event prefix match the SQLite run. This preserves terminal failures when the file write succeeded and the subsequent SQLite write failed.
- `journal_read` rejects another journal-read result. Executive recovery selects an unread original tool step, preventing recursively embedded journals from masquerading as progress.
- Repeated stable requests are checked across retained history. Dynamic resource/status observations retain their rolling refresh window.
- Twelve recovery attempts without substantive successful tool work enter the bounded Codex hotline. Resource checks, skill inspection, and journal reads do not reset that counter. Advisory guidance still requires independent admin review of the next action.
- An empty Hugging Face dataset query now falls back to the authenticated account's dataset inventory using the Hub identity and author-listing endpoints. Only repository metadata is returned; the identity response and credentials remain server-side. Once discovery has succeeded, inspection/download requests must reference a returned repository ID. This prevents a fabricated company namespace or run UUID from becoming a source.

## Verification and retry

Regression coverage includes a greater-than-8-MiB history saved and reopened, legacy migration, rejected oversized-event atomicity, stale writes, deletion, terminal checkpoint recovery, journal recursion, long-lived request deduplication, and environment-file precedence. The HTTP autonomy smoke verifies durable questions, restart recovery, admin vetoes, and protection against replaying uncertain side effects. Mock smoke results do not prove task completion by real models.

The supervised company was restarted on `127.0.0.1:8788`. Retry `d78146b4-4978-4f94-afad-bc2ad7972cda` was admitted using the previous frozen intake and acceptance criteria without supplying task-domain solutions. Its PDF, parts inventory, provenance, and independent QA result must still be verified separately.

The real retry discovered `DonV1to/lego-instructions-public` and `DonV1to/text-to-lego-instructions` through its own tool calls. Subsequent source inspection exposed a false-positive payment-card filter on digit sequences within alphanumeric filenames. After the token-boundary repair, the local agents successfully inspected 3,887 source PDFs at tool step 36. At the subsequent step-39 check the run was active without an unanswered question, but its workspace still contained no deliverables. This is restored source access, not completion. The pre-migration journal and terminal checkpoint are backed up in `data/journal-repair-7Z8A8I/`.

Validation completed: 59 Rust tests passed (5 explicit hardware/API tests skipped), 18 company-tool tests passed, 3 environment-loader tests passed, strict Clippy passed, and both HTTP reliability and autonomy smoke suites passed. The autonomy smoke also verifies hotline escalation after a persistent stall and recovery immediately after a failed tool result. Its cloud credential is removed explicitly. Hugging Face integration code follows the [Hub API reference](https://huggingface.co/docs/hub/api).

The subsequent context/evidence changes are described in [model-independent context](23-model-independent-context.md). Their checks passed 68 Rust tests, 20 company-tool tests, and strict Clippy. An explicit installed-Qwen smoke passed with an oversized optional transcript using native template/token counting and compaction. Local document dependencies imported inside the isolated shell, and the inspection tool opened/rendered an existing three-page report. That report is a capability fixture, not the requested tower kit.

The next repair pass reached a genuine reference download at step 68 and a successful source inspection at step 76. The local agents selected a 42-page reference manual; pages 1-4 have authenticated, hash-bound rendering receipts. Its copy permissions disable text extraction, so image paths are available but visual understanding is not yet established. The requested tower PDF still did not exist at the step-78 check. No 90-percent quality result is established.

This pass verified 78 harness unit tests, 21 company-tool tests, 3 PDF-helper tests, 3 environment-loader tests, strict Clippy, HTTP autonomy/reliability checks, and a real installed-Qwen structured-argument decoder call. The HTTP suite now covers ordinary supervisor termination and explicit owner-authorized checkpoint resume as well as supervised restart. These are infrastructure results, not instruction-kit QA.
