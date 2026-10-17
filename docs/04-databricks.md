# 04 — Azure Databricks and Hugging Face model artifacts

Databricks has two separate roles in the final architecture: (1) serve a model and (2) retrieve governed data or train/evaluate models. The supplied live adapter implements only the first. Follow the first path before adding retrieval.

> **Judge-mode configuration:** the default now selects models from `judges.models`, not the roster's fixed `planner`/`worker` fields. Apply provider settings below to the appropriate catalog entries and configure `judges.router` and `judges.completion` too. Remove unused candidates, or the router can select them. For exact per-round model selection and the full schema, follow chapter 12. Remove the `judges` field to run the original fixed-slot exercises.

## 1. Create or obtain a workspace

1. In Azure Portal, select your development subscription and create a dedicated resource group for this lab if you do not have one.
2. Create **Azure Databricks** in an available region. Use the tier/network settings approved for your account; availability, serverless support and billing differ by region and subscription. Do not start with GPU clusters.
3. Open the workspace. Confirm you can access **Serving** and have permission to query a serving endpoint. If using a work tenant, request that access from the owner.
4. Open an available chat Foundation Model API endpoint or create an approved serving endpoint. Copy its exact endpoint name from Serving. Check the endpoint's query example and supported API format.
5. Use the OpenAI-compatible chat-completion path shown by that endpoint. This example uses `https://<workspace-host>/serving-endpoints/chat/completions` with `model` equal to the endpoint name. If your endpoint documents only `/serving-endpoints/<name>/invocations` or a different payload, implement that adapter instead of guessing.

## 2. Obtain an appropriately scoped credential

For an interactive development check, use the workspace's documented OAuth user-to-machine flow or an allowed short-lived development token. The starter reads a ready-to-use bearer token from `DATABRICKS_TOKEN`; it does not implement OAuth renewal.

For a hosted unattended backend, create a Databricks service principal, assign it to the workspace and grant **CAN QUERY** only on the intended endpoint. Follow the OAuth M2M setup linked in chapter 10. A Databricks OAuth client ID and secret are not an Entra API token.

Once the service principal is configured, the documented workspace token exchange is conceptually:

```bash
curl --fail --silent --show-error \
  --user "$DATABRICKS_CLIENT_ID:$DATABRICKS_CLIENT_SECRET" \
  --data 'grant_type=client_credentials&scope=all-apis' \
  "https://YOUR_WORKSPACE_HOST/oidc/v1/token"
```

This command returns a secret-bearing token response. Do not paste it in chat, commit it, or retain it in terminal logs. A production credential provider must cache tokens server-side, respect `expires_in`, refresh before expiry and avoid logging responses. This token lifecycle is an extension exercise, not implemented by the current environment-variable adapter.

## 3. Configure Vera's worker

In `config/local.json`, set:

- `provider`: `databricks`
- `model`: the actual serving endpoint name
- `url`: the full compatible chat-completion URL from your workspace
- `key_env`: `DATABRICKS_TOKEN`

In your macOS zsh terminal, read the token without shell history:

```zsh
read -s 'DATABRICKS_TOKEN?Databricks access token: '
export DATABRICKS_TOKEN
printf '\n'
CONFIG=config/local.json cargo run -p gents-harness --locked
```

Run the task, inspect Vera's response and compare usage against Databricks endpoint monitoring. An expired token should produce a visible HTTP error. Refresh it using your supported identity flow, restart the server and run again. Do not add indiscriminate retries to hide credential failures.

**Checkpoint:** one run uses OpenRouter, HF and Databricks in different slots. This is a multi-provider inference demonstration, not training a new model.

## 4. Optional: bring a Hugging Face model into Databricks

Only attempt this after hosted inference works:

1. Select a model whose license permits your intended deployment. Record its repository ID, immutable revision, model card, context limits, dependencies and expected hardware.
2. In a Unity Catalog-enabled workspace, create a dedicated catalog/schema and storage location with least-privilege access.
3. On suitable supported Databricks ML compute, load the pinned model/tokenizer through the model's documented Transformers or serving integration. Keep gated-model tokens in the workspace's approved secret mechanism.
4. Evaluate the base model against a small fixed prompt set before fine-tuning. Record results and inference cost.
5. Package it using a Databricks-supported MLflow model flavor or custom serving wrapper. Register the model in Unity Catalog and create a serving endpoint compatible with your model's hardware and dependencies. GPU availability and scale-to-zero support are workload-specific.
6. Test its endpoint with the documented payload, then add/adjust the Rust adapter. The model registry name is not automatically a valid chat API name.
7. If fine-tuning is justified, version the dataset, train an adapter or model on approved compute, evaluate, register a new version and promote only after it passes your acceptance set.

These are later model-lifecycle exercises. The harness calls models; it neither needs nor automatically performs fine-tuning. Reproducible notebooks/training jobs are not supplied in this starter.

## 5. Optional: add RAG rather than model weights

A separate data adapter can query Databricks SQL, Vector Search or governed functions. Define a Rust `Retriever` port returning passages with source IDs, document ACLs and timestamps. Retrieve only data the signed-in user may access, attach citations to worker input, and record retrieval events. Give a sprite a Library state when an actual retrieval occurs; the current Library state marks a plan handoff only.

Do not assume that a shared service principal enforces each user's document permissions. Implement explicit ACL filtering or an approved user-delegation pattern and verify denied-user cases. The LLM is never the authorization boundary.
