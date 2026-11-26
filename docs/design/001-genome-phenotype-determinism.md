# Design note 001 — Genome, phenotype, and the determinism contract

Status: accepted. Written before any code, this note settled the
architecture of the canonical Rust implementation. Where later notes
changed it, the section says so and names the note in force.

Inputs: a source pass over TOG, the author's earlier procedural SVG
creature project that this one descends from (`type1.ts`, `spline.ts`,
`math.ts`, `colors.ts`, `face.tsx`, `pattern.tsx`, `bodylayer.tsx`,
`index.tsx`); and an existing golden-vector pattern from a protocol
library, where a Rust-minted corpus is checked by TypeScript and Swift
conformance tests.

## 1. Pipeline

```
DNA (bits)  --decode-->  Phenotype (typed integers)  --render-->  SVG bytes
                              |                                      |
                              +--> golden boundary 1                 +--> golden boundary 2
```

Three strict layers:

- **Genome**: a fixed-size bit string with a versioned region map.
  Decoding never knows about SVG.
- **Phenotype**: the typed parameter set (spline points, palette roles,
  eye geometry, pattern selection, animation params). All values are
  integers in defined units. Mood is NOT here — it is a render-time input.
- **Renderer**: consumes (phenotype, mood, canvas) and emits SVG bytes.
  Never reads bits.

The phenotype is the contract between ports. 3D renderers would later
consume the same phenotype (3D is deferred, but the layering is what keeps
it one genome).

## 2. The determinism contract (numeric discipline)

This is the highest-stakes section. TOG's math is float-heavy
(`Math.sqrt`, `**`, `cos/sin`, string-split rounding). A faithful float
port cannot be byte-identical across Rust/Swift/TS/platforms. The port is
therefore **visually faithful, arithmetically re-specified**:

1. **Integer-only end to end.** Every phenotype field and every
   intermediate geometric quantity is an `i64` in **canvas milliunits**
   (see §2.1) or a `Q32.32` fixed-point value where fractional precision
   is needed (tensions, ratios, blend factors). Intermediates widen to
   `i128` before multiply; narrowing uses one pinned rounding rule.
2. **One rounding rule.** All divisions and Q-format narrowing round
   **half away from zero** (symmetric, matches human expectation, cheap
   in all three languages). No other rounding appears anywhere.
   This rule explicitly covers **negative values**: sign handling is
   where the three languages' integer division genuinely differs (Rust
   and Swift truncate toward zero, JS `|0` semantics differ again), so
   ports must not lean on native `/` for signed values — the shared
   helper is the only division primitive, and the golden corpus carries
   a negative-value vector at both boundaries to falsify each port's
   sign handling.
