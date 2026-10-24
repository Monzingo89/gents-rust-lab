# Training Room and CEO oversight

The Training Room now runs actual local MLX LoRA updates. The CEO observes company work and can issue an all-hands brief. These are separate from dream housekeeping and from the judge that decides whether a deliverable meets its acceptance criteria.

## What improves

Role profiles tell a model what to do. Context retrieval supplies current evidence. Training changes a small set of adapter weights so a role can learn demonstrated behavior. A useful training set contains verified examples of that role's exact work, including correct structured handoffs, errors to avoid and approved tool-response formats.

More hours of training do not guarantee better results or faster decoding. LoRA adds parameters and can increase per-token work. Better task accuracy, fewer retries and shorter valid outputs may reduce total completion time. Measure those outcomes against the unchanged base or previously approved adapter before claiming a gain. Excessive training can overfit, regress or memorize private examples.

## First supported model

Use the already downloaded `models/gemma-4-E4B-it` Safetensors directory. The pinned local MLX library supports it, and a real one-step training smoke test saved an approximately 898 KiB adapter with all seven LoRA-B tensors nonzero. The test reported roughly 15.1 GB peak GPU memory. Its tiny held-out loss changed from 7.2267 to 7.1798. This is runtime proof, not evidence of business-task improvement.

GGUF files are currently inference artifacts in this harness. To train a Qwen/Phi role later, obtain an MLX-compatible Safetensors version of the exact base, evaluate its memory requirements, train an adapter, then use it through MLX. GGUF adapter conversion and deployment are not implemented here. An adapter must match its base model architecture and weights. Do not attach a Gemma adapter to a Qwen model merely because both represent the same job title.

## Data and lifecycle

```text
Reviewed role examples
  -> disjoint training / validation / test splits
  -> immutable dataset hashes and base fingerprint
  -> local LoRA job under the shared GPU lease
  -> candidate adapter + observed metrics
  -> independent task, regression, privacy and license review
  -> explicit promotion for this company and role
  -> retain old adapter for rollback
```

Every training artifact lives under `data/companies/<company_id>/training/`. Models never train automatically on company handbooks, owner evaluation, raw conversation logs or their own unverified answers. The default base weights remain unchanged. An approved company adapter is never automatically used for another company or promoted globally.

Dream housekeeping now puts completed-job model records into a separate **unreviewed training proposal queue** in the company database. That queue is evidence for curating examples, not a training dataset. Human/independent review must verify the answer, training rights and role suitability before an example enters a dataset. Company knowledge that changes frequently should remain in retrieval, rather than become memorized weights.

## Prepare a role dataset

Create a local directory with `train.jsonl`, `valid.jsonl` and `test.jsonl`. Production candidates require at least 20, 5 and 5 examples respectively. Each row has this shape:

```json
{"company_id":"we-need-gents-lc","role":"backend-engineer","reviewed":true,"license":"YOUR_ACTUAL_TRAINING_RIGHTS","source":"verified-task-and-test-evidence-id","messages":[{"role":"user","content":"A precise role task"},{"role":"assistant","content":"Its independently verified correct response"}]}
```

Replace the illustrative license and content with actual reviewed material. Prompts must be unique across all three splits. Never use `owner-evaluation/` as training, validation or test input. Evaluation data for training is a separate explicitly approved corpus. Keep an additional independent role regression suite outside the trainer for promotion review.

Recommended role targets include returning valid handoff envelopes, preserving company/run IDs, following one API contract, generating tested migrations or summarizing a verified accounting ledger. Do not train every role on every task. Include boundary and general regression cases so specialization does not destroy basic reliability.

```sh
python3 scripts/gents.py training-prepare \
  --role backend-engineer \
  --base-model models/gemma-4-E4B-it \
  --data /absolute/path/to/reviewed-role-examples \
  --iterations 100
python3 scripts/gents.py training-start TRAINING_ID
python3 scripts/gents.py status
```

Defaults are conservative for this 36 GB Mac: LoRA rank 4, one final transformer layer, batch size 1, maximum 512 tokens and learning rate 1e-5. Jobs accept 1–2000 iterations and have a 24-hour execution limit. Adapter checkpoints save every 25 steps and at completion. Automatic retry and exact optimizer-state resume are not implemented. Longer/heavier training requires a separately evaluated resource configuration rather than blindly increasing duration.

Training and inference hold the **same GPU lease**. A long training job pauses other model generations until it finishes or is cancelled. The CEO's lightweight timer can still observe jobs, but its own AI assessment waits for the GPU. No second model server is launched to compete for memory.

```sh
python3 scripts/gents.py training-cancel TRAINING_ID
```

