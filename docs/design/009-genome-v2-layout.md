# Design note 009 — Genome v2 layout (DRAFT, rev 2: wild pipeline)

Status: DRAFT rev 2, re-carved for the wild-spline pipeline after the
grid-7 course correction (rev 1 described the retired blob generator —
superseded, kept in git history). Written against the playground
vocabulary as of grid 10 plus the lab-session fixes. Ufuk's pending
filter pass shrinks selector ranges, never offsets.

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
| `EXTREMITIES` | 512–1279 | 768 | 0–3 mirrored limb pairs + topology |
| `PALETTE` | 1280–1407 | 128 | recipe, hues, belly variant |
| `FACE` | 1408–1919 | 512 | archetypes + params (placement is computed, never encoded) |
| `SURFACE` | 1920–2431 | 512 | voronoi sites + shading |
| `MOOD_RESPONSE` | 2432–2559 | 128 | per-creature reactivity |
| `ANIMATION` | 2560–3071 | 512 | idle personality (breath, blink, sway, fidget) |
| `WONK` | 3072–3327 | 256 | post-mirror asymmetry budget (dormant in v2.0 — see below) |
| `DEPTH` | 3328–3583 | 256 | the 3D door: depth-order + z-hints, dormant |
| `RESERVED` | 3584–131071 | ~124.5 Kib | zero in v2.0 |

~3.5 Kib live, ~97.3% reserved — v1's reserve policy scaled to the
blessed roadmap (denser splines, more limb pairs, texture planes, 3D).

## BODY (byte offsets within region)

The wild pipeline's shape entropy lives here: this region alone carries
more point information than TOG's whole 1024-bit body block.

| Byte | Field | Mapping |
| --- | --- | --- |
| 0 | `spoke_count` | mod 9 → 5..=13 half-side spokes |
| 1 | `domestication` | EASE_LINEAR → Q [0.12, 0.72]; population center ~0.32. The wild↔tame axis: radius smoothing + concavity floor + tension pull-to-1 + ground flatten all key off this one value |
| 2 | `ground_sit` | EASE_LINEAR → Q [0.06, 0.22] flatten band |
| 3 | reserved (body plan door) | |
| 4..=42 | 13 × spoke slots: (radius, tension, θ-jitter) 1 byte each | radius EASE_LINEAR → Q [0.12, 1.35]; tension EASE_LINEAR → Q [0.4, 2.6] (overshoot loops included — fill-rule turns self-crossings into character); jitter EASE_LINEAR → ±0.45 slot-widths |
| 43–63 | reserved (per-spoke overrides, denser rings) | |

All 13 slots decode regardless of `spoke_count` (dormant-region law);
the ring uses the first `spoke_count`. **Mirroring is structural, not
genomic**: every spoke folds into the right half-plane and reflects —
asymmetry is unrepresentable in the ring (grid-7 review; the free-form
flag rev 1 carried is gone, not dormant, since a bit that must never
fire is a footgun, not a reserve).

## EXTREMITIES — the species-makers

| Byte | Field | Mapping |
| --- | --- | --- |
| 0 | `pair_count` | mod 4 → 0..=3 mirrored pairs |
| 1 | `topology` | mod 3 → merged / layered-tone / layered-accent (a character axis: grown vs worn anatomy) |
| 2 | `weld_radius` | EASE_LINEAR → Q [0.08, 0.28] × mean radius (merged only; decoded always) |
| 3 | reserved | |
| 4..=33 | 3 × pair slots, 10 bytes each | per pair: anchor θ (upper+side band), length Q [0.35, 1.15] × mean radius, base width Q [0.10, 0.30] (renderer caps at 0.8·len/\|curl\| — the curvature cap is a RENDER law, not a genome band), curl ±1.3 rad, taper Q [0.15, 0.85], fill-voice (mod 3: body/tone/accent), 4 reserved |
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
| 1 | `eye_scale` | EASE_LINEAR → Q [0.85, 1.18] on the placed radius |
| 2 | `mouth_kind` | mod 6 (arc/cat-ω/open-happy/snaggle/flat/fangs) |
| 3 | `mouth_scale` | EASE_LINEAR → Q [0.8, 1.25] |
| 4 | `blush_strength` | EASE_LINEAR → Q [0.5, 1.1] |
| 5–63 | reserved (iris hue, per-eye overrides) | |

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
| 2..=40 | 13 × site slots: (x, y, shade) 1 byte each | x EASE_LINEAR → [0.04, 1.05] (right half, mirrored); y → [-1.05, 1.05]; shade mod 4 → dark/light/skip/skip (sparse: spots on hide, not tiling) |
| 41–63 | reserved (stripes, back patches) | |

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

## What this note is NOT

Not a phenotype schema (that is 010, written at freeze), not value
mappings beyond the normative bands above (decoder-owned, generator-
versioned), not a commitment to family counts (Ufuk's filter pass may
shrink; mod-N absorbs it). The render laws quoted inline (curvature
cap, ink discipline, placement search, pattern masking, archetype-aware
moods) are 010 material — they appear here so the region map reads with
its consequences attached.
