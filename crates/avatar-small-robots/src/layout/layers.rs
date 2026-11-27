//! Standalone drawings and their projection onto the original drawing frame.

use avatar_svg::{Primitive, RenderTree, Transform, TransformOp};

use crate::body::chassis_rivets;
use crate::draw::{Ctx, Out};
use crate::face::{Eye, Tech};
use crate::face_draw::{face, lens_eye};
use crate::kit::P2;
use crate::marks::draw_head_marks;
use crate::parts::{pod_eye, stalk_at};
use crate::robot::Robot;
use crate::{Creature, Mood, FULL_DETAIL_PX, MOODS};

use super::bounds::Bounds;
use super::GRID;

/// Extra room around each layer in pre-fit robot units, before snapping.
pub const LAYER_MARGIN: f64 = 1.0;

/// A layer's bounds in its original pre-fit drawing frame (negative y is up).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Region {
    /// Top-left corner, snapped outward to the tree's hundredth-unit grid.
    pub origin: [f64; 2],
    /// Width and height before square padding, in robot units.
    pub size: [f64; 2],
    /// Square canvas edge in robot units, rounded up from the longer side.
    pub extent: u32,
    /// Canvas units per pre-fit robot unit on both axes; always one.
    pub scale: f64,
    pub(super) origin_grid: [i64; 2],
}

impl Region {
    fn from_bounds(bounds: Bounds) -> Self {
        let origin_grid = bounds
            .min
            .map(|v| ((v - LAYER_MARGIN) * GRID).floor() as i64);
        let origin = origin_grid.map(|v| v as f64 / GRID);
        let size = std::array::from_fn(|i| bounds.max[i] + LAYER_MARGIN - origin[i]);
        Self {
            origin,
            size,
            extent: size[0].max(size[1]).ceil() as u32,
            scale: 1.0,
            origin_grid,
        }
    }

    /// Whether a point in the original frame is inside the unpadded region.
    pub fn contains(&self, point: [f64; 2]) -> bool {
        (0..2).all(|i| point[i] >= self.origin[i] && point[i] <= self.origin[i] + self.size[i])
    }
}

/// Transparent full-detail drawing and the binding back to its original frame.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Layer {
    /// Tree with its origin at the top left, padded on the right and bottom.
    pub tree: RenderTree,
    /// Binding in pre-fit robot units: head-local for face, stalks and head
    /// marks; upper-body-local for chassis rivets.
    pub region: Region,
}

impl Layer {
    fn new(mut primitives: Vec<Primitive>, region: Region) -> Self {
        let [x, y] = region.origin_grid;
        for primitive in &mut primitives {
            shift(primitive, [-x, -y]);
        }
        Self {
            tree: RenderTree {
                canvas: region.extent,
                background: None,
                pixel_grid: None,
                primitives,
            },
            region,
        }
    }
}

// Prepend in SVG order so local operations run first. The same exact integer
// translation moves the shape, clip and any positioned paint together.
pub(super) fn shift(primitive: &mut Primitive, [tx, ty]: [i64; 2]) {
    if tx == 0 && ty == 0 {
        return;
    }
    let ops = &mut primitive
        .transform
        .get_or_insert_with(|| Transform { ops: Vec::new() })
        .ops;
    if let Some(TransformOp::Translate { tx: x, ty: y }) = ops.first_mut() {
        *x += tx;
        *y += ty;
        if *x == 0 && *y == 0 {
            ops.remove(0);
        }
    } else {
        ops.insert(0, TransformOp::Translate { tx, ty });
    }
    if ops.is_empty() {
        primitive.transform = None;
    }
}

pub(super) fn detail() -> Ctx {
    Ctx::new(f64::from(FULL_DETAIL_PX))
}

