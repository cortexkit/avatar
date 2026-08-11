//! Eight dense mouth-shape families transcribed from the legacy TOG renderer's
//! sparsely numbered mouth variants.

use avatar_phenotype::{Mouth, Palette};

use crate::face::MouthRect;
use crate::fixed::{div_i64, q32_mul, Q32_ONE};
use crate::format::format_milli;
use crate::mood::MoodDeltas;

#[derive(Clone, Debug, PartialEq, Eq)]
struct MouthShape {
    path: String,
    points: Vec<(i64, i64)>,
    fill: String,
    stroke: String,
}

#[derive(Clone, Copy)]
struct Metrics {
    rect: MouthRect,
    width: i64,
    height: i64,
    center_x: i64,
    center_y: i64,
}

pub(crate) fn render(
    mouth: &Mouth,
    rect: MouthRect,
    mood: MoodDeltas,
    palette: &Palette,
) -> String {
    let shape = shape(mouth, rect, mood, palette);
    format!(
        "  <g id=\"av-mouth\" data-family=\"{}\"><path d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\"/></g>\n",
        mouth.family % 8,
        shape.path,
        shape.fill,
        shape.stroke,
        format_milli(196),
    )
}

fn shape(mouth: &Mouth, rect: MouthRect, mood: MoodDeltas, palette: &Palette) -> MouthShape {
    let metrics = Metrics::new(rect);
    match mouth.family % 8 {
        0 => mouth1(mouth, metrics, mood, palette),
        1 => mouth3(mouth, metrics, mood, palette),
        2 => mouth4(mouth, metrics, mood, palette),
        3 => mouth5(mouth, metrics, mood, palette),
        4 => mouth6(mouth, metrics, mood, palette),
        5 => mouth7(mouth, metrics, mood, palette),
        6 => mouth23(mouth, metrics, mood, palette),
        _ => mouth233(mouth, metrics, mood, palette),
    }
}

impl Metrics {
    fn new(rect: MouthRect) -> Self {
        let width = rect.width().max(1);
        let height = rect.height().max(1);
        Self {
            rect,
            width,
            height,
            center_x: div_i64(rect.x1 + rect.x2, 2),
            center_y: div_i64(rect.y1 + rect.y2, 2),
        }
    }

    fn x(self, factor: i64) -> i64 {
        self.rect.x1 + q32_mul(self.width, factor.clamp(0, Q32_ONE))
    }

    fn y(self, factor: i64) -> i64 {
        self.rect.y1 + q32_mul(self.height, factor.clamp(0, Q32_ONE))
    }

    fn clamp(self, point: (i64, i64)) -> (i64, i64) {
        (
            point.0.clamp(self.rect.x1, self.rect.x2),
            point.1.clamp(self.rect.y1, self.rect.y2),
        )
    }
}

/// TOG Mouth1: params[0] baseline, [1] natural bend, [2]/[3] control x;
/// params[4..7] are reserved. Positive mood curve lowers both controls, making
/// the eye-anchored line read as a smile without moving its endpoints.
fn mouth1(mouth: &Mouth, m: Metrics, mood: MoodDeltas, palette: &Palette) -> MouthShape {
    let baseline = m.y(1_431_655_765 + q32_mul(1_431_655_765, p(mouth, 0)));
    let neutral = q32_mul(div_i64(m.height, 3), p(mouth, 1) - div_i64(Q32_ONE, 2));
    let mood_shift = q32_mul(div_i64(m.height, 4), mood.mouth_curve.0);
    let cp_y = (baseline + neutral + mood_shift).clamp(m.rect.y1, m.rect.y2);
    let points = vec![
        (m.rect.x1, baseline),
        (m.x(858_993_459 + q32_mul(1_431_655_765, p(mouth, 2))), cp_y),
        (
            m.x(2_435_496_789 + q32_mul(1_001_390_507, p(mouth, 3))),
            cp_y,
        ),
        (m.rect.x2, baseline),
    ];
    open_shape(points, "none".to_owned(), hex(palette.dark))
}

/// TOG Mouth3: params[0] vertical placement, [1] DNA curvature, [2] left/right
/// skew, [3] control spread. Mood curve adds to the simple curve's two control
/// heights; params[4..7] remain available for later teeth detail.
fn mouth3(mouth: &Mouth, m: Metrics, mood: MoodDeltas, palette: &Palette) -> MouthShape {
    let baseline = m.y(1_288_490_189 + q32_mul(1_717_986_918, p(mouth, 0)));
    let neutral = q32_mul(div_i64(m.height, 3), p(mouth, 1) - div_i64(Q32_ONE, 2));
    let mood_shift = q32_mul(div_i64(m.height, 3), mood.mouth_curve.0);
    let skew = q32_mul(div_i64(m.height, 6), p(mouth, 2) - div_i64(Q32_ONE, 2));
    let spread = 1_073_741_824 + q32_mul(1_073_741_824, p(mouth, 3));
    let points = vec![
        (m.rect.x1, baseline),
        (
            m.x(spread),
            (baseline + neutral + mood_shift + skew).clamp(m.rect.y1, m.rect.y2),
        ),
        (
            m.x(Q32_ONE - spread),
            (baseline + neutral + mood_shift - skew).clamp(m.rect.y1, m.rect.y2),
        ),
        (m.rect.x2, baseline),
    ];
    open_shape(points, "none".to_owned(), hex(palette.primary_darker))
}

