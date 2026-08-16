# Design note 009 — Genome v2 layout (DRAFT, rev 3: measured)

Status: DRAFT rev 3. Rev 1 described the retired blob generator; rev 2
re-carved the regions for the wild-spline pipeline after the grid-7
course correction; rev 3 re-syncs every band to what the playground
actually decodes, after the bit-budget rebalance and the face block.
Both earlier revisions are in git history. Ufuk's pending filter pass
shrinks selector ranges, never offsets.

**Measured consumption** (`cargo run --bin grid -- --dna`, 2000 seeds):
34–91 bytes, mean 62 → **272–728 bits, mean 496**. For scale, the
ancestor allocates 1024 bits (4 × `bytes32`) and consumes 472–984 of
them, with ~488–512 on face and mouth parameters. v2 runs leaner in
total while carrying roughly double TOG's silhouette budget — the
reallocation is the point, not the total.

Genome v2: **16 KiB = 131,072 bits**, replacing the 1 KiB TOG-derived
v1. Same bit-addressing conventions as note 003 (MSB-first, big-endian
fields, byte k = region_start + 8k). Same laws: every bit pattern is
valid DNA (mod-N selectors); dormant fields decode from day one; new
fields carve from reserves only; identity core fills first from the
XOF stream.

## Region map

| Region | Bits | Size | Contents |
| --- | --- | --- | --- |
| `BODY` | 0–511 | 512 | spokes, domestication, ground-sit |
| `EXTREMITIES` | 512–1279 | 768 | 0–4 mirrored limb pairs + topology |
| `PALETTE` | 1280–1407 | 128 | recipe, hues, belly variant |
| `FACE` | 1408–1919 | 512 | archetypes, proportion, resting posture (placement is computed, never encoded) |
| `SURFACE` | 1920–2431 | 512 | pattern selector + site seed |
| `MOOD_RESPONSE` | 2432–2559 | 128 | per-creature reactivity |
| `ANIMATION` | 2560–3071 | 512 | idle personality (breath, blink, sway, fidget) |
| `WONK` | 3072–3327 | 256 | post-mirror asymmetry budget (dormant in v2.0 — see below) |
| `DEPTH` | 3328–3583 | 256 | the 3D door: depth-order + z-hints, dormant |
| `RESERVED` | 3584–131071 | ~124.5 Kib | zero in v2.0 |

Region SIZES are unchanged from rev 2 and are deliberately generous:
rebalancing moved live bytes between regions (SURFACE gave up ~37
bytes, BODY and EXTREMITIES took them) without moving a single region
boundary. That is the reserve policy earning its keep — a budget
reallocation that would otherwise have been a layout break cost
nothing.

~3.5 Kib allocated live, of which 272–728 bits decode per creature;
~97.3% reserved — v1's reserve policy scaled to the blessed roadmap
(denser splines, more limb pairs, texture planes, 3D).

## BODY (byte offsets within region)

The wild pipeline's shape entropy lives here: this region alone carries
more point information than TOG's whole 1024-bit body block.

| Byte | Field | Mapping |
| --- | --- | --- |
| 0 | `spoke_count` | mod 12 → 5..=16 half-side spokes (widened from 5..=13 with budget recovered from SURFACE) |
| 1 | `domestication` | EASE_LINEAR → Q [0.12, 0.72]; population center ~0.32. The wild↔tame axis: radius smoothing + concavity floor + tension pull-to-1 + ground flatten all key off this one value |
| 2 | `ground_sit` | EASE_LINEAR → Q [0.06, 0.22] flatten band |
| 3 | reserved (body plan door) | |
| 4..=51 | 16 × spoke slots: (radius, tension, θ-jitter) 1 byte each | radius EASE_LINEAR → Q [0.12, 1.35]; tension EASE_LINEAR → Q [0.4, 2.6] (overshoot loops included — fill-rule turns self-crossings into character); jitter EASE_LINEAR → ±0.45 slot-widths |
| 43–63 | reserved (per-spoke overrides, denser rings) | |

All 16 slots decode regardless of `spoke_count` (dormant-region law);
the ring uses the first `spoke_count`. **Mirroring is structural, not
genomic**: every spoke folds into the right half-plane and reflects —
asymmetry is unrepresentable in the ring (grid-7 review; the free-form
flag rev 1 carried is gone, not dormant, since a bit that must never
fire is a footgun, not a reserve).