fn face_primitives(c: &Robot, mood: Mood, blinking: bool) -> Vec<Primitive> {
    let mut out = Out::new();
    let m = c.mood(mood);
    let blink;
    let m = if blinking {
        blink = m.blinking();
        &blink
    } else {
        m
    };
    out.group("rb-face", &[], |out| {
        face(out, &detail(), &c.f, &c.p, &c.head, m)
    });
    out.prims
}

fn stalk_primitives(c: &Robot, side: f64, mood: Mood, blinking: bool) -> Vec<Primitive> {
    let mut out = Out::new();
    let m = c.mood(mood);
    let (_, [x, y]) = stalk_at(c, side, m);
    let e = Eye {
        x,
        y,
        ..pod_eye(c, side)
    };
    let blink;
    let m = if blinking {
        blink = m.blinking();
        &blink
    } else {
        m
    };
    lens_eye(&mut out, &detail(), &c.f, &c.p, &e, &m.eye_for(side), m);
    out.prims
}

pub(super) fn face_region(c: &Robot) -> Option<Region> {
    mood_region(
        |mood, blinking| face_primitives(c, mood, blinking),
        |_| None,
    )
}

pub(super) fn stalk_region(c: &Robot, side: f64) -> Option<Region> {
    if c.f.tech != Tech::Stalk {
        return None;
    }
    mood_region(
        |mood, blinking| stalk_primitives(c, side, mood, blinking),
        |mood| Some(stalk_at(c, side, c.mood(mood)).1),
    )
}

fn mood_region(
    draw: impl Fn(Mood, bool) -> Vec<Primitive>,
    centre: impl Fn(Mood) -> Option<P2>,
) -> Option<Region> {
    let mut bounds = Bounds::empty();
    for mood in MOODS {
        for blinking in [false, true] {
            bounds.include(Bounds::primitives(&draw(mood, blinking)));
        }
        if let Some(point) = centre(mood) {
            bounds.point(point);
        }
    }
    bounds.nonempty().map(Region::from_bounds)
}

fn marks_layer(out: Out) -> Option<Layer> {
    let region = Region::from_bounds(Bounds::primitives(&out.prims).nonempty()?);
    Some(Layer::new(out.prims, region))
}

impl Creature {
    /// Full-detail face in head-local robot units. With `blinking`, open eyes
    /// use a flat shut stroke; mood-closed eyes keep their own form. On a
    /// stalk robot the face contains the mouth but not the eyes.
    pub fn face_layer(&self, mood: Mood, blinking: bool) -> Option<Layer> {
        let c = &self.robot;
        Some(Layer::new(
            face_primitives(c, mood, blinking),
            face_region(c)?,
        ))
    }

    /// One full-detail stalk eye alone, in head-local robot units: index 0
    /// is left (−x), 1 right (+x). Returns `None` for other indices and for
    /// a face without stalks. `blinking` uses the same rule as [`Self::face_layer`].
    pub fn stalk_eye_layer(&self, index: usize, mood: Mood, blinking: bool) -> Option<Layer> {
        let side = *[-1.0, 1.0].get(index)?;
        let c = &self.robot;
        let region = stalk_region(c, side)?;
        Some(Layer::new(
            stalk_primitives(c, side, mood, blinking),
            region,
        ))
    }

    /// Full-detail head surface marks in head-local robot units, retaining
    /// hull clipping. The lid on a can-shaped hull is geometry, not a mark.
    pub fn head_marks(&self) -> Option<Layer> {
        let mut out = Out::new();
        out.group("rb-head-shell", &[], |out| {
            draw_head_marks(out, &detail(), &self.robot, false)
        });
        marks_layer(out)
    }

    /// Full-detail chassis rivets in upper-body-local robot units (including
    /// the chassis vertical offset). Returns `None` with no chassis rivets.
    pub fn chassis_marks(&self) -> Option<Layer> {
        let mut out = Out::new();
        out.group("rb-chassis-shell", &[], |out| {
            chassis_rivets(out, &detail(), &self.robot)
        });
        marks_layer(out)
    }
}
