# Explicit local output budgets and cutoff recovery

Four earlier weighted-slot v2 worker replies ended with the backend's `finish_reason=length`. These are observed generation cutoffs, not evidence of RAM exhaustion or task completion. The previous runtime could retain partial final text, then report a generic proposal-JSON failure. This change gives output capacity and cutoff handling their own native generation contract.

An optional trusted model-profile field, `local_final_tokens`, overrides the call's default final-output allowance. It accepts integers from 1 through 4096 for native local inference only; other providers reject the setting. Missing values preserve the previous default. The separate reasoning allowance remains bounded. Their full sum plus the existing context margin is reserved before loading or inference, and the actual native tokenizer still controls prompt admission. A larger allowance cannot silently truncate mandatory frozen task context. It is not an automatic increase, an agent-controlled tool argument, a GPU-memory estimate or a promise that every model uses its reasoning allocation identically. MCP runtime profiles now also require owner review through the source-repair boundary, including case aliases; actors cannot rewrite these reviewed configuration files with a general source-patch grant.

On a reported length cutoff the native response parser discards all final content, even if it happens to be complete JSON. Private reasoning remains discarded. Actual usage, backend/process identity, input hash and selected generation budget remain in the receipt. The central model-call boundary records a failed call with a sanitized payload before context staging and returns a typed `GenerationCutoff`. No partial proposal can become a tool intent or memory source through this path.

The company worker handles this error through bounded recovery without counting it as a load error, stopping the model as an out-of-memory fix or invoking the smaller-model resource fallback. Actor/stage-scoped generation feedback reports the earlier cutoff separately from tool observations. Recovery still cannot change permissions, frozen criteria, source bytes or the trusted profile. Other malformed responses keep their existing validation path.

Benchmark accounting now reports observed `native_generation_cutoffs` and `failed_native_generation_cutoffs`. Historical cutoffs previously recorded as completed replies remain historical events; they are not rewritten. Usage aggregates include received receipts with actual usage, including discarded ones. Unknown usage or missing cutoff provenance is not estimated.

The actual reviewed Qwen3-4B probe passed using local installed weights SHA-256 `7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5`. Its normal structured response stopped with 159 observed output tokens under a 512-reasoning/128-final allocation. A second call requested a default 128 final tokens but explicitly selected a one-token native override: the backend reported `length`, exactly one output token and zero provider charges, while the exposed final text was empty. Both owned backends were stopped afterward. This proves the tested native budget/discard path, not business-task competence or the private reasoning-token subset. See `local-final-actual-native-probe.log`.

## Reviewed profile experiment

`company.mcp-rust-reasoning-wide-output.json` selects 2048 final tokens for the existing author, with other defaults preserved; its digest is `5c53c8e44d455636bbca53980bd2b08424482031aff00af2fb14a5cf8499a330`. A separate `company.mcp-rust-reasoning-specialist-review.json` profile additionally selects the installed coding model for the host QA call, also with an explicit 2048-final allowance; its digest is `d4c785e829e289286e3f53bfc1692c3b4a6af843bcaa88d030f476cae89dd40c`. QA and author use separate calls and roles but the same model weights; statistical error independence is not claimed.

The weighted-slot v2 run `5328eba7-feff-4570-80ae-6e161cbaabed` reached a native eight-test PASS at source `44f7e8c2279aa8257927cd265dd4022ca8fd1d785b93cf18230e78abc97e83bd`, while its former QA repeatedly claimed test failures. It was interrupted for the recorded runtime/profile experiment and continued in report `4a5e9f7b-ab36-459b-9bde-9b4b2b18d500`. The source, frozen acceptance, execution review, grants and all prior events were preserved. The profile and runtime changed explicitly. This is a maintenance-assisted, multiple-variable experiment, not an unassisted new task, controlled causal result or acceptance waiver. It remains active and unqualified at this checkpoint.

The first specialist review correctly described the native PASS and returned a model verdict of accepted, but its criterion indices were `[1,2,3,4,5,6,7,7]`. The host correctly rejected it because criterion 0 was missing and 7 duplicated. That response did not become task completion. The remaining protocol gap is that the host's revision feedback retains the model's positive summary without explicit missing/duplicate-index diagnostics. A future repair must make that protocol failure clear and obtain a fresh valid review, not reindex checks or waive an unreviewed criterion. Events 474/475 and `local-final-budget-verification-receipt.json` retain this observation.

## Owned continuation orchestration

The recorded specialist experiment subsequently completed independent admission. Its outcome and the follow-up review-protocol repair are recorded in [keyed completion review and v2 qualification](70-keyed-completion-review-and-v2-qualification.md).

`scripts/mcp-curriculum-resume.mjs` resumes one stopped failed/interrupted Rust report under the same task UUID. It checks bounded ordinary evidence files and all parent paths, the current fixture digest, approved native intake/policy and a local-only profile before changing the journal. It retains unrelated active leases, records the interrupted continuation as failed, then obtains one new executed continuation lease before launch. Its successful path reuses independent canonical reconciliation; an uncertain/live outcome retains its lease. It does not author a solution or change authority.

Example for a future stopped report, with IDs replaced by actual saved directories:

```sh
cargo build --workspace --locked --offline
node scripts/mcp-curriculum-resume.mjs \
  --report data/mcp-workflow-check/<prior-report-uuid> \
  --store data/mcp-workflow-check/<original-store-uuid>/runs.sqlite3 \
  --runtime-config config/company.mcp-rust-reasoning-specialist-review.json \
  --reason 'Describe the tested runtime or profile repair'
```

That terminal stays running through local execution and canonical admission. The task's stored source and tests remain the local company's work. Invalid/incomplete arguments and unsafe paths are tested to fail before journal mutation or inference. The current experiment exercised the real leased launch path; terminal admission still awaits actual completion.

Strict Clippy and the full generation-change repository check passed with 353 default Rust tests and 19 excluded opt-in probes. The native probe above was separately run and passed. Continuation/profile/accounting Node tests also passed. Evidence is under `data/mcp-basement-verification/local-final-*` and `weighted-slots-v2-final-budget-maintenance.json`. No model was downloaded, trained or promoted, and no paid inference/tool backend was enabled. Broader compliance, other modalities, hardware portability and verified learning remain part of the active goal.
