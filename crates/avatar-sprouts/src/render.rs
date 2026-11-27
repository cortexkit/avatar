//! Turns a posed creature into a render tree, in the reference's painter's
//! order and with its numbers rounded as the reference writes them.

use avatar_svg::wild::render::{rotate, scale, translate};
use avatar_svg::{
    Effect, FillRule, GradientStop, Paint, Primitive, RenderTree, Rgb, Role, Shape, Stroke,
    StrokeCap, StrokeJoin, Subpath, Transform, TransformOp, Q32,
};

use crate::creature::{Creature, Mass};
use crate::geometry::{Geo, Node, PathD, Style};
use crate::maths::{hundredths, js_round, r3, tenths};
use crate::palette::{rgb, WHITE};

/// The outline every mass of overlapping shapes gets, in units of the
/// 240-unit frame: each shape is stroked at twice this, then filled.
const WEIGHT: f64 = 1.7;

/// An opacity rounded to two decimals, as the reference writes it, as a Q32
/// fixed-point value (the render tree's fraction type).
fn opacity(op: f64) -> Q32 {
    let written = js_round(op * 100.0) / 100.0;
    Q32((written * Q32::ONE.0 as f64).round() as i64)
}

/// The shape of path data, or nothing when it draws nothing.
fn path_shape(d: &PathD) -> Option<Shape> {
    let subs: Vec<_> = d.subs.iter().filter(|s| s.segments.len() > 1).collect();
    match subs.as_slice() {
        [] => None,
        [one] => Some(Shape::Path {
            segments: one.segments.clone(),
            closed: one.closed,
            fill_rule: FillRule::NonZero,
        }),
        many => Some(Shape::CompoundPath {
            subpaths: many
                .iter()
                .map(|s| Subpath {
                    segments: s.segments.clone(),
                    closed: s.closed,
                })
                .collect(),
            fill_rule: FillRule::NonZero,
        }),
    }
}

fn primitive(shape: Shape, ops: &[TransformOp], paint: Paint, label: &str) -> Primitive {
    Primitive {
        shape,
        transform: (!ops.is_empty()).then(|| Transform { ops: ops.to_vec() }),
        paint,
        stroke: None,
        opacity: None,
        clip: None,
        effects: Vec::new(),
        label: label.to_owned(),
        role: None,
    }
}

fn solid(colour: Option<Rgb>) -> Paint {
    colour.map_or(Paint::None, |color| Paint::Solid { color })
}

struct Painter {
    /// Stroke widths are given in units of the 240 frame; `px` turns them
    /// into the creature's own units.
    px: f64,
    out: Vec<Primitive>,
}

impl Painter {
    fn item(&mut self, geo: &Geo, style: &Style, ops: &[TransformOp], extra: &Extra) {
        let shape = match geo {
            Geo::Path(d) => match path_shape(d) {
                Some(shape) => shape,
                None => return,
            },
            Geo::Circle { cx, cy, r } => Shape::Circle {
                cx: tenths(*cx),
                cy: tenths(*cy),
                r: hundredths(*r),
            },
            Geo::Ellipse { cx, cy, rx, ry } => Shape::Ellipse {
                cx: tenths(*cx),
                cy: tenths(*cy),
                rx: hundredths(*rx),
                ry: hundredths(*ry),
            },
        };
        let mut p = primitive(shape, ops, solid(style.fill), &style.cls);
        p.stroke = style.stroke.map(|color| Stroke {
            color,
            width: hundredths(style.sw * self.px + style.wm),
            cap: style.cap.unwrap_or(StrokeCap::Butt),
            join: style.join.unwrap_or(StrokeJoin::Miter),
        });
        p.opacity = style.op.filter(|&op| op < 1.0).map(opacity);
        p.clip = extra.clip.clone();
        p.effects = extra.effects.clone();
        self.out.push(p);
    }

    fn node(&mut self, node: &Node, ops: &[TransformOp], extra: &Extra) {
        match node {
            Node::Item { geo, style, .. } => self.item(geo, style, ops, extra),
            Node::Group { items, .. } => {
                for item in items {
                    self.node(item, ops, extra);
                }
            }
        }
    }

    fn nodes(&mut self, nodes: &[Node], ops: &[TransformOp]) {
        for node in nodes {
            self.node(node, ops, &Extra::default());
        }
    }

    /// A mass of overlapping shapes gets one outline: every shape is stroked
    /// at twice the outline's weight, then every shape is filled. The fills
    /// cover the inner half of every stroke and the strokes inside the mass.
    fn strokes(&mut self, list: &[Mass], outline: Rgb, ops: &[TransformOp]) {
        for (d, cls, _) in joined(list) {
            let Some(shape) = path_shape(&d) else {
                continue;
            };
            let mut p = primitive(shape, ops, Paint::None, &format!("{cls} sp-line"));
            p.stroke = Some(Stroke {
                color: outline,
                width: hundredths(2.0 * WEIGHT * self.px),
                cap: StrokeCap::Butt,
                join: StrokeJoin::Round,
            });
            self.out.push(p);
        }
    }

