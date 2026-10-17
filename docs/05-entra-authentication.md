# 05 — Azure authentication and authorization

Use a development Entra tenant you control. This starter uses one workforce tenant and work/school accounts. For a public consumer app, design an appropriate customer identity flow (for example Entra External ID) and change validation accordingly. A single-tenant internal login does not make an app publicly accessible.

## 1. Register the protected API

In Microsoft Entra admin center:

1. Go to **App registrations → New registration**.
2. Name it `Gents Harness API`; select accounts in this organizational directory only. No redirect URI is needed for the API.
3. Record its **Application (client) ID** as `API_CLIENT_ID` and the **Directory (tenant) ID** as `TENANT_ID`.
4. Under **Expose an API**, set Application ID URI to `api://<API_CLIENT_ID>`.
5. Add a delegated scope named `Harness.Run`. Provide clear admin/user consent labels; enable the scope. Choose the consent policy appropriate to your tenant.
6. In the manifest, set the API's access token version to 2 (`api.requestedAccessTokenVersion`: `2` in the current Microsoft Graph-format manifest). Save. The Rust validator intentionally expects v2 issuer and audience conventions.

Do not create an API client secret for the desktop app. The API verifies access tokens using Microsoft's signing keys.

## 2. Register the native client

1. Create another single-tenant registration named `Gents Office Native`.
2. Record its client ID as `NATIVE_CLIENT_ID`.
3. Under **Authentication**, add **Mobile and desktop applications** and register `http://localhost/callback`. The desktop sample uses that path with an ephemeral loopback port. Microsoft's localhost redirect matching ignores the port; the path must match.
4. Under **API permissions → Add a permission → My APIs**, select the Harness API and its delegated `Harness.Run` scope.
5. Grant admin consent if required by your tenant. Configure authorized client applications on the API if you want to preauthorize this native client.
6. Do not enable implicit grants or put a client secret in a native app. This sample uses authorization code + PKCE in the system browser, with a random verifier and state.
7. If your tenant requires explicit enterprise-app user assignment, assign your test user. Entra authentication policies still apply.

## 3. Start the protected backend

Stop the local server. In its terminal, substitute actual IDs:

```bash
export TENANT_ID='YOUR_TENANT_UUID'
export API_CLIENT_ID='YOUR_API_APPLICATION_UUID'
export NATIVE_CLIENT_ID='YOUR_NATIVE_APPLICATION_UUID'
AUTH_MODE=entra CONFIG=config/mock.json cargo run -p gents-harness --locked
```

Authentication mode fails closed for invalid settings. `AUTH_MODE=local` is permitted only on a loopback listener. Never expose a local-auth process using a public tunnel or reverse proxy: loopback binding cannot prevent you from forwarding traffic to it.

## 4. Sign in from the desktop app

Run `npm run tauri dev` in the second terminal. Expand **Microsoft Entra sign-in**. Enter the tenant, native client and API client IDs. Click **Sign in**. Finish MFA in the system browser. Return to the app and connect to the protected API.

The native Rust command creates the PKCE request, validates callback state and exchanges the code. The app retains only the access token in process memory. There is no persistent cache, refresh token flow, or automatic silent renewal in this teaching implementation. After expiry, sign in again. **Clear sign-in** clears app memory; it does not revoke Microsoft tokens or end the system browser's Microsoft session.

Desktop PKCE uses a short-lived local HTTP callback listener. Before Mac App Store release, use the native MSAL integration and registered custom redirect scheme (chapter 09), or fully validate and entitle the loopback design under sandbox restrictions.

## 5. Understand what Rust validates

`auth.rs` checks the JWT signature against tenant JWKS, restricts RS256, validates expiry/not-before, v2 issuer, API audience, tenant, authorized native client (`azp`) and delegated `Harness.Run` scope. The run owner is `(tid, oid)`. Snapshot/cancellation endpoints enforce that ownership.

The ID token is for the client; the access token for **this API** is what the backend accepts. A Graph token, an Azure management token, or a Databricks token has a different audience and must be rejected.

The starter uses delegated scope authorization. It does not implement app roles, group policies, customer tenants or per-tool permissions. Add those deliberately when needed.

## 6. Verify the boundary

Without an access token:

```bash
curl -i -X POST http://127.0.0.1:8787/api/runs \
  -H 'Content-Type: application/json' \
  -d '{"task":"test","consent":false}'
```

Expect 401. With a valid token, expect a run ID. With the wrong audience, tenant, client or scope, expect 401. With a valid token from a different user, requesting another user's run returns 404. Use a second test user to confirm that case; do not merely inspect the code.

`/health`, static UI assets and nonsecret agent/provider names in `/api/config` are public. Access tokens never go in URLs. CORS is a browser policy; authorization is enforced independently on the API.

**Checkpoint:** signed-in desktop runs work; anonymous run creation fails; wrong-audience and different-user checks are verified in your tenant.
