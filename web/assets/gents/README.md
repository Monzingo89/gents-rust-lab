# Company character artwork

The twelve JPEGs and eight new PNGs were supplied by the operator and copied without re-encoding.
The original files in Downloads are unchanged. No third-party license is inferred.

`web/gent-avatars.js` assigns artwork by **position**, not inference model. CEO,
CTO, PM, Law, architecture, engineering, QA, admin, operations and training retain
their own identities even when several positions use the same weights. The generic
Engineer runtime is the web-engineer position. All twenty base positions have
different faces, and all sixteen current live planner/worker/support Gents have
unique character identities. A deterministic crew allocator resolves conflicting
saved choices without cloning anyone. If more than twenty Gents are staffed,
additional characters are explicitly required instead of repeating a face.

The dossier wardrobe saves browser-local choices under
`gents:unique-crew:v2`, keyed by position and planner/worker/support phase. A roster
position opens its actual worker (or first support Gent). A choice changes just
that Gent; another Gent's assigned face is disabled in the wardrobe. Old model-wide
and duplicate-prone position choices are intentionally ignored. Selecting artwork
never changes models, execution, permissions or real activity signals.

The canvas draws transparent full-body sprites (see `SPRITES.md`), not portrait
squares. Original portraits appear in hover previews, dossiers and roster cards.
The skyscraper reserves separate work stations and recharge pods. Resting Gents
are drawn as head crops inside pods, not as standing characters with portrait
squares above them. Activity bubbles and ZZZZ remain tied to actual runtime events;
the artwork never simulates model execution.

New characters: Luna (moon), Coral (octopus), Ember (emerald dragon), Sage (cactus),
Onyx (raven), Rowan (stag), Boreal (yeti), Atlas (lion). See the exact generation
prompt set and sheet order in `SPRITES.md`.
