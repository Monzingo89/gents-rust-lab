# 06 — Deploy the Rust backend to Azure

This deploys a small authenticated learning environment. Run it only after local Entra validation succeeds. Your account needs resource creation, ACR build and role-assignment permissions. Billing begins when you provision billable services. Deploy mock inference first, then attach provider credentials.

## 1. Sign in and set the development context

```bash
az login
az account set --subscription 'YOUR_SUBSCRIPTION_ID'
export GENTS_RG='rg-gents-dev'
export GENTS_LOCATION='eastus'
export GENTS_PREFIX='gentslab'
az group create --name "$GENTS_RG" --location "$GENTS_LOCATION"
az extension add --name containerapp --upgrade
az provider register --namespace Microsoft.App
az provider register --namespace Microsoft.OperationalInsights
az bicep install
```

Use an available region and a short lowercase alphanumeric prefix (3..12 characters). Provider registration may take time. In Azure Cost Management, create a budget and alerts for this lab resource group. A budget does not prevent charges.

## 2. Deploy the foundation

```bash
az deployment group create --resource-group "$GENTS_RG" \
  --name gents-foundation --template-file infra/main.bicep \
  --parameters prefix="$GENTS_PREFIX"
export GENTS_ACR=$(az deployment group show -g "$GENTS_RG" -n gents-foundation --query properties.outputs.registryName.value -o tsv)
export GENTS_KV=$(az deployment group show -g "$GENTS_RG" -n gents-foundation --query properties.outputs.vaultName.value -o tsv)
export GENTS_IDENTITY=$(az deployment group show -g "$GENTS_RG" -n gents-foundation --query properties.outputs.identityId.value -o tsv)
export GENTS_LOGS=$(az deployment group show -g "$GENTS_RG" -n gents-foundation --query properties.outputs.logsName.value -o tsv)
```

This creates a user-assigned identity, RBAC-enabled Key Vault, ACR and Log Analytics. It grants the app identity Key Vault Secrets User at the vault scope. It does not grant your human user permission to write secrets; obtain Key Vault Secrets Officer at that vault if needed. Identity/RBAC propagation can take time.

The initial vault allows public network access with identity-based permissions. Private networking is a later stage in chapter 08.

## 3. Build a Linux image in ACR

The container build runs remotely, so your Mac's ARM architecture is not a blocker:

```bash
az acr build --registry "$GENTS_ACR" --image gents-harness:0.1.0 \
  --file infra/Dockerfile .
export GENTS_ACR_ID=$(az acr show -n "$GENTS_ACR" --query id -o tsv)
export GENTS_LOGIN_SERVER=$(az acr show -n "$GENTS_ACR" --query loginServer -o tsv)
export GENTS_PRINCIPAL=$(az identity show --ids "$GENTS_IDENTITY" --query principalId -o tsv)
az role assignment create --assignee-object-id "$GENTS_PRINCIPAL" \
  --assignee-principal-type ServicePrincipal --role AcrPull --scope "$GENTS_ACR_ID"
```

Use a distinct immutable tag for each later image. Config files in `config/` are copied into the image, including `config/local.json` if present. Keep secrets strictly in environment-backed Key Vault references; inspect build context before deploying. `.dockerignore` excludes `.env` files.

## 4. Create the Container Apps environment

```bash
export GENTS_LOGS_ID=$(az monitor log-analytics workspace show -g "$GENTS_RG" -n "$GENTS_LOGS" --query customerId -o tsv)
export GENTS_LOGS_KEY=$(az monitor log-analytics workspace get-shared-keys -g "$GENTS_RG" -n "$GENTS_LOGS" --query primarySharedKey -o tsv)
az containerapp env create --name gents-env --resource-group "$GENTS_RG" \
  --location "$GENTS_LOCATION" --logs-workspace-id "$GENTS_LOGS_ID" \
  --logs-workspace-key "$GENTS_LOGS_KEY"
unset GENTS_LOGS_KEY
```

Do not echo shared keys or enable shell command tracing.

## 5. Create an authenticated single-replica backend

Keep the three Entra IDs from chapter 05 exported in this terminal. Mock inference is intentional here:

