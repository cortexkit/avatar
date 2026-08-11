// Body geometry: mirroring, spline construction, flattening, bounds,
// rescale, and SVG path serialization (transcribed from
// crates/avatar-svg/src/geometry.rs; design note 004, stages 1–5).
//
// Everything here is integer-only: bigint milliunit coordinates, Q32.32
// tensions, and divRoundHalfAway as the single narrowing primitive.

import type { Body, Phenotype } from "../phenotype";
import { divRoundHalfAway } from "../tables";

import { CANVAS_MILLI, CENTER, Q32_ONE, q32Ratio } from "./fixed";
import { formatMilli } from "./format";

/** Samples per Bézier segment when flattening the body outline. */
const FLATTEN_STEPS = 20n;

/**
 * Flattened body outline plus the derived metrics the paint pass and the
 * later face pass consume.
 *
 * All lengths are milliunits on the 1_000_000 canonical canvas. `allPoints`
 * holds every flattened sample in segment order (segment endpoints appear
 * twice: once as the previous segment's last sample, once as the next
 * segment's first), which the face pass needs for mouth fitting.
 */
export interface BodyGeometry {
  path: string;
  bodyWidth: bigint;
  bodyHeight: bigint;
  top: bigint;
  bottom: bigint;
  maxWidthY: bigint;
  minWidthY: bigint;
  allPoints: Array<[x: bigint, y: bigint]>;
}

/** Computes the body outline for a phenotype (growth = 1 adult shape). */
export function bodyGeometry(phenotype: Phenotype): BodyGeometry {
  const ring = mirrorRing(phenotype.body);
  const spline = buildSpline(ring);
  const bounds = flatten(spline);
  const bodyWidth = rescale(spline, bounds);
  const top = bounds.minY;
  const bottom = bounds.maxY;
  const bodyHeight = bottom - top;
  return {
    path: pathString(spline),
    bodyWidth,
    bodyHeight,
    top,
    bottom,
    maxWidthY: bounds.maxWidthY,
    minWidthY: minWidthY(bounds.samples, top, bodyHeight),
    allPoints: bounds.samples,
  };
}

/** One point of the mirrored closed ring, carrying its spline tension. */
export interface RingPoint {
  x: bigint;
  y: bigint;
  tension: bigint;
}

/**
 * Stage 1 — mirrors the half-side control points into a closed ring.
 *
 * `count` half-side points become a `2·count` ring: the left side walks
 * points `0..count` at `center − x/2`, the right side walks the same points
 * in reverse (`count−1` down to `0`) at `center + x/2`, matching the mirror
 * loop in TOG's `type1.ts`. Every half-side point appears exactly once per
 * side, which is what makes the closed shape horizontally symmetric — a
 * ring that dropped the reversed index 0 would leave that point unpaired
 * and skew the whole body.
 */
export function mirrorRing(body: Body): RingPoint[] {
  // The decoder contract is 2..=16 half-side points; clamping keeps the
  // renderer total for hand-built phenotypes outside that range.
  const count = clampNumber(body.point_count, 2, 16);
  const points = body.points.slice(0, count);
  // Rust's `.max().unwrap_or(0)`: the maximum over the points, not a
  // floor at zero — negative y values keep their own maximum.
  let maxY: bigint | null = null;
  for (const point of points) {
    if (maxY === null || point.y > maxY) maxY = point.y;
  }
  const verticalShift = CENTER - div2(maxY ?? 0n);

  const ring: RingPoint[] = [];
  for (const point of points) {
    ring.push({
      x: CENTER - div2(point.x),
      y: point.y + verticalShift,
      tension: point.tension,
    });
  }
  for (let index = 1; index <= count; index += 1) {
    const point = points[count - index];
    ring.push({
      x: CENTER + div2(point.x),
      y: point.y + verticalShift,
      tension: point.tension,
    });
  }
  return ring;
}

/** The pinned ÷2 used by the mirror: half away from zero, never native `/`. */
function div2(value: bigint): bigint {
  return divRoundHalfAway(value, 2n);
}

function clampNumber(value: bigint, lo: number, hi: number): number {
  return Math.min(Math.max(Number(value), lo), hi);
}

