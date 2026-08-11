# Design note 002 — Phenotype schema v1

Status: DRAFT. Companion to note 001 (which settles the pipeline and the
numeric discipline); this note enumerates the actual phenotype fields.
The schema is derived from the TOG reference implementation's parameter
inventory (`type1.ts` body, `face.tsx` face engine, `pattern.tsx`,
`colors.ts`) re-expressed under the 001 numeric contract.

## Conventions

- All lengths/coordinates: `i64` **canvas milliunits** — the canonical
  canvas is `1_000_000 × 1_000_000` milliunits (rendered SVG viewBox is
  `0 0 10000 10000` centi-units; ×100 between the two, exact).
- All dimensionless coefficients (tensions, ratios, blend factors,
  closure amounts): `Q32.32` in `i64`.
- All counts/selectors: `u8`/`u16` with their valid range documented.
- Angles: BAM16 (`u16`, 65536 = full turn) — matches avatar-tables.
- The phenotype serializes as **avatar canonical JSON v1** (001 §6).
  Field names below are the serialized names (snake_case, sorted at
  serialization time).

## Top-level shape

```
Phenotype {
  genome_version: u16,
  body: Body,
  palette: Palette,
  face: Face,
  pattern: Pattern,
  animation: Animation,
  mood_response: MoodResponse,
  asymmetry: Asymmetry,   // dormant in v1: decoded, rendered as zero
  layers: Layers,         // dormant in v1
}
```

Dormant blocks are REQUIRED fields with defined neutral values — 001 §3's
dormant-region rule means their absence is a schema violation, not a
default.

## Body

TOG `type1.ts` re-expressed:

```
Body {
  point_count: u8,          // 2..=16
  size: i64,                // milliunits; DNA byte mapped into [canvas/1.4, canvas/1.2]
  points: [BodyPoint; 16],  // slots >= point_count carry zeroed points (canonical padding)
}
BodyPoint { x: i64, y: i64, tension: Q32.32 }
```

- `x`, `y` are the half-side control points (pre-mirror); mirroring into
  the closed symmetric shape is renderer geometry, not phenotype.
- Tension folds TOG's `getSplineSmoothing` (xFactor/extra modulation)
  into one final per-point coefficient at decode time — renderers never
  see the modulation logic.
- Fetus/growth morph ratio is a RENDER input (like mood), not phenotype:
  it animates birth, it is not identity.

## Palette

TOG's 15 named roles, kept verbatim (they are the face/body/pattern
vocabulary the renderer already speaks):

```
Palette {
  back, primary, primary_lighter, primary_darkish, primary_darker,
  primary_darkest, harmonic1, harmonic1_darkish, harmonic1_darkest,
  harmonic2, harmonic3, harmonic4, secondary_darker, dark, white: Rgb
}
Rgb { r: u8, g: u8, b: u8 }
```

- Colors are fully resolved at decode time (slot lookup + shade math in
  integer RGB) so renderers do zero color arithmetic. TOG's
  `colorShade`/`reduceSaturation` HSL round-trips are re-specified as
  integer ops in the decoder; the 2048-slot base palette table moves
  into the decoder as pinned data (same spec-as-data discipline as
  avatar-tables).

## Face

The `face.tsx` inventory (dna2/dna3/features map) re-grouped:

