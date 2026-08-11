// Body-pattern mask geometry (transcribed from crates/avatar-svg/src/pattern.rs;
// design note 004 §Body paint).
//
// Pattern classes select the mask content: 0 none, 1 circler above the body,
// 2 circler below, 3 voronoi, 4 cutting Bézier, 5 cutting Bézier flipped.
// Where the historical reference (`pattern.tsx`) read DNA bytes through its
// `uint8*Range` helpers, this renderer reads the decoded phenotype pattern
// block instead — a renderer never sees DNA. The field correspondence:
//
// - `circler.point_reduce` (raw byte) ↔ `uint8InQuintRange(features[10], 0, 8)`:
//   the decoder keeps the byte raw, so the in-quint easing table is applied
//   here to get the point-count reduction 0..=8.
// - `circler.size` ↔ the linear factor of `uint8Range(features[11], 1.2h, 2h)`.
// - `circler.y_offset_div` ↔ `uint8Range(features[12], 2, 3)`, the divisor of
//   the vertical offset.
// - `circler.tension` ↔ `uint8InQuintRange(dna2[8], 0, 0.5)`, the amount
//   subtracted from 1 (final tension = 1 − t/2).
// - `circler.pull_min` ↔ `uint8InOutCircRange(dna2[11], 0.85, 1)`, the radius
//   pull lower bound. The reference jittered each point's pull from a DNA
//   byte; that jitter is dropped (it is DNA, not phenotype), so the pull is
//   uniform at the mapped bound.
// - `voronoi.index` ↔ `uint8Range(features[9], 0, len−1)`.
// - `voronoi.rotation` (BAM16) ↔ the rotation in degrees.
// - `voronoi.scale` ↔ `uint8Range(dna2[9], 0.1, 2.5)`.
// - `cut.spread` ↔ `uint8ExpoRange(features[13], 0.5, 10)` and
//   `cut.y_adjust_div` ↔ `uint8Range(features[14], 2, 100)`: the decoder
//   already applied the easing tables for these Q32 fields, so the renderer
//   only lerps them into `pattern.tsx`'s target ranges.

import { blake3 } from "@noble/hashes/blake3";
import { bytesToHex } from "@noble/hashes/utils";

import type { Circler, Cut, Pattern, Voronoi } from "../phenotype";
import { cosQ30, divRoundHalfAway, EASE_IN_QUINT, sinQ30 } from "../tables";

import {
  CANVAS_MILLI,
  CENTER,
  lerpQ32Milli,
  Q32_ONE,
  q32Mul,
  q32Ratio,
} from "./fixed";
import { formatMilli, formatQ32Decimal } from "./format";
import { buildSpline, pathString, type RingPoint } from "./geometry";
import { VORONOI_PATTERNS } from "../generated/voronoi";

/** Q32.32 constants for `pattern.tsx`'s range endpoints, half-away rounded. */
const Q32_POINT_ONE = 429_496_730n; // 0.1
const Q32_POINT_EIGHTY_FIVE = 3_650_722_202n; // 0.85
const Q32_TWO_POINT_FIVE = 10_737_418_240n; // 2.5 (exact)

/**
 * Builds the mask content for a pattern, or `null` when the class selects
 * no pattern (class 0, or a value outside the decoded 0..=5 range).
 *
 * The returned fragment keeps the nesting `bodylayer.tsx` and `pattern.tsx`
 * emit: the body layer's mask `<g>`, then the pattern's translate group,
 * then the scale/rotate group holding the `#efefef` path.
 */