/** One cubic Bézier segment of the spline (control points + endpoint). */
export interface Curve {
  cp1x: bigint;
  cp1y: bigint;
  cp2x: bigint;
  cp2y: bigint;
  x2: bigint;
  y2: bigint;
}

/** A closed spline: its start point plus one Bézier segment per ring point. */
export interface Spline {
  startX: bigint;
  startY: bigint;
  curves: Curve[];
}

/**
 * Stage 2 — turns the closed ring into cubic Béziers.
 *
 * Catmull-Rom-style with per-point tensions. The cursor starts at
 * `len − 2` (the phase TOG's `spline.ts` uses, kept so curve and tension
 * alignment match the mirrored ring), and each segment reads a four-point
 * window with wraparound. `spline.ts`'s final-window guard
 * (`i == points.length + 3`) can never fire for a window walk of `len`
 * segments; its effect is plain wraparound, which is what happens here.
 *
 * Control points are `p1 + (p2 − p0)·t/6` and `p2 − (p3 − p1)·t/6`. The
 * milliunit delta × Q32 tension product is divided by `6·2^32` in ONE
 * divRoundHalfAway call per coordinate — a single rounding, not a
 * ÷6-then-×t chain.
 */
export function buildSpline(ring: RingPoint[]): Spline {
  const len = ring.length;
  let tensionIter = 0;
  const start = ring[len - 1];
  const spline: Spline = {
    startX: start.x,
    startY: start.y,
    curves: [],
  };

  for (let segment = 0; segment < len; segment += 1) {
    const cursor = len - 2 + segment;
    const p0 = ring[cursor % len];
    const p1 = ring[(cursor + 1) % len];
    const p2 = ring[(cursor + 2) % len];
    const p3 = ring[(cursor + 3) % len];
    const tension = ring[tensionIter].tension;
    spline.curves.push({
      cp1x: p1.x + tensionOffset(p2.x - p0.x, tension),
      cp1y: p1.y + tensionOffset(p2.y - p0.y, tension),
      cp2x: p2.x - tensionOffset(p3.x - p1.x, tension),
      cp2y: p2.y - tensionOffset(p3.y - p1.y, tension),
      x2: p2.x,
      y2: p2.y,
    });
    // The tension walk advances every segment except the seam segment
    // (`len / 2`, integer division): skipping there phase-locks the
    // tensions of the two segments that straddle the mirrored side.
    if (segment !== Math.floor(len / 2)) {
      tensionIter += 1;
    }
  }
  return spline;
}

/** `delta · tension / (6 · 2^32)` as one pinned narrowing. */
function tensionOffset(delta: bigint, tension: bigint): bigint {
  return divRoundHalfAway(delta * tension, 6n * Q32_ONE);
}

/** Outline extremes plus every flattened sample. */
export interface Bounds {
  minX: bigint;
  minXy: bigint;
  maxX: bigint;
  minY: bigint;
  maxY: bigint;
  maxWidthY: bigint;
  samples: Array<[x: bigint, y: bigint]>;
}

/** Sentinel standing in for Rust's i64::MAX initial extreme. */
const I64_MAX = (1n << 63n) - 1n;

/**
 * Stage 3 — flattens every segment at `t = j/20, j in 0..=20` and tracks
 * the outline extremes.
 *
 * The comparison chain of `spline.ts`'s `getBezierTipPoint` is transcribed
 * with its exact tie behavior: strict `<`/`>` in an else-if chain, so the
 * FIRST sample at a tied extreme keeps its companion y. The ordering is
 * observable (it selects `maxWidthY` / `minXy`) and therefore specified,
 * not incidental.
 */
