//! Camera containment: no posed vertex of the plaza's robots leaves the
//! frame that `frame_box` fits to their reach box, through every mood,
//! reaction and status loop, with the idle layer on, a reaction cut short
//! by another and each reaction over each status loop, at the default view
//! and orbiting round them, on square, wide and tall frames, sampled 120
//! times a second.
//!
//! The reach box samples poses 30 times a second with the idle layer off,
//! so the idle layer, the samples in between and a reaction cut short rely
//! on the margin between `FILL` and the frame's edge. A zoom above 1 only
//! moves the eye back along its view, which brings every point in front
//! nearer the frame's centre, so zoom 1 is the tightest the stage allows.
//!
//! The check takes a shortcut first: each part's rest bounding box,
//! carried by each bone the part follows, holds the part's posed vertices
//! (a skinned vertex is a weighted mean of the points its bones carry it
//! to), so when every carried corner projects inside the frame, every
//! vertex does. Where a carried corner lands outside, as a tilted head's
//! box corners can while its vertices stay well inside, the test skins
//! that pose's vertices and fails only if one of them leaves the frame.

use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Instant;

use avatar_3d::motion::math::{m_apply, M4};
use avatar_3d::motion::{Actor, LOOP_BLEND, REACTIONS, STATUS_LOOPS};
use avatar_3d::robot::camera::{frame_box, Camera};
use avatar_3d::robot::{build_with, BuildOptions, Robot, Skin, AZIMUTH, ELEVATION};
use avatar_small_robots::{mood_name, Mood, MOODS};
use avatar_svg::wild::math::{cos, sin};

use crate::cases;

/// The plaza's robots: genomes 0 to 32 of seed 503.
const PLAZA_SEED: u64 = 503;
const PLAZA_ROBOTS: u64 = 33;
/// How often each motion is sampled.
const SAMPLES_PER_SECOND: f64 = 120.0;
/// The frame shapes, width over height.
const ASPECTS: [(&str, f64); 3] = [("square", 1.0), ("16:9", 16.0 / 9.0), ("9:16", 9.0 / 16.0)];

/// The orbits checked, as (yaw, pitch) in degrees: the default view, then
/// yaw every 45° from −180° to 180° at pitch 0° and 60°.
fn orbits() -> Vec<(f64, f64)> {
    let mut out = vec![(AZIMUTH, ELEVATION)];
    for pitch in [0.0, 60.0] {
        out.extend((-4..=4).map(|k| (45.0 * f64::from(k), pitch)));
    }
    out
}

/// A camera's projection with its basis worked out once. It repeats
/// [`Camera::project`]'s arithmetic step for step, so it gives the same
/// bits; the test checks that on the reach box's corners.
struct View {
    name: String,
    eye: [f64; 3],
    f: [f64; 3],
    r: [f64; 3],
    u: [f64; 3],
    k: f64,
    aspect: f64,
}

impl View {
    fn new(name: String, camera: &Camera) -> Self {
        let f = unit(sub(camera.target, camera.eye));
        let r = unit(cross(f, camera.up));
        let u = cross(r, f);
        let a = 0.5 * camera.fov_y_degrees * PI / 180.0;
        Self {
            name,
            eye: camera.eye,
            f,
            r,
            u,
            k: sin(a) / cos(a),
            aspect: camera.aspect,
        }
    }

    fn project(&self, p: [f64; 3]) -> [f64; 3] {
        let v = sub(p, self.eye);
        let depth = dot(v, self.f);
        [
            dot(v, self.r) / (depth * self.k * self.aspect),
            dot(v, self.u) / (depth * self.k),
            depth,
        ]
    }

    /// How far from the frame's centre the point lands, as a share of the
    /// half frame on the farther axis: inside the frame up to 1. A point
    /// not in front of the camera is infinitely far.
    fn spread(&self, p: [f64; 3]) -> f64 {
        let q = self.project(p);
        if q[2] > 0.0 {
            q[0].abs().max(q[1].abs())
        } else {
            f64::INFINITY
        }
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}

/// Every corner of the box from `lo` to `hi`.
fn corners(lo: [f64; 3], hi: [f64; 3]) -> [[f64; 3]; 8] {
    std::array::from_fn(|i| [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] }))
}

/// For each part and each bone it follows, the bone and the corners of
/// the part's rest bounding box relative to the bone's joint, so that a
/// pose's bone matrix carries them straight to model space.
fn carried_boxes(robot: &Robot) -> Vec<(usize, [[f64; 3]; 8])> {
    let mut out = Vec::new();
    for part in &robot.parts {
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for p in &part.mesh.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let bones: BTreeSet<usize> = match &part.skin {
            Skin::Rigid { bone } => BTreeSet::from([*bone]),
            Skin::Skinned { joints, weights } => {
                let mut bones = BTreeSet::new();
                for (js, ws) in joints.iter().zip(weights) {
                    for (j, w) in js.iter().zip(ws) {
                        // A negative weight would carry a vertex outside
                        // its bones' points, and the check would not hold.
                        assert!(*w >= 0.0, "{}: a skin weight is negative", part.name);
                        if *w != 0.0 {
                            bones.insert(*j as usize);
                        }
                    }
                }
                bones
            }
            _ => panic!("a part is rigid or skinned"),
        };
        for bone in bones {
            let joint = robot.bones[bone].joint;
            out.push((bone, corners(lo, hi).map(|c| sub(c, joint))));
        }
    }
    out
}

