# Focused context and one accepted Rust task

November 21, 2026. `scripts/rust_candidate_live.py` now accepts the existing owner-frozen `--prompt-profile full|focused` mode (default `full`) and the installed, pinned `qwen3.8-27b` artifact. It continues to use the existing experiment controller, candidate workflow, native file custody and independent Rust evaluator. No production route, default model, call limit, evaluator or grant changed.

Two new one-call trials used the same original `bounded_chunk_end` task and unchanged acceptance as [the initial trials](170-native-rust-qualification.md). Their native task-input fingerprints match the original full-context 4B trial: `67746dfd0451bdd55ec4916902dadc60d2b33aa1c481928c50db95b52304f5b5`. Neither received a reference implementation, prior candidate, diagnostic repair hint or the held-out acceptance source. Both reserved 512 reasoning / 2,048 final tokens and used an observed 8,192-token context slot.

| Model / context | Frozen task accepted | Input / output tokens | Task elapsed | RSS at reply |
| --- | --- | ---: | ---: | ---: |
| Qwen3-4B / full, earlier | No: empty-input assertion | 2,262 / 800 | 12.099 s | 3,901,931,520 bytes |
| Qwen3-4B / focused | No: strict compilation | 488 / 957 | 10.351 s | 3,894,378,496 bytes |
| Qwen3.8-27B / focused | **Yes** | 527 / 814 | 59.962 s | 8,716,320,768 bytes |

The observed 4B input reduction was **78.426%**, with the same logical task fingerprint. Its candidate still failed: unnecessary parentheses violated the unchanged `-D warnings` policy, and it was not edited or retried. Fewer input tokens did not establish better verified completion. These are single, ordered trials in fresh backend processes, not controlled filesystem-cold measurements or a statistical latency/model ranking. Different labels/tokenizers, cache state and loading can affect measurements.

The 27B candidate passed strict compilation and the original independent Unicode/offset/limit/overflow suite under the existing macOS isolation. The host revalidated exact source, frozen intake, current permission epoch and canonical write/readback receipts before and after evaluation, and again before retaining the [exact candidate review artifact](evidence/focused-rust-candidate-2026-11-21.rs). Its SHA-256 is `9345d9ae6f0622701f0f1b37fdcbc0e6e9d34956ffae01d0a9e1c5b4b97f28dd`. It was not installed into runtime source or promoted to a general profile.

Both new trials used exactly one durable reservation and one native reply, with zero paid calls and $0 provider fees. Separately reported reasoning-token counts, peak/GPU memory, total operating cost and answer-cache effectiveness remain unknown. RSS is process residency only; it excludes GPU allocation and must not be used as a minimum host-memory requirement. Before the larger trial, the host reported 36 GiB total and 87% memory free; the existing owner planning policy lists a 24 GiB capacity floor for this 16,464,440,224-byte model. That floor is an estimate, not fit assurance. The normal native loader still performs device/context admission.

Raw results:

- Focused 4B: `data/local-evaluation/rust-live-914c7a0c01a140e1bb49/report.json`.
- Focused 27B: `data/local-evaluation/rust-live-a6a33362066f41c6b05d/report.json`.
- [Versioned evidence](evidence/focused-rust-task-2026-11-21.json) binds frozen criteria, probe snapshots, model/harness/evaluator/compiler hashes, receipt sequences, command logs and the retained candidate.

Across the four November 21 Rust author trials (including the earlier full-context 8B rejection), **1/4 passed this one task's frozen suite**. Different models/context modes make that a trial ledger, not a general success-rate estimate. All failures remain retained. No model was automatically qualified for production routing.

To deliberately run a fresh focused trial, use the process-group supervisor from [170](170-native-rust-qualification.md) with:

```sh
python3 scripts/rust_candidate_live.py --model qwen3.8-27b --prompt-profile focused
```

Each invocation freezes a new one-call allowance. It does not continue an exhausted trial or download a missing model. Both owned harnesses exited cleanly. Governance/proposal tests (8), focused context/schema tests (3), evaluator tests (21), and probe syntax passed. Skill/discovery/diff checks are recorded separately; the prior full repository pass is not a new broad-suite claim.

Next work: qualify a useful held-out task set and comparable routing evidence without discarding failures; establish the actual Open WebUI boundary. This scoped success does not resolve the combined owner-HTTP/native cancellation latency gap from [171](171-native-cancellation-recovery.md).
