# Design note 005 — Face pass and mood application v1

Status: DRAFT. Companion to note 004 (body pass). The face pass consumes
`(Phenotype.face, Palette, MoodResponse, BodyGeometry, Mood)` and emits
the face group appended inside the body's centering transform. The
reference is `face.tsx` (1,382 lines); this note pins structure, the
mood model, and application order — the per-element curve formulas are
transcribed from the reference under note-001 arithmetic during
implementation, with divergences recorded in the deviations list.

## Assembly order (z-order, back to front)

1. Face rect + mouth rect resolution (geometry only, nothing drawn)
2. Mouth (behind eyes — TOG draws mouth groups before eye groups)
3. Per eye (left, then right = mirrored transform):
   eyeball fill → iris → iris shade → pupil → highlights → lids → lashes
4. Brows (last, above everything)

## Face/mouth rect resolution

Transcribe `getFaceRect` + `findRect` (face.tsx 91–335) in integers:

- eye line anchor from `eyes.position_mode` over BodyGeometry landmarks
  (min_width_y / max_width_y / top+h/3 / top+h/2 / default max_width_y).
- face width/height from placement coefficients with the reference's
  min/max clamps (canvas/2.5, canvas/5, ratio 1.7 rule, eyesHeight cap).
- mouth search: `findRect` recursion becomes an explicit loop (the
  recursion is depth-bounded by the 3000-step counter; a loop with the
  same counter is the pinned form). Iteration order and the
  area > bodyW·bodyH/80 acceptance, widest-so-far fallback, and the
  centerY-reset retry are transcribed exactly — this search is the most
  behavior-critical part of the face (it decides whether a mouth fits
  at all, and TOG's quirks here are the personality of short bodies).
- The all-points input is BodyGeometry.all_points (the flatten samples).

## Mood model

`Mood` enum (note 001 §5): Neutral, Working, Happy, Waiting, Blocked,
Sad, Sleepy. TOG implemented 5 (neutral/angry/sad/sleepy/happy) as
scattered per-element conditionals; v1 centralizes them as a **mood
delta table** — one struct of deltas per mood, applied at defined
points:

```
MoodDeltas {
  upper_lid_close: Q32,   // adds to lid_close_upper_neutral, clamped [0,1]
  lower_lid_close: Q32,
  eye_rotation: Bam16,    // per-eye opposed rotation (left +, right −)
  eyeball_shift_x: Q32,   // fraction of eye width, mirrored per side
  eyeball_shift_y: Q32,   // fraction of eye height
  brow_rotation: Bam16,   // opposed like eyes
  brow_shift_y: Q32,      // fraction of face height
  mouth_curve: Q32,       // −1..1, family-interpreted
  lower_contour_raise: Q32, // happy smiling-eye raise
}
```

The 7 rows are spec data (a pinned table in the crate, values derived
from TOG's per-mood conditionals; Working/Waiting are new rows composed
from the same axes). Per-creature scaling: each delta multiplies the
matching `MoodResponse` reactivity coefficient before application
(pinned Q32 multiply). Neutral is the all-zero row, making
`render(neutral)` exactly the DNA face — an identity the goldens pin.

## Application order (pinned; TOG's implicit JSX nesting made explicit)

For each eye: (1) compute neutral eye geometry from DNA fields;
(2) apply mood lid closures (clamped adds); (3) apply eyeball shift;
(4) rotate eye group by eye_rotation about the eyeball center;
(5) lashes inherit the post-mood upper-lid curve and rotation;
(6) brows: neutral spline → mood rotation about brow center →
y shift. Mouth: family renderer receives mouth_curve and the
mouth rect; interpretation is per-family, pinned by the goldens.

## Mouth families v1

8 families (mouth.family mod 8) ported from the reference set, chosen
for visual distinctness at avatar sizes: TOG's Mouth1 (eye-anchored
line), 3 (simple curve), 4, 5 (teeth), 6, 7 (open oval), 23 (wide),
233 (lipped). The remaining TOG variants (70/71/232/2321/2331–2333) are
variations reachable later via a genome carve if wanted. Family
renderers share the rect/curve helpers; `mouth.params[0..8]` slots are
documented per family in the crate.

## Eye noise filter

TOG's iris shade uses feTurbulence displacement (dna2[5..8]). The SVG
text is byte-stable (attributes from phenotype ints) but rasterization
differs per engine — accepted (002 open item 3): byte identity is the
contract, pixel identity is not, and turbulence is the one element
where engines may differ visibly. Goldens still pin the bytes.

## Deviations from TOG (face)

1. Mood conditionals centralized into the delta table (same axes, same
   magnitudes, one application path).
2. Mouth family set reduced to 8 dense values (see above).
3. `console.log` scaffolding in getFaceRect: not ported.
4. `ab`/`debug`/`mouthFinder=false` prop modes: not ported — the
   fitting search always runs (TOG's default-off was a playground
   performance concession; we are not rendering 10k grids in React).
