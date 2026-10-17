# 11 — Validation and limits

## Included checks

From the project root on your Mac:

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
cargo build --workspace --locked
python3 scripts/smoke.py
python3 scripts/reliability_smoke.py
node --check web/app.js
npm ci
npm run build
```

The smoke check launches isolated local harness instances and checks real HTTP requests. It verifies health/config, bad task rejection, the fixed-mode two-agent/six-call handoff and judge-mode thirteen-call repair lifecycle, monotonic event sequence, unknown mock token counts, cancellation and anonymous rejection in Entra mode. It uses no paid model calls or real tenant credentials.

Unit tests check bounded plan validation and usage normalization (including unknown fields and reasoning-token subsets). They do not replace live provider compatibility checks.

## Account/platform checks you perform

- OpenRouter/HF/Databricks: actual endpoint access, model ID, parameter compatibility, response usage and provider-dashboard reconciliation.
- Entra: interactive PKCE with MFA, correct audience/scope/client, wrong-tenant rejection and different-user run isolation.
- Azure: Bicep validation, deployment permissions, role propagation, ACR pull, Key Vault references, public HTTPS and restart behavior.
- macOS: native build, actual browser callback, Keychain/signing, downloaded app test and optional Mac App Store sandbox review.
- iOS: complete native MSAL bridge, physical-device sign-in, suspension/resume, accessibility, signing, TestFlight and review.

## Known starter limits

- Single-process SQLite journal, bounded to 20 retained runs; oldest terminal record is evicted when needed. Restart preserves history and marks active runs interrupted without replay.
- No worker queue, multi-replica coordination or guaranteed exactly-once side effects.
- Sequential dispatch; judge mode chooses teams and model slots from a server-side approved catalog; a restart is required to change that catalog.
- Text generation from supplied evidence; no file upload, web browsing, arbitrary tools, training, or retrieval implementation.
- No streaming text, token-by-token animation, or time-to-first-token measurement.
- Cancellation stops future calls; an active call can finish and be billed.
- Provider response errors hide raw bodies; inspect vendor diagnostics through an approved, redacted troubleshooting path.
- No guaranteed spend cap, model context-window admission or comprehensive cost reconciliation.
- macOS PKCE token held in memory, no silent renewal. iOS sign-in remains an explicit native integration exercise.
- Public config endpoint exposes only nonsecret agent/provider names. For enterprise deployments, consider protecting that metadata too.
- Development CSP permits general HTTPS destinations and editable backend addresses. Narrow both before release.
- Per-process user quotas and JWKS-refresh throttling are implemented. Public production use still needs edge limits and distributed coordination.

This package is a learning starting point with a concrete local demonstration and staged account-backed setup, not a production security certification or App Store approval guarantee.

## Checks performed when preparing this package

- Rust harness compiled on Linux; all eleven unit tests passed.
- Clippy passed for the harness with warnings treated as errors.
- Real local HTTP smoke checks passed for the mock lifecycle and anonymous rejection.
- Web JavaScript syntax check and Vite production build passed.
- Desktop PKCE core function compiled independently of the Tauri shell.
- Full native macOS/iOS builds and account-backed cloud/provider authentication were not run here.
- Automated browser rendering was unavailable because the browser runtime download failed; visual native-device QA remains required.

## Judge regression coverage

Tests cover approved model/role enforcement, dependency rejection, frozen criteria, full evidenced-check coverage, two-agent-to-one-agent repair, judge usage inclusion, call/round exhaustion remaining incomplete, and invalid judge decisions failing closed. The mock deliberately scripts rejection then acceptance; no real model judgment has been validated here.

## Final reliability coverage

Eleven unit tests cover the original contracts plus deterministic gates, approved provider URLs and stale-write/deletion protection. HTTP reliability checks verify duplicate/conflicting submissions, one active run per owner, refusal to delete active work, call timestamps/IDs, forced-crash recovery without replay, retained history/deletion, run deadlines and deterministic veto of an AI acceptance. npm advisory scanning reported no vulnerabilities during preparation; a separate Rust advisory scan has not been performed. Native platform and account-backed checks remain required.

A DOM-level UI check also exercised connect, duplicate-click prevention, metrics, saved-run reopen/delete and sign-out clearing against the real local API. Canvas rendering was stubbed; this is functional UI logic verification, not visual or native-device QA. Static-file allowlisting and no-store response headers were checked over HTTP.