export function flatten(spline: Spline): Bounds {
  const bounds: Bounds = {
    minX: I64_MAX,
    minXy: 0n,
    maxX: 0n,
    minY: I64_MAX,
    maxY: 0n,
    maxWidthY: 0n,
    samples: [],
  };
  let anchorX = spline.startX;
  let anchorY = spline.startY;
  for (const curve of spline.curves) {
    for (let step = 0n; step <= FLATTEN_STEPS; step += 1n) {
      const x = bezierSample(anchorX, curve.cp1x, curve.cp2x, curve.x2, step);
      const y = bezierSample(anchorY, curve.cp1y, curve.cp2y, curve.y2, step);
      if (x < bounds.minX) {
        bounds.minX = x;
        bounds.minXy = y;
      } else if (x > bounds.maxX) {
        bounds.maxX = x;
        bounds.maxWidthY = y;
      }
      if (y < bounds.minY) {
        bounds.minY = y;
      } else if (y > bounds.maxY) {
        bounds.maxY = y;
      }
      bounds.samples.push([x, y]);
    }
    anchorX = curve.x2;
    anchorY = curve.y2;
  }
  return bounds;
}

/**
 * Integer De Casteljau evaluation of one cubic at `t = step/FLATTEN_STEPS`.
 *
 * Three lerp levels, each `a + (b − a)·step/20` with one pinned rounding,
 * staying in bigint milliunits. This is the v1 specification for curve
 * sampling; it replaces the float Bernstein polynomial of TOG's
 * `math.ts` (`bezierPoint`). The endpoints (`step` 0 and 20) evaluate
 * exactly, so consecutive segments share their junction sample
 * bit-for-bit.
 *
 * Exported (though module-private in geometry.rs) so the port's unit tests
 * can pin the sampled arithmetic directly.
 */
export function bezierSample(p0: bigint, p1: bigint, p2: bigint, p3: bigint, step: bigint): bigint {
  const q0 = lerpStep(p0, p1, step);
  const q1 = lerpStep(p1, p2, step);
  const q2 = lerpStep(p2, p3, step);
  const r0 = lerpStep(q0, q1, step);
  const r1 = lerpStep(q1, q2, step);
  return lerpStep(r0, r1, step);
}

function lerpStep(a: bigint, b: bigint, step: bigint): bigint {
  return a + divRoundHalfAway((b - a) * step, FLATTEN_STEPS);
}

/**
 * Stage 4 — pulls an overflowing body back inside the canvas.
 *
 * Width and height extents are measured against the canvas with a
 * `canvas/6` margin (the branchy logic of `spline.ts`, exact bigint
 * compares); if the larger ratio exceeds 1, every coordinate moves through
 * `adjustPoint` toward the canvas center by `1/ratio`. Returns the final
 * body width.
 */
export function rescale(spline: Spline, bounds: Bounds): bigint {
  const margin = divRoundHalfAway(CANVAS_MILLI, 6n);

  let maxWidth = bounds.maxX - bounds.minX;
  if (bounds.minX - margin < 0n) {
    maxWidth =
      bounds.minX < 0n ? -bounds.minX + bounds.maxX + margin : bounds.maxX - bounds.minX + margin;
  }

  // Height must be checked against both canvas edges: a body can overflow
  // the top, the bottom, or both, and the larger demand wins.
  let maxHeight1 = bounds.maxY - bounds.minY;
  let maxHeight2 = bounds.maxY - bounds.minY;
  if (bounds.minY - margin < 0n) {
    maxHeight1 =
      bounds.minY < 0n
        ? 2n * (-bounds.minY + CENTER) + margin
        : 2n * (CENTER - bounds.minY) + margin;
  }
  if (bounds.maxY + margin > CANVAS_MILLI) {
    maxHeight2 = 2n * (bounds.maxY - CENTER) + margin;
  }
  const maxHeight = maxHeight1 > maxHeight2 ? maxHeight1 : maxHeight2;

  const widthRatio = q32Ratio(maxWidth);
  const heightRatio = q32Ratio(maxHeight);
  const finalRatio = widthRatio > heightRatio ? widthRatio : heightRatio;

  if (finalRatio > Q32_ONE) {
    spline.startX = adjustPoint(spline.startX, finalRatio);
    spline.startY = adjustPoint(spline.startY, finalRatio);
    for (const curve of spline.curves) {
      curve.cp1x = adjustPoint(curve.cp1x, finalRatio);
      curve.cp1y = adjustPoint(curve.cp1y, finalRatio);
      curve.cp2x = adjustPoint(curve.cp2x, finalRatio);
      curve.cp2y = adjustPoint(curve.cp2y, finalRatio);
      curve.x2 = adjustPoint(curve.x2, finalRatio);
      curve.y2 = adjustPoint(curve.y2, finalRatio);
    }
    bounds.maxWidthY = adjustPoint(bounds.maxWidthY, finalRatio);
    bounds.minX = adjustPoint(bounds.minX, finalRatio);
    bounds.minXy = adjustPoint(bounds.minXy, finalRatio);
    bounds.minY = adjustPoint(bounds.minY, finalRatio);
    bounds.maxY = adjustPoint(bounds.maxY, finalRatio);
    maxWidth = divRoundHalfAway(maxWidth * Q32_ONE, finalRatio);
    for (let index = 0; index < bounds.samples.length; index += 1) {
      const [x, y] = bounds.samples[index];
      bounds.samples[index] = [adjustPoint(x, finalRatio), adjustPoint(y, finalRatio)];
    }
  }
  return maxWidth;
}