## EXTREMITIES — the species-makers

| Byte | Field | Mapping |
| --- | --- | --- |
| 0 | `pair_count` | mod 5 → 0..=4 mirrored pairs (limbs proved the strongest species lever, so they got budget too) |
| 1 | `topology` | mod 3 → merged / layered-tone / layered-accent (a character axis: grown vs worn anatomy) |
| 2 | `weld_radius` | EASE_LINEAR → Q [0.08, 0.28] × mean radius (merged only; decoded always) |
| 3 | reserved | |
| 4..=43 | 4 × pair slots, 10 bytes each | per pair: anchor θ (upper+side band), length Q [0.35, 1.15] × mean radius, base width Q [0.10, 0.30] (renderer caps at 0.8·len/\|curl\| — the curvature cap is a RENDER law, not a genome band), curl ±1.3 rad, taper Q [0.15, 0.85], fill-voice (mod 3: body/tone/accent), 4 reserved |
| 34–95 | reserved (more pairs, per-limb texture) | |

## PALETTE

| Byte | Field | Mapping |
| --- | --- | --- |
| 0 | `recipe` | weighted mod-8: pastel-analogous ×3, toy-saturated ×2, muted-earthy ×2, dark-gravitas ×1 (rarity = identity) |
| 1–2 | `base_hue` | 16-bit → [0, 360); stateless uniform per the grid-6 A/B |
| 3 | `belly_variant` | mod 4 → near-white / lighter-tone / warm-cream / accent-tint; belly fills are OPAQUE (immunity to surface pattern by construction) |
| 4 | `hue_jitter` | EASE_LINEAR → ±8° on accent relationships |
| 5–15 | reserved (per-limb accent hues) | |

Ink discipline (render law, recorded here because palette carries it):
`ink` = line-work, flips light on dark bodies; `pupil` = always dark;
`sclera` = light backing behind open eyes on dark bodies only.

## FACE

| Byte | Field | Mapping |
| --- | --- | --- |
| 0 | `eye_kind` | mod 5 (dot-shine/lidded/happy-arc/uneven/half-lid) |
| 1 | `eye_aspect` | EASE_LINEAR → Q [0.80, 1.28]; applied as MIN-scaling (both axis factors ≤ 1) so an eye can only shrink inside its proved radius |
| 2 | `eye_wonk` | EASE_LINEAR → Q [0, 0.20] right-eye shrink. DORMANT: needs a per-eye radius inside the archetype renderers |
| 3 | `mouth_kind` | mod 6 (arc/cat-ω/open-happy/snaggle/flat/fangs) |
| 4 | `mouth_width` | EASE_LINEAR → Q [0.75, 1.30]; feeds the placement fit test |
| 5 | `mouth_depth` | EASE_LINEAR → Q [0.70, 1.35]; feeds the placement fit test |
| 6 | `resting_brow` | weighted mod 8 → none ×5 / focus ×2 / raised ×1 / knit ×1; mood brows override |
| 7 | `blush_strength` | EASE_LINEAR → Q [0.5, 1.1] |
| 8–63 | reserved (iris hue, per-eye overrides) | |

This block exists because measurement caught the vocabulary spending 16
bits on the face against TOG's ~500: `eye_kind × mouth_kind` gave the
whole population 30 faces, and two creatures rolling the same pair
rendered identically. The block restores the VISIBLE half of TOG's face
DNA — proportion and resting posture — and deliberately not the half
avatar sizes cannot resolve (iris saturation, lash removal arcs). Every
field is bounded so no value can breach a placement invariant: mouth
scale is an input to the fit search, so a wide mouth moves the face
rather than overflowing the silhouette.

**Placement is computed, never encoded** — the EVM-free findRect:
SDF-verified search over height-band × spacing, mouth-fit in the score,
eyes-only fallback (a creature always has a face), separation invariant
r ≤ ipd/1.45. Geometry the genome cannot corrupt is geometry that needs
no invalid-value defense; the `--check` sweep proves the invariants over
seed populations and ships with the generator.

## SURFACE

| Byte | Field | Mapping |
| --- | --- | --- |
| 0 | `pattern` | weighted mod 5 → none ×3 / voronoi ×2 |
| 1 | `site_pairs` | mod 6 → 8..=13 mirrored pairs (16..=26 cells) |
| 2–3 | `site_seed` | 16-bit seed; sites are GENERATED from it (x, y, shade per pair) rather than stored |
| 4–63 | reserved (stripes, back patches) | |