```bash
az containerapp create --name gents-api --resource-group "$GENTS_RG" \
  --environment gents-env --image "$GENTS_LOGIN_SERVER/gents-harness:0.1.0" \
  --user-assigned "$GENTS_IDENTITY" \
  --registry-server "$GENTS_LOGIN_SERVER" --registry-identity "$GENTS_IDENTITY" \
  --ingress external --target-port 8787 --transport http \
  --min-replicas 1 --max-replicas 1 --cpu 0.5 --memory 1Gi \
  --env-vars AUTH_MODE=entra BIND=0.0.0.0:8787 CONFIG=config/mock.json \
  TENANT_ID="$TENANT_ID" API_CLIENT_ID="$API_CLIENT_ID" NATIVE_CLIENT_ID="$NATIVE_CLIENT_ID"
export GENTS_FQDN=$(az containerapp show -g "$GENTS_RG" -n gents-api --query properties.configuration.ingress.fqdn -o tsv)
curl --fail "https://$GENTS_FQDN/health"
```

Ingress supplies public HTTPS; HTTP is internal between ingress and container. Use only the HTTPS address in the client. Check the revision's readiness and console logs if the health check fails. Configure HTTP readiness/liveness probes on `/health` in the app's container settings for later releases.

Keep a single active revision in single-revision mode. The final starter checkpoints runs to SQLite. The default container writable layer is ephemeral: mount durable storage and set RUN_STORE before relying on recovery across replacement. Chapter 13 explains this distinction. Multiple replicas would return inconsistent snapshots; do not scale this version out. The minimum replica also incurs idle resource cost.

## 6. Connect the desktop

In the native office app, enter `https://<GENTS_FQDN>` as the harness address. Sign in with the IDs from chapter 05, connect, and run mock inference. `/api/runs` without a token must return 401 over public HTTPS too.

The server's default CORS origins allow Tauri macOS and the local dev web origin. If using a hosted browser frontend or an iOS WKWebView with a different origin, inspect its actual Origin and set an explicit comma-separated `CORS_ORIGINS` list. Do not replace it with a wildcard to solve authentication failures.

## 7. Attach provider secrets

In Key Vault, create secrets named `openrouter`, `hf-token` and `databricks-token` using the portal's secret editor. Then attach references:

```bash
export GENTS_VAULT_URI=$(az keyvault show -n "$GENTS_KV" --query properties.vaultUri -o tsv)
az containerapp secret set -g "$GENTS_RG" -n gents-api --secrets \
  "openrouter=keyvaultref:${GENTS_VAULT_URI}secrets/openrouter,identityref:$GENTS_IDENTITY" \
  "hf-token=keyvaultref:${GENTS_VAULT_URI}secrets/hf-token,identityref:$GENTS_IDENTITY" \
  "db-token=keyvaultref:${GENTS_VAULT_URI}secrets/databricks-token,identityref:$GENTS_IDENTITY"
```

Configure `config/local.json` with real endpoint/model IDs, rebuild a new image tag (for example `0.1.1`) using step 3's build command, then:

```bash
az containerapp update -g "$GENTS_RG" -n gents-api \
  --image "$GENTS_LOGIN_SERVER/gents-harness:0.1.1" \
  --set-env-vars CONFIG=config/local.json \
  OPENROUTER_API_KEY=secretref:openrouter HF_TOKEN=secretref:hf-token \
  DATABRICKS_TOKEN=secretref:db-token
```

For a long-running service, replace the Databricks access-token secret with a credential provider implementing M2M renewal (chapter 04). A short-lived access token in Key Vault is only a development checkpoint and eventually expires.

## 8. Operate the lab

Watch revision logs, errors and provider dashboards. The application does not log task text to stdout, but run outputs are checkpointed in the SQLite journal and can be exported by the owner. Decide retention and access policies before persisting them. Configure/test durable storage before relying on recovery, and add a shared repository/queue before scaling out.

For cleanup, use Azure Portal to review this resource group and delete it when done. Delete any separate Databricks workspace/serving resources you created too; they may live outside this group. Delete only resources created for this lab. Vault soft-delete and retained resources may require additional cleanup according to their policies.

**Checkpoint:** your Mac app calls the authenticated Rust API over HTTPS; the API retrieves provider secrets through its Azure identity. No provider key is present in the app bundle.