/// TOG Mouth4: params[0]/[1] top and bottom thickness, [2] slant, [3]
/// horizontal control inset; params[4..7] are reserved. Mood curve moves the
/// top controls down and bottom controls up, changing the blade's expression.
fn mouth4(mouth: &Mouth, m: Metrics, mood: MoodDeltas, palette: &Palette) -> MouthShape {
    let top = m.center_y
        - q32_mul(
            div_i64(m.height, 3),
            1_073_741_824 + q32_mul(1_073_741_824, p(mouth, 0)),
        );
    let bottom = m.center_y
        + q32_mul(
            div_i64(m.height, 3),
            1_073_741_824 + q32_mul(1_073_741_824, p(mouth, 1)),
        );
    let slant = q32_mul(div_i64(m.height, 5), p(mouth, 2) - div_i64(Q32_ONE, 2));
    let curve = q32_mul(div_i64(m.height, 5), mood.mouth_curve.0);
    let inset = 644_245_094 + q32_mul(644_245_094, p(mouth, 3));
    closed_shape(
        vec![
            m.clamp((m.rect.x1, m.center_y + slant)),
            m.clamp((m.x(inset), top + curve)),
            m.clamp((m.x(Q32_ONE - inset), top - curve)),
            m.clamp((m.rect.x2, m.center_y - slant)),
            m.clamp((m.x(Q32_ONE - inset), bottom - curve)),
            m.clamp((m.x(inset), bottom + curve)),
        ],
        hex(palette.primary_darker),
        hex(palette.dark),
    )
}

/// TOG Mouth5 (teeth): params[0] opening, [1] upper crown, [2] lower crown,
/// [3] corner inset; params[4..7] are reserved for individual teeth. Positive
/// mood curve lowers the lower crown and raises the upper crown into a grin.
fn mouth5(mouth: &Mouth, m: Metrics, mood: MoodDeltas, palette: &Palette) -> MouthShape {
    let opening = div_i64(m.height, 8) + q32_mul(div_i64(m.height, 4), p(mouth, 0));
    let upper_crown = q32_mul(div_i64(m.height, 6), p(mouth, 1));
    let lower_crown = q32_mul(div_i64(m.height, 6), p(mouth, 2));
    let curve = q32_mul(div_i64(m.height, 6), mood.mouth_curve.0);
    let inset = 429_496_730 + q32_mul(429_496_730, p(mouth, 3));
    closed_shape(
        vec![
            (m.x(inset), m.center_y),
            m.clamp((
                m.x(1_431_655_765),
                m.center_y - opening - upper_crown - curve,
            )),
            m.clamp((
                m.x(2_863_311_531),
                m.center_y - opening - upper_crown - curve,
            )),
            (m.x(Q32_ONE - inset), m.center_y),
            m.clamp((
                m.x(2_863_311_531),
                m.center_y + opening + lower_crown + curve,
            )),
            m.clamp((
                m.x(1_431_655_765),
                m.center_y + opening + lower_crown + curve,
            )),
        ],
        hex(palette.white),
        hex(palette.dark),
    )
}

/// TOG Mouth6: params[0] top bulge, [1] bottom bulge, [2] asymmetry, [3]
/// control inset; params[4..7] are reserved. Mood curve shifts the two top
/// controls oppositely, preserving the component's characteristic skew.
fn mouth6(mouth: &Mouth, m: Metrics, mood: MoodDeltas, palette: &Palette) -> MouthShape {
    let top = m.y(1_073_741_824);
    let bottom = m.y(3_221_225_472);
    let top_bulge = q32_mul(div_i64(m.height, 4), p(mouth, 0));
    let bottom_bulge = q32_mul(div_i64(m.height, 4), p(mouth, 1));
    let asymmetry = q32_mul(div_i64(m.height, 5), p(mouth, 2) - div_i64(Q32_ONE, 2));
    let curve = q32_mul(div_i64(m.height, 5), mood.mouth_curve.0);
    let inset = 858_993_459 + q32_mul(429_496_730, p(mouth, 3));
    closed_shape(
        vec![
            (m.rect.x1, m.center_y),
            m.clamp((m.x(inset), top - top_bulge + asymmetry + curve)),
            m.clamp((m.x(Q32_ONE - inset), top - top_bulge - asymmetry - curve)),
            (m.rect.x2, m.center_y),
            m.clamp((m.x(Q32_ONE - inset), bottom + bottom_bulge - asymmetry)),
            m.clamp((m.x(inset), bottom + bottom_bulge + asymmetry)),
        ],
        hex(palette.harmonic1),
        hex(palette.dark),
    )
}

