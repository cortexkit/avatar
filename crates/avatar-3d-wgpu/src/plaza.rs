//! The plaza: many robots on one floor, seen from one camera, each labelled
//! at its `headTop` anchor.
//!
//! [`layout`] places the robots in staggered rows so that, in the frame,
//! no robot covers another robot's face and no label touches another label
//! or another robot. [`clashes`] checks a placement against those rules
//! with the real perspective camera, from boxes on the screen: each robot's
//! box holds all its projected vertices, and its face's box the projected
//! vertices of every textured-front triangle a face texture lies on (the
//! head's front, and stalk eyes).

use avatar_3d::motion::math::m_apply;
use avatar_3d::motion::Pose;
use avatar_3d::robot::{Robot, Skin};

use crate::frame;
use crate::view::{framing, View};

/// Robots in a long row; long rows and rows of one fewer alternate, so
/// each short row stands in the gaps of the rows round it.
pub const COLUMNS: usize = 9;
/// The camera's elevation in degrees, looking straight along −z.
pub const ELEVATION: f64 = 28.0;
/// The camera's vertical field of view in degrees.
pub const FOV_Y: f64 = 12.0;
/// Clear space between neighbours in a row, in model units.
pub const GAP: f64 = 0.25;
/// A label's height in model units at the plaza's middle; its pixel size
/// follows from the framing ([`Plaza::label_scale`]).
pub const LABEL_HEIGHT: f64 = 0.28;
/// The share of the frame's half size the plaza may fill.
pub const FILL: f64 = 0.94;

/// What the layout needs of one robot in its pose, in model space.
#[derive(Clone, Debug, PartialEq)]
pub struct Footprint {
    /// Every posed vertex.
    pub vertices: Vec<[f64; 3]>,
    /// The posed vertices of the textured-front triangles a face or
    /// stalk-eye texture lies on.
    pub face: Vec<[f64; 3]>,
    /// The posed `headTop` anchor, where the label stands.
    pub head_top: [f64; 3],
    /// The posed vertices' bounds.
    pub lo: [f64; 3],
    /// See [`Footprint::lo`].
    pub hi: [f64; 3],
}

/// A robot's footprint in a pose.
pub fn footprint(robot: &Robot, pose: &Pose) -> Footprint {
    let skins = frame::skin_matrices(robot, pose);
    let mut vertices = Vec::new();
    let mut face = Vec::new();
    for (index, part) in robot.parts.iter().enumerate() {
        let start = vertices.len();
        for (i, v) in part.mesh.positions.iter().enumerate() {
            vertices.push(match &part.skin {
                Skin::Rigid { bone } => m_apply(&skins[*bone], *v),
                Skin::Skinned { joints, weights } => {
                    let mut p = [0.0; 3];
                    for (j, w) in joints[i].iter().zip(&weights[i]) {
                        let q = m_apply(&skins[*j as usize], *v);
                        for k in 0..3 {
                            p[k] += w * q[k];
                        }
                    }
                    p
                }
                _ => *v,
            });
        }
        if frame::face_slot(robot, index) > 0 {
            for (t, tri) in part.mesh.triangles.iter().enumerate() {
                if part.mesh.textured_front[t] {
                    face.extend(tri.iter().map(|v| vertices[start + *v as usize]));
                }
            }
        }
    }
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in &vertices {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let head_top = frame::anchor(robot, pose, "headTop").unwrap_or([
        0.5 * (lo[0] + hi[0]),
        hi[1],
        0.5 * (lo[2] + hi[2]),
    ]);
    Footprint {
        vertices,
        face,
        head_top,
        lo,
        hi,
    }
}

/// Robots placed on the plaza and the camera that frames them.
#[derive(Clone, Debug, PartialEq)]
pub struct Plaza {
    /// The camera.
    pub view: View,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Where each robot's model origin stands.
    pub offsets: Vec<[f64; 3]>,
    /// Pixels per digit pixel of the labels ([`label_size`]).
    pub label_scale: u32,
}

/// A two-digit label's size in pixels at `scale` pixels per pixel of its
/// 3 × 5 digits: the digits with a one-pixel gap between them and a
/// one-pixel paper margin.
pub fn label_size(scale: u32) -> (u32, u32) {
    ((4 * 2 + 1) * scale, 7 * scale)
}

/// Why a placement fails, by robot index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clash {
    /// Robot `by` stands in front of robot `face`'s face.
    Face {
        /// The robot whose face is covered.
        face: usize,
        /// The robot in front of it.
        by: usize,
    },
    /// Two labels touch.
    Labels(usize, usize),
    /// A robot's label touches another robot.
    LabelOnRobot {
        /// The label's robot.
        label: usize,
        /// The robot it touches.
        robot: usize,
    },
    /// A robot or its label leaves the frame.
    Outside(usize),
}

