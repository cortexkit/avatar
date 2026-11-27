//! Portable maths shared by every part of the generator.
//!
//! The reference does its geometry in JavaScript numbers, which are `f64`, so
//! every helper here keeps the reference's operation order: a discrete choice
//! that depends on geometry (whether a flower sits too close to the face, for
//! example) must come out the same on both sides. Sine and cosine come from
//! `avatar_svg::wild::math`, which the reference copies term for term.

use avatar_svg::wild::math;

pub(crate) use std::f64::consts::{PI, TAU};

/// One degree in radians, computed as the reference computes it.
pub(crate) const DEG: f64 = PI / 180.0;

/// A point or a direction.
pub(crate) type V = [f64; 2];

/// Straight up on screen (y grows downwards).
pub(crate) const UP: V = [0.0, -1.0];
/// Straight down on screen.
pub(crate) const DOWN: V = [0.0, 1.0];

pub(crate) fn sin(angle: f64) -> f64 {
    math::sin(angle)
}

pub(crate) fn cos(angle: f64) -> f64 {
    math::cos(angle)
}

/// JavaScript's `Math.round`: the nearest integer, halves towards positive
/// infinity. `x - floor(x)` is exact for every finite `f64`, so the half test
/// is exact too.
pub(crate) fn js_round(x: f64) -> f64 {
    let whole = x.floor();
    if x - whole >= 0.5 {
        whole + 1.0
    } else {
        whole
    }
}

/// The reference's `r3`: rounded to three decimals.
pub(crate) fn r3(v: f64) -> f64 {
    js_round(v * 1000.0) / 1000.0
}

/// A coordinate as the reference writes it into path data (one decimal),
/// in the render tree's hundredths of a unit.
pub(crate) fn tenths(v: f64) -> i64 {
    js_round(v * 10.0) as i64 * 10
}

/// A length as the reference writes it for a radius or a stroke width (two
/// decimals), in the render tree's hundredths of a unit.
pub(crate) fn hundredths(v: f64) -> i64 {
    js_round(v * 100.0) as i64
}

/// `Math.min` for two finite numbers.
pub(crate) fn min(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

/// `Math.max` for two finite numbers.
pub(crate) fn max(a: f64, b: f64) -> f64 {
    if b > a {
        b
    } else {
        a
    }
}

pub(crate) fn max3(a: f64, b: f64, c: f64) -> f64 {
    max(max(a, b), c)
}

pub(crate) fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

pub(crate) fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

pub(crate) fn hyp(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

/// `hyp(x, y) || fallback`: a zero length is replaced by the fallback.
pub(crate) fn hyp_or(x: f64, y: f64, fallback: f64) -> f64 {
    let length = hyp(x, y);
    if length == 0.0 || length.is_nan() {
        fallback
    } else {
        length
    }
}

pub(crate) fn unit(x: f64, y: f64) -> V {
    let length = hyp_or(x, y, 1e-9);
    [x / length, y / length]
}

/// Turns a vector by `a` radians, clockwise as seen on screen.
pub(crate) fn rot(v: V, a: f64) -> V {
    let c = cos(a);
    let s = sin(a);
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
}

/// Turns the unit vector `h` towards the unit vector `t` by at most `amount`
/// radians, never past it. `tie` says which way round to go when the two
/// point exactly opposite ways.
pub(crate) fn toward(h: V, t: V, amount: f64, tie: f64) -> V {
    if amount <= 0.0 {
        return h;
    }
    let cross = h[0] * t[1] - h[1] * t[0];
    let dot = h[0] * t[0] + h[1] * t[1];
    if dot > 0.99995 {
        return h;
    }
    let sign = if cross > 1e-6 {
        1.0
    } else if cross < -1e-6 {
        -1.0
    } else if tie == 0.0 {
        1.0
    } else {
        tie
    };
    rot(
        h,
        sign * min(amount, 2.0 * max(0.0, (1.0 - dot) / 2.0).sqrt()),
    )
}

/// The unit vector `dir`, flattened if need be so that something `len` long
/// growing from `p` along it stays above the height `floor`.
pub(crate) fn grounded(p: V, dir: V, len: f64, floor: f64) -> V {
    let most = (floor - p[1]) / len;
    if dir[1] <= most {
        return dir;
    }
    let d = clamp(most, -1.0, 1.0);
    [
        (if dir[0] < 0.0 { -1.0 } else { 1.0 }) * (1.0 - d * d).sqrt(),
        d,
    ]
}

/// `Math.sign` for a number known not to be zero.
pub(crate) fn sign(v: f64) -> f64 {
    if v < 0.0 {
        -1.0
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_round_takes_halves_up() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(-2.6), -3.0);
        assert_eq!(js_round(0.49999999999999994), 0.0);
        assert_eq!(tenths(1.25), 130);
        assert_eq!(tenths(-1.25), -120);
        assert_eq!(hundredths(0.555), 56);
    }

    #[test]
    fn toward_never_overshoots() {
        // A quarter turn away, the cap is twice the sine of half the angle,
        // which stops short of the target.
        let turned = toward([1.0, 0.0], DOWN, 10.0, 1.0);
        assert_eq!(turned, rot([1.0, 0.0], 2.0 * 0.5_f64.sqrt()));
        assert!(turned[0] > 0.0 && turned[1] > 0.0);
        assert_eq!(toward([1.0, 0.0], DOWN, 0.0, 1.0), [1.0, 0.0]);
    }
}