Storing sites cost 200–320 bits — more than the entire body silhouette
in some genomes — for a mottle that is subtle at full size and invisible
at icon size. Lloyd relaxation then moved every site anyway, which is
the tell that their exact stored positions were never worth their
price. The region is now 32 bits and the recovered budget bought spokes
and a fourth limb pair. Perceptual weight decides bit budget.

Render laws riding this region: sites Lloyd-relax ×2; cells draw as
midpoint-rounded pebbles (inset 0.8) in same-hue translucent shades;
the face-and-belly capsule masks the pattern out (mottle belongs to the
hide, never the face); the opaque belly covers whatever remains.

## MOOD_RESPONSE

Per-mood-axis reactivity, one byte each (lid, brow, mouth, eye-scale,
adjunct-propensity), EASE_LINEAR → Q [0.6, 1.4]; 11 bytes reserved.
Adjunct-propensity gates whether a creature shows Zz/sweat at all —
personality, not amplitude. Expression is **archetype-aware** (render
law): happy-arc takes brows only; half-lid takes only sleepy-scale
lids; uneven's overlays track per-disc radii.

## ANIMATION / WONK / DEPTH

Animation: breath period/amplitude, blink interval/jitter, idle sway,
fidget seed — 8 bytes live, rest reserved. WONK: fully dormant in v2.0
(the mirror fold is absolute; if subtle post-mirror displacement ever
earns its way in, it decodes from here behind a generator_version bump
— grid-2 already showed face/part asymmetry beats body asymmetry).
DEPTH: per-limb depth-order + z-elevation hints, decoded and carried in
the phenotype, rendered flat in v2.0.

## Identity cut

Identity core = `BODY` + `EXTREMITIES` + `PALETTE` + `FACE` (bits
0–1919): the recognizable creature. `SURFACE` onward is the cosmetic
region — mutable via era events once the era store ships. Same
blake3-XOF fill discipline; identity-core/v2 domain string unchanged
(the derivation contract is independent of genome layout).

## Portability spike (run before committing to this layout)

The wild pipeline adds machinery v1 never had — SDF fields, smooth-min
welds, marching squares, Lloyd relaxation, arc-length resampling — and
the freeze promises three byte-identical implementations of all of it.
`playground/src/bin/fixedspike.rs` answers whether that promise is
keepable, measured rather than assumed:

| Probe | Result |
| --- | --- |
| integer sqrt vs `f64::sqrt`, [0.001, 90000] | max error 1.9e-9 (Q32.32 ulp 2.3e-10) |
| SDF over a real body, 3600 probes | max divergence 1.5e-8 units; **0** inside/outside disagreements |
| smooth-min weld, 6400 pairs | max divergence 3.6e-10 units |
| marching-squares sign tests, 6400 grid nodes | **0** branch flips |

The one new primitive is **square root**, and it is exactly integer: a
binary restoring algorithm with a fixed 64-step loop, so there is no
convergence-count variance to diverge between languages. Divergence
everywhere else is ulp-scale, four orders below a milliunit, and — the
load-bearing result — **no sign test flips**, so marching squares takes
identical branches and the extracted contour has identical topology.
The field is the same field.

The spike's real value was the risks it found that are NOT arithmetic:

1. **Segment chaining tiebreak.** The contour chainer took the *first*
   candidate within eps. Deterministic, but arbitrary at saddles. Now
   nearest-with-lowest-index, compared on SQUARED distances so the
   ordering needs no sqrt and survives the port exactly.
2. **Longest-loop tiebreak.** Equal-length loops resolve to the
   earliest in grid-scan order (strict `>`), now stated rather than
   incidental.
3. **Lloyd centroid** needs the Q32 multiply's 128-bit intermediate —
   already solved in v1 (Swift limbs, JS BigInt), so it is a port task,
   not a design risk.

Verdict: this layout is portable. Note 010 inherits the sqrt primitive
and both tiebreak rules as normative.

## What this note is NOT

Not a phenotype schema (that is 010, written at freeze), not value
mappings beyond the normative bands above (decoder-owned, generator-
versioned), not a commitment to family counts (Ufuk's filter pass may
shrink; mod-N absorbs it). The render laws quoted inline (curvature
cap, ink discipline, placement search, pattern masking, archetype-aware
moods) are 010 material — they appear here so the region map reads with
its consequences attached.
