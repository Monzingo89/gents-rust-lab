# Shared model identity admission and real local trial

November 21, 2026. This slice extracts one proven construction-independent boundary: local model and company adapter pin admission. The grounded executor and workflow adapter now use the app-owned `ModelPins` service. The workflow adapter no longer constructs grounded-task settings with unrelated TTL, pending-job and output-token fields merely to check model bytes. No new crate, service, model pool or route was added. The two executors retain their distinct lifecycle and authority contracts; retiring an entire executor still requires parity evidence.

## Changed behavior

The identity cache is keyed by repository root, artifact kind/path and expected digest. MLX package snapshots and company-confined adapter checks retain their existing rules. GGUF checks now inspect the root as well as every path component on every admission, including a warm hit. Changed bytes, root permissions, symlinks, file links and expected pins cannot reuse a stale admission. After a cache miss hashes bytes, the path fingerprint must match the pre-hash observation before caching. Both callers keep their existing pre-call and post-call checks.

A new regression first failed against the old implementation: changing the root to group-writable after a successful check was accepted by the warm cache. The same regression passes with the shared verifier. Two further tests exercise changed identity/bytes, forbidden provider selection, symlink roots, hard links and special files. The low-level artifact checker rejects nonordinary files before opening, so an unserved FIFO does not block admission. Package digest encoding is unchanged; the duplicate filesystem metadata encoder is shared.

These are trusted-host read-only checks, not an OS sandbox against a hostile privileged or same-user process. Cache admission does not grant tool, memory or model execution capabilities. The broker's existing protected-source list now includes this shared boundary, both callers, native task admission and the real-model evaluator.

## Real-model evidence

The existing `scripts/task_engine_live.py` trial now records frozen criteria before generation, harness/model hashes and actual dispatch/return receipts from its isolated company database. Its original accepted answer forms and one-call oracle are unchanged. A separate reviewed extraction binding is installed only after the RAG trial to measure exact repetition. Generated drafts remain ineligible for the verified-answer cache.

The installed Qwen3-4B Q4_K_M GGUF passed the fixed question “Who was US president in 1976?” with “Gerald Ford”. This is **one bounded literal/reference task**, not general factual acceptance or production profile qualification. Runtime status remains `generated` / `citations_checked_answer_unverified`; the independent script oracle passed. No model download, cloud call or production configuration change occurred.

| Measurement | Observation |
| --- | --- |
| Independent fixed oracle | 1/1 passed; no retries |
| Task elapsed | 8.813317 s, including admission; native reply interval separately retained in receipts |
| Whole supervised command | 11.662234 s including startup/shutdown |
| Generative calls | 1, native dispatch/return receipt |
| Tokens | 661 input / 304 output |
| Model slot context | 8,192 configured and observed tokens |
| Backend RSS | 3,934,797,824 bytes at reply; peak and GPU allocation unknown |
| Exact extraction cache | One miss, one hit in two requests; repeat 0.259 ms; zero generation in both extraction requests |
| Paid calls / actual provider fee | 0 / $0 |
| Total hardware, electricity and development cost | Unknown |

Raw private evidence is in `data/task-engine-live/5631f2dc-f2c3-4875-bd25-cfbf57950e06/`; its frozen criteria, report and native checkpoint are retained. The outer bounded check report is `data/local-evaluation/real-model-b11c12e5-1424-4a1f-9591-4835de1459bf/report.json`. The [portable evidence record](evidence/shared-model-pins-2026-11-21.json) records hashes, metrics and verification outcomes without depending on those ignored directories being published.

## Verification and reproduction

`cargo test --workspace --offline --locked` passed **704 tests**, with 33 opt-in tests ignored. Strict offline all-target Clippy passed. All eight `python3 scripts/local_evaluate.py` checks passed in **127.29 s**: build, lifecycle fixtures, actual Rust lifecycle, frontdoor, workflows, native stdio, native SSE and screens. The 32 Python broker/governance tests passed. These fixtures establish execution contracts, not model-solving competence. The full platform-dependent `scripts/check.sh` was not rerun in this slice; no UI source changed here.

To repeat the actual installed-model trial with bounded owned-process cleanup, build offline first, then run from the repository root:

```sh
cargo build --workspace --offline --locked
python3 - <<'PY'
import json, os, sys, uuid
from pathlib import Path
sys.path.insert(0, str(Path('scripts').resolve()))
from local_evaluate import run_check
output = Path('data/local-evaluation') / ('real-model-' + str(uuid.uuid4()))
output.mkdir(parents=True, mode=0o700)
env = {k: v for k, v in os.environ.items()
       if k in {'PATH', 'HOME', 'USER', 'TMPDIR', 'LANG', 'SHELL'}}
result = run_check('existing-gguf-rag', [sys.executable, 'scripts/task_engine_live.py'],
                   240, output, env)
(output / 'report.json').write_text(json.dumps(result, indent=2) + '\n')
print(output / 'report.json')
raise SystemExit(0 if result['passed'] else 1)
PY
```

The harness and native backend were absent after the measured trial. Existing company services were not restarted. Repeated trials must retain failed results; do not modify the oracle or label this single case an evaluated production profile. Next work remains broader representative task evidence, grounded/RAG lifecycle parity, structured handoff recovery and deduplicated accounting coverage in [the backlog](162-local-execution-baseline.md).
