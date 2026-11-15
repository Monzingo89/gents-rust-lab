# Autonomous recovery

The existing frozen task and acceptance criteria survive restart. The local
Qwen3-Coder worker owns execution and recovery proposals. Each proposed tool
still passes independent authorization and broker validation.

- `recovery::DeliveryWatch` persists authored-file fingerprints and time since
  the last change. Downloaded references and hidden inspection files do not
  count as authored output. Twelve tool steps without a change trigger an
  executable recovery proposal. This is a stall signal, never proof of quality.
- Local executive calls receive the frozen task, actual workspace, bounded
  tool results and human answers. Advisory answers cannot become operator
  authority or reset progress. Repeated ineffective advice requests a concrete
  local action before the bounded OpenAI fallback is permitted.
- Preview questions require an authored artifact with a current inspection
  receipt. A real requested human preview review still pauses for the user.
- `pdf_render` consumes a JSON page specification authored by the local agent.
  It provides text/vector rendering without requiring shell programming or
  pdftk. Its receipt binds output and source hashes but never certifies content.
  The executive has planner skills/capabilities, including document-workflow;
  missing optional executables are operational discovery, not human authority.
- A requested first-step preview uses a typed local-author stage instead of the
  unrestricted planning loop. The strongest local worker must return either
  its own one-page text/vector specification or an exact read-only research
  action. Generated specifications become independently judged write, render,
  inspect and visual-review intents. The workflow then requests the original
  human checkpoint; it cannot approve its own preview or finish the mission.
- Every three hours the local reviewer checks for observed failures. Its actual
  suggested repair actions are carried into the next worker directive.
- The supervisor restarts unexpected runtime exits with backoff. On macOS,
  `node scripts/install-company-service.mjs` installs the login service so a
  terminal or chat ending does not stop the supervisor. macOS must be running
  and the user logged in; power loss cannot be recovered by application code.

Service status: `launchctl print gui/$(id -u)/com.gents.local-company`.
To stop and unload: `launchctl bootout gui/$(id -u)/com.gents.local-company`.
Logs are in `data/service/`; no credentials are written into the service plist.
