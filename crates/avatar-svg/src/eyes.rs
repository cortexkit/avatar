//! Paired eye geometry and SVG assembly.

use avatar_phenotype::{Eyes, Face, Palette};
use avatar_tables::div_round_half_away;

use crate::face::{face_width, FaceRect};
use crate::fixed::{bam_degrees_q32, div_i64, opposite_bam, q32_mul, ratio_q32, Q32_ONE};
use crate::format::{format_milli, format_q32_decimal};
use crate::lashes::{self, Cubic, EyeSide};
use crate::mood::MoodDeltas;

const Q32_ONE_POINT_TWO: i64 = 5_153_960_755;
const Q32_TWO_POINT_EIGHT: i64 = 12_025_908_429;
const Q32_ZERO_POINT_THREE: i64 = 1_288_490_189;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EyeDimensions {
    pub width: i64,
    pub upper: i64,
    pub lower: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EyeGeometry {
    pub outline_top: Cubic,
    pub outline_bottom: Cubic,
    pub upper_lash_curve: Cubic,
    pub center_x: i64,
    pub center_y: i64,
    pub iris_radius: i64,
    pub pupil_radius: i64,
    pub upper_close: i64,
    pub lower_close: i64,
    pub rotation: avatar_phenotype::Bam16,
    pub side: EyeSide,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EyePair {
    pub left: EyeGeometry,
    pub right: EyeGeometry,
    pub dimensions: EyeDimensions,
    pub lash_length: i64,
    pub lower_edge_y: i64,
    pub gap: i64,
}

/// Resolves the pre-face-rect dimensions used by getFaceRect's eye-height cap.
pub(crate) fn preliminary_dimensions(face: &Face, body_width: i64) -> EyeDimensions {
    let available = face_width(body_width, face);
    let width = q32_mul(available, face.eyes.size_h.0)
        .checked_div(3)
        .unwrap_or(0)
        .max(div_i64(available, 5))
        .max(1);
    let upper_divisor = Q32_ONE_POINT_TWO + q32_mul(Q32_TWO_POINT_EIGHT, face.eyes.height_upper.0);
    let lower_divisor = Q32_ONE_POINT_TWO + q32_mul(Q32_TWO_POINT_EIGHT, face.eyes.height_lower.0);
    let mut upper = divide_by_q32(width, upper_divisor.max(1));
    let mut lower = divide_by_q32(width, lower_divisor.max(1));
    if face.eyes.height_bias_gate {
        let bias = q32_mul(div_i64(upper, 3), face.eyes.height_bias.0);
        upper += bias;
        lower = (lower - bias).max(1);
    }
    EyeDimensions {
        width,
        upper: upper.max(1),
        lower: lower.max(1),
    }
}

impl EyePair {
    /// Computes neutral outlines first, then adds lid closures, shifts the
    /// eyeballs, assigns opposed group rotations, and raises the lower contour.
    pub(crate) fn new(
        face: &Face,
        rect: FaceRect,
        mut dimensions: EyeDimensions,
        mood: MoodDeltas,
    ) -> Self {
        let total_height = dimensions.upper + dimensions.lower;
        if total_height > rect.height {
            let scale = ratio_q32(rect.height, total_height);
            dimensions.width = q32_mul(dimensions.width, scale).max(1);
            dimensions.upper = q32_mul(dimensions.upper, scale).max(1);
            dimensions.lower = q32_mul(dimensions.lower, scale).max(1);
        }

        let eye = &face.eyes;
        let lash_length = q32_mul(
            dimensions.upper + dimensions.lower,
            face.lashes.length_scale.0,
        );
        let remaining = (rect.width - dimensions.width * 2).max(0);
        let gap = q32_mul(remaining, eye.ipd.0).max(0);
        let face_x = crate::fixed::CENTER - div_i64(rect.width, 2);
        let left_start_x = face_x + div_i64(rect.width - (dimensions.width * 2 + gap), 2);
        let pull = if eye.vertical_pull_gate {
            q32_mul(
                dimensions.upper,
                q32_mul(eye.vertical_pull.0, Q32_ZERO_POINT_THREE),
            )
        } else {
            0
        };
        let start_y = rect.y + dimensions.upper - pull;
        let end_y = rect.y + dimensions.upper;
        let top_y = rect.y;
        let contour_raise = q32_mul(dimensions.lower, mood.lower_contour_raise.0);
        let bottom_y = rect.y + dimensions.upper + dimensions.lower - contour_raise;
        let inset1 = q32_mul(div_i64(dimensions.width, 2), eye.contour_inset1.0);
        let inset2 = q32_mul(div_i64(dimensions.width, 2), eye.contour_inset2.0);

        // face.tsx 569–596 defines the left cubic and obtains the right eye by
        // reflection. Keeping one source curve prevents side-specific drift.
        let left_top = Cubic {
            x0: left_start_x,
            y0: start_y,
            cp1x: left_start_x + inset1,
            cp1y: top_y,
            cp2x: left_start_x + dimensions.width - inset2,
            cp2y: top_y,
            x3: left_start_x + dimensions.width,
            y3: end_y,
        };
        let left_bottom = Cubic {
            cp1y: bottom_y,
            cp2y: bottom_y,
            ..left_top
        };

        let upper_factor = clamp_q32(eye.lid_close_upper_neutral.0 + mood.upper_lid_close.0);
        let lower_factor = clamp_q32(eye.lid_close_lower_neutral.0 + mood.lower_lid_close.0);
        let upper_close = q32_mul(dimensions.upper, upper_factor);
        let lower_close = q32_mul(dimensions.lower, lower_factor);
        let upper_lash_curve = Cubic {
            cp1y: left_top.cp1y + upper_close,
            cp2y: left_top.cp2y + upper_close,
            ..left_top
        };

        let neutral_center_x = left_start_x + div_i64(dimensions.width, 2);
        let neutral_center_y = rect.y + div_i64(dimensions.upper + dimensions.lower, 2);
        let shift_x = q32_mul(dimensions.width, mood.eyeball_shift_x.0);
        let shift_y = q32_mul(dimensions.upper + dimensions.lower, mood.eyeball_shift_y.0);
        let iris_radius = q32_mul(
            div_i64(dimensions.upper + dimensions.lower, 4),
            eye.eyeball_radius_scale.0,
        )
        .max(1);
        let pupil_radius = q32_mul(iris_radius, eye.pupil_scale.0).clamp(1, iris_radius);

        let left = EyeGeometry {
            outline_top: left_top,
            outline_bottom: left_bottom,
            upper_lash_curve,
            center_x: neutral_center_x + shift_x,
            center_y: neutral_center_y + shift_y,
            iris_radius,
            pupil_radius,
            upper_close,
            lower_close,
            rotation: mood.eye_rotation,
            side: EyeSide::Left,
        };
        let right = mirror_eye(left);

        Self {
            left,
            right,
            dimensions,
            lash_length,
            lower_edge_y: bottom_y,
            gap,
        }
    }
}

pub(crate) fn render_defs(pair: &EyePair, eyes: &Eyes) -> String {
    let left_path = outline_path(pair.left);
    let right_path = outline_path(pair.right);
    let freq_h = 42_949_673 + q32_mul(386_547_057, eyes.noise_freq_h.0); // 0.01..0.10.
    let freq_v = 42_949_673 + q32_mul(386_547_057, eyes.noise_freq_v.0);
    let displacement = 3_000 + q32_mul(47_000, eyes.iris_shade_reduce.0);
    format!(
        "  <defs>\n    <clipPath id=\"av-eye-left-clip\"><path d=\"{left_path}\"/></clipPath>\n    <clipPath id=\"av-eye-right-clip\"><path d=\"{right_path}\"/></clipPath>\n    <filter id=\"av-eye-noise\"><feTurbulence type=\"fractalNoise\" stitchTiles=\"stitch\" baseFrequency=\"{} {}\" numOctaves=\"5\" seed=\"{}\" result=\"warp\"/><feDisplacementMap xChannelSelector=\"R\" yChannelSelector=\"G\" scale=\"{}\" in=\"SourceGraphic\" in2=\"warp\"/></filter>\n  </defs>\n",
        format_q32_decimal(freq_h),
        format_q32_decimal(freq_v),
        eyes.noise_seed,
        format_milli(displacement),
    )
}

pub(crate) fn render_eye(eye: EyeGeometry, genes: &Face, palette: &Palette) -> String {
    let side_name = match eye.side {
        EyeSide::Left => "left",
        EyeSide::Right => "right",
    };
    let (iris, lid) = colors(eye.side, &genes.eyes, palette);
    let shade_radius = eye.iris_radius
        - q32_mul(
            div_i64(eye.iris_radius * 2, 5),
            genes.eyes.iris_shade_reduce.0,
        );
    let rotation = format_q32_decimal(bam_degrees_q32(eye.rotation));
    let mut output = format!(
        "  <g id=\"av-eye-{side_name}\" transform=\"rotate({rotation},{},{})\">\n",
        format_milli(eye.center_x),
        format_milli(eye.center_y),
    );
    output.push_str(&format!(
        "    <path class=\"av-eyeball\" d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
        outline_path(eye),
        hex(palette.white),
        hex(palette.dark),
        format_milli(196),
    ));
    output.push_str(&format!(
        "    <g clip-path=\"url(#av-eye-{side_name}-clip)\">\n"
    ));
    output.push_str(&format!(
        "      <circle class=\"av-iris\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
        format_milli(eye.center_x),
        format_milli(eye.center_y),
        format_milli(eye.iris_radius),
        hex(iris),
        hex(palette.dark),
        format_milli(80),
    ));
    output.push_str(&format!(
        "      <ellipse class=\"av-iris-shade\" cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{}\" opacity=\"{}\" filter=\"url(#av-eye-noise)\"/>\n",
        format_milli(eye.center_x),
        format_milli(eye.center_y),
        format_milli(shade_radius.max(1)),
        format_milli(div_i64(shade_radius.max(1) * 5, 6)),
        hex(palette.primary_darkest),
        format_q32_decimal(genes.eyes.iris_shade_opacity.0),
    ));
    output.push_str(&format!(
        "      <circle class=\"av-pupil\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>\n",
        format_milli(eye.center_x),
        format_milli(eye.center_y),
        format_milli(eye.pupil_radius),
        hex(palette.dark),
    ));
    highlight(&mut output, eye, -5, -5, 12, palette.white, 3_865_470_566);
    highlight(&mut output, eye, 6, 6, 19, palette.harmonic1, 1_288_490_189);
    highlight(&mut output, eye, 5, 5, 28, palette.white, 3_435_973_837);
    output.push_str("    </g>\n");
    output.push_str(&format!(
        "    <path class=\"av-upper-lid\" d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
        upper_lid_path(eye, genes.eyes.lid_extension_gate, genes.eyes.lid_extension_side),
        hex(lid),
        hex(palette.dark),
        format_milli(196),
    ));
    output.push_str(&format!(
        "    <path class=\"av-lower-lid\" d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
        lower_lid_path(eye, genes.eyes.lid_extension_gate, genes.eyes.lid_extension_side),
        hex(lid),
        hex(palette.dark),
        format_milli(196),
    ));
    output.push_str(&lashes::render(
        &genes.lashes,
        eye.upper_lash_curve,
        eye.outline_bottom.cp1y - eye.outline_top.cp1y,
        eye.side,
        palette.dark,
    ));
    output.push_str("  </g>\n");
    output
}

fn mirror_eye(left: EyeGeometry) -> EyeGeometry {
    EyeGeometry {
        outline_top: mirror_curve(left.outline_top),
        outline_bottom: mirror_curve(left.outline_bottom),
        upper_lash_curve: mirror_curve(left.upper_lash_curve),
        center_x: crate::fixed::CANVAS_MILLI - left.center_x,
        rotation: opposite_bam(left.rotation),
        side: EyeSide::Right,
        ..left
    }
}

fn mirror_curve(curve: Cubic) -> Cubic {
    Cubic {
        x0: crate::fixed::CANVAS_MILLI - curve.x0,
        cp1x: crate::fixed::CANVAS_MILLI - curve.cp1x,
        cp2x: crate::fixed::CANVAS_MILLI - curve.cp2x,
        x3: crate::fixed::CANVAS_MILLI - curve.x3,
        ..curve
    }
}

fn outline_path(eye: EyeGeometry) -> String {
    let top = eye.outline_top;
    let bottom = eye.outline_bottom;
    format!(
        "M{} {} C{} {},{} {},{} {} C{} {},{} {},{} {} Z",
        format_milli(top.x0),
        format_milli(top.y0),
        format_milli(top.cp1x),
        format_milli(top.cp1y),
        format_milli(top.cp2x),
        format_milli(top.cp2y),
        format_milli(top.x3),
        format_milli(top.y3),
        format_milli(bottom.cp2x),
        format_milli(bottom.cp2y),
        format_milli(bottom.cp1x),
        format_milli(bottom.cp1y),
        format_milli(bottom.x0),
        format_milli(bottom.y0),
    )
}

fn upper_lid_path(eye: EyeGeometry, gate: u8, side_gene: u8) -> String {
    let curve = eye.outline_top;
    let (top_extension, _) = lid_extensions(curve, gate, side_gene);
    let signed_extension = match eye.side {
        EyeSide::Left => -top_extension,
        EyeSide::Right => top_extension,
    };
    format!(
        "M{} {} C{} {},{} {},{} {} C{} {},{} {},{} {} Z",
        format_milli(curve.x0 + signed_extension),
        format_milli(curve.y0),
        format_milli(curve.cp1x + signed_extension),
        format_milli(curve.cp1y),
        format_milli(curve.cp2x),
        format_milli(curve.cp2y),
        format_milli(curve.x3),
        format_milli(curve.y3),
        format_milli(curve.cp2x),
        format_milli(curve.cp2y + eye.upper_close),
        format_milli(curve.cp1x),
        format_milli(curve.cp1y + eye.upper_close),
        format_milli(curve.x0),
        format_milli(curve.y0),
    )
}

fn lower_lid_path(eye: EyeGeometry, gate: u8, side_gene: u8) -> String {
    let curve = eye.outline_bottom;
    let (_, lower_extension) = lid_extensions(curve, gate, side_gene);
    let signed_extension = match eye.side {
        EyeSide::Left => -lower_extension,
        EyeSide::Right => lower_extension,
    };
    format!(
        "M{} {} C{} {},{} {},{} {} C{} {},{} {},{} {} Z",
        format_milli(curve.x0 + signed_extension),
        format_milli(curve.y0),
        format_milli(curve.cp1x + signed_extension),
        format_milli(curve.cp1y),
        format_milli(curve.cp2x),
        format_milli(curve.cp2y),
        format_milli(curve.x3),
        format_milli(curve.y3),
        format_milli(curve.cp2x),
        format_milli(curve.cp2y - eye.lower_close),
        format_milli(curve.cp1x),
        format_milli(curve.cp1y - eye.lower_close),
        format_milli(curve.x0),
        format_milli(curve.y0),
    )
}

fn lid_extensions(curve: Cubic, gate: u8, side: u8) -> (i64, i64) {
    if gate <= 220 {
        return (0, 0);
    }
    let base = div_i64((curve.x3 - curve.x0).abs(), 30);
    let extension = base + div_i64(base * 3 * i64::from(gate - 220), 35);
    if side > 127 {
        (div_i64(extension, 3), extension)
    } else {
        (extension, div_i64(extension, 3))
    }
}

fn highlight(
    output: &mut String,
    eye: EyeGeometry,
    x_divisor: i64,
    y_divisor: i64,
    radius_divisor: i64,
    color: avatar_phenotype::Rgb,
    opacity: i64,
) {
    output.push_str(&format!(
        "      <circle class=\"av-highlight\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" opacity=\"{}\"/>\n",
        format_milli(eye.center_x + div_i64(eye.iris_radius, x_divisor)),
        format_milli(eye.center_y + div_i64(eye.iris_radius, y_divisor)),
        format_milli(div_i64(eye.iris_radius, radius_divisor).max(1)),
        hex(color),
        format_q32_decimal(opacity),
    ));
}

fn colors(
    side: EyeSide,
    eyes: &Eyes,
    palette: &Palette,
) -> (avatar_phenotype::Rgb, avatar_phenotype::Rgb) {
    let iris = match eyes.iris_color_gene {
        254..=u8::MAX => match side {
            EyeSide::Left => palette.harmonic3,
            EyeSide::Right => palette.harmonic4,
        },
        241..=253 => palette.harmonic4,
        236..=240 => palette.harmonic3,
        211..=235 => palette.harmonic1,
        0..=1 => match side {
            EyeSide::Left => palette.harmonic2,
            EyeSide::Right => palette.harmonic3,
        },
        _ => palette.harmonic2,
    };
    let lid = match eyes.lid_color_gene {
        254..=u8::MAX => match side {
            EyeSide::Left => palette.harmonic1,
            EyeSide::Right => palette.harmonic4,
        },
        241..=253 => match side {
            EyeSide::Left => palette.primary,
            EyeSide::Right => palette.harmonic1,
        },
        231..=240 => match side {
            EyeSide::Left => palette.harmonic1,
            EyeSide::Right => palette.primary,
        },
        211..=230 => match side {
            EyeSide::Left => palette.primary_darkest,
            EyeSide::Right => palette.primary,
        },
        0..=39 => palette.primary_darker,
        _ => palette.primary,
    };
    (reduce_saturation(iris, eyes.iris_saturation_reduce.0), lid)
}

fn reduce_saturation(color: avatar_phenotype::Rgb, amount: i64) -> avatar_phenotype::Rgb {
    let gray = div_i64(
        i64::from(color.r) + i64::from(color.g) + i64::from(color.b),
        3,
    );
    let channel = |value: u8| {
        (i64::from(value) + q32_mul(gray - i64::from(value), amount.clamp(0, Q32_ONE)))
            .clamp(0, 255) as u8
    };
    avatar_phenotype::Rgb {
        r: channel(color.r),
        g: channel(color.g),
        b: channel(color.b),
    }
}

fn divide_by_q32(value: i64, divisor: i64) -> i64 {
    div_round_half_away(i128::from(value) * i128::from(Q32_ONE), i128::from(divisor)) as i64
}

fn clamp_q32(value: i64) -> i64 {
    value.clamp(0, Q32_ONE)
}

fn hex(color: avatar_phenotype::Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}
