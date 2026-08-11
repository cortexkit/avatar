//! Three-point eyebrow splines derived from face.tsx 337–377.

use avatar_phenotype::{Face, Palette};

use crate::eyes::EyePair;
use crate::face::FaceRect;
use crate::fixed::{bam_degrees_q32, div_i64, opposite_bam, q32_mul, CANVAS_MILLI, Q32_ONE};
use crate::format::{format_milli, format_q32_decimal};
use crate::geometry::{build_spline, path_string, RingPoint};
use crate::mood::MoodDeltas;

pub(crate) fn render(
    face: &Face,
    rect: FaceRect,
    eyes: &EyePair,
    mood: MoodDeltas,
    palette: &Palette,
) -> String {
    let left_points = neutral_points(face, rect, eyes);
    let right_points = left_points.map(|point| RingPoint {
        x: CANVAS_MILLI - point.x,
        ..point
    });
    let left_path = path_string(&build_spline(&left_points));
    let right_path = path_string(&build_spline(&right_points));
    let shift_y = q32_mul(rect.height, mood.brow_shift_y.0);
    let left_center = brow_center(&left_points);
    let right_center = brow_center(&right_points);
    let left_angle = format_q32_decimal(bam_degrees_q32(mood.brow_rotation));
    let right_angle = format_q32_decimal(bam_degrees_q32(opposite_bam(mood.brow_rotation)));
    format!(
        "  <path id=\"av-brow-left\" d=\"{left_path}\" transform=\"translate({},{}) rotate({left_angle},{},{})\" fill=\"{}\"/>\n  <path id=\"av-brow-right\" d=\"{right_path}\" transform=\"translate({},{}) rotate({right_angle},{},{})\" fill=\"{}\"/>\n",
        format_milli(0),
        format_milli(shift_y),
        format_milli(left_center.0),
        format_milli(left_center.1),
        hex(palette.secondary_darker),
        format_milli(0),
        format_milli(shift_y),
        format_milli(right_center.0),
        format_milli(right_center.1),
        hex(palette.secondary_darker),
    )
}

fn neutral_points(face: &Face, rect: FaceRect, eyes: &EyePair) -> [RingPoint; 3] {
    let eye_width = eyes.dimensions.width;
    // face.tsx maps the brow width coefficient to 0.4..1.2 before coupling it
    // to eye width; the phenotype stores the normalized controlling axis.
    let width_factor = 1_717_986_918 + q32_mul(3_435_973_837, face.brows.width_coupling.0);
    let total = q32_mul(eye_width, width_factor);
    let center_x = eyes.left.center_x - div_i64(eye_width, 10);
    let peak = div_i64(rect.height, 12);
    let eye_height = eyes.dimensions.upper + eyes.dimensions.lower;
    let low_clearance = div_i64(eye_height, 16);
    let high_clearance = div_i64(eye_height, 10);
    let clearance = low_clearance
        + q32_mul(high_clearance - low_clearance, face.brows.clearance.0)
        + eyes.lash_length;
    let top = eyes.left.outline_top.cp1y - peak - clearance;
    let first_y = top + q32_mul(peak * 2, face.brows.end_height1.0) - peak;
    let last_y = top + q32_mul(peak * 2, face.brows.end_height2.0) - peak;
    let mid_min = -div_i64(total, 5);
    let mid_max = div_i64(total, 4);
    let mid_offset = mid_min + q32_mul(mid_max - mid_min, face.brows.mid_x.0);

    [
        RingPoint {
            x: center_x - div_i64(total, 2),
            y: first_y,
            tension: face.brows.tensions[0].0.clamp(0, Q32_ONE),
        },
        RingPoint {
            x: center_x + mid_offset,
            y: top - peak,
            tension: face.brows.tensions[1].0.clamp(0, Q32_ONE),
        },
        RingPoint {
            x: center_x + div_i64(total, 2),
            y: last_y,
            tension: face.brows.tensions[2].0.clamp(0, Q32_ONE),
        },
    ]
}

fn brow_center(points: &[RingPoint; 3]) -> (i64, i64) {
    let min_x = points.iter().map(|point| point.x).min().unwrap_or(0);
    let max_x = points.iter().map(|point| point.x).max().unwrap_or(0);
    let min_y = points.iter().map(|point| point.y).min().unwrap_or(0);
    let max_y = points.iter().map(|point| point.y).max().unwrap_or(0);
    (div_i64(min_x + max_x, 2), div_i64(min_y + max_y, 2))
}

fn hex(color: avatar_phenotype::Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}
