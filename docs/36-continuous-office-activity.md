# Continuous local office activity

## Run and stop

Build with `cargo build -p gents-harness --locked`, then run
`npm run company:activity`. Keep that terminal running. Open
<http://127.0.0.1:8788/?autoconnect=1&tour=1>. Ctrl-C stops the listener and
research worker. The supervisor restarts an unexpectedly stopped listener;
macOS `caffeinate` prevents idle system sleep while that listener lives.
This is not a boot service or a guarantee against power loss/forced shutdown.

The listener binds loopback, uses the isolated `data/office-view.sqlite3`
journal and disables the normal company scheduler. Existing real task history
and research campaigns are not restarted or rewritten. No credentials,
downloads, external email, cloud inference or paid API fallback are used.

## Presentation and measured work are different

Office activity is staged choreography: seventeen distinct Gents in assigned
rooms, five working poses, ten seated poses (including CEO and CFO) and two
sleeping Gents in identifiable nap pods. The lobby receptionist and top-suite
character remain NPC artwork. Staging never changes actual task/busy state.
Gent hovers describe both the displayed activity and that staging boundary.

Eleven floor elevations are 460 units tall (lobby 480, top suite 540), versus
the former compressed 124–184-unit bands. Preview images preserve aspect;
the exterior scrolls rather than squeezing the whole tower into one viewport.
A glass elevator shaft traverses the building; interior floors have brushed
metal lift doors and a floor indicator. This is visual lift motion, not a
claim that tasks execute while sprites ride it. Reduced motion is supported.

Each room has live, hoverable screen surfaces and keyboard/touch-accessible
dashboard buttons. Operations has eleven dashboards across its video wall and
desk screens: calls, errors, budget, GPU, handoffs, latency, tokens, GPU memory,
in-flight calls, research state and active task status. CFO has one high-level provider-budget
chart. CEO has company overview, current task count, research status, errors,
GPU and notebook-review charts. Screen values come from
`GET /api/office/activity`, never animation clocks or random numbers.

## Continuous low-duty-cycle learning

The always-running observer samples actual local journal counters and macOS
IOAccelerator metrics every five seconds. GPU usage/memory is host-wide,
including other apps, not per-agent attribution. Unsupported sensors return
null/unavailable. Charts retain 120 recent samples; browser polling and
rendering incur no model tokens. Hidden browser pages stop drawing/polling.

With `GENTS_OFFICE_ACTIVITY=1`, installed Qwen3.5-4B reviews compact numeric
evidence and its previous advisory. Attempts are at least three hours apart
and require changed journal evidence; GPU animation changes alone do not
trigger reviews. Each attempt permits at most 192 output tokens, a 4096-token
context and a two-minute timeout. The attempt timestamp is saved before the
call, so restarting cannot reset this budget. Active company work/training
cancels or defers research. The owned model session is released afterward.

The private `data/office-activity/notebook.json` retains cumulative local
token/call/failure counters and at most 48 advisory records with native-call
receipts. Directory/file permissions are 0700/0600 and replacement is atomic;
this is trusted-host local storage, not regulated encrypted storage. Notes
are untrusted data in subsequent prompts. The researcher has no tools,
network authority or ability to execute patches. Learning here means a
persistent evidence-grounded advisory notebook, not weight training or
proven automatic improvement. Hypotheses require independent experiments
before adoption; a plausible recommendation is not a verified optimization.

Actual inference cannot use literally zero tokens. The first retained native
review used 167 input and 53 output tokens, with zero provider charges.
Hardware/electricity and this implementation session are not free inference
claims. Unchanged evidence reuses the notebook indefinitely without calls.

## Verification

- `cargo test --workspace --locked`: 173 passed, 17 opt-in native tests ignored.
- Strict Clippy and the production web build passed.
- `node --test scripts/gent-avatars.test.mjs`: 20 passed.
- `node --test scripts/office-activity.test.mjs`: occupancy, unique identities,
  non-overlap, floor proportions, lift motion and measured/unknown metrics.
- `npm run test:activity:browser`: isolated headless regression checks all
  rooms, all seventeen Gent hovers, actual screen hover, live refresh without extra inference,
  local-only browser requests and no page errors. Evidence and screenshots:
  `data/office-activity/verification/`.
- Actual native researcher receipt is retained in the private notebook;
  the local API reports its measured tokens and current research window.
- Untrusted-origin requests to the new endpoint return HTTP 403.
- The supervised listener was deliberately stopped after confirming no active
  run; it restarted with the same notebook, one native review and the original
  three-hour budget timestamp, without additional inference. A corrupt-notebook
  fixture disables only research and preserves the file, not the whole office.

The expanded hover check initially timed out; the fixture now waits for the
canvas to settle after floor navigation. Foreground Gent hit targets also
take priority over a screen behind them, so the CEO remains inspectable even
where its head overlaps the video wall. The final browser result verifies all
seventeen individual hovers and 31 screen targets, with no page errors or
external browser requests. Screen refreshes and two supervised restarts kept
the actual native-review count at one (220 local tokens, USD 0 provider cost).

The host-browser accessibility view confirms the office is open with all
seventeen Gents. Its native screenshot/extension capture was unavailable;
visual verification used screenshots produced by the dedicated app regression
fixture instead. Existing papers, failed trials and production refusals are
unchanged.
