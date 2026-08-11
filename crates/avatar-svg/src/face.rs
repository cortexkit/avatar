//! Face placement and the bounded mouth-rectangle search.

use avatar_phenotype::Face;
use avatar_tables::div_round_half_away;

use crate::eyes;
use crate::fixed::{q32_mul, CANVAS_MILLI, CENTER};
use crate::geometry::BodyGeometry;

const MOUTH_SEARCH_MAX: u32 = 3_000;

/// Axis-aligned area reserved for one mouth renderer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MouthRect {
    pub x1: i64,
    pub y1: i64,
    pub x2: i64,
    pub y2: i64,
}

impl MouthRect {
    pub(crate) fn width(self) -> i64 {
        self.x2 - self.x1
    }

    pub(crate) fn height(self) -> i64 {
        self.y2 - self.y1
    }
}

/// DNA-neutral face rectangle plus the mouth-fit result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FaceRect {
    pub width: i64,
    pub height: i64,
    pub y: i64,
    pub mouth: MouthRect,
}

/// Selects the eye-line landmark exactly as face.tsx 179–195 does.
pub(crate) fn eye_line_anchor(position_mode: u8, body: &BodyGeometry) -> i64 {
    match position_mode {
        0 => body.min_width_y,
        1 => body.max_width_y,
        2 => body.top + div(body.body_height, 3),
        3 => body.top + div(body.body_height, 2),
        _ => body.max_width_y,
    }
}

/// Uses one shared face width for preliminary eye sizing and final placement.
pub(crate) fn face_width(body_width: i64, face: &Face) -> i64 {
    let mut width = q32_mul(div(body_width, 2), face.placement.width_coeff.0);
    let minimum = div(div(CANVAS_MILLI * 2, 5), 2); // canvas/2.5, then /2.
    width = width.max(minimum);
    width.min(div(CANVAS_MILLI * 10, 11)) // canvas/1.1.
}

/// Resolves getFaceRect and its always-enabled mouth search (face.tsx 160–335).
pub(crate) fn resolve_face_rect(body: &BodyGeometry, face: &Face, eyes_height: i64) -> FaceRect {
    let mut center_y = eye_line_anchor(face.eyes.position_mode, body);
    let width = face_width(body.body_width, face);
    let mut height = q32_mul(div(body.body_height, 6), face.placement.height_coeff.0);
    height = height.max(div(div(CANVAS_MILLI, 5), 2));

    // The 1.7 rule caps height when the face would otherwise become too tall.
    if i128::from(width) * 10 < i128::from(height) * 17 {
        height = div(width * 10, 17);
    }
    height = height.min(eyes_height.max(1));

    // This odd subtraction is preserved from face.tsx 228–230 rather than
    // replaced by a conventional clamp; short-body placement depends on it.
    if center_y + height > CANVAS_MILLI {
        center_y -= CANVAS_MILLI - center_y + height;
    }

    let left_space = body.top + body.body_height - center_y;
    let min_mouth_height = div(body.body_height, 15).max(div(CANVAS_MILLI, 20));
    let mut counter = 0_u32;
    let mut mouth = search_mouth_rows(
        body,
        center_y,
        height,
        left_space,
        min_mouth_height,
        &mut counter,
    );

    // face.tsx 284–318 retries from the body top without resetting either the
    // row count or the shared 3,000-point budget.
    if mouth.y1 >= body.top + body.body_height {
        center_y = body.top;
        mouth = search_mouth_rows(
            body,
            center_y,
            height,
            left_space,
            min_mouth_height,
            &mut counter,
        );
    }

    if mouth.y1 != 0 {
        center_y = mouth.y1 - div(height, 2);
    }

    FaceRect {
        width,
        height,
        y: center_y - div(height, 2),
        mouth,
    }
}