/// The motions checked in one mood, each an actor with its events and the
/// end of its sampling: each reaction alone; each status loop for its
/// fade-in and two periods; each reaction cut at its peak by the next one
/// in `REACTIONS` order; and each reaction played over each status loop
/// once the loop has faded in. The idle layer is on throughout.
fn motions(robot: &Robot, mood: Mood) -> Vec<(String, Actor<'_>, f64)> {
    let m = &robot.motion;
    let mut out = Vec::new();
    for r in REACTIONS {
        let mut actor = Actor::new(robot, mood);
        actor.play(0.0, r);
        out.push((r.name().to_owned(), actor, m.reaction(r).duration));
    }
    for l in STATUS_LOOPS {
        let mut actor = Actor::new(robot, mood);
        actor.set_loop(0.0, Some(l));
        let end = LOOP_BLEND + 2.0 * m.status_loop(l).duration;
        out.push((format!("loop.{}", l.name()), actor, end));
    }
    for (i, r) in REACTIONS.into_iter().enumerate() {
        let next = REACTIONS[(i + 1) % REACTIONS.len()];
        let at = m.reaction(r).peak.expect("every reaction has a peak");
        let mut actor = Actor::new(robot, mood);
        actor.play(0.0, r);
        actor.play(at, next);
        let name = format!("{} cut by {}", r.name(), next.name());
        out.push((name, actor, at + m.reaction(next).duration));
    }
    for l in STATUS_LOOPS {
        for r in REACTIONS {
            let mut actor = Actor::new(robot, mood);
            actor.set_loop(0.0, Some(l));
            actor.play(LOOP_BLEND, r);
            let name = format!("{} over loop.{}", r.name(), l.name());
            out.push((name, actor, LOOP_BLEND + m.reaction(r).duration));
        }
    }
    out
}

/// Every 1/120 s from 0 to `end`, and `end` itself.
fn samples(end: f64) -> impl Iterator<Item = f64> {
    let steps = (end * SAMPLES_PER_SECOND).floor() as u32;
    (0..=steps)
        .map(|k| f64::from(k) / SAMPLES_PER_SECOND)
        .chain(std::iter::once(end))
}

/// What one robot's check found.
struct Found {
    index: u64,
    /// The largest spread of any carried corner, and where.
    worst: f64,
    worst_at: String,
    /// How many (pose, view) pairs had a carried corner outside the frame,
    /// so that their vertices were checked one by one, and the largest
    /// spread of a posed vertex among them.
    vertex_checks: usize,
    worst_vertex: f64,
    poses: usize,
    failures: Vec<String>,
}

/// One rest vertex as the skinning carries it: by one bone from its
/// joint, or blended over up to four bones.
enum Vertex {
    Rigid(usize, [f64; 3]),
    Skinned([u32; 4], [f64; 4], [f64; 3]),
}

/// Every distinct rest vertex of the robot, ready to pose.
fn vertices(robot: &Robot) -> Vec<Vertex> {
    let (mut rigid, mut skinned) = (BTreeSet::new(), BTreeSet::new());
    let mut out = Vec::new();
    for part in &robot.parts {
        for (i, v) in part.mesh.positions.iter().enumerate() {
            match &part.skin {
                Skin::Rigid { bone } => {
                    let offset = sub(*v, robot.bones[*bone].joint);
                    if rigid.insert((*bone, offset.map(f64::to_bits))) {
                        out.push(Vertex::Rigid(*bone, offset));
                    }
                }
                Skin::Skinned { joints, weights } => {
                    let key = (joints[i], weights[i].map(f64::to_bits), v.map(f64::to_bits));
                    if skinned.insert(key) {
                        out.push(Vertex::Skinned(joints[i], weights[i], *v));
                    }
                }
                _ => panic!("a part is rigid or skinned"),
            }
        }
    }
    out
}

/// Every vertex in a pose, skinned as [`avatar_3d::robot`] describes.
fn posed(robot: &Robot, vertices: &[Vertex], bones: &[M4]) -> Vec<[f64; 3]> {
    vertices
        .iter()
        .map(|v| match v {
            Vertex::Rigid(bone, offset) => m_apply(&bones[*bone], *offset),
            Vertex::Skinned(joints, weights, v) => {
                let mut p = [0.0; 3];
                for (j, w) in joints.iter().zip(weights) {
                    if *w != 0.0 {
                        let j = *j as usize;
                        let q = m_apply(&bones[j], sub(*v, robot.bones[j].joint));
                        for k in 0..3 {
                            p[k] += w * q[k];
                        }
                    }
                }
                p
            }
        })
        .collect()
}

