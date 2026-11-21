# Actual native cancellation and recovery

November 21, 2026. One new opt-in test in `crates/harness/src/local_runtime/native_cancel.rs` reuses `LocalRuntime::at_port` and the installed pinned Qwen3-4B model. It is compiled only under `cfg(test)` and ignored by normal checks. Production routing, construction, permissions and accounting remain unchanged.

Before inference, it freezes the model/test-binary/probe-source hashes, both requests, generation/context limits, two-call ceiling and acceptance criteria. It waits for its own authenticated backend slot to report `is_processing: true`, then sets the existing cancellation token. It requires the typed `cancelled` failure and process reap within five seconds. A distinct session with a fresh token must return a non-cutoff native reply from a different owned process; final shutdown must reap that process too. Keys and complete slot/prompt payloads are never persisted or printed. Process observation errors fail the check.

| Measurement | Actual result |
| --- | --- |
| Accepted native cancellation/recovery cases | 1 / 1 |
| Cancellation token through returned receipt/reap | 210.008 ms |
| Observed native requests | 2: one interrupted, one completed |
| Observed context | 8,192 tokens in both processes |
| Fresh recovery reply | `OK.`, finish `stop`, 78 ms inference latency |
| Recovery input / output tokens | 26 / 3 |
| Recovery RSS at reply | 3,866,017,792 bytes |
| Probe operation / supervised Cargo elapsed | 2.356 s / 14.485 s |
| Paid runtime calls / provider fees | 0 / $0 |
| Interrupted/aggregate tokens, peak/GPU memory, answer-cache effectiveness, total cost | Unknown |

The interrupted receipt has zero received bytes and unknown usage. A busy slot does not establish completed token counts. Provider fees are deterministically zero for this owned local backend; hardware/electricity/development costs are unmeasured. Operation elapsed excludes pin hashing and compilation; recovery inference latency excludes loading; RSS is a sample, not a peak. No speed comparison with the failed Rust tasks is justified.

Raw records: `data/local-evaluation/native-cancel-809dd9d0-d69e-44e2-b860-3d99ee903840/{criteria,report}.json`. [Versioned evidence](evidence/native-cancellation-recovery-2026-11-21.json) retains hashes and validation results. The original probe source was retained after the trial and verified against its frozen hash; future runs snapshot it and set directory mode 0700 before writing records. These metadata changes were compiled/checked without repeating inference. Frozen criteria do not separately hash the installed backend executable; the existing runtime factory selects and capability-checks it. This is not an independently sealed backend-installation claim.

```sh
cargo test -p gents-harness --offline --locked \
  local_runtime::native_cancel::busy_native_cancel_and_fresh_recovery \
  -- --ignored --exact --nocapture --test-threads=1
```

The command passed under `local_evaluate.run_check`'s owned process-group supervisor with a 300-second outer deadline and whitelist/local-only/offline environment. Use the supervisor pattern in [the Rust probe](170-native-rust-qualification.md) with this Cargo argv when repeating. Internal observation/cancellation bounds are 60/70 seconds, with a five-second acceptance limit after the token; recovery is bounded at 120 seconds. No automatic retry or download occurs. Do not run all ignored tests or overlap heavyweight native suites.

Formatting, strict offline workspace Clippy across all targets, and the existing runtime group passed (8 tests, 10 unrelated opt-in tests ignored). No owned runtime remained afterward. The previous full repository pass predates this test; it is not a fresh full-suite claim.

Owner HTTP cancellation, coalesced subscribers, durable budgets and terminal checkpoints have separate fixture integration checks. Their combined latency with real inference remains unmeasured. This test neither revives a cancelled task nor qualifies a business-task profile. Both [Rust authoring failures](170-native-rust-qualification.md) remain failures.

Routing review confirmed that configured evaluated profiles, evaluation references and latency scalars are owner declarations, not machine-resolved qualification records. Model/CEO proposals cannot install them. No new benchmark registry or automatic promotion was justified by the failed trials.

A filename-only configuration search found `com.gents.local-company.plist` pointing to this checkout's `scripts/start-company.mjs`, with RunAtLoad/KeepAlive enabled. Its launchd query reported the service was not loaded; it was not started or changed. The inspected listeners/configuration paths did not locate Open WebUI, which does not prove absence. Next work remains useful held-out task qualification, combined owner/native cancellation evidence and the actual Open WebUI ownership/concurrency/accounting boundary.
