# Design note 009 — Genome v2 layout (DRAFT)

Status: DRAFT against the playground vocabulary as of grids 1–4.
Ufuk's filter pass over the grids may shrink selector ranges; it does
not move offsets — that robustness is designed in (selectors decode
mod family-count, so families can be added from reserved values or
retired to aliases without layout breaks).

Genome v2: **16 KiB = 131,072 bits**, replacing the 1 KiB TOG-derived
v1. Same bit-addressing conventions as note 003 (MSB-first, big-endian
fields, byte k = region_start + 8k). Same laws: every bit pattern is
valid DNA (mod-N selectors); dormant fields decode from day one; new
fields carve from reserves only; identity core fills first from the
XOF stream.

## Region map

| Region | Bits | Size | Contents |
| --- | --- | --- | --- |
| `BODY` | 0–255 | 256 | proportions, plan |
| `PALETTE` | 256–383 | 128 | recipe, hues |
| `PARTS` | 384–1151 | 768 | 4 slots × family + params |
| `FACE` | 1152–1663 | 512 | eye/mouth archetypes + params |
| `PATTERN` | 1664–1919 | 256 | belly/back/spots (dormant beyond belly in v2.0) |
| `MOOD_RESPONSE` | 1920–2047 | 128 | per-creature reactivity |
| `ANIMATION` | 2048–2559 | 512 | idle personality (breath, blink, sway, fidget) |
| `WONK` | 2560–2815 | 256 | asymmetry budget (part/face-directed, per grid-2 finding) |
| `DEPTH` | 2816–3071 | 256 | the 3D door: per-part depth-order + z-hints, dormant |
| `RESERVED` | 3072–131071 | 125 Kib | zero in v2.0 |

~2.9 Kib live, ~97.8% reserved — deliberately: v1's 6.5 KiB reserve made
the floor change and every future carve cheap; v2 scales that policy to
the blessed roadmap (more part slots, texture planes, 3D).

## BODY (byte offsets within region)

| Byte | Field | Mapping |
| --- | --- | --- |
| 0 | `plan` | mod plan-count (v2.0: 1 — blob; tall/floating candidates carve later) |
| 1 | `height` | EASE_LINEAR → [140, 205] playground-units × canvas scale |
| 2 | `w_top` | EASE_LINEAR → [50, 88] |
| 3 | `w_bot` | EASE_LINEAR → [56, 96] |
| 4 | `n_top` | EASE_LINEAR → Q [2.0, 4.6] |
| 5 | `n_bot` | EASE_LINEAR → Q [2.0, 5.0] |
| 6 | `waist` | EASE_LINEAR → Q [0.40, 0.70] |
| 7–31 | reserved | |

The playground presets all live inside these bands; the bands ARE the
always-presentable manifold for the blob plan (design law 3).

## PALETTE

| Byte | Field | Mapping |
| --- | --- | --- |
| 0 | `recipe` | weighted selector over kept bands: pastel-analogous ×3, toy-saturated ×2, muted-earthy ×2, dark-gravitas ×1 (rarity = identity; weights via mod-8 alias table) |
| 1–2 | `base_hue` | 16-bit → [0, 360) via BAM-style scaling |
| 3 | `hue_jitter` | EASE_LINEAR → ±8° on accent relationships |
| 4–15 | reserved (per-part accent hues carve here) | |

## PARTS — 4 slots × 24 bytes

Slot order: `top`, `side`, `bottom`, `cheek`. Each slot:

| Byte | Field |
| --- | --- |
| 0 | `family` (mod slot-family-count; 0 = none where sanctioned) |
| 1–6 | 6 × family param bytes (length, spread, droop, curl, thickness, count — family-documented) |
| 7–23 | reserved |

v2.0 family counts: top 6 (none/ears/horns/antennae/tuft/swoop), side 3
(none/stub-arms/wings — wings unexplored, gated on a grid), bottom 4
(none/feet-nubs/duck-feet/tail — tail placement fix first), cheek 2
(none/fluff; blush is not a part, it is always-on face paint).

## FACE

| Byte | Field | Mapping |
| --- | --- | --- |
| 0 | `eye_kind` | mod 5 (dot-shine/lidded/happy-arc/uneven/half-lid) |
| 1 | `eye_scale` | EASE_LINEAR → Q [0.85, 1.18] |
| 2 | `eye_sep` | EASE_LINEAR → Q [0.36, 0.48] of local width |
| 3 | `eye_y` | EASE_LINEAR → Q [-0.14, -0.02] of height |
| 4 | `mouth_kind` | mod 6 (arc/cat-ω/open-happy/snaggle/flat/fangs) |
| 5 | `mouth_scale` | EASE_LINEAR → Q [0.8, 1.25] |
| 6 | `blush_strength` | EASE_LINEAR → Q [0.5, 1.1] |
| 7 | `belly_strength` | EASE_LINEAR → Q [0.3, 0.6] |
| 8–63 | reserved (per-eye overrides, iris hue, lash revival) | |

## MOOD_RESPONSE

Per-mood-axis reactivity, one byte each (lid, brow, mouth, eye-scale,
adjunct-propensity), EASE_LINEAR → Q [0.6, 1.4]; 11 bytes reserved.
Adjunct-propensity gates whether a creature shows Zz/sweat at all —
personality, not just amplitude.

## ANIMATION / WONK / DEPTH

Animation: breath period/amplitude, blink interval/jitter, idle sway,
fidget seed — 8 bytes live, rest reserved. WONK: 8 delta bytes +
strength byte, spent on PART length asymmetry and FACE tilt (grid-2
finding: body-level wonk is dead weight; body lean byte stays reserved).
DEPTH: per-slot depth-order overrides + z-elevation hints, decoded and
carried in the phenotype, rendered flat in v2.0 — the 3D door stays a
door.

## Identity cut

Identity core = `BODY` + `PALETTE` + `PARTS` + `FACE` (bits 0–1663):
the recognizable creature. Cosmetic/mutable = everything after. Same
blake3-XOF fill discipline as v1 (identity-core/v2 domain string
unchanged — the derivation contract is independent of genome layout).

## What this note is NOT

Not a phenotype schema (that is 010, written at freeze), not value
mappings beyond the normative bands above (decoder-owned, generator-
versioned), not a commitment to family counts (Ufuk's filter pass may
shrink; mod-N absorbs it).