3. **No platform libm.** `sin`/`cos` (needed by fetus blending and the
   circler pattern) come from a **pinned quarter-wave lookup table**
   (4096 entries, Q1.30, minted once from a high-precision source and
   committed as data — the table bytes are part of the spec, like a
   cipher's S-box). `sqrt` (fetus distance sort) is integer `isqrt` with
   floor semantics. Powers are repeated multiplication. No `pow`, no
   `exp2` — TOG's `uint8ExpoRange` easing curves are re-specified as
   256-entry pinned tables (input is always a byte, so the entire
   function is enumerable — this is the cheapest possible determinism).
4. **Easing tables, not easing formulas.** Every `uint8*Range` variant
   (linear, expo, inCirc, inQuint, inOutCirc) becomes
   `table[byte] → Q32.32 in [0,1]`, then one shared integer lerp into the
   target range. 5 tables × 256 entries × 8 bytes = 10 KiB of spec data.
   Ports embed the same tables; a conformance test asserts table hashes.
   The **minting script** (high-precision source + generation code) is
   committed beside the table bytes — a table whose derivation is
   unreproducible is a constant nobody can audit.
5. **SVG serialization is specified, not incidental.** Coordinate output
   is centi-units: an `i64` milliunit value is divided by 10 (pinned
   rounding), then formatted by a specified integer algorithm
   (sign, integer part, `.` + exactly the non-zero fractional digits,
   trailing zeros stripped, `-0` normalized to `0`). Attribute order,
   whitespace, element order, and id-generation are all specified.
   No float ever reaches a format call in any port.

**Consequence:** TOG behaviors that exist only as float accidents (the
`(x-x)²+(x-x)²` fetus-distance bug, string-split `roundDown`) are NOT
carried over. We re-mint the genome; visual character is preserved by
construction and by eye, not by bit-archaeology of the old outputs.

## 3. Genome layout

**Size: 8192 bits (1 KiB)**, sized by phenotype need with explicit
reserves. Region map (v1):

| Region | Bits | Contents |
| --- | --- | --- |
| `BODY` | 0–511 | point count (4b: value v → count = 2 + (v mod 15), range **2–16**, v=15 aliasing to count 2), body size (8b), 16 × (x 8b, y 8b, tension 8b) point slots |
| `PALETTE` | 512–575 | palette slot (11b), shade offsets, background selector |
| `FACE` | 576–1087 | eye geometry (size, IPD, lid coefficients, rotation seeds), lash count/length/stroke/direction, brow shape, iris/pupil coefficients, mouth family + params |
| `PATTERN` | 1088–1215 | pattern class selector, circler/cutter/voronoi params |
| `ASYMMETRY` | 1216–1279 | controlled symmetry-breaking deltas (v1: decoded but rendered as zero until the feature ships) |
| `LAYERS` | 1280–1535 | multi-layer body params (v1: dormant, same rule) |
| `ANIMATION` | 1536–1663 | idle-breath period/amplitude, blink cadence, mood-transition tempo seeds |
| `RESERVED` | 1664–8191 | zero in v1; regions are only ever carved from here |

Rules:

- **Bit layout is the compatibility contract** (TOG's every-bit-counts
  discipline). New features carve from `RESERVED`; existing offsets never
  move. A layout change that can't follow this rule is a new genome
  major version.
- **Dormant regions decode from day one** (they appear in the phenotype
  with defined neutral semantics) so that lighting them up later changes
  renderers, not the decoder — and existing DNA gains the feature
  without re-rolling.
- TOG mapped bytes → parameters directly; we keep that byte-granular
  style (fields are 4/8/11-bit unsigned ints, no varints, MSB-first
  within the bit string).

## 4. Identity core vs cosmetic region

Part of a creature must never change, so that it stays recognisable,
and part may. The partition is **by region**:

- **Identity core** = `BODY` + `PALETTE`. Derivation (revised twice, see
  §10): hash of the durable stable ID alone —
  `blake3("cortexkit-avatar/identity-core/v2" || 0x00 || registered_id)`
  — expanded via `blake3` XOF to fill exactly the core regions.
  **No name is ever hashed** (v2 change): names — configured agent
  names, display labels, room titles — are presentation strings:
  user-editable, non-unique (two projects can configure the same agent
  name), and renameable. A name in the hash means a rename
  re-rolls a creature and a collision merges two. Identity is one
  opaque value from one registry authority; every string renders
  BESIDE the creature, never inside it. The derivation contract
  versions through its domain string (v1→v2), independently of
  `generator_version`: a derivation change alters which DNA a seat
  gets, not what any DNA renders as.
  The identity input is **the registry's authoritative id for the
  agent** (its seat), not for its project. The derivation takes an
  opaque string and cannot tell which id-space it came from, so two
  producers feeding different ids draw two different creatures for one
  agent with nothing failing. **`agent_id` is the input**, because an
  avatar identifies an agent while a project id identifies a project:
  while every project hosts one agent the two look interchangeable,
  which hides the flaw, but a project hosting two agents would give them
  identical creatures and no error. Consumers pass through whatever the
  wire delivers, verbatim, and never substitute or normalise it.
  The original derivation used the registry's minted stable project ID,
  which survives workspace moves and path renames, and the reasoning for
  preferring it over the canonical path still holds for any id chosen: a
  path is mutable and aliasable, and deriving a durable key from it means
  a routine folder rename re-rolls the agent's face — the exact failure
  the identity core exists to prevent. The domain-separation prefix pins
  this derivation against colliding with any other blake3 hash of the
  same identity.
  **Unregistered projects**: fall back to path-derived identity, with
  the fallback recorded; when registration happens, an
  **identity-migration era event** repoints the derivation — a repoint
  recorded as its own event, never a silent re-roll. The registry reports whether an id is
  a minted registration or a path-derived fallback, so the transition
  from one to the other is the migration-event trigger, with no
  heuristics needed.
  Immutable for the life of the seat; survives restarts and machine
  moves; differs per agent.
  **Identity is host-authored, only.** Consumers (the phone app, the
  desktop app, the web) never construct identity from strings they hold; they render from the
  identity value the wire delivers, and where the wire carries none
  they render a deliberate placeholder — never a guessed creature. A
  client-derived identity means the same agent wears different faces on
  different surfaces with no error anywhere; this rule deletes that
  class.
  **Scope of the rule, stated because it is easily read without its
  qualifier.** The prohibition is on a surface DECIDING WHICH STRING
  identifies a seat. It is not a prohibition on running the generator:
  rendering from a wire-delivered authoritative id, using the pinned
  library, cannot diverge, because byte-identity across the three
  languages is gated by the committed golden corpus and the pinned
  identity vectors in every conformance leg. Put sharply: client
  derivation is safe exactly when a multi-leg corpus GATES THE
  EQUALITY, and a restatement of the rule that drops that qualifier
  flips its polarity.
  Consequence for the wire: it carries the INPUT (the seat's
  authoritative id), not the derived genome — until mutation makes the
  genome stateful, at which point an additive optional `avatarGenome`
  field appears and takes precedence over derivation. Consumers should
  implement that precedence before the field exists, so the day it
  lands is a no-op for them. It is also the v1 of the eventual contract: once mutation
  ships, the cosmetic region is stateful and the wire carries genome
  data outright — clients are pure renderers from day one.
- **Cosmetic region** = everything else. Initially seeded from the same
  XOF stream (so a fresh seat is fully determined by identity), then
  mutable via the governed mutation interface.
- **Era history**: every mutation appends `(era_n, full_cosmetic_region,
  actor, timestamp)` — append-only, so "what did my agent look like in
  August" replays exactly. Store design is deferred until mutation
  ships; the DNA/phenotype layers don't care where eras live.

Open sub-question for review: does `FACE` belong (partly) in the identity
core? Eyes are highly recognizable. Current lean: keep `FACE` cosmetic
but make mutation *budgeted per era* so drift is gradual (governance
design comes with the mutation note, later).

## 5. Mood

A closed enum, render-time input, versioned with the generator:

| Mood | Typical source (the consumer's mapping, not ours) | Visual axes (from TOG's proven set + extensions) |
| --- | --- | --- |
| `Neutral` | default | DNA-neutral lids, brows, mouth |
| `Working` | board status `working` | slight eye narrowing, focused pupils, tempo up |
| `Happy` | task done / settled | eye rotation out, brows up, smile mouth curve |
| `Waiting` | open ask | wide eyes, raised brows, head-tilt posture |
| `Blocked` | blocked status | TOG-angry: opposed eye rotation, dropped brows, closed upper lids |
| `Sad` | failure/error state | TOG-sad: half-closed lids, outward rotation, dropped brows |
| `Sleepy` | idle / no recent activity | TOG-sleepy: near-closed lids, eyeballs high |

TOG precedent: mood 0–4 (neutral/angry/sad/sleepy/happy) already drives
lid closure, eye/brow rotation, eyeball translation, and mouth-component
curvature — the extension to 7 is additive. Mapping an agent's state to a
mood is the consuming surface's business; the phenotype
carries per-creature mood *response coefficients* (how strongly this
creature's face reacts), the renderer takes `(phenotype, mood)`.

## 6. Golden-vector conformance

Following the golden-corpus pattern named under Inputs, upgraded where
our constraint is stricter (a JSON corpus that pins semantic shape is not
enough; we must pin **bytes**):

- **Corpus layout** (in-repo, minted by the canonical Rust impl):

  ```
  vectors/
    manifest.json          # generator_version, genome_version, producer sha, vector index
    dna/<name>.dna         # raw 1 KiB DNA inputs (hex)
    phenotype/<name>.json  # boundary 1: canonical-JSON phenotype bytes
    svg/<name>__<mood>.svg # boundary 2: exact SVG bytes per (dna, mood)
  ```

- **Boundary 1** (DNA → phenotype): phenotype JSON in a canonicalized
  form — **"avatar canonical JSON v1"**: UTF-8, lexicographically sorted
  keys, no insignificant whitespace, values restricted to integers,
  strings, arrays, and objects. This simple canonicalization is
  sufficient ONLY because the phenotype contains no floats (§2 makes
  float fields unrepresentable); any future field type beyond that set
  must revisit the canonicalization spec first. Consumers assert **byte
  equality** of their canonical serialization AND field-level equality
  after parse (the second gives usable diffs when the first fails).
- **Boundary 2** (phenotype → SVG): raw byte equality. Nothing weaker.
- **Coverage**: every region min/max, every point count 2–16, every mood
  × a body sample, degenerate shapes (all-zero DNA, all-ones DNA,
  min-area bodies that stress the mouth-fit search), every easing table
  boundary byte (0, 1, 127, 128, 254, 255), plus DNA derived from
  sample ids as realistic samples.
- **Minting**: `UPDATE_GOLDEN=1 cargo test -p avatar-vectors tests::mint -- --ignored`,
  which also stamps `manifest.json` with the producer commit. CI never
  regenerates, only verifies. Ports vendor the corpus with the recorded
  producer SHA.
- **Ports don't exist until their conformance leg is in their CI**.
  The easing/trig tables ship with a hash assertion in every
  port so table drift fails loudly and separately from geometry drift.

## 7. Versioning

> **Superseded by `docs/design/012`.** The definition below treats any
> observable output change as a version bump, which would move every
> agent's face whenever the renderer improves. The project settled on the
> opposite: a version exists ONLY to fix a generation bug, an upgrade is
> chosen per row, and a change of depiction is a new TYPE rather than a
> version. A version arm is added and never edited. The paragraph is kept
> as written because later notes argue against it; read 012 for the rule
> in force.
>
> Two further drifts from this section: `era history` describes a store
> this library never owned — the genome is stored on the agent's record in
> the host's registry, which owns it. And the corpus manifest field once called
> `generator_version` is now `corpus_revision`, because it counts
> re-mints and says nothing about what a creature looks like.

- `genome_version` — bit-layout major. Bumps only on layout breaks
  (ideally never; reserves exist to prevent this).
- `generator_version` — any observable output change (decoder or
  renderer). Corpus is regenerated at the canonical impl per bump;
  every consumer re-passes before the version ships. Old versions are
  kept renderable (era history references the generator version that
  produced each look).

## 8. Repo shape (v1)

Cargo workspace, library-first (start as a library; a serving module can
wrap it later if a consumer needs one):

```
crates/
  avatar-genome/     # bit reader, region map, identity-core derivation
  avatar-phenotype/  # typed phenotype, canonical JSON serialization
  avatar-tables/     # pinned easing + trig tables (data + hash tests)
  avatar-svg/        # renderer: (phenotype, mood) -> SVG bytes
  avatar-vectors/    # corpus + minting tests
ports/
  ts/                # later; conformance leg required at birth
  swift/             # later; conformance leg required at birth
vectors/             # golden corpus (see §6)
```

## 9. Decisions settled in review

1. **Rounding rule**: half away from zero, everywhere.
2. **Genome size 8192 bits** with the §3 region map.
3. **Identity core = BODY + PALETTE**, with the blake3-of-identity
   derivation in §4.
4. **Mood vocabulary**: the 7-value enum in §5.
5. **Easing and trig as pinned tables** (spec-as-data) rather than
   re-specified formulas.
6. **TOG's float accidents are dropped** (no bit-archaeology of old
   outputs): no earlier TOG output needs replaying bit for bit.

## 10. Review log

- 2026-08-11 wire-protocol review: approved after folding in negative-
  value rounding vectors, an explicit point-count mapping, a committed
  table minting script, identity derivation from the registry's
  registration ID (not the path) with domain separation and audited
  migration for unregistered projects, and a named canonical-JSON spec
  with its no-floats dependency stated. The genome crate waited for the
  derivation fix (folded above); tables, phenotype and renderer started.
- 2026-08-11 identity-core v2 (integration review for the phone app): the
  agent NAME is removed from the derivation — identity is the stable ID
  alone. Every candidate name source, measured on a working registry,
  failed as an identity input (room titles were all session-derived;
  configured agent names were user-editable and already colliding, with
  two projects sharing one name). The v1 shape (name ‖ id) made the
  hash's distinguishing power come entirely from the id while exposing
  the name as a re-roll hazard; v2 removes the hazard structurally.
  Method note for future revisions: each name source LOOKED durable in
  documentation and failed only when measured — measure fields, don't
  read about them.
- 2026-08-23: the identity input is the agent's id, not its project's
  id (§4), because one project can host more than one agent.
