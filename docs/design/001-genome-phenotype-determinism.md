# Design note 001 — Genome, phenotype, and the determinism contract

Status: DRAFT for review (Ufuk + SUBC). Nothing here is code yet; this note
settles the architecture the charter demands before the canonical Rust
implementation starts.

Inputs: `docs/charter.md`; TOG source pass (`type1.ts`, `spline.ts`,
`math.ts`, `colors.ts`, `face.tsx`, `pattern.tsx`, `bodylayer.tsx`,
`index.tsx`); subconscious golden-vector pattern
(`crates/subc-protocol/tests/golden/`, TS `golden-conformance.test.ts`,
Swift `ProtocolConstantsTests` / `ParityTests`).

## 1. Pipeline

```
DNA (bits)  --decode-->  Phenotype (typed integers)  --render-->  SVG bytes
                              |                                      |
                              +--> golden boundary 1                 +--> golden boundary 2
```

Three strict layers, per charter constraint #1:

- **Genome**: a fixed-size bit string with a versioned region map.
  Decoding never knows about SVG.
- **Phenotype**: the typed parameter set (spline points, palette roles,
  eye geometry, pattern selection, animation params). All values are
  integers in defined units. Mood is NOT here — it is a render-time input.
- **Renderer**: consumes (phenotype, mood, canvas) and emits SVG bytes.
  Never reads bits.

The phenotype is the contract between ports. 3D renderers later consume the
same phenotype (charter open question #3 — deferred, but the layering is
what keeps it one genome).

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

Charter constraint #4. The partition is **by region**:

- **Identity core** = `BODY` + `PALETTE`. Derivation (settling charter
  open question #1, revised in review): hash of durable seat identity —
  `blake3("cortexkit-avatar/identity-core/v1" || 0x00 || canonical_seat_name || 0x00 || entorhinal_registration_id)`
  — expanded via `blake3` XOF to fill exactly the core regions. The
  identity input is the **entorhinal registration ID** (the registry's
  minted stable project ID, which survives workspace moves and path
  renames via the journal), NOT the canonical path: a path is mutable
  and aliasable, and deriving a durable key from it means a routine
  folder rename re-rolls the seat's face — the exact failure the
  identity core exists to prevent (and the exact class that stranded 24
  broca sessions in the brocatui→alfonso-tui rename). The domain-
  separation prefix pins this derivation against colliding with any
  other blake3-of-seat-identity in the fleet.
  **Unregistered projects**: fall back to path-derived identity, with
  the fallback recorded; when registration happens, an
  **identity-migration era event** repoints the derivation — an audited
  repoint, never a silent re-roll.
  Wire mapping (per SUBC): `ck-entorhinal resolve <dir> --json` reports
  `via:"root"` with the minted stable `projectId` for registered
  projects (use it directly), or `via:"implicit"` with a path-derived
  `projectId` for unregistered ones — which is exactly the fallback
  case, no heuristics needed. The implicit→registered transition is the
  migration-event trigger. Registration is idempotent-by-name (response
  carries `noop`), so register-if-implicit bootstrap is safe to repeat.
  Immutable for the life of the seat; survives restarts and machine
  moves; differs per project.
- **Cosmetic region** = everything else. Initially seeded from the same
  XOF stream (so a fresh seat is fully determined by identity), then
  mutable via the governed mutation interface.
- **Era history**: every mutation appends `(era_n, full_cosmetic_region,
  actor, timestamp)` — append-only, so "what did my Alfonso look like in
  August" replays exactly. Store design is deferred until mutation ships
  (fusiform era-store pattern applies); the DNA/phenotype layers don't
  care where eras live.

Open sub-question for review: does `FACE` belong (partly) in the identity
core? Eyes are highly recognizable. Current lean: keep `FACE` cosmetic
but make mutation *budgeted per era* so drift is gradual (governance
design comes with the mutation note, later).

## 5. Mood

A closed enum, render-time input, versioned with the generator:

| Mood | Fleet source (consumer's mapping, not ours) | Visual axes (from TOG's proven set + extensions) |
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
curvature — the extension to 7 is additive. Mapping fleet state → mood is
the consuming surface's business (charter constraint #5); the phenotype
carries per-creature mood *response coefficients* (how strongly this
creature's face reacts), the renderer takes `(phenotype, mood)`.

## 6. Golden-vector conformance

Following the subc-protocol pattern, upgraded where our constraint is
stricter (subc's JSON corpus pins semantic shape; we must pin **bytes**):

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
  boundary byte (0, 1, 127, 128, 254, 255), plus N seats' real derived
  DNA as realistic samples.
- **Minting**: `UPDATE_GOLDEN=1 cargo test -p avatar-vectors` (subc
  convention), which also stamps `manifest.json` with the producer
  commit. CI never regenerates, only verifies. Ports vendor the corpus
  with the recorded producer SHA (cross-repo payload rule).
- **Ports don't exist until their conformance leg is in their CI**
  (charter). The easing/trig tables ship with a hash assertion in every
  port so table drift fails loudly and separately from geometry drift.

## 7. Versioning

- `genome_version` — bit-layout major. Bumps only on layout breaks
  (ideally never; reserves exist to prevent this).
- `generator_version` — any observable output change (decoder or
  renderer). Corpus is regenerated at the canonical impl per bump;
  every consumer re-passes before the version ships. Old versions are
  kept renderable (era history references the generator version that
  produced each look).

## 8. Repo shape (v1)

Cargo workspace, library-first (charter open question #5: start as a
library; a serving module can wrap it later if a consumer needs one):

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

## 9. Decisions requested from review

1. **Rounding rule**: half-away-from-zero everywhere — agree?
2. **Genome size 8192 bits** with the §3 region map — agree, or reshape
   regions before anything is minted?
3. **Identity core = BODY + PALETTE** and the blake3-of-seat-identity
   derivation — settle charter open question #1 as specced?
4. **Mood vocabulary**: the 7-value enum — right set for v1 surfaces?
5. **Easing/trig as pinned tables** (spec-as-data) rather than
   re-specified formulas — agree?
6. **TOG float accidents dropped** (no bit-archaeology of old outputs) —
   SUBC confirms TOG never shipped publicly, so no on-chain/archival
   output needs replaying; awaiting Ufuk's co-sign.

## 10. Review log

- 2026-08-11 SUBC (wire-protocol bar): approved after folding — negative-
  value rounding vectors, explicit point-count mapping, committed table
  minting script, identity derivation from entorhinal registration ID
  (not path) with domain separation and audited migration for
  unregistered projects, named canonical-JSON spec with its no-floats
  dependency stated. Identity/genome crate was held on the derivation
  fix (folded above); tables, phenotype, and renderer cleared to start.
