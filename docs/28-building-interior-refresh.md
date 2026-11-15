# Realistic building interiors

All eleven floors now use distinct high-detail architectural backdrops, with
the existing live roster, movement, hit targets, hover/dossier UI and escalation
phone rendered separately. The images are decorative: people and fabricated
telemetry are not baked into them. Blank wall displays do not represent missing
data as successful operations. Existing actual observability remains in the
CEO trace panel.

## Assets and generation

The built-in image-generation tool produced one backdrop for each room, saved
as `web/assets/rooms/floor-1.png` through `floor-11.png`. The full prompt set,
generation method and per-floor filenames are recorded in
`web/assets/rooms/manifest.json`. Original generated files were retained.

The separately generated equipment asset is
`web/assets/gents/equipment-realistic.png`, used by the code-native fallback.
Its prompt requested a transparent architectural equipment sprite with a
brass/burgundy glass popcorn cabinet, visible kettle and popcorn, next to a
realistic twin transparent-bowl red/blue slush dispenser with augers, black
handles, stainless base and drip tray. It specified photorealistic materials,
front three-quarter view, aligned ground, complete machines, no people, text,
logos or background. The full prompt appears in the image-generation receipt
for this turn; no CLI/API-key fallback was used.

Native canvas material, furniture and equipment rendering remains available
when a room image cannot load. Floor surfaces use deterministic textures.
Each floor keeps only its latest cached background, and re-renders when its
image becomes available. Actual actor state never changes merely to animate
the office.

## Occupants and layout

Removed the fixed hand-drawn executive mannequins. The CEO comes from the real
roster renderer, with the existing walking animation and hover pause; no second
figure is painted over its desk. Image-space foreground lanes avoid reusing the
old desk coordinates. The conference occupants use aisle positions, the lobby
receptionist is clipped behind its counter, and the gold-suite avatar stands
beside the equipment rather than on a chair. These are authored 2D compositing
lanes, not a full 3D navigation/occlusion engine.

## Verification and running office

Twenty visual/layout/asset tests pass, including all eleven local PNGs and
prompts, the single moving CEO and separated foreground lanes. The Vite
production build passes and includes the special executive/receptionist assets
using statically analyzable URLs. `git diff --check` passes.

Chrome was refreshed after asset imports. The CEO office, gold suite and its
recognizable machines, laboratory, and lobby/receptionist were inspected in the
browser. All eleven source renders were visually inspected. The local asset
routes return HTTP 200. No browser-console inspection is claimed.

The office is open at
`http://127.0.0.1:8788/?autoconnect=1&tour=1`, with sixteen visible Gents.
Walking mode is visualization, not inference/task execution. The separately
started runtime uses `RUN_STORE=data/office-view.sqlite3` and
`GENTS_DISABLE_SCHEDULER=1`; the run API remains empty. Existing task journals,
company data and the Vite server were not cleared. The usual scheduler default
is unchanged for normal company startup.

The independent synthetic banking results and architecture work are documented
in `docs/24-local-agent-campaign.md` and `docs/27-local-agent-assurance.md`.
They are separate from the visual room refresh and do not establish production
regulatory certification.
