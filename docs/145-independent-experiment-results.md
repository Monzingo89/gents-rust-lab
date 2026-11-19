# Independent results for source experiments

This records the independent-evaluation milestone. [146](146-conditional-experiment-continuation.md) subsequently adds bounded continuation from those results; the earlier measurements and remaining scope below are historical.

The [evidence-triggered queue](144-evidence-triggered-experiments.md) now supports independent acceptance for bounded pure Rust source. Source submission and independent acceptance remain separate outcomes. This stage was implemented by the publishing assistant. The full company goal remains active.

## Reuse the native verifier

The governed `rust_verify` tool and the private `gents-rust-component` host adapter share one snapshot verifier. The adapter's new `verify` request receives source text, acceptance text and both expected SHA-256 values through bounded stdin. It accepts no source paths, compiler flags, shell commands or promotion requests. Candidate and acceptance code compile separately under the existing macOS Seatbelt sandbox and dialect/process/output/time limits. No unsandboxed fallback was added. The MCP inventory remains sixteen tools.

The private adapter returns a host observation, **not** a native run receipt. The original `rust_verify` tool still records its canonical receipt and source-bound result. Snapshot evaluation does not satisfy a job's `required_rust_checks` gate or change its terminal state.

An optional owner-frozen `evaluation` descriptor selects this stage. Freezing checks the test hash, runs native parser preflight, copies the acceptance bytes into the plan and pins the installed adapter and compiler executables. Source is bounded to 8,192 UTF-8 bytes. Executable pins are checked again before execution; system libraries and the entire compiler toolchain are not content-addressed by this version.

After `candidate_submitted`, the host reads the company journal through the owned SQLite bridge in read-only mode. It verifies the local owner, intake/approval digest, actor grants, policy epoch, proposal checksum, actual source bytes, confirmed write/full readback and canonical receipt bodies. The intake must match the frozen experiment. Source custody is checked again after execution. Changed source, missing receipts, revoked permissions or a changed executable cannot produce acceptance.

The queue persists `evaluation_pending` before dispatch. A crash or unconfirmed result holds that claim for reconciliation. Restart does not replay execution or invent a PASS. Successful and failed checks become separate retained results. Only a verified PASS can set the trial's `independently_accepted:true`, explicitly scoped to its frozen suite. `promotion_authorized` stays false. Runs ending before source submission do not enter this evaluator.

## Actual evidence with zero new inference

The integration fixture uses actual HTTP admission, native writes/full readbacks, company databases and isolated Rust execution. Its source intents are seeded, so it tests enforcement rather than model competence. One candidate passes, another fails the same suite, and a third actually passes before the fixture loses its evaluator result. The third stays unaccepted after reopening the controller, without repeated execution. Its six source-operation receipts are actual; no model calls occur.

Six retained **real company-authored** submissions were also rechecked without new inference, source edits, policy changes or promotion. Current source hashes, approval digests, policy revisions and twelve canonical source receipts matched. Five still fail the original UTF-8 no-progress boundary: `bounded_chunk_end("α", 0, 1)` must return `None`, rather than `Some(0)`. The previously mentored repair passes the unchanged finite reference contract.

The six submissions contain five distinct source bodies. These are finite regression checks, not blind held-out comparisons. The full Rust workspace passed 568 tests with 27 opt-in tests ignored; strict offline Clippy and build passed. The related Python checks passed 101 tests, both experiment HTTP fixtures passed, the existing governed Rust submission over SSE passed, and the ten-page website checks passed.

Only the original fixture's outer `include!`/`env!` candidate module line was removed, because the native adapter separately compiles and links that library. Its reference function, test body, Unicode strings, invalid offsets, zero limits and overflow cases are unchanged. Both hashes are retained. This is the same behavioral contract, adapted to separate compilation.

The passing source was already identified in an earlier guided review. This confirms result continuity through the new evaluator, rather than a new coding discovery or autonomous improvement. No new success is claimed for the rejected Python observer-test repairs. [Scoped results and hashes](evidence/local-independent-experiments-2026-11-15.json) preserve these distinctions.

## Run a reviewed Rust trial

Keep the configured local company harness running in its terminal. Use an already registered local company and review [the Rust example](../config/experiment-loop.rust-example.json), including its actor, task and one-call limit. The supported installed Rust compiler and native binaries are required; this workflow does not download them.

Prepare the existing owner acceptance for separate candidate compilation and create the baseline once:

```sh
python3 - <<'PY'
import hashlib
from pathlib import Path
directory = Path('data/experiment-inputs/local-rust')
directory.mkdir(parents=True, exist_ok=True)
text = Path('scripts/fixtures/chunk-boundary-acceptance.rs').read_text()
wrapper = 'mod candidate {include!(env!("GENTS_CANDIDATE_SOURCE"));}\n'
assert text.count(wrapper) == 1
adapted = text.replace(wrapper, '')
assert hashlib.sha256(adapted.encode()).hexdigest() == '94f2bb5de1a1197070e971dea1348e23f25daafe9e6c0bdd520a771aed23b3ba'
for name, body in [('acceptance.rs', adapted), ('evidence.json', '{"phase":"baseline"}\n')]:
    with (directory / name).open('x') as output:
        output.write(body)
PY
python3 scripts/experiment_loop.py --id local-rust-v1 freeze --plan config/experiment-loop.rust-example.json
python3 scripts/experiment_loop.py --id local-rust-v1 watch
```

The last command stays running and waits for changed evidence. Update the selected input with actual new evidence when available; freezing itself dispatches nothing. Inspect `status` with the same ID afterward. These commands deliberately do not overwrite existing baseline files or frozen plans.

## Remaining scope

The queue connects source generation, governed write/readback and independently executed Rust acceptance. Recorded acceptance is an observation of a particular source hash and policy epoch; later source or permission changes require fresh review before use. It cannot yet design its own independent suites, adapt its trial agenda, select winners on held-out comparisons, promote/roll back procedures or models, or demonstrate a 24/7 soak. Other languages and arbitrary media need their own trustworthy evaluation adapters. Python test authoring remains unresolved. No new company-model inference, download, deployment or adoption occurred in this pass, and no service was left running.
