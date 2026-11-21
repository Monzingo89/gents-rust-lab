# Interactive local company launcher

From the Git root, run `npm run company`. This existing equivalent is retained instead of adding a duplicate `company:local` command. Enter a task at <http://127.0.0.1:8788/?autoconnect=1>, select **Plan with the CEO**, and send it with **Discuss with local CEO**. The CEO proposes staffing and tools; the existing independent local judge and frozen owner review still gate execution.

The launcher reuses `run-company.sh`, the offline Cargo workspace build, the sealed native runtime bundle, the Rust harness and the existing building UI. The shared runtime pool loads the selected local model on demand. Native MCP children remain actor/run scoped; no extra unscoped MCP daemon or replacement executor is introduced. Ollama is not required by this GGUF runtime path and an existing Ollama service is left alone.

The terminal follows persisted company audit and run events, including CEO requests, selected models, native inference receipts, tool names/result status, failures and completion. It excludes prompt/reply bodies, tool arguments and raw errors; detailed results remain in the authenticated building UI. Receipt dispatch/receive times establish actual inference; a queued request or a startup readiness check does not. Reconnection and audit gaps are reported explicitly. Architecture call snapshots identify pinned candidates and recorded native timing without copying generated text.

The CEO building activity now follows the selected conversation's persisted pending/completed events. It expires stale observations and clears them on connection/company/auth changes. This is a pending-work animation, not a claim that queued work is executing on the GPU. Existing worker/tool animations remain driven by run events; staged office tours stay separate.

Concrete repairs:

- The default company config previously failed startup validation (`max_steps=100`, `max_tokens=10000`, 300-day duration). It now uses the existing legal limits: 3, 4096 and 30 days.
- Added exact SHA-256 pins for the four model profiles already configured in the CEO/worker roster, covering three distinct installed weights. No actor, model path or assignment was replaced. Capacity floors use weight size plus 2 GiB; they are admission metadata, not measured fit or performance claims. Native fit checks remain required.
- The default six-actor CEO contract measured 27,154 prompt tokens, exceeding the previous 16,384-token ceiling. The existing CEO GGUF profile explicitly requests 32,768 tokens. The bounded native GGUF option now supports that ceiling while MLX retains 16,384. Single-slot observed-context equality, generation reservations, tokenization and fit/recovery checks remain enforced. Other profiles retain their configured/default context; availability for a large fallback planning prompt is not established.
- Compatible-service reuse now keeps the terminal event follower alive. Ctrl+C stops owned process groups and the follower; it leaves reused services alone.
- Intermittent process-group `EPERM` errors during child exit receive a bounded 250 ms retry. Persistent permission failures still fail cleanup explicitly. Regression coverage exercises both cases and owned descendants.
- Local-only/offline controls are pinned across environment reloads. Interactive startup defaults to email/scheduler/research disabled and no automatic resume of old interrupted jobs. Explicit process environment opt-ins preserve those existing operations. The harness's other launch modes retain their prior recovery default; manual resume remains available.

## Validation

On 2026-11-21:

- `cargo fmt --all --check`, strict workspace Clippy for all targets, and the frontend build passed.
- Harness tests: 429 passed, 24 ignored. Launcher/environment/event/building tests: 38 passed.
- `python3 scripts/local_evaluate.py`: all 11 bounded offline fixture checks passed. Report: `data/local-evaluation/3f84d68e-23ab-419b-ada6-f12b785d4bc9/report.json`. These exercise mock-model orchestration and actual native tool transports, not real-model task competence. After the final lifecycle change, the Node lifecycle suite and actual-Rust startup/readiness/shutdown smoke also passed.
- Started the actual company using the existing command. Checked loopback local-only configuration and a verified sealed native bundle. A second launcher reused it, kept following events and exited on SIGINT without stopping the incumbent.
- Stopping the owning launcher closed ports 8788 (harness/UI) and 8791 (owned llama-server); the unrelated Ollama listener on 11434 survived. Restarted the final launcher and left its office available for task entry.

### Real task result: blocked, not completed

Submitted free text through the actual building interface and local CEO, with native model receipts recorded in the terminal. The multi-operation arithmetic/file trial produced duplicate staffing. A smaller read-only arithmetic trial reached a ready proposal but its local architecture judge requested revision; subsequent CEO revisions were rejected for unsupported numeric planning claims. These outcomes were retained, not promoted to successful tool execution.

A separate bounded text smoke task requested exactly `Welcome to Gents`. The CEO chose **Engineer Gent**, using its existing **Qwen3-Coder-30B** worker profile. The plan became ready, but the independent architecture judge returned **revise**, including when it agreed with the one-agent ordered architecture. No worker run was approved, dispatched or completed. A diagnostic architecture trial supplied facts about existing final review and still received revise; that input change was removed. Original evaluators and review gates remain intact.

Retained planning ID: `5e518ba4-def5-45e7-b806-a84d6ab2ea17`. Private local trial record: `data/local-company-launch-verification-2026-11-21.json`. Provider inference fees were $0 for these local calls; electricity is excluded. **Verified completed real tasks: 0.** Model/policy qualification and a successful worker/tool trial remain unresolved; the launcher must not claim general orchestration capability from startup or fixture passes.

Downloaded models, journals, receipts, bundles and build products remain under existing ignored paths. Rust workspace, desktop and frontend component boundaries are unchanged; no repositories or Git histories were moved or removed. Existing Python/dependency edits were left in place.