export function maskContent(
  pattern: Pattern,
  bodyWidth: bigint,
  bodyHeight: bigint,
  top: bigint,
): string | null {
  let path: string;
  let tx = 0n;
  let ty = 0n;
  let scale = Q32_ONE;
  let rotate = 0n;
  // Classes 1/2/4/5 rotate about the canvas center; the voronoi class
  // rotates about the center of its own scaled pattern canvas.
  let rotateCenter = CENTER;

  switch (pattern.class) {
    case 1n:
      path = circlerPath(pattern.circler, true, bodyHeight, top);
      break;
    case 2n:
      path = circlerPath(pattern.circler, false, bodyHeight, top);
      break;
    case 3n:
      [path, tx, ty, scale, rotate, rotateCenter] = voronoiPlacement(
        pattern.voronoi,
        bodyWidth,
        bodyHeight,
      );
      break;
    case 4n:
      path = cuttingBezierPath(pattern.cut, bodyHeight, top);
      break;
    case 5n:
      path = cuttingBezierPath(pattern.cut, bodyHeight, top);
      rotate = 180n;
      break;
    default:
      return null;
  }
  return `<g><g transform="translate(${formatMilli(tx)}, ${formatMilli(ty)})"><g transform="scale(${formatQ32Decimal(scale)} ${formatQ32Decimal(scale)}) rotate(${rotate} ${formatMilli(rotateCenter)} ${formatMilli(rotateCenter)})"><path d="${path}" fill="#efefef"/></g></g></g>`;
}

/**
 * Circler classes 1/2: a ring of `2·numPoints` bumps above (class 1) or
 * below (class 2) the body, splined into a closed path without flattening
 * or rescale (`pattern.tsx` calls `spline` in its `simpleSpline` mode).
 */
export function circlerPath(
  circler: Circler,
  aboveBody: boolean,
  bodyHeight: bigint,
  top: bigint,
): string {
  const reduction = divRoundHalfAway(8n * EASE_IN_QUINT[Number(circler.point_reduce)], Q32_ONE);
  const numPoints = 16n - reduction;

  // size = lerp(1.2·h, 2·h, size); 1.2 = 6/5 via one pinned division.
  const sizeLo = divRoundHalfAway(6n * bodyHeight, 5n);
  const size = lerpQ32Milli(sizeLo, 2n * bodyHeight, circler.size);

  const divisor = lerpQ32Milli(2n * Q32_ONE, 3n * Q32_ONE, circler.y_offset_div);
  const yOffset = divRoundHalfAway(bodyHeight * Q32_ONE, divisor);
  const position = aboveBody ? top - size + yOffset : top + size + yOffset;

  const tension = Q32_ONE - divRoundHalfAway(circler.tension, 2n);
  const pull = lerpQ32Milli(Q32_POINT_EIGHTY_FIVE, Q32_ONE, circler.pull_min);
  // The radius is shared by x and y exactly as `pattern.tsx` shares its
  // `size * pull` product before the separate cos/sin multiplications.
  const radius = q32Mul(size, pull);

  // BAM16 angle step for a full turn over 2·numPoints points; the +π/2
  // phase `pattern.tsx` applies is BAM16 16384. u16 arithmetic wraps
  // mod 65536, reproduced by the mask.
  const angleStep = divRoundHalfAway(65_536n, 2n * numPoints) & 0xffffn;
  const count = Number(2n * numPoints);
  const ring: RingPoint[] = [];
  for (let index = 1; index <= count; index += 1) {
    const angle = (BigInt(index) * angleStep + 16_384n) & 0xffffn;
    ring.push({
      x: CENTER + trigOffset(radius, cosQ30(angle)),
      y: position + trigOffset(radius, sinQ30(angle)),
      tension,
    });
  }
  return pathString(buildSpline(ring));
}

/** Scales a Q1.30 trig value against a milliunit radius, one pinned narrowing. */
function trigOffset(radius: bigint, trigQ30: bigint): bigint {
  return divRoundHalfAway(radius * trigQ30, 1n << 30n);
}

/**
 * Voronoi class 3: a vendored path string with the scale/rotate/translate
 * fit `pattern.tsx` applies, re-expressed in fixed point.
 *
 * Returns `[path, tx, ty, scale_q32, rotateDegrees, rotateCenter]` with
 * tx/ty in milliunits and the scale as Q32.
 */