pub(crate) fn render_mouth_rect(
    body: &BodyGeometry,
    face_rect: FaceRect,
    eyes: &eyes::EyePair,
    face: &Face,
) -> MouthRect {
    let eye_distance = (eyes.right.center_x - eyes.left.center_x).abs();
    let mut half_width = div(div(eye_distance, 2).min(div(body.body_width, 7)), 2)
        .max(div(face_rect.width, 12))
        .max(1);
    if face.mouth.width_scale_gate {
        // face.tsx 827–830 widens by 0.8..2.0 when its feature gate is set.
        let factor = 3_435_973_837 + q32_mul(5_153_960_755, face.mouth.width_scale.0);
        half_width = q32_mul(half_width, factor).max(1);
    }

    let mut rect = face_rect.mouth;
    if rect.x1 == 0 || rect.width() <= 0 || rect.height() <= 0 {
        let height = div(body.body_height, 12).max(div(CANVAS_MILLI, 25));
        rect = MouthRect {
            x1: CENTER - half_width,
            y1: eyes.lower_edge_y + div(face_rect.height, 4),
            x2: CENTER + half_width,
            y2: eyes.lower_edge_y + div(face_rect.height, 4) + height,
        };
    } else {
        rect.x1 = CENTER - half_width;
        rect.x2 = CENTER + half_width;
    }

    if face.mouth.aspect_widen_gate && rect.width() * 20 < rect.height() * 23 {
        let required_width = div(rect.height() * 23, 20);
        rect.x1 = CENTER - div(required_width, 2);
        rect.x2 = CENTER + div(required_width, 2);
    }

    // face.tsx 858–865 raises the mouth only for exceptionally short bodies.
    let eye_span = eyes.right.outline_top.x3 - eyes.left.outline_top.x3;
    if eye_span * 10 > rect.width() * 11 && body.body_height < div(CANVAS_MILLI, 20) {
        rect.y1 -= div(face_rect.height, 2);
        rect.y2 -= div(face_rect.height, 2);
    }
    rect
}

fn search_mouth_rows(
    body: &BodyGeometry,
    center_y: i64,
    face_height: i64,
    left_space: i64,
    min_mouth_height: i64,
    counter: &mut u32,
) -> MouthRect {
    let mut widest = MouthRect::default();
    let rows = ceil_positive(left_space, min_mouth_height);
    for row in 0..rows {
        let top = center_y + div(face_height, 2) + row * min_mouth_height;
        let candidate = find_rect(
            &body.all_points,
            body.body_width,
            body.body_height,
            top,
            min_mouth_height,
            counter,
        );
        if *counter > MOUTH_SEARCH_MAX {
            return MouthRect::default();
        }
        let area = i128::from(candidate.width()) * i128::from(candidate.height());
        let threshold = i128::from(body.body_width) * i128::from(body.body_height) / 80;
        if area > threshold {
            return candidate;
        }
        if candidate.width() > widest.width() {
            widest = candidate;
        }
    }
    widest
}

/// Iterative transcription of findRect (face.tsx 91–140).
///
/// `counter` is incremented for every point inspected, including the scan that
/// crosses 3,000. The next loop iteration observes the overrun, matching the
/// reference recursion's entry guard while guaranteeing a finite search.
pub(crate) fn find_rect(
    points: &[(i64, i64)],
    body_width: i64,
    _body_height: i64,
    top: i64,
    min_mouth_height: i64,
    counter: &mut u32,
) -> MouthRect {
    let mut active = points.to_vec();
    let mut reduction_percent = 5_i64;

    loop {
        let inset = div(body_width * reduction_percent, 100);
        let rect = MouthRect {
            x1: CENTER - div(body_width, 2) + inset,
            y1: top,
            x2: CENTER + div(body_width, 2) - inset,
            y2: top + min_mouth_height,
        };
        if rect.x1 > rect.x2 || *counter > MOUTH_SEARCH_MAX {
            return rect;
        }

        let mut inside = Vec::new();
        for &(x, y) in &active {
            *counter = counter.saturating_add(1);
            if x >= rect.x1 && x <= rect.x2 && y >= rect.y1 && y <= rect.y2 {
                inside.push((x, y));
            }
        }
        if inside.is_empty() {
            return rect;
        }
        active = inside;
        reduction_percent += 2;
    }
}

