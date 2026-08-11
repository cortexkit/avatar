//! Shared fixed-point helpers for the renderer.
//!
//! All geometry runs in i64 canvas milliunits (the canonical canvas is
//! 1_000_000 milliunits wide) with Q32.32 factors where a quantity needs
//! fractions. Every product widens to i128 before narrowing through
//! `avatar_tables::div_round_half_away`, the single pinned division primitive
//! shared by every port.

use avatar_phenotype::{Bam16, Q32};
use avatar_tables::div_round_half_away;

/// The canvas edge length in milliunits (10_000 viewBox units × 100).
pub(crate) const CANVAS_MILLI: i64 = 1_000_000;

/// The canvas center coordinate in milliunits.
pub(crate) const CENTER: i64 = CANVAS_MILLI / 2;

/// Q32.32 1.0, cached as a plain i64 for arithmetic contexts.
pub(crate) const Q32_ONE: i64 = Q32::ONE.0;

/// Multiplies a milliunit value by a Q32 factor, one pinned narrowing.
pub(crate) fn q32_mul(value: i64, factor: i64) -> i64 {
    div_round_half_away(i128::from(value) * i128::from(factor), i128::from(Q32_ONE)) as i64
}

/// Linear interpolation between milliunit endpoints by a Q32 factor.
///
/// `factor` may exceed 1.0 (callers use it for ranges such as 0.5..10), so
/// this is extrapolation-safe by construction.
pub(crate) fn lerp_q32_milli(lo: i64, hi: i64, factor: i64) -> i64 {
    lo + q32_mul(hi - lo, factor)
}

/// Expresses a milliunit dimension as a ratio of the canvas edge, in Q32.
pub(crate) fn q32_ratio(dimension: i64) -> i64 {
    div_round_half_away(
        i128::from(dimension) * i128::from(Q32_ONE),
        i128::from(CANVAS_MILLI),
    ) as i64
}

/// Divides signed milliunits with the renderer's one rounding rule.
pub(crate) fn div_i64(value: i64, divisor: i64) -> i64 {
    assert_ne!(divisor, 0, "div_i64 requires a nonzero denominator");
    let sign = if divisor < 0 { -1 } else { 1 };
    div_round_half_away(
        i128::from(value) * i128::from(sign),
        i128::from(divisor.abs()),
    ) as i64
}

/// Expresses `numerator / denominator` as Q32.32.
pub(crate) fn ratio_q32(numerator: i64, denominator: i64) -> i64 {
    if denominator == 0 {
        return 0;
    }
    let sign = if denominator < 0 { -1 } else { 1 };
    div_round_half_away(
        i128::from(numerator) * i128::from(Q32_ONE) * i128::from(sign),
        i128::from(denominator.abs()),
    ) as i64
}

/// Converts wrapped signed BAM16 into Q32.32 SVG degrees.
pub(crate) fn bam_degrees_q32(angle: Bam16) -> i64 {
    let signed = i64::from(angle.0 as i16);
    div_round_half_away(i128::from(signed) * 360 * i128::from(Q32_ONE), 65_536) as i64
}

/// Reverses a signed BAM16 rotation without changing its wrapped encoding.
pub(crate) fn opposite_bam(angle: Bam16) -> Bam16 {
    Bam16((-i32::from(angle.0 as i16)) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q32_mul_rounds_half_away_from_zero() {
        assert_eq!(q32_mul(3, Q32_ONE / 2), 2);
        assert_eq!(q32_mul(-3, Q32_ONE / 2), -2);
        assert_eq!(q32_mul(100, Q32_ONE), 100);
    }

    #[test]
    fn lerp_hits_endpoints_exactly() {
        assert_eq!(lerp_q32_milli(10, 20, 0), 10);
        assert_eq!(lerp_q32_milli(10, 20, Q32_ONE), 20);
        assert_eq!(lerp_q32_milli(0, 100, Q32_ONE / 2), 50);
    }

    #[test]
    fn ratio_of_full_canvas_is_one() {
        assert_eq!(q32_ratio(CANVAS_MILLI), Q32_ONE);
        assert_eq!(q32_ratio(CANVAS_MILLI / 2), Q32_ONE / 2);
    }
}
