//! Turning the drawing into render-tree primitives.
//!
//! The drawing is made in toy units, inside one transform that scales it
//! into the 200-unit frame. Every number is rounded to a tenth of a unit, as
//! the drawing's own SVG writes it, so a coordinate is a whole number of
//! hundredths and is stored exactly.

use avatar_svg::{
    Bam16, Effect, FillRule, GradientStop, Paint, Primitive, Rgb, Role, Segment, Shape, Stroke,
    StrokeCap, StrokeJoin, Subpath, Transform, TransformOp, Q32,
};

use crate::geom::{js_round, Pt};

/// A length in toy units as a whole number of hundredths, rounded to a
/// tenth first (JavaScript's `Math.round(v * 10) / 10`).
pub fn m(v: f64) -> i64 {
    (js_round(v * 10.0) as i64) * 10
}

/// A fraction from 0 to 1 as Q32.32.
pub fn q(x: f64) -> Q32 {
    Q32((x * Q32::ONE.0 as f64).round() as i64)
}

/// A turn of `degrees` (rounded to a tenth of a degree first) as a binary
/// angle, 65,536 to the turn.
pub fn bam(degrees: f64) -> Bam16 {
    let tenths = js_round(degrees * 10.0) as i64;
    let num = tenths * 65_536;
    // Round to the nearest binary angle, halves away from zero.
    let rounded = if num >= 0 {
        (num + 1_800) / 3_600
    } else {
        -((-num + 1_800) / 3_600)
    };
    Bam16(rounded.rem_euclid(65_536) as u16)
}

/// One open or closed run of points.
pub type Run = (Vec<Pt>, bool);

fn segments(points: &[Pt]) -> Vec<Segment> {
    let mut out = Vec::with_capacity(points.len());
    for (i, p) in points.iter().enumerate() {
        let (x, y) = (m(p[0]), m(p[1]));
        out.push(if i == 0 {
            Segment::MoveTo { x, y }
        } else {
            Segment::LineTo { x, y }
        });
    }
    out
}

/// A path through `points`.
pub fn poly(points: &[Pt], closed: bool) -> Shape {
    Shape::Path {
        segments: segments(points),
        closed,
        fill_rule: FillRule::NonZero,
    }
}

/// One stitch from a to b: the drawing writes its start and then the move
/// to its end, each rounded, so the end is the rounded start plus the
/// rounded difference.
pub fn seg(a: Pt, b: Pt) -> Vec<Segment> {
    let (x, y) = (m(a[0]), m(a[1]));
    vec![
        Segment::MoveTo { x, y },
        Segment::LineTo {
            x: x + m(b[0] - a[0]),
            y: y + m(b[1] - a[1]),
        },
    ]
}

/// Several runs in one path. `None` when there is nothing to draw.
pub fn compound(runs: Vec<Run>) -> Option<Shape> {
    let subpaths: Vec<Subpath> = runs
        .into_iter()
        .filter(|(pts, _)| pts.len() >= 2)
        .map(|(pts, closed)| Subpath {
            segments: segments(&pts),
            closed,
        })
        .collect();
    (!subpaths.is_empty()).then_some(Shape::CompoundPath {
        subpaths,
        fill_rule: FillRule::NonZero,
    })
}

/// Several stitches in one path. `None` when there are none.
pub fn stitches(list: Vec<Vec<Segment>>) -> Option<Shape> {
    (!list.is_empty()).then(|| Shape::CompoundPath {
        subpaths: list
            .into_iter()
            .map(|segments| Subpath {
                segments,
                closed: false,
            })
            .collect(),
        fill_rule: FillRule::NonZero,
    })
}

pub fn circle(c: Pt, r: f64) -> Shape {
    Shape::Circle {
        cx: m(c[0]),
        cy: m(c[1]),
        r: m(r),
    }
}

pub fn ellipse(c: Pt, rx: f64, ry: f64) -> Shape {
    Shape::Ellipse {
        cx: m(c[0]),
        cy: m(c[1]),
        rx: m(rx),
        ry: m(ry),
    }
}

pub const WHITE: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};

pub fn solid(color: Rgb) -> Paint {
    Paint::Solid { color }
}

fn stop(offset: f64, color: Rgb, opacity: f64) -> GradientStop {
    GradientStop::with_opacity(q(offset), color, q(opacity))
}

/// The soft shading over every knitted part and pompom: lit at the upper
/// left, darker towards the edge, in the shape's own box.
pub fn shade() -> Paint {
    let black = Rgb { r: 0, g: 0, b: 0 };
    Paint::radial_gradient_in_box(
        q(0.42),
        q(0.36),
        q(0.72),
        Some((q(0.36), q(0.28))),
        vec![
            stop(0.0, WHITE, 0.2),
            stop(0.42, WHITE, 0.0),
            stop(0.74, black, 0.05),
            stop(1.0, black, 0.3),
        ],
    )
}