/// TOG Mouth7 (open oval): params[0]/[1] top and bottom bulges, [2] side
/// height, [3] horizontal inset; params[4..7] are reserved. Mood curve expands
/// or contracts the bottom controls while all points stay inside the fit rect.
fn mouth7(mouth: &Mouth, m: Metrics, mood: MoodDeltas, palette: &Palette) -> MouthShape {
    let top = m.center_y - div_i64(m.height, 5) - q32_mul(div_i64(m.height, 4), p(mouth, 0));
    let bottom = m.center_y + div_i64(m.height, 5) + q32_mul(div_i64(m.height, 4), p(mouth, 1));
    let side_shift = q32_mul(div_i64(m.height, 8), p(mouth, 2) - div_i64(Q32_ONE, 2));
    let curve = q32_mul(div_i64(m.height, 6), mood.mouth_curve.0);
    let inset = 429_496_730 + q32_mul(644_245_094, p(mouth, 3));
    closed_shape(
        vec![
            (m.x(inset), m.center_y + side_shift),
            m.clamp((m.x(1_431_655_765), top)),
            m.clamp((m.x(2_863_311_531), top)),
            (m.x(Q32_ONE - inset), m.center_y - side_shift),
            m.clamp((m.x(2_863_311_531), bottom + curve)),
            m.clamp((m.x(1_431_655_765), bottom + curve)),
        ],
        hex(palette.primary_darkest),
        hex(palette.dark),
    )
}

/// TOG Mouth23 (wide): params[0] edge inset, [1] opening, [2] center pull,
/// [3] vertical placement; params[4..7] are reserved. Mood curve separates the
/// upper and lower controls equally so the wide corners remain fixed.
fn mouth23(mouth: &Mouth, m: Metrics, mood: MoodDeltas, palette: &Palette) -> MouthShape {
    let inset = q32_mul(div_i64(Q32_ONE, 10), p(mouth, 0));
    let center = m.y(1_288_490_189 + q32_mul(1_717_986_918, p(mouth, 3)));
    let opening = div_i64(m.height, 8) + q32_mul(div_i64(m.height, 5), p(mouth, 1));
    let pull = q32_mul(div_i64(m.height, 6), p(mouth, 2));
    let curve = q32_mul(div_i64(m.height, 5), mood.mouth_curve.0);
    closed_shape(
        vec![
            (m.x(inset), center),
            m.clamp((m.x(1_073_741_824), center - opening - pull - curve)),
            m.clamp((m.x(3_221_225_472), center - opening - pull - curve)),
            (m.x(Q32_ONE - inset), center),
            m.clamp((m.x(3_221_225_472), center + opening + pull + curve)),
            m.clamp((m.x(1_073_741_824), center + opening + pull + curve)),
        ],
        hex(palette.primary_darkest),
        hex(palette.primary_darker),
    )
}

/// TOG Mouth233 (lipped): params[0] upper lip, [1] lower lip, [2] cupid bow,
/// [3] corner height; params[4..7] are reserved. Mood curve raises the cupid
/// bow and lowers the bottom center, exaggerating this family's lip profile.
fn mouth233(mouth: &Mouth, m: Metrics, mood: MoodDeltas, palette: &Palette) -> MouthShape {
    let upper = div_i64(m.height, 8) + q32_mul(div_i64(m.height, 6), p(mouth, 0));
    let lower = div_i64(m.height, 8) + q32_mul(div_i64(m.height, 5), p(mouth, 1));
    let bow = q32_mul(div_i64(m.height, 8), p(mouth, 2));
    let corner = q32_mul(div_i64(m.height, 8), p(mouth, 3) - div_i64(Q32_ONE, 2));
    let curve = q32_mul(div_i64(m.height, 6), mood.mouth_curve.0);
    let points = vec![
        m.clamp((m.rect.x1, m.center_y + corner)),
        m.clamp((m.x(1_073_741_824), m.center_y - upper - curve)),
        m.clamp((m.center_x, m.center_y - upper + bow - curve)),
        m.clamp((m.x(3_221_225_472), m.center_y - upper - curve)),
        m.clamp((m.rect.x2, m.center_y - corner)),
        m.clamp((m.x(3_221_225_472), m.center_y + lower + curve)),
        m.clamp((m.center_x, m.center_y + lower + bow + curve)),
        m.clamp((m.x(1_073_741_824), m.center_y + lower + curve)),
    ];
    multi_lip_shape(
        points,
        hex(palette.harmonic3),
        hex(palette.secondary_darker),
    )
}

