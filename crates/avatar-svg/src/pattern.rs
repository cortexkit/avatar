//! Body-pattern mask geometry (design note 004 §Body paint).
//!
//! Pattern classes select the mask content: 0 none, 1 circler above the body,
//! 2 circler below, 3 voronoi, 4 cutting Bézier, 5 cutting Bézier flipped.
//! Where the historical reference (`pattern.tsx`) read DNA bytes through its
//! `uint8*Range` helpers, this renderer reads the decoded phenotype pattern
//! block instead — a renderer never sees DNA. The field correspondence:
//!
//! - `circler.point_reduce` (raw byte) ↔ `uint8InQuintRange(features[10], 0, 8)`:
//!   the decoder keeps the byte raw, so the in-quint easing table is applied
//!   here to get the point-count reduction 0..=8.
//! - `circler.size` ↔ the linear factor of `uint8Range(features[11], 1.2h, 2h)`.
//! - `circler.y_offset_div` ↔ `uint8Range(features[12], 2, 3)`, the divisor of
//!   the vertical offset.
//! - `circler.tension` ↔ `uint8InQuintRange(dna2[8], 0, 0.5)`, the amount
//!   subtracted from 1 (final tension = 1 − t/2).
//! - `circler.pull_min` ↔ `uint8InOutCircRange(dna2[11], 0.85, 1)`, the radius
//!   pull lower bound. The reference jittered each point's pull from a DNA
//!   byte; that jitter is dropped (it is DNA, not phenotype), so the pull is
//!   uniform at the mapped bound.
//! - `voronoi.index` ↔ `uint8Range(features[9], 0, len−1)`.
//! - `voronoi.rotation` (BAM16) ↔ the rotation in degrees.
//! - `voronoi.scale` ↔ `uint8Range(dna2[9], 0.1, 2.5)`.
//! - `cut.spread` ↔ `uint8ExpoRange(features[13], 0.5, 10)` and
//!   `cut.y_adjust_div` ↔ `uint8Range(features[14], 2, 100)`: the decoder
//!   already applied the easing tables for these Q32 fields, so the renderer
//!   only lerps them into `pattern.tsx`'s target ranges.

use avatar_phenotype::{Bam16, Circler, Cut, Pattern, Rgb, Voronoi, Q32};
use avatar_tables::{cos_q30, div_round_half_away, sin_q30, EASE_IN_QUINT};

use crate::fixed::{lerp_q32_milli, q32_mul, q32_ratio, CANVAS_MILLI, CENTER, Q32_ONE};
use crate::geometry::{build_spline, RingPoint};
use crate::tree::{FillRule, Paint, Primitive, Segment, Shape, Transform, TransformOp};
use crate::voronoi::VORONOI_PATTERNS;

/// Q32.32 constants for `pattern.tsx`'s range endpoints, half-away rounded.
const Q32_POINT_ONE: i64 = 429_496_730; // 0.1
const Q32_POINT_EIGHTY_FIVE: i64 = 3_650_722_202; // 0.85
const Q32_TWO_POINT_FIVE: i64 = 10_737_418_240; // 2.5 (exact)

