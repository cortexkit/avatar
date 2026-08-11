// Shared fixed-point helpers for the renderer (transcribed from
// crates/avatar-svg/src/fixed.rs).
//
// All geometry runs in bigint milliunits (the canonical canvas is 1_000_000
// milliunits wide) with Q32.32 factors where a quantity needs fractions.
// Every product narrows through divRoundHalfAway, the single pinned division
// primitive shared by every port.

import type { Bam16, Q32 } from "../phenotype";
import { divRoundHalfAway } from "../tables";

/** The canvas edge length in milliunits (10_000 viewBox units × 100). */
export const CANVAS_MILLI = 1_000_000n;

/** The canvas center coordinate in milliunits. */
export const CENTER = CANVAS_MILLI / 2n;

/** Q32.32 1.0. */
export const Q32_ONE = 1n << 32n;

/** Multiplies a milliunit value by a Q32 factor, one pinned narrowing. */
export function q32Mul(value: bigint, factor: bigint): bigint {
  return divRoundHalfAway(value * factor, Q32_ONE);
}

/**
 * Linear interpolation between milliunit endpoints by a Q32 factor.
 *
 * `factor` may exceed 1.0 (callers use it for ranges such as 0.5..10), so
 * this is extrapolation-safe by construction.
 */
export function lerpQ32Milli(lo: bigint, hi: bigint, factor: bigint): bigint {
  return lo + q32Mul(hi - lo, factor);
}

/** Expresses a milliunit dimension as a ratio of the canvas edge, in Q32. */
export function q32Ratio(dimension: bigint): Q32 {
  return divRoundHalfAway(dimension * Q32_ONE, CANVAS_MILLI);
}

/** Divides signed milliunits with the renderer's one rounding rule. */
export function divI64(value: bigint, divisor: bigint): bigint {
  if (divisor === 0n) {
    throw new RangeError("divI64 requires a nonzero denominator");
  }
  const sign = divisor < 0n ? -1n : 1n;
  return divRoundHalfAway(value * sign, divisor < 0n ? -divisor : divisor);
}

/** Expresses `numerator / denominator` as Q32.32. */
export function ratioQ32(numerator: bigint, denominator: bigint): Q32 {
  if (denominator === 0n) {
    return 0n;
  }
  const sign = denominator < 0n ? -1n : 1n;
  return divRoundHalfAway(numerator * Q32_ONE * sign, denominator < 0n ? -denominator : denominator);
}

/** Interprets a wrapped BAM16 word as its signed value (two's complement). */
export function signedBam16(angle: Bam16): bigint {
  const wrapped = angle & 0xffffn;
  return wrapped >= 0x8000n ? wrapped - 0x10000n : wrapped;
}

/** Converts wrapped signed BAM16 into Q32.32 SVG degrees. */
export function bamDegreesQ32(angle: Bam16): Q32 {
  return divRoundHalfAway(signedBam16(angle) * 360n * Q32_ONE, 65_536n);
}

/** Reverses a signed BAM16 rotation without changing its wrapped encoding. */
export function oppositeBam(angle: Bam16): Bam16 {
  return (-signedBam16(angle)) & 0xffffn;
}