/// Boxes on the screen, `[x0, y0, x1, y1]` with y down, and depths away
/// from the camera.
#[derive(Clone, Copy)]
struct Shapes {
    robot: [f64; 4],
    face: [f64; 4],
    label: [f64; 4],
    /// The robot's nearest depth.
    near: f64,
    /// The face's farthest depth.
    face_far: f64,
}

fn overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

/// The rules, over every pair of robots.
fn rules(shapes: &[Shapes]) -> Vec<Clash> {
    let mut out = Vec::new();
    for (i, a) in shapes.iter().enumerate() {
        for (j, b) in shapes.iter().enumerate() {
            if i == j {
                continue;
            }
            if b.near < a.face_far && overlap(a.face, b.robot) {
                out.push(Clash::Face { face: i, by: j });
            }
            if i < j && overlap(a.label, b.label) {
                out.push(Clash::Labels(i, j));
            }
            if overlap(a.label, b.robot) {
                out.push(Clash::LabelOnRobot { label: i, robot: j });
            }
        }
    }
    out
}

/// The screen box and depth range of points under a projection that gives
/// each point's screen position (y down) and depth.
fn extent(
    points: &[[f64; 3]],
    project: impl Fn([f64; 3]) -> Option<([f64; 2], f64)>,
) -> Option<([f64; 4], f64, f64)> {
    let mut r = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let (mut near, mut far) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in points {
        let (s, d) = project(*p)?;
        r = [
            r[0].min(s[0]),
            r[1].min(s[1]),
            r[2].max(s[0]),
            r[3].max(s[1]),
        ];
        near = near.min(d);
        far = far.max(d);
    }
    Some((r, near, far))
}

fn moved(p: [f64; 3], o: [f64; 3]) -> [f64; 3] {
    [p[0] + o[0], p[1] + o[1], p[2] + o[2]]
}

/// Every rule a placement breaks in its frame, with labels of the plaza's
/// pixel size centred above their anchors; empty when it is clean.
pub fn clashes(plaza: &Plaza, prints: &[Footprint]) -> Vec<Clash> {
    let view = plaza.view;
    let (w, h) = (f64::from(plaza.width), f64::from(plaza.height));
    let (lw, lh) = label_size(plaza.label_scale);
    let (lw, lh) = (f64::from(lw), f64::from(lh));
    let mut outside = Vec::new();
    let mut shapes = Vec::new();
    for (i, (f, o)) in prints.iter().zip(&plaza.offsets).enumerate() {
        let project = |p: [f64; 3]| {
            let p = moved(p, *o);
            Some((
                view.to_screen(p, plaza.width, plaza.height)?,
                view.project(p)?[2],
            ))
        };
        let (Some((robot, near, _)), Some(top)) = (
            extent(&f.vertices, project),
            view.to_screen(moved(f.head_top, *o), plaza.width, plaza.height),
        ) else {
            outside.push(Clash::Outside(i));
            continue;
        };
        let (face, face_far) = match extent(&f.face, project) {
            Some((face, _, far)) if !f.face.is_empty() => (face, far),
            _ => ([0.0; 4], f64::NEG_INFINITY),
        };
        // A label is drawn centred above its anchor at whole pixels, so its
        // box here is a pixel larger all round.
        let label = [
            top[0] - 0.5 * lw - 1.0,
            top[1] - lh - 1.0,
            top[0] + 0.5 * lw + 1.0,
            top[1] + 1.0,
        ];
        let inside = |r: [f64; 4]| r[0] >= 0.0 && r[1] >= 0.0 && r[2] <= w && r[3] <= h;
        if !inside(robot) || !inside(label) {
            outside.push(Clash::Outside(i));
        }
        shapes.push(Shapes {
            robot,
            face,
            label,
            near,
            face_far,
        });
    }
    if !outside.is_empty() {
        return outside;
    }
    rules(&shapes)
}

fn sin_cos(degrees: f64) -> (f64, f64) {
    let a = degrees * std::f64::consts::PI / 180.0;
    (
        avatar_3d::motion::math::sin(a),
        avatar_3d::motion::math::cos(a),
    )
}

/// Each robot's shapes at its own origin in a parallel projection along
/// the view direction, in model units: labels are [`LABEL_HEIGHT`] high,
/// and robot boxes grow by `margin` above and below, since rows at other
/// depths move a little in perspective.
fn parallel_shapes(prints: &[Footprint], margin: f64) -> Vec<Shapes> {
    let (s, c) = sin_cos(ELEVATION);
    let project = |p: [f64; 3]| Some(([p[0], -(p[1] * c - p[2] * s)], -(p[1] * s + p[2] * c)));
    let (lw, lh) = label_size(1);
    let label_w = LABEL_HEIGHT * f64::from(lw) / f64::from(lh);
    prints
        .iter()
        .map(|f| {
            let (r, near, _) = extent(&f.vertices, project).expect("parallel projection");
            let (face, face_far) = match extent(&f.face, project) {
                Some((face, _, far)) if !f.face.is_empty() => (face, far),
                _ => ([0.0; 4], f64::NEG_INFINITY),
            };
            let (top, _) = project(f.head_top).expect("parallel projection");
            Shapes {
                robot: [r[0], r[1] - margin, r[2], r[3] + margin],
                face,
                label: [
                    top[0] - 0.5 * label_w - margin,
                    top[1] - LABEL_HEIGHT - margin,
                    top[0] + 0.5 * label_w + margin,
                    top[1] + margin,
                ],
                near,
                face_far,
            }
        })
        .collect()
}

