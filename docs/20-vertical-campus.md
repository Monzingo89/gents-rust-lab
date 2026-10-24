# The Gents vertical campus

This is a visualization change, not a model replacement or a new execution mode.
The existing local laptop agents, permissions, admin review, mobile HTTPS gateway
and persistent task runtime are unchanged. Refresh the browser to load it.

## Floors

- 06: executive strategy, leadership, planning and all-hands.
- 05: knowledge/research and training laboratory.
- 04: architecture/production and quality/security.
- 03: systems/operations and admin review.
- 02: recharge pods 01–10.
- 01: recharge pods 11–20.

The interactive cutaway uses architectural glass partitions, graphite surfaces,
steel structure, a city backdrop and restrained cyan lighting. Floor numbers,
room titles, activity bubbles and actor nameplates occupy different vertical
bands. The opening view is a reflective, glass-windowed building exterior. Select
a window band or floor button to enter just that floor; Building exterior returns
outside. Desktop and mobile share the same floor navigation.

Once selected, a floor opens in an elevated oblique architectural view between
overhead and eye level. Ground surfaces, glass walls, furniture, walking feet and
click targets use the same projection; characters stay upright. Eight work rooms
have different palettes, materials and architectural features: walnut/marble
leadership, cognac/black-stone boardroom, oak/velvet library, platinum/violet
training lab, basalt/copper production atelier, pearl/sage QA suite, onyx/teal
control room and burgundy/bronze admin review. Both recharge floors intentionally
share identical pod styling. The roof/front is cut away for visibility.

Each Gent has a reserved station and pod, selected deterministically. There are
no crossing paths: large sprites walk within separate reserved lanes, using a
runtime leg rig on the existing artwork. Hover pauses a sprite for easier
selection; reduced-motion preferences disable walking. Inactive actors show their sprite head inside their
numbered recharge pod. Actors with an actual model/tool activity event appear at
their station; only active training jobs wake the training Gent. Resting is not
presented as proof of a dream/consolidation job. Thought bubbles show public
activity only, not private model reasoning. Preview walking is an optional,
explicitly labeled visual demonstration on work floors: it starts no task or
model, changes no activity counts or signal lights, and resets on floor changes.

## Identities

All eight newly supplied portraits are used, alongside the original twelve.
Two new transparent four-character atlases were generated with the built-in
image tool; originals remain unmodified. Files and exact prompts are documented
in [the sprite manifest](../web/assets/gents/SPRITES.md).

Every visible Gent, including planner/worker halves, receives a different face.
This is independent of shared inference weights. Twenty unique base positions
are supported (generic Engineer aliases Web Engineer). Conflicting saved choices
are reallocated deterministically; already-assigned faces are disabled in the
wardrobe. The UI reports a need for more art instead of cloning a twenty-first
simultaneous crew member. Old duplicate-prone choices are not migrated.

The new names are Luna, Coral, Ember, Sage, Onyx, Rowan, Boreal and Atlas. Atlas
leads as CEO; Coral handles project management; Ember handles architecture;
Boreal handles operations; Rowan handles training. Other new identities appear
in distinct planner roles. These are visual identities, not new model downloads.

## Navigation and verification

Select a floor for a close-up. Use Scene scale to zoom, and swipe/scroll across
the scene. Floor close-ups start at 200% on mobile, with horizontal panning inside
the scene rather than page overflow. Click/tap a Gent or pod to open its dossier. The keyboard-accessible
Crew manifest below the scene offers the same dossiers and shows each identity
and recharge assignment. Mobile task submission/questions/logs still use the
laptop backend; nothing is executed on the phone.

Checks: `node --test scripts/gent-avatars.test.mjs`, `npm run test:office`,
`npm run test:mobile`, `npm run test:mobile:browser`, and `npm run build`.
Browser job activity in the smoke tests is explicitly simulated and never
submits model tasks or emails to the live company. Geometry tests cover idle,
mixed and fully active crews, and actual browser hit boxes are checked for
overlap and unique identities.
