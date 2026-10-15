//! Deterministic trig for wild geometry. To keep visual previews faithful to
//! shipped geometry, playground/web/index.html must mirror the sine coefficients,
//! evaluation order, and half-away-from-zero argument reduction used here.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

/// Sine for the bounded angles used by wild geometry, without platform libm.
pub fn sin(angle: f64) -> f64 {
    let x = angle - (angle / TAU).round() * TAU;
    let x2 = x * x;
    // Taylor through degree 25 on [-pi, pi]; the first omitted term is less
    // than 3e-15. Horner evaluation fixes the operation order for every port.
    let mut p = 1.0 / 15_511_210_043_330_985_984_000_000.0;
    p = -1.0 / 25_852_016_738_884_976_640_000.0 + x2 * p;
    p = 1.0 / 51_090_942_171_709_440_000.0 + x2 * p;
    p = -1.0 / 121_645_100_408_832_000.0 + x2 * p;
    p = 1.0 / 355_687_428_096_000.0 + x2 * p;
    p = -1.0 / 1_307_674_368_000.0 + x2 * p;
    p = 1.0 / 6_227_020_800.0 + x2 * p;
    p = -1.0 / 39_916_800.0 + x2 * p;
    p = 1.0 / 362_880.0 + x2 * p;
    p = -1.0 / 5_040.0 + x2 * p;
    p = 1.0 / 120.0 + x2 * p;
    p = -1.0 / 6.0 + x2 * p;
    x * (1.0 + x2 * p)
}

/// Cosine uses the same reduced sine polynomial, not a second approximation.
pub fn cos(angle: f64) -> f64 {
    sin(angle + FRAC_PI_2)
}

/// Tangent on arc subdivision's reachable domain, [-pi/8, pi/8].
pub fn tan(angle: f64) -> f64 {
    sin(angle) / cos(angle)
}

/// Quadrant-correct angle for finite coordinates, including signed-zero axes.
pub fn atan2(y: f64, x: f64) -> f64 {
    let negative_x = x < 0.0 || (x == 0.0 && 1.0 / x < 0.0);
    let negative_y = y < 0.0 || (y == 0.0 && 1.0 / y < 0.0);
    let ax = x.abs();
    let ay = y.abs();
    let mut angle = if ax == 0.0 && ay == 0.0 {
        0.0
    } else if ay > ax {
        FRAC_PI_2 - atan_unit(ax / ay)
    } else {
        atan_unit(ay / ax)
    };
    if negative_x {
        angle = PI - angle;
    }
    if negative_y {
        -angle
    } else {
        angle
    }
}

fn atan_unit(ratio: f64) -> f64 {
    // Range reduction maps [0, 1] to [-1/3, 1/2], bounding the alternating
    // series' omitted term below 4e-17 after 24 terms. Basic arithmetic keeps
    // the result independent of each platform's maths-library implementation.
    // This atan2 approximation is not mirrored in playground/web/index.html:
    // that page draws native SVG arcs instead of converting them to cubics.
    let (x, offset) = if ratio > 0.5 {
        ((ratio - 1.0) / (ratio + 1.0), FRAC_PI_4)
    } else {
        (ratio, 0.0)
    };
    let x2 = x * x;
    let mut term = x;
    let mut sum = x;
    for index in 1..24 {
        term *= -x2;
        sum += term / (2 * index + 1) as f64;
    }
    offset + sum
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_ERROR: f64 = 1e-12;

    #[test]
    // Std trig is an independent accuracy oracle, never used by generation.
    #[allow(clippy::disallowed_methods)]
    fn sine_cosine_match_std_on_10000_points() {
        let mut samples = 0;
        let mut sine_error: f64 = 0.0;
        let mut cosine_error: f64 = 0.0;
        for index in 0..10_000 {
            let x = -TAU + 2.0 * TAU * index as f64 / 9_999.0;
            let sine_difference = (sin(x) - x.sin()).abs();
            let cosine_difference = (cos(x) - x.cos()).abs();
            assert!(sine_difference.is_finite() && cosine_difference.is_finite());
            sine_error = sine_error.max(sine_difference);
            cosine_error = cosine_error.max(cosine_difference);
            samples += 1;
        }
        assert_eq!(samples, 10_000);
        assert!(sine_error <= MAX_ERROR, "max sine error: {sine_error}");
        assert!(
            cosine_error <= MAX_ERROR,
            "max cosine error: {cosine_error}"
        );
    }

    #[test]
    fn sine_cosine_symmetry_and_periodicity() {
        for index in 0..1_001 {
            let x = -TAU + 2.0 * TAU * index as f64 / 1_000.0;
            assert!((sin(-x) + sin(x)).abs() <= MAX_ERROR);
            assert!((cos(-x) - cos(x)).abs() <= MAX_ERROR);
            for periods in [-2, -1, 1, 2] {
                let shifted = x + periods as f64 * TAU;
                assert!((sin(shifted) - sin(x)).abs() <= MAX_ERROR);
                assert!((cos(shifted) - cos(x)).abs() <= MAX_ERROR);
            }
        }
    }

    #[test]
    // Std tangent checks accuracy independently of the shared sine polynomial.
    #[allow(clippy::disallowed_methods)]
    fn tan_matches_std_on_1001_points() {
        let mut samples = 0;
        let mut error: f64 = 0.0;
        for index in 0..1_001 {
            let x = -PI / 8.0 + (PI / 4.0) * index as f64 / 1_000.0;
            let difference = (tan(x) - x.tan()).abs();
            assert!(difference.is_finite());
            error = error.max(difference);
            samples += 1;
        }
        assert_eq!(samples, 1_001);
        assert!(error <= MAX_ERROR, "max tangent error: {error}");
    }

    #[test]
    // Std atan2 supplies the quadrant and accuracy oracle for finite vectors.
    #[allow(clippy::disallowed_methods)]
    fn atan2_matches_std_on_65_by_65_grid() {
        let mut samples = 0;
        let mut x_axis = 0;
        let mut y_axis = 0;
        let mut origins = 0;
        let mut error: f64 = 0.0;
        // An odd-sized grid includes zero on both axes and both endpoints.
        for row in -32..=32 {
            for column in -32..=32 {
                let y = row as f64 / 2.0;
                let x = column as f64 / 2.0;
                let difference = (atan2(y, x) - y.atan2(x)).abs();
                assert!(difference.is_finite());
                error = error.max(difference);
                samples += 1;
                x_axis += usize::from(row == 0);
                y_axis += usize::from(column == 0);
                origins += usize::from(row == 0 && column == 0);
            }
        }
        assert_eq!(samples, 4_225);
        assert_eq!((x_axis, y_axis, origins), (65, 65, 1));
        assert!(error <= MAX_ERROR, "max atan2 error: {error}");
    }

    #[test]
    // Signed-zero comparisons use std only in this oracle, not generation.
    #[allow(clippy::disallowed_methods)]
    fn atan2_signed_zero_axes_match_std() {
        for y in [-1.0_f64, -0.0, 0.0, 1.0] {
            for x in [-1.0, -0.0, 0.0, 1.0] {
                assert_eq!(atan2(y, x).to_bits(), y.atan2(x).to_bits());
            }
        }
    }
}