/// `local` shapes moved by a robot's offset in the parallel projection.
fn placed(local: &Shapes, o: [f64; 3]) -> Shapes {
    let (s, c) = sin_cos(ELEVATION);
    let (dx, dy, dd) = (o[0], -(o[1] * c - o[2] * s), -(o[1] * s + o[2] * c));
    let shift = |r: [f64; 4]| [r[0] + dx, r[1] + dy, r[2] + dx, r[3] + dy];
    Shapes {
        robot: shift(local.robot),
        face: shift(local.face),
        label: shift(local.label),
        near: local.near + dd,
        face_far: local.face_far + dd,
    }
}

/// The robots' rows, front first: long rows of [`COLUMNS`] and short rows
/// of one fewer alternate.
fn rows(n: usize) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < n {
        let len = if out.len() % 2 == 0 {
            COLUMNS
        } else {
            COLUMNS - 1
        };
        out.push(start..(start + len).min(n));
        start += len;
    }
    out
}

/// Places the robots in staggered rows, front row first, each robot
/// [`GAP`] from its neighbours and every row centred. Each row stands as
/// close behind the rows in front as it can while no rule breaks in a
/// parallel view along the camera's direction; the camera then frames
/// everything, labels included, from straight ahead and [`ELEVATION`] up.
/// If the perspective frame still breaks a rule, the robots' boxes are
/// grown and the rows placed again.
pub fn layout(prints: &[Footprint], width: u32, height: u32) -> Plaza {
    let n = prints.len();
    let max_d = prints.iter().map(|f| f.hi[2] - f.lo[2]).fold(0.0, f64::max);
    let mut margin = 0.05;
    let mut best = None;
    for _ in 0..16 {
        let local = parallel_shapes(prints, margin);
        let mut offsets = vec![[0.0; 3]; n];
        let mut z = 0.0;
        for (r, row) in rows(n).into_iter().enumerate() {
            // Neighbours stand GAP apart, the row centred on x = 0.
            let widths: Vec<f64> = row
                .clone()
                .map(|i| prints[i].hi[0] - prints[i].lo[0])
                .collect();
            let total = widths.iter().sum::<f64>() + GAP * (widths.len() as f64 - 1.0);
            let mut xs = Vec::with_capacity(widths.len());
            let mut x = -0.5 * total;
            for w in &widths {
                xs.push(x + 0.5 * w);
                x += w + GAP;
            }
            let place = |offsets: &mut Vec<[f64; 3]>, z: f64| {
                for (k, i) in row.clone().enumerate() {
                    let f = &prints[i];
                    offsets[i] = [
                        xs[k] - 0.5 * (f.lo[0] + f.hi[0]),
                        0.0,
                        z - 0.5 * (f.lo[2] + f.hi[2]),
                    ];
                }
            };
            if r > 0 {
                z -= max_d + GAP;
            }
            place(&mut offsets, z);
            let clear = |offsets: &[[f64; 3]]| {
                let shapes: Vec<Shapes> = (0..row.end)
                    .map(|i| placed(&local[i], offsets[i]))
                    .collect();
                rules(&shapes).is_empty()
            };
            let mut steps = 0;
            while !clear(&offsets) && steps < 2000 {
                z -= 0.02;
                steps += 1;
                place(&mut offsets, z);
            }
        }
        // Frame the robots and the room their labels take above them.
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for (f, o) in prints.iter().zip(&offsets) {
            let top = moved(f.head_top, *o);
            let label_top = [top[0], top[1] + 1.5 * LABEL_HEIGHT, top[2]];
            for p in [moved(f.lo, *o), moved(f.hi, *o), top, label_top] {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
        let view = framing(
            lo,
            hi,
            0.0,
            ELEVATION,
            FOV_Y,
            f64::from(width) / f64::from(height),
            FILL,
        );
        // Pixels per model unit at the target's distance.
        let (s, c) = sin_cos(0.5 * FOV_Y);
        let ppu = 0.5 * f64::from(height) / (view.distance() * s / c);
        let (_, unit_h) = label_size(1);
        let label_scale = ((LABEL_HEIGHT * ppu / f64::from(unit_h)).floor() as u32).max(2);
        let plaza = Plaza {
            view,
            width,
            height,
            offsets,
            label_scale,
        };
        let clean = clashes(&plaza, prints).is_empty();
        best = Some(plaza);
        if clean {
            break;
        }
        margin += 0.05;
    }
    best.expect("the layout runs at least once")
}