fn open_shape(points: Vec<(i64, i64)>, fill: String, stroke: String) -> MouthShape {
    let path = format!(
        "M{} {} C{} {},{} {},{} {}",
        format_milli(points[0].0),
        format_milli(points[0].1),
        format_milli(points[1].0),
        format_milli(points[1].1),
        format_milli(points[2].0),
        format_milli(points[2].1),
        format_milli(points[3].0),
        format_milli(points[3].1),
    );
    MouthShape {
        path,
        points,
        fill,
        stroke,
    }
}

fn closed_shape(points: Vec<(i64, i64)>, fill: String, stroke: String) -> MouthShape {
    let path = format!(
        "M{} {} C{} {},{} {},{} {} C{} {},{} {},{} {} Z",
        format_milli(points[0].0),
        format_milli(points[0].1),
        format_milli(points[1].0),
        format_milli(points[1].1),
        format_milli(points[2].0),
        format_milli(points[2].1),
        format_milli(points[3].0),
        format_milli(points[3].1),
        format_milli(points[4].0),
        format_milli(points[4].1),
        format_milli(points[5].0),
        format_milli(points[5].1),
        format_milli(points[0].0),
        format_milli(points[0].1),
    );
    MouthShape {
        path,
        points,
        fill,
        stroke,
    }
}

fn multi_lip_shape(points: Vec<(i64, i64)>, fill: String, stroke: String) -> MouthShape {
    let path = format!(
        "M{} {} C{} {},{} {},{} {} C{} {},{} {},{} {} C{} {},{} {},{} {} C{} {},{} {},{} {} Z",
        format_milli(points[0].0),
        format_milli(points[0].1),
        format_milli(points[1].0),
        format_milli(points[1].1),
        format_milli(points[1].0),
        format_milli(points[1].1),
        format_milli(points[2].0),
        format_milli(points[2].1),
        format_milli(points[3].0),
        format_milli(points[3].1),
        format_milli(points[3].0),
        format_milli(points[3].1),
        format_milli(points[4].0),
        format_milli(points[4].1),
        format_milli(points[5].0),
        format_milli(points[5].1),
        format_milli(points[5].0),
        format_milli(points[5].1),
        format_milli(points[6].0),
        format_milli(points[6].1),
        format_milli(points[7].0),
        format_milli(points[7].1),
        format_milli(points[7].0),
        format_milli(points[7].1),
        format_milli(points[0].0),
        format_milli(points[0].1),
    );
    MouthShape {
        path,
        points,
        fill,
        stroke,
    }
}

fn p(mouth: &Mouth, index: usize) -> i64 {
    mouth.params[index].0.clamp(0, Q32_ONE)
}

fn hex(color: avatar_phenotype::Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use avatar_phenotype::{Rgb, Q32};

    fn palette() -> Palette {
        let color = |value| Rgb {
            r: value,
            g: value,
            b: value,
        };
        Palette {
            back: color(1),
            primary: color(2),
            primary_lighter: color(3),
            primary_darkish: color(4),
            primary_darker: color(5),
            primary_darkest: color(6),
            harmonic1: color(7),
            harmonic1_darkish: color(8),
            harmonic1_darkest: color(9),
            harmonic2: color(10),
            harmonic3: color(11),
            harmonic4: color(12),
            secondary_darker: color(13),
            dark: color(14),
            white: color(15),
        }
    }

    #[test]
    fn every_family_emits_one_group_and_one_bounded_path() {
        let rect = MouthRect {
            x1: 300_000,
            y1: 500_000,
            x2: 700_000,
            y2: 650_000,
        };
        for family in 0..8 {
            let mouth = Mouth {
                family,
                width_scale_gate: false,
                width_scale: Q32(0),
                aspect_widen_gate: false,
                params: [Q32(Q32_ONE / 2); 8],
            };
            let geometry = shape(&mouth, rect, MoodDeltas::ZERO, &palette());
            assert!(geometry
                .points
                .iter()
                .all(|&(x, y)| { x >= rect.x1 && x <= rect.x2 && y >= rect.y1 && y <= rect.y2 }));
            let svg = render(&mouth, rect, MoodDeltas::ZERO, &palette());
            assert_eq!(svg.matches("<g ").count(), 1, "family {family}");
            assert_eq!(svg.matches("<path ").count(), 1, "family {family}");
        }
    }
}