/// Checks one plaza robot through every motion in every mood, at every
/// orbit and frame shape.
fn check(index: u64) -> Found {
    let robot = build_with(
        &cases::random_genome(PLAZA_SEED, index),
        BuildOptions::untextured(),
    );
    let reach = robot.reach();
    let mut views = Vec::new();
    for (yaw, pitch) in orbits() {
        for (shape, aspect) in ASPECTS {
            let camera = frame_box(reach.lo, reach.hi, yaw, pitch, aspect, 1.0);
            let view = View::new(format!("yaw {yaw}, pitch {pitch}, {shape}"), &camera);
            for p in corners(reach.lo, reach.hi) {
                assert_eq!(
                    view.project(p).map(f64::to_bits),
                    camera.project(p).map(f64::to_bits),
                    "the test's projection repeats Camera::project"
                );
            }
            views.push(view);
        }
    }
    let boxes = carried_boxes(&robot);
    let vertices = vertices(&robot);
    let mut points: Vec<[f64; 3]> = Vec::with_capacity(8 * boxes.len());
    let mut found = Found {
        index,
        worst: 0.0,
        worst_at: String::new(),
        vertex_checks: 0,
        worst_vertex: 0.0,
        poses: 0,
        failures: Vec::new(),
    };
    for mood in MOODS {
        for (motion, actor, end) in motions(&robot, mood) {
            for t in samples(end) {
                let bones: Vec<M4> = actor.evaluate(t).bones;
                found.poses += 1;
                points.clear();
                let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
                for (bone, carried) in &boxes {
                    for c in carried {
                        let p = m_apply(&bones[*bone], *c);
                        for k in 0..3 {
                            lo[k] = lo[k].min(p[k]);
                            hi[k] = hi[k].max(p[k]);
                        }
                        points.push(p);
                    }
                }
                // The frame is convex, so when the corners of the box
                // round this pose's carried corners are inside it, every
                // carried corner is, and so is every vertex. Only a view
                // where that bound passes the largest spread seen so far,
                // or the frame's edge, needs the carried corners one by
                // one, and only one where a carried corner leaves the frame
                // needs the vertices.
                let bound = corners(lo, hi);
                let mut skinned: Option<Vec<[f64; 3]>> = None;
                for view in &views {
                    let limit = bound.iter().fold(0.0, |m: f64, p| m.max(view.spread(*p)));
                    if limit <= found.worst.min(1.0) {
                        continue;
                    }
                    let spread = points.iter().fold(0.0, |m: f64, p| m.max(view.spread(*p)));
                    if spread > found.worst {
                        found.worst = spread;
                        found.worst_at =
                            format!("{}, {motion} at {t:.4} s, {}", mood_name(mood), view.name);
                    }
                    if spread <= 1.0 {
                        continue;
                    }
                    let posed = skinned.get_or_insert_with(|| posed(&robot, &vertices, &bones));
                    let spread = posed.iter().fold(0.0, |m: f64, p| m.max(view.spread(*p)));
                    found.vertex_checks += 1;
                    found.worst_vertex = found.worst_vertex.max(spread);
                    if spread > 1.0 && found.failures.len() < 20 {
                        found.failures.push(format!(
                            "genome {index}: {}, {motion} at {t:.4} s, {}: a posed vertex lands {spread:.4} of the half frame out",
                            mood_name(mood),
                            view.name
                        ));
                    }
                }
            }
        }
    }
    found
}

#[test]
fn the_plaza_stays_in_frame_through_every_mood_and_motion() {
    let started = Instant::now();
    let next = AtomicU64::new(0);
    let results = Mutex::new(Vec::new());
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= PLAZA_ROBOTS {
                    break;
                }
                let found = check(index);
                results.lock().expect("no check panicked").push(found);
            });
        }
    });
    let mut results = results.into_inner().expect("no check panicked");
    results.sort_by_key(|f| f.index);
    let poses: usize = results.iter().map(|f| f.poses).sum();
    for f in &results {
        println!(
            "genome {}: largest carried-corner spread {:.4} ({}); {} pairs with a corner out, largest vertex spread among them {:.4}",
            f.index, f.worst, f.worst_at, f.vertex_checks, f.worst_vertex
        );
    }
    println!(
        "{poses} poses over {PLAZA_ROBOTS} robots and {} views in {:.1} s on {workers} threads",
        orbits().len() * ASPECTS.len(),
        started.elapsed().as_secs_f64()
    );
    let failures: Vec<&String> = results.iter().flat_map(|f| &f.failures).collect();
    assert!(
        failures.is_empty(),
        "posed vertices leave the frame:\n{}",
        failures
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
