# Frozen native Rust qualification

November 21, 2026. The new opt-in `scripts/rust_candidate_live.py` reuses the existing experiment controller, candidate workflow, native file receipts and independent Rust evaluator. It starts an isolated loopback harness/company with no scheduler or office inference, uses one pinned installed model, and freezes the original request, acceptance hash, model/configuration, grants and one-call allowance before dispatch. It does not add a service, runtime route or paid fallback.

The unchanged `config/experiment-loop.rust-example.json` requests `bounded_chunk_end`: safe byte limits, UTF-8 boundaries, empty/end-of-input handling and overflow protection. The suite comes from `scripts/fixtures/chunk-boundary-acceptance.rs`. As already documented in [the existing evaluator instructions](145-independent-experiment-results.md), only its standalone candidate-module include is removed because the native adapter supplies that module. All oracle assertions remain unchanged; the adapted SHA-256 remains `94f2bb5de1a1197070e971dea1348e23f25daafe9e6c0bdd520a771aed23b3ba`.

The author receives the task, not the held-out acceptance source. Only `files:read` and `files:write` within its run workspace are granted. Host-owned stages perform the write and exact readback. The evaluator revalidates current policy, frozen intake, source bytes and canonical native receipts before and after isolated execution. A generated submission is not acceptance, and acceptance would still not authorize source promotion or production profile admission.

## Actual outcomes

| Metric | Qwen3-4B Q4_K_M | Qwen3-8B Q4_K_M |
| --- | ---: | ---: |
| Independently accepted / attempts | 0 / 1 | 0 / 1 |
| Terminal runtime submission | `candidate_submitted` | `candidate_submitted` |
| Independent rejection stage | Acceptance | Candidate compilation |
| Task elapsed, dispatch response through observation/evaluation | 12.099 s | 19.813 s |
| Model reply latency | 10.285 s | 18.354 s |
| Backend RSS at reply | 3,901,931,520 bytes | 6,448,381,952 bytes |
| Input / output tokens | 2,262 / 800 | 2,256 / 927 |
| Observed context slot | 8,192 tokens | 8,192 tokens |
| Native model replies / durable attempt reservations | 1 / 1 | 1 / 1 |
| Paid runtime calls / provider fees | 0 / $0 | 0 / $0 |
| Cache effectiveness, peak/GPU memory, total cost | Unknown | Unknown |

The 4B candidate returned `None` for empty input, offset zero and positive limit, where the frozen oracle requires `Some(0)`. The 8B candidate failed the existing strict compiler policy on unused variables; warnings were not suppressed and the candidate was not edited. Both models' one-call trials are exhausted. Native audit sequence references 8 and 9 bind each run's write/readback, with distinct exact source hashes. Models ran serially in fresh processes under the same original task, full prompt profile and generation ceilings. Model identity/context metadata differ; these are single cold trials, not statistically useful latency rankings or general model comparisons.

Raw records:

- 4B: `data/local-evaluation/rust-live-721b63ce61f849a78516/report.json`, experiment ledger `data/experiment-loops/rust-live-721b63ce61f849a78516/`.
- 8B: `data/local-evaluation/rust-live-5d4e71e0b23d4a23b5b3/report.json`, experiment ledger `data/experiment-loops/rust-live-5d4e71e0b23d4a23b5b3/`.
- [Versioned evidence summary](evidence/native-rust-qualification-2026-11-21.json) records report/log hashes, binary/model identities and failures. Company databases, candidate workspaces and observer snapshots remain at their original private data paths for reconciliation.

The first probe preflight incorrectly compared the standalone wrapper to the native acceptance hash. It stopped before any harness dispatch/model call; its failed supervised log is retained. The corrected probe used the already documented adaptation. The completed 4B/8B reports freeze harness/model/test/configuration identity but did not snapshot the Python probe source before dispatch. A post-run source snapshot for 8B is labelled as such. Future invocations additionally freeze probe/example/fixture hashes and retain the probe source. This distinction is preserved rather than backfilling earlier criteria.

## Repeating deliberately

This is an opt-in real-model check, outside the ten-command fixture suite. Each invocation creates a new frozen trial and burns a fresh one-call allowance. Both installed model choices are pinned; missing or changed files fail rather than download. Build current native binaries with `cargo build --workspace --offline --locked`. Run no other heavy native verifier suite concurrently.

For example, the following bounds the whole process group, retains exit/log evidence and cleans up owned descendants. A rejected task returns nonzero; it must not be counted as verified completion.

```sh
python3 - <<'PY'
import json, os, sys, uuid
from pathlib import Path
sys.path.insert(0, 'scripts')
from local_evaluate import run_check
folder = Path('data/local-evaluation') / ('rust-supervised-' + uuid.uuid4().hex)
folder.mkdir(mode=0o700)
env = {k: v for k, v in os.environ.items()
       if k in {'PATH', 'HOME', 'USER', 'TMPDIR', 'LANG', 'SHELL'}}
result = run_check('rust-candidate-live',
    [sys.executable, 'scripts/rust_candidate_live.py', '--model', 'qwen3-4b'],
    330, folder, env)
(folder / 'report.json').write_text(json.dumps(result, indent=2) + '\n')
print(folder / 'report.json')
raise SystemExit(0 if result['passed'] else 1)
PY
```

Both owned harnesses exited cleanly, with no remaining harness/model process observed. The existing evaluator's 21 tests passed, including revoked/forged custody, hash changes, native executable identity and current-source revalidation. Probe syntax, discovery adapters and diff checks also passed. The prior full repository pass remains documented in [the requirement audit](169-requirement-audit-and-routing.md); it predates this opt-in Python probe and is not a fresh broad-suite claim.

## Next bounded work

These failures disqualify automatic routing/promotion based on these trials. A future owner-frozen held-out set may evaluate the existing focused prompt profile or separately bounded diagnostic repair; it must retain these failures and the unchanged acceptance criteria. Broader routing qualification remains open. Open WebUI location/configuration is still needed to establish its actual startup, shared-model and accounting boundary; no external service has been started or changed.