    fn fills(&mut self, list: &[Mass], fill: Rgb, ops: &[TransformOp]) {
        for (d, cls, own) in joined(list) {
            let Some(shape) = path_shape(&d) else {
                continue;
            };
            self.out.push(primitive(
                shape,
                ops,
                Paint::Solid {
                    color: own.unwrap_or(fill),
                },
                &format!("{cls} sp-fill"),
            ));
        }
    }
}

/// What a skin mark carries besides its own paint: the body as its clip and
/// the fade round the face as its mask.
#[derive(Default)]
struct Extra {
    clip: Option<Shape>,
    effects: Vec<Effect>,
}

/// Shapes drawn as one mass: consecutive ones of the same part and fill are
/// joined into one path.
fn joined(list: &[Mass]) -> Vec<(PathD, String, Option<Rgb>)> {
    let mut out: Vec<(PathD, String, Option<Rgb>)> = Vec::new();
    for u in list {
        match out.last_mut() {
            Some(last) if last.1 == u.cls && last.2 == u.fill => last.0.append(u.d.clone()),
            _ => out.push((u.d.clone(), u.cls.clone(), u.fill)),
        }
    }
    out
}

/// The tree of one creature in mood `index`.
pub(crate) fn render(c: &Creature, index: usize) -> RenderTree {
    let pose = &c.poses[index];
    let m = &pose.mood;
    let b = &c.b;
    let f = &c.f;
    let p = &c.p;
    let fit = c.fit;
    let mut painter = Painter {
        px: 1.0 / fit.s,
        out: Vec::new(),
    };
    let creature = [translate(fit.x, fit.y), scale(fit.s, fit.s)];
    let mut posed = creature.to_vec();
    posed.push(translate(pose.shift, -pose.lift));
    if m.tilt != 0.0 {
        posed.push(rotate(m.tilt, 0.0, 0.0));
    }
    if m.sx != 1.0 || m.sy != 1.0 {
        posed.push(scale(m.sx, m.sy));
    }
    posed.push(translate(0.0, r3(-b.bottom)));

    // The ground shadow, under the feet.
    let feet = pose.feet;
    let mut shadow = primitive(
        Shape::Ellipse {
            cx: tenths((feet[0] + feet[2]) / 2.0),
            cy: hundredths(1.5 * painter.px),
            rx: tenths((feet[2] - feet[0]) / 2.0 + 9.0),
            ry: hundredths(5.5 * painter.px),
        },
        &creature,
        Paint::Solid { color: p.shadow },
        "sp-shadow",
    );
    shadow.opacity = Some(opacity(0.14));
    shadow.role = Some(Role::GroundShadow);
    painter.out.push(shadow);

    if !pose.back_legs.is_empty() {
        painter.strokes(&pose.back_legs, p.outline, &creature);
        painter.fills(&pose.back_legs, p.tone_down, &creature);
    }
    painter.nodes(&pose.leaves, &posed);
    painter.strokes(&pose.stems, p.outline, &posed);
    painter.fills(&pose.stems, p.body, &posed);
    painter.nodes(&pose.blooms, &posed);
    painter.strokes(&pose.body, p.outline, &posed);
    painter.strokes(&pose.legs, p.outline, &creature);
    painter.fills(&pose.body, p.body, &posed);
    painter.fills(&pose.legs, p.body, &creature);

    if !c.skin.items.is_empty() {
        // The skin's marks fade out round the face: a mask that is black at
        // the middle of the face and white beyond it.
        let cy = (f.ey + f.my) / 2.0;
        let rx = (f.ex + f.rx) * 1.2 + f.r * 0.4;
        let ry = (f.my - f.ey) / 2.0 + f.ry + f.r * 0.9;
        let bx = b.bx;
        let backing = primitive(
            Shape::Rect {
                x: tenths(bx[0] - 30.0),
                y: tenths(bx[1] - 30.0),
                w: tenths(bx[2] - bx[0] + 60.0),
                h: tenths(bx[3] - bx[1] + 60.0),
            },
            &posed,
            Paint::Solid { color: WHITE },
            "sp-mask",
        );
        let fade = primitive(
            Shape::Ellipse {
                cx: 0,
                cy: tenths(cy),
                rx: tenths(rx),
                ry: tenths(ry),
            },
            &posed,
            Paint::RadialGradient {
                stops: vec![
                    GradientStop {
                        offset_q32: Q32((0.62 * Q32::ONE.0 as f64).round() as i64),
                        color: rgb(0x000000),
                        opacity_q32: None,
                    },
                    GradientStop {
                        offset_q32: Q32::ONE,
                        color: WHITE,
                        opacity_q32: None,
                    },
                ],
            },
            "sp-mask-fade",
        );
        let extra = Extra {
            clip: path_shape(&b.d),
            effects: vec![Effect::Masked {
                mask: vec![backing, fade],
            }],
        };
        for item in &c.skin.items {
            painter.node(item, &posed, &extra);
        }
    }
    painter.nodes(&pose.front, &posed);
    painter.nodes(&pose.face, &posed);
    RenderTree {
        canvas: 240,
        background: None,
        pixel_grid: None,
        primitives: painter.out,
    }
}
