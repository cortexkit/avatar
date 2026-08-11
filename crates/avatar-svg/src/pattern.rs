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

use avatar_phenotype::{Circler, Cut, Pattern, Voronoi};
use avatar_tables::{cos_q30, div_round_half_away, sin_q30, EASE_IN_QUINT};

use crate::fixed::{lerp_q32_milli, q32_mul, q32_ratio, CANVAS_MILLI, CENTER, Q32_ONE};
use crate::format::{format_milli, format_q32_decimal};
use crate::geometry::{build_spline, path_string, RingPoint};
use crate::voronoi::VORONOI_PATTERNS;

/// Q32.32 constants for `pattern.tsx`'s range endpoints, half-away rounded.
const Q32_POINT_ONE: i64 = 429_496_730; // 0.1
const Q32_POINT_EIGHTY_FIVE: i64 = 3_650_722_202; // 0.85
const Q32_TWO_POINT_FIVE: i64 = 10_737_418_240; // 2.5 (exact)

/// Builds the mask content for a pattern, or `None` when the class selects
/// no pattern (class 0, or a value outside the decoded 0..=5 range).
///
/// The returned fragment keeps the nesting `bodylayer.tsx` and `pattern.tsx`
/// emit: the body layer's mask `<g>`, then the pattern's translate group,
/// then the scale/rotate group holding the `#efefef` path.
pub(crate) fn mask_content(
    pattern: &Pattern,
    body_width: i64,
    body_height: i64,
    top: i64,
) -> Option<String> {
    let (path, tx, ty, scale, rotate, rotate_center) = match pattern.r#class {
        1 => (
            circler_path(&pattern.circler, true, body_height, top),
            0,
            0,
            Q32_ONE,
            0,
            CENTER,
        ),
        2 => (
            circler_path(&pattern.circler, false, body_height, top),
            0,
            0,
            Q32_ONE,
            0,
            CENTER,
        ),
        3 => voronoi_placement(&pattern.voronoi, body_width, body_height),
        4 => (
            cutting_bezier_path(&pattern.cut, body_height, top),
            0,
            0,
            Q32_ONE,
            0,
            CENTER,
        ),
        5 => (
            cutting_bezier_path(&pattern.cut, body_height, top),
            0,
            0,
            Q32_ONE,
            180,
            CENTER,
        ),
        _ => return None,
    };
    Some(format!(
        "<g><g transform=\"translate({}, {})\"><g transform=\"scale({} {}) rotate({} {} {})\"><path d=\"{}\" fill=\"#efefef\"/></g></g></g>",
        format_milli(tx),
        format_milli(ty),
        format_q32_decimal(scale),
        format_q32_decimal(scale),
        rotate,
        format_milli(rotate_center),
        format_milli(rotate_center),
        path,
    ))
}

/// Circler classes 1/2: a ring of `2·numPoints` bumps above (class 1) or
/// below (class 2) the body, splined into a closed path without flattening
/// or rescale (`pattern.tsx` calls `spline` in its `simpleSpline` mode).
fn circler_path(circler: &Circler, above_body: bool, body_height: i64, top: i64) -> String {
    let reduction = div_round_half_away(
        8 * i128::from(EASE_IN_QUINT[usize::from(circler.point_reduce)]),
        i128::from(Q32_ONE),
    ) as i64;
    let num_points = 16 - reduction;

    // size = lerp(1.2·h, 2·h, size); 1.2 = 6/5 via one pinned division.
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
    // The radius is shared by x and y exactly as `pattern.tsx` shares its
    // `size * pull` product before the separate cos/sin multiplications.
    let radius = q32_mul(size, pull);

    // BAM16 angle step for a full turn over 2·numPoints points; the +π/2
    // phase `pattern.tsx` applies is BAM16 16384. u16 arithmetic wraps
    // mod 65536.
    let angle_step = div_round_half_away(65_536, i128::from(2 * num_points)) as u16;
    let count =
        usize::try_from(2 * num_points).expect("the point count is small and never negative");
    let mut ring = Vec::with_capacity(count);
    for index in 1..=count {
        let angle = (index as u32 * u32::from(angle_step) + 16_384) as u16;
        ring.push(RingPoint {
            x: CENTER + trig_offset(radius, cos_q30(angle)),
            y: position + trig_offset(radius, sin_q30(angle)),
            tension,
        });
    }
    path_string(&build_spline(&ring))
}

/// Scales a Q1.30 trig value against a milliunit radius, one pinned narrowing.
fn trig_offset(radius: i64, trig_q30: i32) -> i64 {
    div_round_half_away(i128::from(radius) * i128::from(trig_q30), 1i128 << 30) as i64
}

