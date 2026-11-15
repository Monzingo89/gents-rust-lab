# Metivolve: first task and computer check

Public site: **https://metivolve.com**. `https://www.metivolve.com` redirects to the apex using HTTP 308. Both domains are attached to the existing `gents-office` Vercel project in VCV Vercel Team; registration, nameservers and mail DNS were preserved. The support origin allowlist includes the new HTTPS origin. No domain purchase or Git push was performed by this change.

## New-user journey

1. **Your goal:** describe one outcome in plain language.
2. **What done means:** specify the deliverable and checks required for acceptance.
3. **Your computer:** automatically read available browser hints; optionally open the local companion for native GPU/memory observations and save its minimal computer report.
4. **Next step:** review the brief and readiness limits, save the brief, then load it into a new local CEO conversation. Loading fills a draft; the user must still submit it and review the plan/permissions.

The homepage now leads to this guide rather than presenting a technical calculator first. Detailed time scenarios are expandable. Original project/backend naming stays Gents; Metivolve is the public website brand.

## Detection boundary

The public site cannot reliably discover free GPU memory, local model fit, installed runtime support or task duration. Browser CPU counts and memory hints can be restricted or rounded; WebGPU adapter information can be absent or privacy-limited. The website performs no GPU compute benchmark and never sends these hints to a server.

The existing local hardware port supplies actual supported OS observations in the private office. The new local panel reuses that owner/company-authenticated endpoint; no CORS, private-network access, authentication or tool permissions were relaxed. Opening the explicit loopback link is a separate user action. The website never fetches the local daemon.

The local panel compares the current effective memory limit with the maximum owner-configured planning-candidate memory floor. It never adds Apple shared GPU memory to RAM or calls a floor match verified model fit. Unknown profiles, invalid observations and reports older than five minutes stay unresolved. The imported report is user-supplied data, not independent attestation or execution authority.

Reports contain only GPU names/memory kind, host/effective/available memory, observation time and model memory floors. They omit credentials, serials, model paths, task data and actor memory. Optional files are read only inside the browser tab; downloads are explicit. The task brief contains outcome text and a non-authority scope statement, never tool grants or approval.

## Timing boundary

Automatic task-duration forecasting is **not complete**. A real estimate needs the agreed task, selected model, tool steps and measurements on this computer. The UI says **not yet measured** rather than inventing throughput from a GPU name. The existing time calculator still accepts explicitly supplied rates and workload; it is scenario arithmetic, not a completion guarantee. The next implementation step is a governed local calibration and task-workload estimate with provenance and uncertainty, followed by runtime admission/acceptance checks.

This scope is deliberate: the new journey and native detection are implemented and tested, while predictive timing is an explicit outstanding capability. No arbitrary-task readiness, regulated-use compliance, model download or task completion is inferred.

## Verification

- Focused readiness tests cover shared-memory accounting, below-floor setup, stale/invalid reports, export redaction and no authority from import.
- Static checks cover nine pages, links, canonical domain, structured metadata, CSP and packaging.
- The browser test exercised the actual local Apple M4 Max / 36 GiB host report and the existing 24 GiB profile floor, exported/imported that report, saved a synthetic task brief and loaded it into a new CEO draft. No inference/task POST occurred and native model-call counters stayed unchanged.
- Desktop/mobile checks cover all nine pages, no overflow, browser-only task handling and calculator scenarios. Routine checks send no support mail.

Run the optional actual companion path while the owner’s local office is running:

```sh
npm run site:preview
# Second terminal:
node scripts/introduction-browser-smoke.mjs --with-local-companion
```

Without the flag, the computer-report UI uses an explicitly synthetic fixture. Native hardware observations and simulated fixtures must remain distinguishable. The website remains an unindexed evaluation preview; source publication, the complete guided execution experience and calibrated automatic duration estimates remain open.