export function voronoiPlacement(
  voronoi: Voronoi,
  bodyWidth: bigint,
  bodyHeight: bigint,
): [string, bigint, bigint, bigint, bigint, bigint] {
  const index = Number(divRoundHalfAway(voronoi.index * BigInt(VORONOI_PATTERNS.length - 1), 255n));

  const baseScale = lerpQ32Milli(Q32_POINT_ONE, Q32_TWO_POINT_FIVE, voronoi.scale);
  let scale = baseScale;
  const widthRatio = q32Ratio(bodyWidth);
  if (widthRatio > scale) scale = widthRatio;
  const heightRatio = q32Ratio(bodyHeight);
  if (heightRatio > scale) scale = heightRatio;

  // Translate centers the scaled pattern canvas; both divisions fold into
  // one pinned narrowing each.
  const scaledCanvas = q32Mul(CANVAS_MILLI, scale);
  const tx = divRoundHalfAway((CANVAS_MILLI - scaledCanvas) * Q32_ONE, 2n * scale);
  const ty = tx - divRoundHalfAway(scaledCanvas, 2n);

  // `pattern.tsx` multiplies the scale by canvas/255 AFTER deriving the
  // translate; the order is visible in the output and kept.
  const strokeUnit = divRoundHalfAway(CANVAS_MILLI, 255n);
  const finalScale = q32Mul(scale, strokeUnit);
  const rotateCenter = divRoundHalfAway(CANVAS_MILLI * Q32_ONE, 2n * finalScale);
  const degrees = divRoundHalfAway(voronoi.rotation * 360n, 65_536n);

  return [VORONOI_PATTERNS[index], tx, ty, finalScale, degrees, rotateCenter];
}

/**
 * Cut classes 4/5: a horizontal Bézier band across the body. Class 5 is the
 * same path rotated 180° by the mask transform.
 */
export function cuttingBezierPath(cut: Cut, bodyHeight: bigint, top: bigint): string {
  const spread = lerpQ32Milli(Q32_ONE / 2n, 10n * Q32_ONE, cut.spread);
  const lr = q32Mul(CANVAS_MILLI, spread);
  const divisor = lerpQ32Milli(2n * Q32_ONE, 100n * Q32_ONE, cut.y_adjust_div);
  const yAdjust = divRoundHalfAway(bodyHeight * Q32_ONE, divisor);

  const yBottom = top + bodyHeight + yAdjust;
  const yTop = top + yAdjust;
  const yMid = top + lr + yAdjust;
  // String shape is pinned verbatim to `pattern.tsx`, including the space
  // after the M pair and the missing space before the second C.
  return `M 0, ${formatMilli(yBottom)} C ${formatMilli(CENTER - lr)} ${formatMilli(yBottom)}, ${formatMilli(CENTER)} ${formatMilli(yMid)}, ${formatMilli(CENTER)} ${formatMilli(yTop)}C ${formatMilli(CENTER)} ${formatMilli(yMid)}, ${formatMilli(CENTER + lr)} ${formatMilli(yBottom)}, ${formatMilli(CANVAS_MILLI)} ${formatMilli(yBottom)}Z`;
}

const encoder = new TextEncoder();

/**
 * blake3 pin of the vendored voronoi table, computed exactly like the Rust
 * crate's `voronoi_table_hash_is_pinned` test (avatar-svg/src/lib.rs):
 * each pattern's UTF-8 bytes followed by a 0x1f unit separator, so the
 * string bounds are pinned, not just the concatenated blob.
 */
export function voronoiPatternsHash(): string {
  const hasher = blake3.create();
  for (const pattern of VORONOI_PATTERNS) {
    hasher.update(encoder.encode(pattern));
    hasher.update(new Uint8Array([0x1f]));
  }
  return bytesToHex(hasher.digest());
}
