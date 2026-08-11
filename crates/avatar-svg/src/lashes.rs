//! Eyelash sampling along the post-mood upper-lid Bézier.

use avatar_phenotype::{Lashes, Rgb};
use avatar_tables::{cos_q30, div_round_half_away, sin_q30};

use crate::fixed::{q32_mul, Q32_ONE};
use crate::tree::{
    FillRule, Paint, Primitive, Segment, Shape, Stroke, StrokeCap, StrokeJoin, Transform,
};

/// One cubic curve in canonical milliunits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cubic {
    pub x0: i64,
    pub y0: i64,
    pub cp1x: i64,
    pub cp1y: i64,
    pub cp2x: i64,
    pub cp2y: i64,
    pub x3: i64,
    pub y3: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EyeSide {
    Left,
    Right,
}

/// face.tsx 707–759 samples at most indices 0..=10 and removes the tail arc.
pub(crate) fn sample_count(count: u8, removed: u8) -> usize {
    if count == 0 || removed > count {
        0
    } else {
        usize::from((count - removed).min(10)) + 1
    }
}

/// Produces lashes after mood closure has shifted the upper-lid curve.
pub(crate) fn lash_primitives(
    genes: &Lashes,
    curve: Cubic,
    eye_height: i64,
    side: EyeSide,
    color: Rgb,
    transform: &Transform,
) -> Vec<Primitive> {
    let samples = sample_count(genes.count, genes.removed);
    if samples == 0 {
        return Vec::new();
    }

    let length = q32_mul(eye_height, genes.length_scale.0).max(0);
    let stroke = q32_mul(eye_height, genes.stroke_scale.0).max(1);
    let angular_offset = lash_angle_offset(genes.direction);
    let mut primitives = Vec::with_capacity(samples);

    // face.tsx 707–759 derives a tangent angle and then adds or subtracts
    // π/direction. The BAM lookup keeps the construction deterministic.
    for index in 0..samples {
        let point = cubic_point(curve, index as i64, i64::from(genes.count));
        let tangent = cubic_tangent(curve, index as i64, i64::from(genes.count));
        let tangent_angle = atan2_bam(tangent.1, tangent.0);
        let angle = match side {
            EyeSide::Left => tangent_angle.wrapping_sub(angular_offset),
            EyeSide::Right => tangent_angle.wrapping_add(angular_offset),
        };
        let end_x = point.0 + trig_length(length, cos_q30(angle));
        let end_y = point.1 + trig_length(length, sin_q30(angle));
        let sign = match side {
            EyeSide::Left => -1,
            EyeSide::Right => 1,
        };
        primitives.push(Primitive {
            shape: Shape::Path {
                segments: vec![
                    Segment::MoveTo {
                        x: point.0,
                        y: point.1,
                    },
                    Segment::CubicTo {
                        cp1x: point.0 + sign * stroke * 3,
                        cp1y: point.1 - stroke,
                        cp2x: end_x + sign * stroke,
                        cp2y: end_y - stroke,
                        x: end_x + sign * stroke * 2,
                        y: end_y - stroke * 2,
                    },
                ],
                closed: false,
                fill_rule: FillRule::NonZero,
            },
            transform: Some(transform.clone()),
            paint: Paint::Solid { color },
            stroke: Some(Stroke {
                color,
                width: stroke,
                cap: StrokeCap::Round,
                join: StrokeJoin::Miter,
            }),
            opacity: None,
            clip: None,
            effects: Vec::new(),
            label: "av-lash".to_owned(),
        });
    }
    primitives
}

fn lash_angle_offset(direction: u8) -> u16 {
    let fraction = div_round_half_away(i128::from(direction) * i128::from(Q32_ONE), 255) as i64;
    // Normalizing the source byte to 0..1 makes this divisor span 1..2.
    let divisor = Q32_ONE + fraction;
    div_round_half_away(
        i128::from(32_768_i64) * i128::from(Q32_ONE),
        i128::from(divisor),
    ) as u16
}

fn cubic_point(curve: Cubic, numerator: i64, denominator: i64) -> (i64, i64) {
    let p01 = lerp_point(
        (curve.x0, curve.y0),
        (curve.cp1x, curve.cp1y),
        numerator,
        denominator,
    );
    let p12 = lerp_point(
        (curve.cp1x, curve.cp1y),
        (curve.cp2x, curve.cp2y),
        numerator,
        denominator,
    );
    let p23 = lerp_point(
        (curve.cp2x, curve.cp2y),
        (curve.x3, curve.y3),
        numerator,
        denominator,
    );
    let p012 = lerp_point(p01, p12, numerator, denominator);
    let p123 = lerp_point(p12, p23, numerator, denominator);
    lerp_point(p012, p123, numerator, denominator)
}

fn cubic_tangent(curve: Cubic, numerator: i64, denominator: i64) -> (i64, i64) {
    let d0 = (3 * (curve.cp1x - curve.x0), 3 * (curve.cp1y - curve.y0));
    let d1 = (3 * (curve.cp2x - curve.cp1x), 3 * (curve.cp2y - curve.cp1y));
    let d2 = (3 * (curve.x3 - curve.cp2x), 3 * (curve.y3 - curve.cp2y));
    let d01 = lerp_point(d0, d1, numerator, denominator);
    let d12 = lerp_point(d1, d2, numerator, denominator);
    lerp_point(d01, d12, numerator, denominator)
}

fn lerp_point(a: (i64, i64), b: (i64, i64), numerator: i64, denominator: i64) -> (i64, i64) {
    (
        a.0 + ratio_mul(b.0 - a.0, numerator, denominator),
        a.1 + ratio_mul(b.1 - a.1, numerator, denominator),
    )
}

fn ratio_mul(value: i64, numerator: i64, denominator: i64) -> i64 {
    if denominator == 0 {
        return 0;
    }
    div_round_half_away(
        i128::from(value) * i128::from(numerator),
        i128::from(denominator),
    ) as i64
}

/// Finds the nearest table-defined BAM direction by maximum dot product.
fn atan2_bam(y: i64, x: i64) -> u16 {
    if x == 0 && y == 0 {
        return 0;
    }
    let mut best_angle = 0_u16;
    let mut best_dot = i128::MIN;
    for raw in (0_u32..65_536).step_by(4) {
        let angle = raw as u16;
        let dot =
            i128::from(x) * i128::from(cos_q30(angle)) + i128::from(y) * i128::from(sin_q30(angle));
        if dot > best_dot {
            best_dot = dot;
            best_angle = angle;
        }
    }
    best_angle
}

fn trig_length(length: i64, trig_q30: i32) -> i64 {
    div_round_half_away(i128::from(length) * i128::from(trig_q30), 1_i128 << 30) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_and_removal_match_the_reference_loop() {
        assert_eq!(sample_count(3, 0), 4);
        assert_eq!(sample_count(16, 2), 11);
        assert_eq!(sample_count(8, 3), 6);
        assert_eq!(sample_count(2, 3), 0);
        assert_eq!(sample_count(0, 0), 0);
    }

    #[test]
    fn cubic_sampling_hits_both_endpoints() {
        let curve = Cubic {
            x0: 10,
            y0: 20,
            cp1x: 30,
            cp1y: 0,
            cp2x: 70,
            cp2y: 0,
            x3: 90,
            y3: 20,
        };
        assert_eq!(cubic_point(curve, 0, 10), (10, 20));
        assert_eq!(cubic_point(curve, 10, 10), (90, 20));
    }
}
