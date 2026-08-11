# Design note 003 — Genome bit map v1

Status: DRAFT. Fixes the bit-level layout inside the regions that note
001 §3 reserved. This layout is the compatibility contract: offsets in
this note never move; new fields carve from region-local or global
reserves. Value MAPPINGS (which easing table, which target range) are
deliberately NOT in this note — they live in the decoder as one
normative mapping table, are covered by the golden corpus, and may
evolve under `generator_version` bumps without touching this layout.

## Conventions

- Offsets are bits from the start of the 8192-bit genome, MSB-first
  within the bit string; multi-bit fields read big-endian.
- `byte k` inside a region = bits `[region_start + 8k, region_start + 8k + 8)`.
- Selector fields with fewer valid values than their width decode
  modulo the value count (e.g. 4-bit point count → `2 + (v mod 15)`),
  so every bit pattern is valid DNA — there is no invalid genome.
- Flag bits: 1 bit per gate, packed into named flag bytes; unassigned
  flag bits are reserved-zero.

## BODY — bits 0–511

| Rel. bits | Field |
| --- | --- |
| 0–3 | `body.point_count` (→ 2–16, mod-15 rule) |
| 4–11 | `body.size` seed |
| 12–395 | 16 point slots × 24 bits: x 8, y 8, tension 8 |
| 396–511 | reserved |

## PALETTE — bits 512–575

| Rel. bits | Field |
| --- | --- |
| 0–10 | palette slot (0–2047, base-table index) |
| 11–13 | background variant |
| 14–63 | reserved |

Shade-derivation offsets (darker/lighter steps) are fixed constants in
the decoder, not DNA — TOG precedent, keeps the region small.

## FACE — bits 576–1087 (region bytes 0–63)

| Byte | Field |
| --- | --- |
| 0 | `eyes.position_mode` (mod 5) |
| 1 | `eyes.size_h` seed |
| 2 | `eyes.ipd` seed |
| 3 | `eyes.height_upper` |
| 4 | `eyes.height_lower` |
| 5 | flags1: b0 `eyes.height_bias_gate`, b1 `eyes.vertical_pull_gate`, b2 `mouth.width_scale_gate`, b3 `mouth.aspect_widen_gate`, b4–7 reserved |
| 6 | `eyes.height_bias` |
| 7 | `eyes.vertical_pull` |
| 8 | `eyes.contour_inset1` |
| 9 | `eyes.contour_inset2` |
| 10 | `eyes.eyeball_radius_scale` |
| 11 | `eyes.pupil_scale` |
| 12 | `eyes.iris_shade_opacity` |
| 13 | `eyes.iris_shade_reduce` |
| 14 | `eyes.iris_saturation_reduce` |
| 15 | `eyes.lid_close_upper_neutral` |
| 16 | `eyes.lid_close_lower_neutral` |
| 17 | `eyes.lid_extension_gate` (byte-valued: gate + strength) |
| 18 | `eyes.lid_extension_side` |
| 19 | `eyes.noise_freq_h` |
| 20 | `eyes.noise_freq_v` |
| 21 | `eyes.noise_seed` |
| 22 | `eyes.rotation_seed` |
| 23 | `eyes.lid_color_gene` |
| 24 | `eyes.iris_color_gene` |
| 25 | `brows.end_height1` |
| 26 | `brows.mid_x` |
| 27 | `brows.end_height2` |
| 28–30 | `brows.tensions[0..3]` |
| 31 | `brows.width_coupling` |
| 32 | `brows.clearance` |
| 33 | `lashes.count` |
| 34 | `lashes.removed` |
| 35 | `lashes.length` |
| 36 | `lashes.stroke` |
| 37 | `lashes.direction` |
| 38 | `mouth.family` (mod family count) |
| 39 | `mouth.width_scale` |
| 40–47 | `mouth.params[0..8]` |
| 48 | `placement.width_coeff` |
| 49 | `placement.height_coeff` |
| 50–63 | reserved |

## PATTERN — bits 1088–1215 (region bytes 0–15)

| Byte | Field |
| --- | --- |
| 0 | `pattern.class` (mod 6) |
| 1 | `circler.point_reduce` |
| 2 | `circler.size` |
| 3 | `circler.y_offset_div` |
| 4 | `circler.tension` |
| 5 | `circler.pull_min` |
| 6 | `voronoi.index` |
| 7–8 | `voronoi.rotation` (BAM16) |
| 9 | `voronoi.scale` |
| 10 | `cut.spread` |
| 11 | `cut.y_adjust_div` |
| 12–15 | reserved |

All sub-blocks decode regardless of `class` (002's always-present rule).

## ASYMMETRY — bits 1216–1279

8 × 8-bit deltas → `asymmetry.deltas[0..8]`.

## LAYERS — bits 1280–1535 (region bytes 0–31)

| Byte | Field |
| --- | --- |
| 0 | `layers.count` |
| 1–16 | `layers.params[0..16]` |
| 17–31 | reserved |

## ANIMATION — bits 1536–1663 (region bytes 0–15)

| Byte | Field |
| --- | --- |
| 0 | `animation.breath_period` seed |
| 1 | `animation.breath_amplitude` |
| 2 | `animation.blink_interval` seed |
| 3 | `animation.blink_jitter_seed` |
| 4 | `animation.mood_transition_tempo` |
| 5 | `mood_response.lid_reactivity` |
| 6 | `mood_response.brow_reactivity` |
| 7 | `mood_response.eye_rotation_reactivity` |
| 8 | `mood_response.mouth_reactivity` |
| 9–15 | reserved |

## RESERVED — bits 1664–8191

Zero in v1. Regions are only ever carved from here; carving is a
`genome_version`-visible event recorded by extending this note.

## Identity core cut

Per 001 §4: identity core = bits 0–575 (BODY + PALETTE), filled from
the blake3 XOF stream first; the cosmetic region (bits 576–8191) is
filled from the stream's continuation at first derivation and mutable
thereafter. The XOF fills reserved bits too — they are DNA, dormant
today, meaningful under future decoders (001's dormant-region rule).
