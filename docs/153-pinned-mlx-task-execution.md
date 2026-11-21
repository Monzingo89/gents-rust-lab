# Pinned MLX task execution

The task workflow and grounded-answer adapters now accept the existing owned `mlx` provider alongside `local` GGUF workers. They reuse `LocalRuntime` and the shared runtime pool. No new serving stack, model download, remote inference, weight conversion or training is introduced. Production profile admission still requires independent evaluations; a successful protocol trial does not qualify a model for general tasks.

`crates/harness/src/model_package.rs` hashes the bounded exact set of inference inputs: weights, configuration, tokenizer, template and other package files. The versioned manifest contains relative file names, sizes and SHA-256 values plus a canonical package digest. Documentation, `.git` and download `.cache` are explicitly outside this digest. Symlinks, hard links, special files, writable-by-other-user inputs, pickle/executable packages and custom-code mappings are rejected. The loader retains `trust_remote_code=false` and offline flags. These checks protect against accidental replacement and unsupported package layouts; they do not claim protection from a privileged process changing the runtime itself.

From the repository root, inspect existing packages without loading weights:

```sh
cargo build --workspace --locked --offline
target/debug/gents-harness --model-package-pin models/phi-1-mlx-4bit
target/debug/gents-harness --adapter-package-pin data/companies/<company>/training/runs/<existing-adapter>
```

Use the returned base `sha256` as `task_workflows.models.<id>.sha256` (or `task_answers.model_sha256`), with `model.provider="mlx"` and `model.path="models/<existing-package>"`. A configured `model.adapter` additionally requires `adapter_sha256`. The adapter must remain inside the current company's `training/runs/` directory. Base and adapter digests enter the resolved model identity and job/cache compatibility. Pins are checked at admission, before dispatch and before accepting output; an unchanged metadata fingerprint avoids repeatedly hashing gigabytes. Changed input metadata triggers full re-verification.

Workflow memory estimates must separately reserve base weights, adapters, KV, contexts, runtime and system headroom. The adapter rejects weight-file sizes above the declared weight/adapter reservations before hashing or inference. Disk bytes are a lower-bound artifact check, not a measured memory-fit calculation. MLX execution uses the existing exclusive runtime lane. Changing adapter or company session stops the owned process and reloads; no concurrent adapter-sharing or instant hot-swap claim is made.

The installed runtime used for compatibility is **MLX-LM 0.32.2**, MLX 0.32.4 and Transformers 5.20.1. Its local server has no tokenize endpoint and does not implement native JSON-schema response enforcement. For task calls, the host inserts the required output schema into the private inference packet, requests final output with thinking disabled through the model's chat-template option, and then applies the existing strict Rust deserialization, evidence/tool validation and global bounded-repair policy. This is prompt guidance plus host validation, not grammar-constrained decoding or a private reasoning trace.

Before loading task weights, `scripts/mlx_prompt_tokens.py` loads only the local tokenizer offline. It mirrors this installed server's text-chat template path, or its `convert_chat` fallback, and reports the actual prompt token count and finite pinned metadata context limit. Rust reserves generation plus 256 tokens and rejects overflow without dropping the objective or evidence. The default task cap is the smaller of metadata and 16,384; explicit `context_tokens` is bounded to 2,048–16,384 and cannot exceed metadata. Unknown runtime versions fail closed for this preflight. Existing non-task MLX callers without an explicit context override retain their previous behavior.

The helper bounds input/output and the entire subprocess exchange to 30 seconds, with cancellation and kill-on-drop. Actual server prompt usage must equal preflight before accepting a reply. `LocalExecution.prompt_preflight` and workflow model-return audit receipts retain this provenance; metadata capacity is never relabeled as backend-observed context. Context overflow reaches the coordinator as `context_pressure`, without wasting same-node schema repairs. MLX startup has one supported load attempt; native llama.cpp's GPU/CPU recovery is not claimed for MLX.

A bounded isolated protocol check uses an existing package, a synthetic source literal, one RAG generation, a frozen expected answer, and server/preflight token agreement:

```sh
python3 scripts/task_mlx_live.py --model models/gemma-4-E4B-it
```

The command hashes inputs, creates an isolated local harness/company, records its report under ignored `data/task-mlx-live/<uuid>/`, and archives the private company database after stopping its own harness. It spends no provider fees and makes no downloads. It is an explicit opt-in live check, separate from deterministic repository CI.

The 2026-11-19 Gemma trial `7f21aab5-2920-41d0-8cb1-8226e67ecc6c` passed its frozen literal/citation/protocol checks with package SHA-256 `bea43fdaa35b44cc0ef56cb550727657f135d30160b1e6f51050fbdcdd2fe9b1`: one generation, 1,007 input tokens (equal in preflight and actual server usage), 80 output tokens, 37,541,279 microseconds for the task, and process RSS 12,266,962,944 bytes. RSS is not a measurement of total Metal/unified-memory allocation. No adapter was loaded and no model was admitted into production by this test. Phi's tokenizer-only check reported 23 tokens, context metadata 2,048 and no chat template; that did not establish generative quality or serveability.

The first Gemma trial `a15cb1ae-89a2-4ada-a0a8-f5d77b8d7f57` stopped at capacity admission with zero model/tool calls. The trial had set its capacity to the exact byte estimate, although scheduler capacity rounds down to MiB and requests round up; it was one MiB short. The trial now pads its explicit configured capacity to a whole MiB. The conservative scheduler was preserved, and both reports remain available. This initial refusal is not evidence that this model exceeded observed machine capacity.

Unit coverage includes full-package invalidation, non-inference exclusions, unsupported code/link/permission rejection, company-bound adapter pins, adapter identity changes, missing pin rejection, metadata context bounds and output reservation. The wider task-engine completion gaps remain recorded in [`151-task-engine-completion-audit.md`](151-task-engine-completion-audit.md).