/// Voronoi class 3: a vendored path string with the scale/rotate/translate
/// fit `pattern.tsx` applies, re-expressed in fixed point.
///
/// Returns `(path, tx, ty, scale_q32, rotate_degrees, rotate_center)` with
/// tx/ty in milliunits and the scale as Q32.
fn voronoi_placement(
    voronoi: &Voronoi,
    body_width: i64,
    body_height: i64,
) -> (String, i64, i64, i64, i64, i64) {
    let index = div_round_half_away(
        i128::from(voronoi.index) * (VORONOI_PATTERNS.len() as i128 - 1),
        255,
    ) as usize;

    let base_scale = lerp_q32_milli(Q32_POINT_ONE, Q32_TWO_POINT_FIVE, voronoi.scale.0);
    let scale = base_scale
        .max(q32_ratio(body_width))
        .max(q32_ratio(body_height));

    // Translate centers the scaled pattern canvas; both divisions fold into
    // one pinned narrowing each.
    let scaled_canvas = q32_mul(CANVAS_MILLI, scale);
    let tx = div_round_half_away(
        i128::from(CANVAS_MILLI - scaled_canvas) * i128::from(Q32_ONE),
        2 * i128::from(scale),
    ) as i64;
    let ty = tx - div_round_half_away(i128::from(scaled_canvas), 2) as i64;

    // `pattern.tsx` multiplies the scale by canvas/255 AFTER deriving the
    // translate; the order is visible in the output and kept.
    let stroke_unit = div_round_half_away(i128::from(CANVAS_MILLI), 255) as i64;
    let final_scale = q32_mul(scale, stroke_unit);
    let rotate_center = div_round_half_away(
        i128::from(CANVAS_MILLI) * i128::from(Q32_ONE),
        2 * i128::from(final_scale),
    ) as i64;
    let degrees = div_round_half_away(i128::from(voronoi.rotation.0) * 360, 65_536) as i64;

    (
        VORONOI_PATTERNS[index].to_owned(),
        tx,
        ty,
        final_scale,
        degrees,
        rotate_center,
    )
}

/// Cut classes 4/5: a horizontal Bézier band across the body. Class 5 is the
/// same path rotated 180° by the mask transform.
fn cutting_bezier_path(cut: &Cut, body_height: i64, top: i64) -> String {
    let spread = lerp_q32_milli(Q32_ONE / 2, 10 * Q32_ONE, cut.spread.0);
    let lr = q32_mul(CANVAS_MILLI, spread);
    let divisor = lerp_q32_milli(2 * Q32_ONE, 100 * Q32_ONE, cut.y_adjust_div.0);
    let y_adjust = div_round_half_away(
        i128::from(body_height) * i128::from(Q32_ONE),
        i128::from(divisor),
    ) as i64;

    let y_bottom = top + body_height + y_adjust;
    let y_top = top + y_adjust;
    let y_mid = top + lr + y_adjust;
    // String shape is pinned verbatim to `pattern.tsx`, including the space
    // after the M pair and the missing space before the second C.
    format!(
        "M 0, {} C {} {}, {} {}, {} {}C {} {}, {} {}, {} {}Z",
        format_milli(y_bottom),
        format_milli(CENTER - lr),
        format_milli(y_bottom),
        format_milli(CENTER),
        format_milli(y_mid),
        format_milli(CENTER),
        format_milli(y_top),
        format_milli(CENTER),
        format_milli(y_mid),
        format_milli(CENTER + lr),
        format_milli(y_bottom),
        format_milli(CANVAS_MILLI),
        format_milli(y_bottom),
    )
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
    fn class_zero_has_no_mask_content() {
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
        assert!(mask_content(&pattern, 500_000, 500_000, 250_000).is_none());
    }

    #[test]
    fn circler_ring_has_the_expected_point_count() {
        // point_reduce byte 0 → EASE_IN_QUINT[0] = 0 → numPoints = 16 → 32
        // ring points → 32 Bézier segments in the path.
        let path = circler_path(
            &circler(0, Q32_ONE / 2, Q32_ONE / 2, 0, Q32_ONE),
            true,
            400_000,
            300_000,
        );
        assert_eq!(path.matches('C').count(), 32);
        assert!(path.starts_with('M'));
    }

    #[test]
    fn circler_point_reduce_shrinks_the_ring() {
        // point_reduce byte 255 → EASE_IN_QUINT[255] = 1.0 → reduction 8 →
        // numPoints 8 → 16 segments.
        let path = circler_path(
            &circler(255, Q32_ONE / 2, Q32_ONE / 2, 0, Q32_ONE),
            false,
            400_000,
            300_000,
        );
        assert_eq!(path.matches('C').count(), 16);
    }

    #[test]
    fn cutting_bezier_layout_is_pinned() {
        let cut = Cut {
            spread: Q32(0),       // lerp(0.5, 10, 0) = 0.5 → lr = canvas/2
            y_adjust_div: Q32(0), // lerp(2, 100, 0) = 2 → yAdjust = h/2
        };
        let path = cutting_bezier_path(&cut, 200_000, 100_000);
        // lr = 500_000, yAdjust = 100_000: yBottom = 400_000, yTop = 200_000,
        // yMid = 700_000, cxLeft = 0, cxRight = 1_000_000.
        assert_eq!(
            path,
            "M 0, 4000 C 0 4000, 5000 7000, 5000 2000C 5000 7000, 10000 4000, 10000 4000Z"
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
        let (path, _tx, _ty, scale, degrees, rotate_center) =
            voronoi_placement(&voronoi, 900_000, 400_000);
        assert_eq!(path, VORONOI_PATTERNS[0]);
        assert_eq!(degrees, 90);
        // Scale is at least the body-width ratio times canvas/255.
        let stroke_unit = div_round_half_away(i128::from(CANVAS_MILLI), 255) as i64;
        let min_scale = q32_mul(q32_ratio(900_000), stroke_unit);
        assert!(scale >= min_scale);
        assert!(rotate_center > 0);
    }

    #[test]
    fn mask_content_wraps_the_path_in_three_groups() {
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
        let content = mask_content(&pattern, 500_000, 200_000, 100_000).unwrap();
        assert_eq!(content.matches("<g").count(), 3);
        assert_eq!(content.matches("</g>").count(), 3);
        assert!(content.contains("fill=\"#efefef\""));
        assert!(content.contains("rotate(0 5000 5000)"));
    }
}