```
Face {
  eyes: Eyes, brows: Brows, lashes: Lashes, mouth: Mouth,
  placement: FacePlacement,
}
Eyes {
  position_mode: u8,          // 0..=4: which body landmark anchors eye line
  size_h: i64, ipd: i64,      // horizontal eye size, inter-pupil distance
  height_upper: Q32.32, height_lower: Q32.32,
  height_bias_gate: bool, height_bias: Q32.32,
  vertical_pull_gate: bool, vertical_pull: Q32.32,
  contour_inset1: Q32.32, contour_inset2: Q32.32,
  eyeball_radius_scale: Q32.32,
  pupil_scale: Q32.32,
  iris_shade_opacity: Q32.32, iris_shade_reduce: Q32.32,
  iris_saturation_reduce: Q32.32,
  lid_close_upper_neutral: Q32.32, lid_close_lower_neutral: Q32.32,
  lid_extension_gate: u8, lid_extension_side: u8,
  noise_freq_h: Q32.32, noise_freq_v: Q32.32, noise_seed: u8,
  rotation_seed: u8,          // feeds mood-driven rotation magnitude
  lid_color_gene: u8, iris_color_gene: u8,
}
Brows {
  end_height1: Q32.32, mid_x: Q32.32, end_height2: Q32.32,
  tensions: [Q32.32; 3], width_coupling: Q32.32, clearance: Q32.32,
}
Lashes {
  count: u8, removed: u8, length: i64, stroke: i64, direction: u8,
}
Mouth {
  family: u8,                 // v1 families re-keyed 0..N from TOG's sparse ids (1,3..7,70,71,23,232,...)
  width_scale_gate: bool, width_scale: Q32.32,
  aspect_widen_gate: bool,
  params: [Q32.32; 8],        // family-specific curve/teeth/lip coefficients
}
FacePlacement {
  width_coeff: Q32.32, height_coeff: Q32.32,
}
```

- The mouth-rect **fitting search** (findRect) is renderer geometry (it
  needs the flattened body outline). The phenotype carries only the
  parameters the search consumes.
- TOG's sparse mouth ids (1, 3–7, 23, 70, 71, 232, 233, 2321, 2331–2333
  — historical accretion) are re-keyed to a dense `family` enum; the
  visual designs port, the numbering does not. No fallback-less gap
  either: family is decoded modulo the family count, every DNA value
  renders a mouth.

## Pattern

```
Pattern {
  class: u8,        // 0 none, 1 circler_top, 2 circler_bottom, 3 voronoi, 4 cut, 5 cut_flipped
  circler: { point_reduce: u8, size: Q32.32, y_offset_div: Q32.32,
             tension: Q32.32, pull_min: Q32.32 },
  voronoi: { index: u8, rotation: u16 /* BAM16 */, scale: Q32.32 },
  cut: { spread: Q32.32, y_adjust_div: Q32.32 },
}
```

All three sub-blocks always present (canonical padding when unselected)
— same rationale as dormant regions: byte-stable serialization.

## Animation

```
Animation {
  breath_period: u16,     // render ticks; renderer defines tick→time
  breath_amplitude: Q32.32,
  blink_interval: u16, blink_jitter_seed: u8,
  mood_transition_tempo: Q32.32,
}
```

Static renders ignore this block; animated surfaces consume it. The SVG
byte-identity constraint applies at fixed animation phase (goldens pin
phase 0 — the static pose).

## MoodResponse

Per-creature coefficients scaling how strongly the face reacts (001 §5):

```
MoodResponse {
  lid_reactivity: Q32.32, brow_reactivity: Q32.32,
  eye_rotation_reactivity: Q32.32, mouth_reactivity: Q32.32,
}
```

## Asymmetry / Layers (dormant v1)

```
Asymmetry { deltas: [Q32.32; 8] }   // decoded from DNA, rendered ×0 in v1
Layers { count: u8, params: [Q32.32; 16] }  // v1: count always decodes, renderer draws layer 0 only
```

## Open items

1. `Mouth.params: [Q32.32; 8]` is a placeholder width — the exact
   per-family parameter count gets fixed when the mouth families port
   (the array stays 8 wide; families document which slots they read).
2. The 2048-slot palette table: TOG's `svgConfig.json` colors blob is
   the source; needs one-time conversion + pinning with its own hash.
3. Eye noise (`feTurbulence` filter params in TOG) must be checked for
   cross-renderer byte-stability — filters are SVG-spec'd but the
   *rendered* result is rasterizer-dependent. Byte-identity of SVG text
   is unaffected; flagging so nobody assumes pixel-identity.