/// Builds the drawable geometry inside a body-pattern mask.
pub(crate) fn pattern_mask_primitives(
    pattern: &Pattern,
    body_width: i64,
    body_height: i64,
    top: i64,
    root_ops: &[TransformOp],
) -> Option<Vec<Primitive>> {
    let (shape, tx, ty, scale, angle, center, label) = match pattern.r#class {
        1 => (
            circler_shape(&pattern.circler, true, body_height, top),
            0,
            0,
            Q32_ONE,
            Bam16(0),
            CENTER,
            "av-pattern-circler".to_owned(),
        ),
        2 => (
            circler_shape(&pattern.circler, false, body_height, top),
            0,
            0,
            Q32_ONE,
            Bam16(0),
            CENTER,
            "av-pattern-circler".to_owned(),
        ),
        3 => {
            let placement = voronoi_placement(&pattern.voronoi, body_width, body_height);
            (
                parse_voronoi_path(VORONOI_PATTERNS[placement.index]),
                placement.tx,
                placement.ty,
                placement.scale,
                pattern.voronoi.rotation,
                placement.center,
                format!("av-pattern-voronoi-{}", placement.index),
            )
        }
        4 => (
            cutting_bezier_shape(&pattern.cut, body_height, top),
            0,
            0,
            Q32_ONE,
            Bam16(0),
            CENTER,
            "av-pattern-cut".to_owned(),
        ),
        5 => (
            cutting_bezier_shape(&pattern.cut, body_height, top),
            0,
            0,
            Q32_ONE,
            Bam16(32_768),
            CENTER,
            "av-pattern-cut-flipped".to_owned(),
        ),
        _ => return None,
    };

    let mut ops = root_ops.to_vec();
    ops.extend([
        TransformOp::Translate { tx, ty },
        TransformOp::Scale {
            sx: Q32(scale),
            sy: Q32(scale),
        },
        TransformOp::Rotate {
            angle,
            cx: center,
            cy: center,
        },
    ]);
    Some(vec![Primitive {
        shape,
        transform: Some(Transform { ops }),
        paint: Paint::Solid {
            color: Rgb {
                r: 0xef,
                g: 0xef,
                b: 0xef,
            },
        },
        stroke: None,
        opacity: None,
        clip: None,
        effects: Vec::new(),
        label,
    }])
}

/// Circler classes 1/2: a ring of `2·numPoints` bumps above or below the body.
fn circler_shape(circler: &Circler, above_body: bool, body_height: i64, top: i64) -> Shape {
    let reduction = div_round_half_away(
        8 * i128::from(EASE_IN_QUINT[usize::from(circler.point_reduce)]),
        i128::from(Q32_ONE),
    ) as i64;
    let num_points = 16 - reduction;

    let size_lo = div_round_half_away(6 * i128::from(body_height), 5) as i64;
    let size = lerp_q32_milli(size_lo, 2 * body_height, circler.size.0);
    let divisor = lerp_q32_milli(2 * Q32_ONE, 3 * Q32_ONE, circler.y_offset_div.0);
    let y_offset = div_round_half_away(
        i128::from(body_height) * i128::from(Q32_ONE),
        i128::from(divisor),
    ) as i64;
    let position = if above_body {
        top - size + y_offset
    } else {
        top + size + y_offset
    };

    let tension = Q32_ONE - div_round_half_away(i128::from(circler.tension.0), 2) as i64;
    let pull = lerp_q32_milli(Q32_POINT_EIGHTY_FIVE, Q32_ONE, circler.pull_min.0);
    let radius = q32_mul(size, pull);
    let angle_step = div_round_half_away(65_536, i128::from(2 * num_points)) as u16;
    let count = usize::try_from(2 * num_points).expect("the point count is small and nonnegative");
    let mut ring = Vec::with_capacity(count);
    for index in 1..=count {
        let angle = (index as u32 * u32::from(angle_step) + 16_384) as u16;
        ring.push(RingPoint {
            x: CENTER + trig_offset(radius, cos_q30(angle)),
            y: position + trig_offset(radius, sin_q30(angle)),
            tension,
        });
    }
    build_spline(&ring).shape()
}

fn trig_offset(radius: i64, trig_q30: i32) -> i64 {
    div_round_half_away(i128::from(radius) * i128::from(trig_q30), 1i128 << 30) as i64
}

struct VoronoiPlacement {
    index: usize,
    tx: i64,
    ty: i64,
    scale: i64,
    center: i64,
}

