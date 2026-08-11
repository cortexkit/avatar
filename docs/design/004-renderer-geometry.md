# Design note 004 — Renderer geometry v1 (body pass)

Status: DRAFT. Re-specifies TOG's body geometry pipeline (`type1.ts` +
`spline.ts`) under the note-001 numeric contract. Face geometry follows
in a later note; this one covers everything up to a drawn body: point
mirroring, spline construction, flattening, bounds, rescale, and path
serialization. The renderer consumes `(Phenotype, RenderParams)` and
never reads DNA.

## RenderParams (render-time inputs, not identity)

```
RenderParams {
  mood: Mood,           // note 001 §5
  growth: Q32.32,       // 0..1 fetus→adult morph; 1 for static renders
  canvas: u32,          // output viewBox size in centi-units; goldens pin 10_000
}
```

Internally all geometry runs on the 1_000_000-milliunit canonical
canvas; the canvas param only scales the final serialization (exact
÷100 for the pinned 10_000 viewBox).

## Stage 1 — mirror points (TOG `drawType`)

From `body.point_count` (N) and `body.points[0..N]` (half-side points,
already value-mapped by the decoder):

1. `max_y = max(points[i].y)` over the N points (i64 compare, exact).
2. Left side, i in 0..N:
   `x = center − points[i].x / 2` (pinned div), `y = points[i].y +
   center − max_y / 2`.
3. Right side mirrors indices N−1..=1... [TOG mirrors 1..=N of the
   reversed list]: for i in 1..=N−1 append
   `x = center + points[N−i].x / 2`, same y formula, same tension.
   (TOG iterates `i in 1..=numPoints` but slot N is the loop's own
   guard case; net effect is 2N−1... — v1 pins **2N points** with the
   duplicate-endpoint quirk dropped: the mirror emits exactly
   `left N + right N−1` points ONLY when TOG's `points[numPoints−i]`
   guard fires; we specify the unconditional 2N−1-point closed ring.
   The visual difference is nil — the spline closes the ring either
   way — and the count rule becomes stateable in one sentence.)
4. Growth morph (`growth < 1`): deferred to the animation note — v1
   goldens pin `growth = 1` and the renderer rejects other values
   until the morph is specced (fail-closed, not silently wrong).

## Stage 2 — closed spline (TOG `spline()`)

Catmull-Rom-style, tension-weighted, cubic Bézier output. For the
closed ring of M points (M = 2N−1), starting cursor at index M−2
(TOG's `points.length − 2` — preserved so curve/tension phase matches
the reference visually):

For each of M segments (window x0..x3 with wraparound):
```
cp1 = p1 + (p2 − p0) · t_k / 6
cp2 = p2 − (p3 − p1) · t_k / 6
```
where `t_k` is the tension of TOG's `tensionIter` walk (advances every
segment EXCEPT when segment index == M/2 with pinned integer division —
the reference's skip is kept: it phase-locks tensions to the mirrored
side). All products Q32.32 × milliunits via i128, one pinned narrow.

## Stage 3 — flattening + bounds

Each Bézier sampled at `t = j/20, j in 0..=20` (TOG's 20-step
`bezierPointSteps` for the body). `bezier_point` evaluated in integer
Horner form with pinned narrowing per multiply. Accumulate:

- `min_x, max_x, min_y, max_y` (+ the y at max-x: `max_width_y`, and at
  min-x: `min_bzx_y`) with TOG's comparison order (strict `<`/`>`, so
  first-seen wins ties — order-dependent and therefore specified).
- `all_points`: every sample, for the face pass (mouth fitting).
- `min_width_y` (TOG `getMinWidthY`): the 10-band scan over left-side
  samples, bands `top + i·h/10` for i in 1..9, pinned divisions;
  selects the y of the band with the largest left inset. Kept verbatim
  — face placement depends on its quirks.

## Stage 4 — rescale (TOG `rescale=true` path)

`margin = canvas/6`. Width/height ratios vs canvas computed per TOG's
branchy logic (all comparisons exact in milliunits); if the body
overflows, every curve point moves through `adjust_point(p, canvas,
ratio)` — re-specified: `half + (p − half) · 2^32 / ratio_q32` with
pinned rounding, handling the p<0 branch as in the reference.
`final_ratio` is Q32.32; the `wRatio > hRatio` select is exact.

## Stage 5 — SVG path serialization

```
M{x},{y} then M-1 × C{cp1x} {cp1y}, {cp2x} {cp2y}, {x} {y}
```
Coordinates: milliunits → centi-units (÷100 exact), formatted by the
001 §2.5 integer algorithm (trailing zeros stripped, `-0` → `0`).
Separator layout pinned exactly as TOG emits it (comma after M pair;
`C` groups comma-separated between pairs, space within pairs) — the
reference's exact string shape is the compatibility surface consumers
already parse, so we keep it rather than beautify.

## Body paint (TOG `bodylayer.tsx`), v1 scope

- Base fill `palette.primary`, stroke `palette.dark`, stroke-width
  canvas/255.
- Radial `gradox` gradient (89% primary → 100% primary_darkish) and the
  `feDiffuseLighting`+`feComposite` light filter: KEPT (they're what
  makes TOG bodies read as bodies), emitted with pinned attribute
  order/values.
- Pattern mask (`pattern.class` selecting circler/voronoi/cut per 002):
  KEPT, geometry from the pattern block through the same fixed-point
  helpers; voronoi paths come from a pinned data table (TOG's pre-baked
  `voronoiPatterns` strings, vendored as spec data with a hash pin).
- Element ids: TOG interpolates a per-instance `ssbdId` into SVG ids
  (`#bp{id}`, `#gradox{id}`). v1 replaces this with a fixed `av-` id
  prefix and NO instance number — the SVG is self-contained and
  byte-stable per (DNA, mood); embedders needing DOM-unique ids wrap or
  rewrite. This is a deliberate deviation from the reference, for byte
  determinism.

## Deviations from TOG (running list)

1. Mirror count rule: unconditional 2N−1 ring (§Stage 1).
2. Fetus-blend nearest-assignment bug not ported; growth morph respecced
   in the animation note.
3. Instance ids dropped from SVG ids (§Body paint).
4. All float accidents (string-split rounding, `(x−x)²` distance) gone
   per note 001 §2.
5. Tension `|| 1` guard dropped: TOG's JS definedness fallback turned
   tension 0 into 1; the typed phenotype has no undefined tensions and
   the decoder's range legitimately includes 0.
6. Tension-walk seam skip pinned at segment `(2N−1)/2` = N−1 — the same
   seam segment TOG skips on its even ring; for our odd ring the JS
   expression would never have skipped. Verified against TOG spline.ts
   via harness: max path-coordinate divergence 0.34 milliunits.
7. Stroke-width pinned to canvas/255. TOG disagrees with itself
   (bodylayer.tsx strokes at width/500, index.tsx margins at
   width/255); one value serves both roles here.
8. Circler pattern per-point pull jitter came from dna1 bytes in TOG —
   renderer-side DNA reads are forbidden, so v1 uses uniform pull from
   the pattern block; a per-point jitter field can join the PATTERN
   region under a future genome carve if the uniformity reads flat.

## Open for the face note (005)

- findRect mouth-fitting in integer form; face rect sizing; eye/brow/
  lash/mouth assembly; mood application order (lids → rotation →
  translation must be pinned — TOG applies them in JSX transform
  nesting order).