A safe demonstration that needs no business data is `python3 scripts/gents.py training-smoke`. It creates synthetic examples and runs one step. Smoke adapters cannot be promoted.

## Evaluate, promote and roll back

Training measures held-out test loss before and after the update. A later training job can warm-start from the same role's currently approved adapter; its baseline is that incumbent adapter, preserving lineage across projects. The optimizer starts fresh. Falling loss alone cannot establish task accuracy, security, privacy or speed.

Create a review JSON after running independent checks:

```json
{"role_tasks_passed":true,"general_regression_passed":true,"privacy_passed":true,"license_reviewed":true,"adapter_sha256":"EXACT_HASH_FROM_METRICS","evidence":"Actual test report IDs and evaluation findings"}
```

These fields are operator attestations, not automatic security certification. The server rejects smoke/non-improving candidates and verifies base/adapter hashes before promotion. Keep measurable task success, retries, valid handoff rate, token counts and wall-clock duration in the evaluation evidence.

```sh
python3 scripts/gents.py training-promote TRAINING_ID /absolute/path/to/review.json
python3 scripts/gents.py training-retire TRAINING_ID
```

Promotion retires the prior active adapter for that role/base and retains its files. To roll back, retire the new adapter and re-review/promote a retained candidate after validation. Automatic rollback selection is not implemented. Retired adapters are not deleted.

To use an approved adapter, configure that role's planner or worker slot with `provider: "mlx"` and its exact `base_model` path in `config/company.local.json`, then restart. The harness selects the active matching company/role adapter automatically. A GGUF slot continues using its configured GGUF until you explicitly change its provider/base. No promotion silently changes a role's base model.

Current role mapping: CEO -> `ceo`, CTO -> `cto`, PM Gent -> `pm`, Law Gent -> `law`, Lead Architect Gent -> `lead-architect`, Engineer Gent -> `backend-engineer`, QA -> `compliance-security-qa`. Other role IDs can be used directly as Gent IDs in an extended roster. Use separate training IDs/adapters per role, even when roles share one base.

## CEO, deadlines and all hands

The CEO uses the CTO's local planner model, initially Qwen3-8B. It sits above the CTO in the organization view. A server timer checks active work every 30 seconds. AI review is triggered by completed role stages, explicit missed deadlines, overdue milestones or five minutes without progress. It does not repeatedly run expensive inference while idle. At most one CEO review is pending per run.

The intake CLI accepts an optional ISO deadline with timezone. Without an explicit deadline, the CEO reports it as unknown. Track granular milestones with:

```sh
python3 scripts/gents.py milestone RUN_ID \
  --id first-draft --title "Verified first draft" --agent "PM Gent" \
  --due 2026-10-21T17:00:00-05:00 --state in_progress
```

Marking a milestone done requires evidence. The final job audit records whether a completed job met its explicit deadline. Automatic PM extraction of a complete typed milestone plan is still pending; the CLI supplies authoritative milestones for this stage.

On observed risk, the CEO may call an all-hands meeting. Automatic meetings have a 15-minute cooldown per run. The file lives at `data/companies/<company_id>/runs/<run_id>/meetings/<meeting_id>.md`. It restates the approved task, frozen acceptance criteria, zero budget, company isolation and the corrective brief. Each Gent acknowledges the latest brief before its next model call. Running inference is not interrupted mid-response. Meetings cannot change the SOW, sign contracts, authorize tools, bypass a judge or mark work complete.

You can also call one directly:

```sh
python3 scripts/gents.py all-hands RUN_ID "Restate the current milestone, identify missing evidence and assign the next concrete action."
```

The office shows a Training Room, a CEO character, live training metrics and the latest all-hands brief. Gents gather at the conference area for a new meeting. Handoffs now carry version, company ID, run ID, sender/recipient, phase and model identity to reduce ambiguity. These controls improve traceability; evaluate actual multi-step task performance before claiming they eliminate confusion.

## Verification

`sh scripts/check.sh` covers compilation, lifecycle checks, tenant boundaries, training-data review/split checks and meeting isolation. `npm run test:office` checks the running company office. Actual training output is in the company training run, with `manifest.json`, `training.log`, `progress.json`, `metrics.json` and `adapter/`.

The synthetic smoke proves local gradient updates and adapter loading. It does not promote a role-specialized production model. Next, curate one narrow role dataset from independently verified work, evaluate it against the incumbent and compare retries, quality and duration across projects.

Primary technical reference: [MLX LoRA and QLoRA documentation](https://github.com/ml-explore/mlx-lm/blob/main/mlx_lm/LORA.md).
