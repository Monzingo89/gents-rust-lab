# 03 — Connect OpenRouter and Hugging Face

Complete the mock chapter first. No accounts or paid endpoints were provisioned by this package. Choose model IDs currently enabled for your accounts; the example intentionally avoids assuming a particular model remains available.

> **Judge-mode configuration:** the default now selects models from `judges.models`, not the roster's fixed `planner`/`worker` fields. Apply provider settings below to the appropriate catalog entries and configure `judges.router` and `judges.completion` too. Remove unused candidates, or the router can select them. For exact per-round model selection and the full schema, follow chapter 12. Remove the `judges` field to run the original fixed-slot exercises.

## 1. Prepare a local configuration

```bash
cp config/live.example.json config/local.json
```

Initially keep all but Atlas's planner set to mock by copying the model object from `config/mock.json`. Change one slot at a time so errors have a clear cause.

## 2. OpenRouter

1. Create an OpenRouter account, configure credits or billing as required, and create an API key at [OpenRouter settings](https://openrouter.ai/settings/keys).
2. Use [the models catalog](https://openrouter.ai/models) to select a text chat model with the parameters used here, including `max_tokens`.
3. Replace `REPLACE_WITH_CURRENT_MODEL_ID` with the exact model ID.
4. Keep the full URL `https://openrouter.ai/api/v1/chat/completions` and `key_env` equal to `OPENROUTER_API_KEY`.
5. Enter the key without recording it in shell history (macOS zsh):

```zsh
read -s 'OPENROUTER_API_KEY?OpenRouter key: '
export OPENROUTER_API_KEY
printf '\n'
CONFIG=config/local.json cargo run -p gents-harness --locked
```

6. Reconnect in the UI, read the displayed providers, check the consent box, and start a task with no confidential information.
7. Confirm the planner response is bare JSON matching `summary` and `steps`. If the model returns Markdown fences, the harness rejects the plan. Choose a more suitable model or add a structured-output capability adapter; do not silently treat malformed text as a plan.
8. Inspect usage fields. Confirm response totals in the provider dashboard. Failed calls can still be billed.

The adapter requests usage accounting. If cost is absent, it remains unknown. It is not estimated from a hardcoded price table.

## 3. Hugging Face

Hugging Face is both a model/dataset hub and an inference ecosystem. Downloading model weights does not expose a hosted chat API. This stage uses the hosted Inference Providers router.

1. Create a Hugging Face account. At [Access Tokens](https://huggingface.co/settings/tokens), create a fine-grained token with the inference-provider permissions required by the current HF documentation.
2. Configure billing or credits if needed. Accept any selected model's license/gating conditions.
3. Choose a supported chat model/provider from the [Inference Providers playground](https://huggingface.co/playground). Copy its exact supported model identifier. Hub availability alone does not guarantee inference availability.
4. In Atlas's worker configuration, use provider `huggingface`, URL `https://router.huggingface.co/v1/chat/completions`, and key environment variable `HF_TOKEN`.
5. In the server terminal:

```zsh
read -s 'HF_TOKEN?Hugging Face inference token: '
export HF_TOKEN
printf '\n'
CONFIG=config/local.json cargo run -p gents-harness --locked
```

6. Run the same vendor-comparison task. Verify the worker responds using supplied facts. HF usage/cost fields vary by serving provider; missing fields stay unknown.

For a dedicated HF Inference Endpoint, deploy a compatible serving engine and use its documented chat-completion URL. Do not assume the hub download URL or arbitrary generation endpoint accepts this schema.

## 4. Capability boundaries

These adapters handle non-streamed text chat completions. Models that only support Responses, different token limit names, tools, images, or proprietary schemas need a different implementation behind `LanguageModel`. A recommended next change is a typed `Capabilities` struct (JSON schema, tools, streaming, max input, reasoning policy) validated before a run.

**Checkpoint:** Atlas's planner calls OpenRouter and his worker calls HF; Vera remains mock. You can see the two provider names, inspect real outputs and distinguish reported tokens from unknown usage.