fn voronoi_placement(voronoi: &Voronoi, body_width: i64, body_height: i64) -> VoronoiPlacement {
    let index = div_round_half_away(
        i128::from(voronoi.index) * (VORONOI_PATTERNS.len() as i128 - 1),
        255,
    ) as usize;
    let base_scale = lerp_q32_milli(Q32_POINT_ONE, Q32_TWO_POINT_FIVE, voronoi.scale.0);
    let scale = base_scale
        .max(q32_ratio(body_width))
        .max(q32_ratio(body_height));
    let scaled_canvas = q32_mul(CANVAS_MILLI, scale);
    let tx = div_round_half_away(
        i128::from(CANVAS_MILLI - scaled_canvas) * i128::from(Q32_ONE),
        2 * i128::from(scale),
    ) as i64;
    let ty = tx - div_round_half_away(i128::from(scaled_canvas), 2) as i64;
    let stroke_unit = div_round_half_away(i128::from(CANVAS_MILLI), 255) as i64;
    let final_scale = q32_mul(scale, stroke_unit);
    let center = div_round_half_away(
        i128::from(CANVAS_MILLI) * i128::from(Q32_ONE),
        2 * i128::from(final_scale),
    ) as i64;
    VoronoiPlacement {
        index,
        tx,
        ty,
        scale: final_scale,
        center,
    }
}

fn cutting_bezier_shape(cut: &Cut, body_height: i64, top: i64) -> Shape {
    let spread = lerp_q32_milli(Q32_ONE / 2, 10 * Q32_ONE, cut.spread.0);
    let lr = q32_mul(CANVAS_MILLI, spread);
    let divisor = lerp_q32_milli(2 * Q32_ONE, 100 * Q32_ONE, cut.y_adjust_div.0);
    let y_adjust = div_round_half_away(
        i128::from(body_height) * i128::from(Q32_ONE),
        i128::from(divisor),
    ) as i64;
    let bottom = top + body_height + y_adjust;
    let upper = top + y_adjust;
    let middle = top + lr + y_adjust;
    Shape::Path {
        segments: vec![
            Segment::MoveTo { x: 0, y: bottom },
            Segment::CubicTo {
                cp1x: CENTER - lr,
                cp1y: bottom,
                cp2x: CENTER,
                cp2y: middle,
                x: CENTER,
                y: upper,
            },
            Segment::CubicTo {
                cp1x: CENTER,
                cp1y: middle,
                cp2x: CENTER + lr,
                cp2y: bottom,
                x: CANVAS_MILLI,
                y: bottom,
            },
        ],
        closed: true,
        fill_rule: FillRule::NonZero,
    }
}

/// Parses the vendored relative m/c/s paths into absolute tree geometry.
/// The serializer intentionally reuses the source spelling selected by the
/// primitive label, preserving the corpus while native consumers get segments.
fn parse_voronoi_path(source: &str) -> Shape {
    let tokens = svg_path_tokens(source);
    let mut index = 0;
    let mut command = b'm';
    let mut current = (0, 0);
    let mut previous_control = current;
    let mut segments = Vec::new();
    let mut first_move = true;

    while index < tokens.len() {
        if let PathToken::Command(next) = tokens[index] {
            command = next;
            index += 1;
            first_move = command == b'm';
            continue;
        }
        match command {
            b'm' => {
                let (Some(x), Some(y)) = (
                    token_number(&tokens, index),
                    token_number(&tokens, index + 1),
                ) else {
                    break;
                };
                current = (current.0 + x, current.1 + y);
                segments.push(if first_move {
                    Segment::MoveTo {
                        x: current.0,
                        y: current.1,
                    }
                } else {
                    Segment::LineTo {
                        x: current.0,
                        y: current.1,
                    }
                });
                previous_control = current;
                first_move = false;
                index += 2;
            }
            b'c' => {
                let values = (0..6)
                    .map(|offset| token_number(&tokens, index + offset))
                    .collect::<Option<Vec<_>>>();
                let Some(values) = values else { break };
                let control1 = (current.0 + values[0], current.1 + values[1]);
                let control2 = (current.0 + values[2], current.1 + values[3]);
                current = (current.0 + values[4], current.1 + values[5]);
                segments.push(Segment::CubicTo {
                    cp1x: control1.0,
                    cp1y: control1.1,
                    cp2x: control2.0,
                    cp2y: control2.1,
                    x: current.0,
                    y: current.1,
                });
                previous_control = control2;
                index += 6;
            }
            b's' => {
                let values = (0..4)
                    .map(|offset| token_number(&tokens, index + offset))
                    .collect::<Option<Vec<_>>>();
                let Some(values) = values else { break };
                let control1 = (
                    2 * current.0 - previous_control.0,
                    2 * current.1 - previous_control.1,
                );
                let control2 = (current.0 + values[0], current.1 + values[1]);
                current = (current.0 + values[2], current.1 + values[3]);
                segments.push(Segment::CubicTo {
                    cp1x: control1.0,
                    cp1y: control1.1,
                    cp2x: control2.0,
                    cp2y: control2.1,
                    x: current.0,
                    y: current.1,
                });
                previous_control = control2;
                index += 4;
            }
            _ => index += 1,
        }
    }

    Shape::Path {
        segments,
        closed: false,
        fill_rule: FillRule::NonZero,
    }
}