/**
 * Moves one coordinate toward the canvas center by `1/ratio`.
 *
 * The three branches cover `p` above the center, below it but positive, and
 * non-positive (mirrored coordinates can cross zero); each scaled offset is
 * one pinned division. Exported (though module-private in geometry.rs) so
 * the port's unit tests can pin each branch directly.
 */
export function adjustPoint(p: bigint, ratio: bigint): bigint {
  const scaled = (delta: bigint) => divRoundHalfAway(delta * Q32_ONE, ratio);
  if (p > CENTER) {
    return CENTER + scaled(p - CENTER);
  }
  if (p > 0n) {
    return CENTER - scaled(CENTER - p);
  }
  if (p < 0n) {
    return CENTER - scaled(-p + CENTER);
  }
  return p;
}

/**
 * The y of the band with the largest left-side inset.
 *
 * Transcribed from `spline.ts`'s `getMinWidthY` 10-band scan because face
 * placement depends on its exact behavior: bands
 * `top + i·h/10 < y ≤ top + (i+1)·h/10` for `i in 1..9` over left-side
 * samples (`x < center`), selecting the midpoint y of the band whose
 * minimum x is largest. An empty band keeps the i64-max sentinel as its
 * minimum and therefore outranks every real inset — a quirk preserved
 * verbatim.
 */
export function minWidthY(samples: ReadonlyArray<[bigint, bigint]>, top: bigint, height: bigint): bigint {
  let bestInsetX = 0n;
  let bestY = 0n;
  for (let band = 1; band < 9; band += 1) {
    const bandTop = top + heightTenths(height, BigInt(band));
    const bandBottom = top + heightTenths(height, BigInt(band + 1));
    let minXinBand = I64_MAX;
    for (const [x, y] of samples) {
      if (x < CENTER && y > bandTop && y <= bandBottom && x < minXinBand) {
        minXinBand = x;
      }
    }
    if (minXinBand > bestInsetX) {
      bestInsetX = minXinBand;
      bestY = bandTop + heightTwentieth(height);
    }
  }
  return bestY;
}

function heightTenths(height: bigint, count: bigint): bigint {
  return divRoundHalfAway(height * count, 10n);
}

function heightTwentieth(height: bigint): bigint {
  return divRoundHalfAway(height, 20n);
}

/**
 * Stage 5 — serializes the spline as an SVG path.
 *
 * Layout is pinned to the exact string shape `spline.ts` emits (comma after
 * the `M` pair; `C` groups comma-separated between pairs, space within
 * pairs): consumers already parse that surface, so it is kept rather than
 * beautified.
 */
export function pathString(spline: Spline): string {
  let path = `M${formatMilli(spline.startX)},${formatMilli(spline.startY)}`;
  for (const curve of spline.curves) {
    path += `C${formatMilli(curve.cp1x)} ${formatMilli(curve.cp1y)}, ${formatMilli(curve.cp2x)} ${formatMilli(curve.cp2y)}, ${formatMilli(curve.x2)} ${formatMilli(curve.y2)}`;
  }
  return path;
}