fn ceil_positive(value: i64, divisor: i64) -> i64 {
    if value <= 0 || divisor <= 0 {
        0
    } else {
        (value + divisor - 1) / divisor
    }
}

fn div(value: i64, divisor: i64) -> i64 {
    div_round_half_away(i128::from(value), i128::from(divisor)) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use avatar_phenotype::{Brows, Eyes, FacePlacement, Lashes, Mouth, Q32};

    use crate::tree::{FillRule, Shape};

    fn geometry() -> BodyGeometry {
        BodyGeometry {
            shape: Shape::Path {
                segments: Vec::new(),
                closed: false,
                fill_rule: FillRule::NonZero,
            },
            body_width: 600_000,
            body_height: 600_000,
            top: 100_000,
            bottom: 700_000,
            max_width_y: 420_000,
            min_width_y: 240_000,
            all_points: Vec::new(),
        }
    }

    fn face(position_mode: u8) -> Face {
        Face {
            eyes: Eyes {
                position_mode,
                size_h: Q32::ONE,
                ipd: Q32::ONE,
                height_upper: Q32::ONE,
                height_lower: Q32::ONE,
                height_bias_gate: false,
                height_bias: Q32(0),
                vertical_pull_gate: false,
                vertical_pull: Q32(0),
                contour_inset1: Q32(0),
                contour_inset2: Q32(0),
                eyeball_radius_scale: Q32::ONE,
                pupil_scale: Q32::ONE,
                iris_shade_opacity: Q32(0),
                iris_shade_reduce: Q32(0),
                iris_saturation_reduce: Q32(0),
                lid_close_upper_neutral: Q32(0),
                lid_close_lower_neutral: Q32(0),
                lid_extension_gate: 0,
                lid_extension_side: 0,
                noise_freq_h: Q32(0),
                noise_freq_v: Q32(0),
                noise_seed: 0,
                rotation_seed: 0,
                lid_color_gene: 0,
                iris_color_gene: 0,
            },
            brows: Brows {
                end_height1: Q32(0),
                mid_x: Q32(0),
                end_height2: Q32(0),
                tensions: [Q32::ONE; 3],
                width_coupling: Q32::ONE,
                clearance: Q32(0),
            },
            lashes: Lashes {
                count: 0,
                removed: 0,
                length_scale: Q32(0),
                stroke_scale: Q32(0),
                direction: 0,
            },
            mouth: Mouth {
                family: 0,
                width_scale_gate: false,
                width_scale: Q32(0),
                aspect_widen_gate: false,
                params: [Q32(0); 8],
            },
            placement: FacePlacement {
                width_coeff: Q32::ONE,
                height_coeff: Q32::ONE,
            },
        }
    }

    #[test]
    fn position_modes_select_the_five_pinned_landmarks() {
        let body = geometry();
        let expected = [240_000, 420_000, 300_000, 400_000, 420_000];
        for (mode, expected_y) in expected.into_iter().enumerate() {
            assert_eq!(eye_line_anchor(mode as u8, &body), expected_y);
            assert_eq!(face(mode as u8).eyes.position_mode, mode as u8);
        }
    }

    #[test]
    fn empty_row_accepts_the_reference_five_percent_inset() {
        let mut counter = 0;
        let rect = find_rect(&[], 600_000, 600_000, 300_000, 50_000, &mut counter);
        assert_eq!(
            rect,
            MouthRect {
                x1: 230_000,
                y1: 300_000,
                x2: 770_000,
                y2: 350_000,
            }
        );
        assert_eq!(counter, 0);
    }

    #[test]
    fn point_sliver_terminates_with_some_rectangle() {
        let points = vec![(500_000, 500_000); 2];
        let mut counter = 0;
        let rect = find_rect(&points, 2, 2, 500_000, 50_000, &mut counter);
        assert!(counter <= MOUTH_SEARCH_MAX + points.len() as u32);
        assert!(rect.y2 > rect.y1);
    }
}