/// The glossy dark of a safety eye or nose.
pub fn eye_gloss() -> Paint {
    Paint::radial_gradient_in_box(
        q(0.38),
        q(0.34),
        q(0.7),
        None,
        vec![
            stop(0.0, crate::palette::rgb(0x5d626c), 1.0),
            stop(0.55, crate::palette::rgb(0x1d1d23), 1.0),
            stop(1.0, crate::palette::rgb(0x060608), 1.0),
        ],
    )
}

/// A soft round shadow fading from `opacity` at its middle to nothing.
pub fn soft_shadow(color: Rgb, opacity: f64) -> Paint {
    Paint::RadialGradient {
        stops: vec![stop(0.0, color, opacity), stop(1.0, color, 0.0)],
    }
}

/// How a primitive is painted.
#[derive(Clone, Debug)]
pub struct Style {
    pub paint: Paint,
    pub stroke: Option<Stroke>,
    pub opacity: Option<f64>,
}

impl Style {
    pub fn fill(color: Rgb) -> Self {
        Style {
            paint: solid(color),
            stroke: None,
            opacity: None,
        }
    }

    pub fn paint(paint: Paint) -> Self {
        Style {
            paint,
            stroke: None,
            opacity: None,
        }
    }

    /// A stroke with no fill.
    pub fn line(color: Rgb, width: f64, cap: StrokeCap, join: StrokeJoin) -> Self {
        Style {
            paint: Paint::None,
            stroke: Some(stroke(color, width, cap, join)),
            opacity: None,
        }
    }

    pub fn with_stroke(mut self, color: Rgb, width: f64) -> Self {
        self.stroke = Some(stroke(color, width, StrokeCap::Butt, StrokeJoin::Miter));
        self
    }

    pub fn with_join(mut self, join: StrokeJoin) -> Self {
        if let Some(s) = &mut self.stroke {
            s.join = join;
        }
        self
    }

    pub fn with_opacity(mut self, opacity: f64) -> Self {
        self.opacity = Some(opacity);
        self
    }
}

pub fn stroke(color: Rgb, width: f64, cap: StrokeCap, join: StrokeJoin) -> Stroke {
    Stroke {
        color,
        width: m(width),
        cap,
        join,
    }
}

/// Collects primitives in painter's order under a stack of group
/// transforms.
#[derive(Default)]
pub struct Emitter {
    pub primitives: Vec<Primitive>,
    ops: Vec<TransformOp>,
}

impl Emitter {
    /// Opens a group with these transform operations; returns the depth to
    /// give [`Emitter::leave`] when the group ends.
    pub fn enter(&mut self, ops: Vec<TransformOp>) -> usize {
        let depth = self.ops.len();
        self.ops.extend(ops);
        depth
    }

    pub fn leave(&mut self, depth: usize) {
        self.ops.truncate(depth);
    }

    pub fn push(&mut self, shape: Shape, style: Style, clip: Option<&Shape>, label: &str) {
        self.push_role(shape, style, clip, label, None);
    }

    fn transform(&self) -> Option<Transform> {
        (!self.ops.is_empty()).then(|| Transform {
            ops: self.ops.clone(),
        })
    }

    /// The clip a run of primitives shares as one group, as an SVG group's
    /// `clip-path` clips its children together: where the clip's edge only
    /// partly covers a pixel, the children are composited first and then
    /// clipped once, which is not the same as clipping each child.
    pub fn group_clip(&self, clip: &Shape) -> Vec<Effect> {
        vec![Effect::Masked {
            mask: vec![Primitive {
                shape: clip.clone(),
                transform: self.transform(),
                paint: solid(WHITE),
                stroke: None,
                opacity: None,
                clip: None,
                effects: Vec::new(),
                label: "clip".to_owned(),
                role: None,
            }],
        }]
    }

    /// A primitive in a run that shares `effects` (see [`Emitter::group_clip`]).
    pub fn push_in(&mut self, shape: Shape, style: Style, effects: &[Effect], label: &str) {
        self.push(shape, style, None, label);
        if let Some(last) = self.primitives.last_mut() {
            last.effects = effects.to_vec();
        }
    }

    pub fn push_role(
        &mut self,
        shape: Shape,
        style: Style,
        clip: Option<&Shape>,
        label: &str,
        role: Option<Role>,
    ) {
        self.primitives.push(Primitive {
            shape,
            transform: self.transform(),
            paint: style.paint,
            stroke: style.stroke,
            opacity: style.opacity.map(q),
            clip: clip.cloned(),
            effects: Vec::new(),
            label: label.to_owned(),
            role,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_angles_round_tenths_of_a_degree() {
        assert_eq!(bam(0.0).0, 0);
        assert_eq!(bam(90.0).0, 16_384);
        assert_eq!(bam(-90.0).0, 49_152);
        assert_eq!(bam(4.96).0, bam(5.0).0);
    }

    #[test]
    fn lengths_round_to_tenths() {
        assert_eq!(m(1.25), 130);
        assert_eq!(m(-1.25), -120);
        assert_eq!(m(0.04), 0);
    }
}