#[derive(Clone, Copy)]
enum PathToken {
    Command(u8),
    Number(i64),
}

fn svg_path_tokens(source: &str) -> Vec<PathToken> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            command @ (b'm' | b'c' | b's') => {
                tokens.push(PathToken::Command(command));
                index += 1;
            }
            b' ' | b',' => index += 1,
            _ => {
                let start = index;
                if matches!(bytes[index], b'-' | b'+') {
                    index += 1;
                }
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if index < bytes.len() && bytes[index] == b'.' {
                    index += 1;
                    while index < bytes.len() && bytes[index].is_ascii_digit() {
                        index += 1;
                    }
                }
                let text =
                    std::str::from_utf8(&bytes[start..index]).expect("vendored path is ASCII");
                tokens.push(PathToken::Number(decimal_milli(text)));
            }
        }
    }
    tokens
}

fn token_number(tokens: &[PathToken], index: usize) -> Option<i64> {
    match tokens.get(index) {
        Some(PathToken::Number(value)) => Some(*value),
        _ => None,
    }
}

fn decimal_milli(text: &str) -> i64 {
    let (negative, unsigned) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let mut pieces = unsigned.splitn(2, '.');
    let whole = pieces.next().unwrap_or("0").parse::<i64>().unwrap_or(0);
    let fraction_text = pieces.next().unwrap_or("");
    let tens = fraction_text
        .as_bytes()
        .first()
        .map_or(0, |byte| i64::from(byte - b'0'));
    let ones = fraction_text
        .as_bytes()
        .get(1)
        .map_or(0, |byte| i64::from(byte - b'0'));
    let value = whole * 100 + tens * 10 + ones;
    if negative {
        -value
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use avatar_phenotype::{Bam16, Q32};

    fn circler(
        point_reduce: u8,
        size: i64,
        y_offset_div: i64,
        tension: i64,
        pull_min: i64,
    ) -> Circler {
        Circler {
            point_reduce,
            size: Q32(size),
            y_offset_div: Q32(y_offset_div),
            tension: Q32(tension),
            pull_min: Q32(pull_min),
        }
    }

    #[test]
    fn class_zero_has_no_mask_primitives() {
        let pattern = Pattern {
            r#class: 0,
            circler: circler(0, 0, 0, 0, 0),
            voronoi: Voronoi {
                index: 0,
                rotation: Bam16(0),
                scale: Q32(0),
            },
            cut: Cut {
                spread: Q32(0),
                y_adjust_div: Q32(0),
            },
        };
        assert!(pattern_mask_primitives(&pattern, 500_000, 500_000, 250_000, &[]).is_none());
    }

    #[test]
    fn circler_ring_has_the_expected_point_count() {
        // point_reduce byte 0 → EASE_IN_QUINT[0] = 0 → numPoints = 16 → 32
        // ring points → 32 Bézier segments in the path.
        let shape = circler_shape(
            &circler(0, Q32_ONE / 2, Q32_ONE / 2, 0, Q32_ONE),
            true,
            400_000,
            300_000,
        );
        let Shape::Path { segments, .. } = shape else {
            panic!("circler must be a path");
        };
        assert_eq!(segments.len(), 33);
        assert!(matches!(segments[0], Segment::MoveTo { .. }));
    }

    #[test]
    fn circler_point_reduce_shrinks_the_ring() {
        // point_reduce byte 255 → EASE_IN_QUINT[255] = 1.0 → reduction 8 →
        // numPoints 8 → 16 segments.
        let shape = circler_shape(
            &circler(255, Q32_ONE / 2, Q32_ONE / 2, 0, Q32_ONE),
            false,
            400_000,
            300_000,
        );
        let Shape::Path { segments, .. } = shape else {
            panic!("circler must be a path");
        };
        assert_eq!(segments.len(), 17);
    }

    #[test]
    fn cutting_bezier_layout_is_pinned() {
        let cut = Cut {
            spread: Q32(0),       // lerp(0.5, 10, 0) = 0.5 → lr = canvas/2
            y_adjust_div: Q32(0), // lerp(2, 100, 0) = 2 → yAdjust = h/2
        };
        let shape = cutting_bezier_shape(&cut, 200_000, 100_000);
        // The input resolves to a 500_000 horizontal spread and a 100_000
        // vertical adjustment, pinning every control point below.
        assert_eq!(
            shape,
            Shape::Path {
                segments: vec![
                    Segment::MoveTo { x: 0, y: 400_000 },
                    Segment::CubicTo {
                        cp1x: 0,
                        cp1y: 400_000,
                        cp2x: 500_000,
                        cp2y: 700_000,
                        x: 500_000,
                        y: 200_000,
                    },
                    Segment::CubicTo {
                        cp1x: 500_000,
                        cp1y: 700_000,
                        cp2x: 1_000_000,
                        cp2y: 400_000,
                        x: 1_000_000,
                        y: 400_000,
                    },
                ],
                closed: true,
                fill_rule: FillRule::NonZero,
            }
        );
    }

    #[test]
    fn voronoi_index_maps_into_the_vendored_table() {
        for (byte, expected) in [(0u8, 0usize), (64, 1), (128, 2), (191, 3), (255, 4)] {
            let index =
                div_round_half_away(i128::from(byte) * (VORONOI_PATTERNS.len() as i128 - 1), 255)
                    as usize;
            assert_eq!(index, expected, "byte {byte}");
        }
    }

    #[test]
    fn voronoi_placement_scales_to_cover_the_body() {
        let voronoi = Voronoi {
            index: 0,
            rotation: Bam16(16_384), // quarter turn → 90°
            scale: Q32(0),           // lerp(0.1, 2.5, 0) = 0.1, below the body ratio
        };
        let placement = voronoi_placement(&voronoi, 900_000, 400_000);
        assert_eq!(placement.index, 0);
        // Scale is at least the body-width ratio times canvas/255.
        let stroke_unit = div_round_half_away(i128::from(CANVAS_MILLI), 255) as i64;
        let min_scale = q32_mul(q32_ratio(900_000), stroke_unit);
        assert!(placement.scale >= min_scale);
        assert!(placement.center > 0);
    }

    #[test]
    fn mask_primitive_carries_resolved_transform_and_paint() {
        let pattern = Pattern {
            r#class: 4,
            circler: circler(0, 0, 0, 0, 0),
            voronoi: Voronoi {
                index: 0,
                rotation: Bam16(0),
                scale: Q32(0),
            },
            cut: Cut {
                spread: Q32(0),
                y_adjust_div: Q32(0),
            },
        };
        let primitives = pattern_mask_primitives(&pattern, 500_000, 200_000, 100_000, &[]).unwrap();
        assert_eq!(primitives.len(), 1);
        assert_eq!(primitives[0].label, "av-pattern-cut");
        assert_eq!(primitives[0].transform.as_ref().unwrap().ops.len(), 3);
        assert_eq!(
            primitives[0].paint,
            Paint::Solid {
                color: Rgb {
                    r: 0xef,
                    g: 0xef,
                    b: 0xef,
                }
            }
        );
    }
}
