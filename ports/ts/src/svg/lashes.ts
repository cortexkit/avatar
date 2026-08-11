// Eyelash sampling along the post-mood upper-lid Bézier (transcribed from
// crates/avatar-svg/src/lashes.rs).

import type { Lashes, Rgb } from "../phenotype";
import { cosQ30, divRoundHalfAway, sinQ30 } from "../tables";

import { Q32_ONE, q32Mul } from "./fixed";
import { formatMilli } from "./format";
import { hexColor } from "./colors";

/** One cubic curve in canonical milliunits. */
export interface Cubic {
  x0: bigint;
  y0: bigint;
  cp1x: bigint;
  cp1y: bigint;
  cp2x: bigint;
  cp2y: bigint;
  x3: bigint;
  y3: bigint;
}

export type EyeSide = "left" | "right";

/** face.tsx 707–759 samples at most indices 0..=10 and removes the tail arc. */
export function sampleCount(count: bigint, removed: bigint): number {
  if (count === 0n || removed > count) {
    return 0;
  }
  const kept = count - removed;
  return Number(kept < 10n ? kept : 10n) + 1;
}

/** Produces lashes after mood closure has shifted the upper-lid curve. */
export function renderLashes(
  genes: Lashes,
  curve: Cubic,
  eyeHeight: bigint,
  side: EyeSide,
  color: Rgb,
): string {
  const samples = sampleCount(genes.count, genes.removed);
  if (samples === 0) {
    return "";
  }

  const length = max(q32Mul(eyeHeight, genes.length_scale), 0n);
  const stroke = max(q32Mul(eyeHeight, genes.stroke_scale), 1n);
  const angularOffset = lashAngleOffset(genes.direction);
  let output = "";

  // face.tsx 707–759 derives a tangent angle and then adds or subtracts
  // π/direction. The BAM lookup keeps the construction deterministic.
  for (let index = 0; index < samples; index += 1) {
    const point = cubicPoint(curve, BigInt(index), genes.count);
    const tangent = cubicTangent(curve, BigInt(index), genes.count);
    const tangentAngle = atan2Bam(tangent[1], tangent[0]);
    // BAM16 angles wrap mod 65536; the mask reproduces Rust's
    // wrapping_sub/wrapping_add on u16.
    const angle =
      side === "left"
        ? (tangentAngle - angularOffset) & 0xffffn
        : (tangentAngle + angularOffset) & 0xffffn;
    const endX = point[0] + trigLength(length, cosQ30(angle));
    const endY = point[1] + trigLength(length, sinQ30(angle));
    const sign = side === "left" ? -1n : 1n;
    output += `    <path class="av-lash" d="M${formatMilli(point[0])} ${formatMilli(point[1])} C${formatMilli(point[0] + sign * stroke * 3n)} ${formatMilli(point[1] - stroke)}, ${formatMilli(endX + sign * stroke)} ${formatMilli(endY - stroke)}, ${formatMilli(endX + sign * stroke * 2n)} ${formatMilli(endY - stroke * 2n)}" stroke="${hexColor(color)}" stroke-width="${formatMilli(stroke)}" stroke-linecap="round" fill="${hexColor(color)}"/>\n`;
  }
  return output;
}

function lashAngleOffset(direction: bigint): bigint {
  const fraction = divRoundHalfAway(direction * Q32_ONE, 255n);
  // Normalizing the source byte to 0..1 makes this divisor span 1..2.
  const divisor = Q32_ONE + fraction;
  return divRoundHalfAway(32_768n * Q32_ONE, divisor) & 0xffffn;
}

/**
 * De Casteljau point evaluation at `t = numerator/denominator`. Exported
 * (though module-private in lashes.rs) so the port's unit tests can pin the
 * endpoint behavior directly.
 */
export function cubicPoint(
  curve: Cubic,
  numerator: bigint,
  denominator: bigint,
): [bigint, bigint] {
  const p01 = lerpPoint([curve.x0, curve.y0], [curve.cp1x, curve.cp1y], numerator, denominator);
  const p12 = lerpPoint([curve.cp1x, curve.cp1y], [curve.cp2x, curve.cp2y], numerator, denominator);
  const p23 = lerpPoint([curve.cp2x, curve.cp2y], [curve.x3, curve.y3], numerator, denominator);
  const p012 = lerpPoint(p01, p12, numerator, denominator);
  const p123 = lerpPoint(p12, p23, numerator, denominator);
  return lerpPoint(p012, p123, numerator, denominator);
}

function cubicTangent(curve: Cubic, numerator: bigint, denominator: bigint): [bigint, bigint] {
  const d0: [bigint, bigint] = [3n * (curve.cp1x - curve.x0), 3n * (curve.cp1y - curve.y0)];
  const d1: [bigint, bigint] = [3n * (curve.cp2x - curve.cp1x), 3n * (curve.cp2y - curve.cp1y)];
  const d2: [bigint, bigint] = [3n * (curve.x3 - curve.cp2x), 3n * (curve.y3 - curve.cp2y)];
  const d01 = lerpPoint(d0, d1, numerator, denominator);
  const d12 = lerpPoint(d1, d2, numerator, denominator);
  return lerpPoint(d01, d12, numerator, denominator);
}

function lerpPoint(
  a: [bigint, bigint],
  b: [bigint, bigint],
  numerator: bigint,
  denominator: bigint,
): [bigint, bigint] {
  return [
    a[0] + ratioMul(b[0] - a[0], numerator, denominator),
    a[1] + ratioMul(b[1] - a[1], numerator, denominator),
  ];
}

function ratioMul(value: bigint, numerator: bigint, denominator: bigint): bigint {
  if (denominator === 0n) {
    return 0n;
  }
  return divRoundHalfAway(value * numerator, denominator);
}

/**
 * Finds the nearest table-defined BAM direction by maximum dot product.
 *
 * The scan visits every fourth BAM16 angle exactly like lashes.rs's
 * `step_by(4)`, so the picked direction is always a sinQ30/cosQ30 table
 * sample and ties resolve to the lowest angle (strict `>` keeps the first).
 */
function atan2Bam(y: bigint, x: bigint): bigint {
  if (x === 0n && y === 0n) {
    return 0n;
  }
  let bestAngle = 0n;
  let bestDot: bigint | null = null;
  for (let raw = 0n; raw < 65_536n; raw += 4n) {
    const dot = x * cosQ30(raw) + y * sinQ30(raw);
    if (bestDot === null || dot > bestDot) {
      bestDot = dot;
      bestAngle = raw;
    }
  }
  return bestAngle;
}

function trigLength(length: bigint, trigQ30: bigint): bigint {
  return divRoundHalfAway(length * trigQ30, 1n << 30n);
}

function max(value: bigint, floor: bigint): bigint {
  return value > floor ? value : floor;
}
