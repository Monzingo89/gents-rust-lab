# Local settings and architecture review

Local inference needs no API key. Run `npm run env:check` to inspect settings and credential **presence**, without printing values or contacting a provider.

| File | Purpose | Loaded by the company? | Tracked? |
| --- | --- | --- | --- |
| `.env.local` | Private optional shared credentials and generic development overrides | Yes, first | No |
| `.env.company` | Private local company runtime settings | Yes, after `.env.local` | No |
| `.env.website.local` | Hosted support settings/signing secret | No | No |
| Corresponding `.example` files | Safe documented templates | No | Yes |

Use `.env.company`, not `.env.compny`. Explicit pinned launcher settings take precedence over files. The ordinary company launcher pins local authentication, loopback port 8788, local configuration and its normal run store. The separately supervised MCP development office explicitly retains its own configuration and `data/mcp-curriculum.sqlite3`; changing the environment files does not migrate or replace that store.

Never upload these private files, use a client-public secret prefix or paste credentials into a prompt. Keep private files owner-readable only (`chmod 600`). Existing optional HF and mail credentials do not authorize paid inference, downloads, model promotion or mail tool access. Do not run `vercel env pull .env.local`: it can overwrite the existing private company setup. Vercel support uses separately scoped server Secrets; see [website operations](123-website-support-and-deployment.md).

## Two local model opinions

A current ready CEO plan can request an advisory architecture assessment:

```text
POST /api/companies/{company}/planning/{planning}/architecture/{assessment}
{"expected_revision":2,"speed":"patient","maximum_elapsed_seconds":1209600}
GET /api/companies/{company}/planning/{planning}/architecture/{assessment}
GET /api/companies/{company}/hardware
```

IDs and the revision above are placeholders. Use the actual owner-scoped plan revision. The first owner-listed planning weight recommends an architecture and actor count. A distinct owner-listed weight reviews that exact recommendation. Both calls remain local and sequential. They receive the explicit task brief, frozen plan, real actor roster, supported executor menu and fresh hardware observations. Neither can grant tools, change staffing, start work or accept the task.

Hardware, inference and journal ports are separate. macOS reports Apple GPU identity and shared unified memory without adding it to host RAM. Installed NVIDIA probes report dedicated memory where supported. Missing probes leave values unknown. Current observations are refreshed for each model review; the visible panel refreshes hardware every 30 seconds and when preferences change. Memory/model integrity admission is rechecked before each model call. These are observations and admission checks, not throughput predictions or continuous monitoring while the app is closed.

`fast` is a preference. It does not authorize paid services or prove concurrency. A requested 14-day allowance does not silently extend the configured one-day run limit; the result flags that configuration mismatch. More time cannot guarantee a result or supply missing tools.

The journal retains pending dispatch before inference. A lost response is recovered by GET; it is never automatically reissued. Cached terminal results reuse the same assessment without inference. Review disagreement, stale plans, insufficient staffing and missing executors require a new owner-reviewed plan. Each planning session has a bounded assessment count.

Factory preparation can bind the canonical assessment ID, exact record digest and current model-policy digest. Normal intake/execution rechecks that binding. Legacy manual preparation remains available; this change does not yet require every old path to obtain an assessment. Tool-enabled hub/peer execution and a complete guided UI remain unfinished.

## Actual evidence and limits

- Original probe `4c16e6b2-a8a1-415d-b1e1-310f5aec3154` remains **unqualified** with a pending judge stage. Recorded CEO usage: 819 input / 76 output. Judge usage is unknown. No replay was performed.
- A regression reproduced a whole-record privacy scan falsely identifying a valid timestamp as payment-card data. The journal now scans task/model content separately from typed native metadata; sensitive text still fails closed.
- Repaired probe `1af73020-e69b-4e64-9e23-014639c21cab` used actual Qwen3-Coder-30B and distinct Qwen3.8-27B weights. Both selected an ordered, one-actor flow for a seeded text-only plan. 1,747 input + 1,092 output = **2,839 local tokens**, about 129 seconds, **$0 provider/tool fees**, excluding electricity/hardware/development. The 14-day request correctly flagged the configured run-limit mismatch.
- Independent read-only verification checked copied binary bytes, both installed weight bytes, canonical SQLite row integrity, exact input packet hashes, native receipts, factory binding, zero jobs and closed private listeners. This seeded-plan probe is not a native CEO conversation or completed task.

Opt-in commands (require the existing reviewed local weights; do not download them automatically):

```sh
python3 scripts/architecture_native_probe.py
python3 scripts/architecture_native_probe.py --native-conversation
python3 scripts/verify_architecture_assessment.py <probe-id>
```

The conversation mode requires actual initial clarification and a ready second reply; it never seeds that plan. Consult its retained results before claiming it passed. Default checks use clearly simulated model receipts; native probes are separate and consume local compute.

## Picture fine-tuning

No annotated pictures have been supplied yet. No image training, dataset conversion or download has started. The existing MLX LoRA workflow is for text; it is not an image trainer. Before selecting a vision backend, inspect the actual dataset, labeling format, intended task, licenses, memory limits and held-out evaluation. Detection boxes, captions and image-generation annotations are different contracts and must not be silently interchanged. Training results remain candidates until independently evaluated and owner-promoted.

## Smaller text-only planning contract

The office offers an explicit **Text-only planning** selection when starting a conversation. Its API equivalent is `planning_mode: "reasoning"` on the initial planning turn. This mode stays fixed: it rejects attached files, native source inputs, owned components/data and native separation constraints instead of silently dropping them. The decoder requires empty tool arrays and empty native contracts; runtime validation independently rejects an invented file or tool. The small reasoning prompt omits unrelated native tool catalogs and graph instructions. The mode does not classify tasks automatically or bypass owner review.

An initial unseeded general-mode trial (`c143fc8c-d58d-4a19-9f7f-0d42247f5c9f`) asked a genuine clarification question, then failed because the CEO invented `synthetic_launch_note_update.txt` with an undeclared source despite a no-file request. The fixed contract rejected it; no architecture review or job followed. Both measured replies are retained: 14,133 input and 352 output local tokens, $0 provider fees. That is a failed planning case, not a completed-task benchmark. The separate reasoning-mode probe `c1e3e8d3-2aec-4f1d-a8a0-1043a5a20531` passed clarification, ready plan, two-weight architecture review and factory binding. Its planning used 726 input + 280 output = **1,006 local tokens**. See the [actual conversation and steps](124-local-ceo-reasoning-conversation.md). This remains preparation, not completed-task evidence.
