# Design note 012 — Changing what a creature looks like

Status: NORMATIVE procedure. Written after the type/version semantics
were settled on 2026-08-25, because the library had accepted an
obligation that nothing described how to discharge.

## The rule this exists to protect

**An agent's face does not change unless someone decided it should.**

Everything below follows from that one sentence. A stored row names its
`avatar_type` and `avatar_version`; the library renders at those, never
at its own newest. So changing rendered output is not something that
happens by editing code — it is something that happens when a row moves,
and a row moves only by consent.

## Which change is this? Three kinds, and only one is a version bump

| You want to | That is | Existing rows |
| --- | --- | --- |
| Fix a bug in a type's generation | version bump | keep their old face until upgraded |
| Offer a different depiction of the same genome | **new type** | untouched |
| Change the genome layout | **new type** (needs its own genome) | untouched |

The middle row is the one people get wrong. A redesign that makes
creatures look different is NOT a version bump, however much better it
is. It is a new type, and it reaches an agent when someone chooses it.
The wild-creature redesign in flight is `creature.wild`, not
`creature.classic` v3, for exactly this reason — and that is what
removed the flag day where every stored avatar would have changed
at once.

A version bump is for when the generation is WRONG: a spline that
self-crosses, an eye placed outside a silhouette, a mouth that renders
off the body. Something a user would call a defect rather than a style.

## Bumping a version

1. **Add an arm; do not edit one.** In `crates/avatar/src/dispatch.rs`
   the version match gets a new arm. The old arm stays, forever, because
   rows still name it. Deleting it silently changes the faces of every
   row that has not upgraded — the precise failure this note exists to
   prevent. Old generation paths are a permanent obligation; treat them
   like published API, not like dead code.
2. **Mint corpus vectors for the new version alongside the old.** The
   old version's vectors are not replaced. Byte-identity for version N
   must remain provable after version N+1 exists, or nothing enforces
   step 1.
3. **RENDER AND LOOK before committing the re-mint.** Non-negotiable,
   and the reason is that verification derived from a spec cannot catch
   a bug in that spec: the ports' byte-parity chains to whatever the
   canonical mint produced, so a wrong mint propagates to every language
   as agreement. `cargo run --bin grid -- --review` and
   `--icons` exist for this; look at the sheet, at icon size too.
4. **Port the new arm to Swift and TS.** Dispatch is per-language; a
   version that renders in Rust and errors in Swift is a row that draws
   on one surface and blanks on another.

   > Note (2026-10-26): Native Swift/TS builders were removed; new version arms are implemented once in Rust and consumed via render trees.
5. **Announce the upgrade path, not the change.** Nobody's face moves on
   deploy. Rows move when the person running the agents, or the agent
   itself, chooses, through the registry's controlled update path.

## Adding a type

1. Pick a name for its **character, not a number**: `creature.classic`,
   `creature.wild`. Numbered type names collide with the version axis
   and make a reader assume one of the two columns is wrong.
2. Declare its genome layout size. The registry that stores genomes
   enforces it fail-closed on write, so a wrong-length genome is refused
   rather than rendered as a partial creature.
3. Implement its dispatch arm in all three languages, plus corpus
   vectors. Unknown types must keep erroring cleanly on older clients —
   that path is what makes shipping a type safe while phones lag.

   > Note (2026-10-26): Native Swift/TS builders were removed; new types are implemented once in Rust and consumed via render trees.
4. If the layout differs from an existing type, existing agents need a
   **new genome** to wear it. That is a deliberate new genome written to
   the row, never an automatic migration.
5. Hand consumers a **fixture row** with the corpus tag: one avatar object
   shaped exactly as the wire delivers it (genome fingerprint, type,
   version), so a client can pin its wire decode and its render in the
   same change. Requested by the phone app, and the reason is worth
   keeping: without a fixture, the first row of a new type a consumer
   ever sees is a live one, which makes production the place where a
   decode mistake surfaces. The corpus proves renderers agree on bytes;
   it says nothing about a consumer's ability to parse the envelope
   those bytes arrive in.

## Requirements banked for the next phenotype schema

Not implemented, deliberately: adding these to the tree now would change
a contract two ports have already pinned, for no benefit while the look
is still under review. They land with the schema.

- **A `role` on each primitive**, a small closed enum, distinct from
  `label`. Label is presentational and the generic serializer ignores
  it; role is semantic and a consumer may act on it. The first case is
  `ground_shadow`: a creature drawn beside a floating element has no
  ground for its shadow to fall on, and that is a property of the
  PLACEMENT rather than of the creature, so the consumer decides and one
  tree still serves both. Requested by the phone app, which correctly
  refused to identify the shadow by primitive index — that coupling
  fails silently the day the pipeline emits something before it.
  The corpus must gate that exactly one primitive carries the role, so
  "skip the shadow" cannot quietly become "skip nothing".
- ~~**Ink-coverage normalization.**~~ DECIDED: LEAVE AS IS. The extent fit
  already scales every creature until one axis touches the wall, so
  bounding boxes are normalized and that part works — measured from
  pixels, every creature fills 0.854 to 0.997 of its largest axis. Ink
  coverage still spans 0.146 to 0.407, a 2.8x spread, because equal
  boxes are not equal visual weight: a spindly star and a solid blob
  both touch the wall and the star has a third of the ink. Offered as
  compress-toward-1.5x or equalize; neither was chosen. Density
  is a trait at full range, so a dense little imp and an airy sprawling
  thing stay as different in weight as their genomes make them.

  The consequence worth stating, because it binds consumers: NOTHING
  DOWNSTREAM MAY NORMALIZE VISUAL WEIGHT EITHER. A client-side crop to
  alpha bounds, a per-avatar scale nudge to make a row look even — each
  of those quietly re-decides this, in one surface, with nothing
  failing. The variance is chosen, not tolerated.

## What a client does with a version or type it does not know

Draws nothing. Not a default creature, not a previous version's
rendering, not a placeholder shaped like an avatar. A confidently wrong
face is worse than an empty slot precisely because it looks right:
nothing fails, and two surfaces disagree about who an agent is while
both appear healthy.

Unknown type or version is a NORMAL state — a client older than a row is
routine when surfaces update independently — and is not
worth reporting. A wrong-length genome is not normal: the registry's
length check should have refused it, so that one is a bug worth raising.

## Never

- Never fix a generation bug by editing an existing version's arm.
- Never re-run a genome seed manifest as a repair path: it would
  overwrite re-rolls and take the row owner's choice away silently.
  Seeding applies only where a row has no genome.
- Never key a cache or derived artifact by agent id. A re-roll changes
  the genome under a stable id, so id-keyed entries serve the previous
  creature forever with nothing failing.
