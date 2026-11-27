//! Faces read at rest: from the stage camera, every face target is the first
//! thing its ray meets, on its part's textured front, and every face element
//! lies on the head's textured front.

use avatar_3d::mesh::Mesh;
use avatar_3d::robot::{build_with, to_model, BuildOptions, Robot};
use avatar_small_robots::decode;
use avatar_small_robots::layout::Layout;

use crate::cases;

/// How far, in model units, the first hit may lie from its target.
pub const HIT_TOLERANCE: f64 = 1e-7;

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The barycentric weights of a projection point in a triangle's
/// projection, if it lies inside or on it.
fn weights(mesh: &Mesh, t: [u32; 3], p: [f64; 2]) -> Option<[f64; 3]> {
    let [a, b, c] = t.map(|i| mesh.projection[i as usize]);
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if area == 0.0 {
        return None;
    }
    let w = |p0: [f64; 2], p1: [f64; 2]| {
        ((p1[0] - p0[0]) * (p[1] - p0[1]) - (p1[1] - p0[1]) * (p[0] - p0[0])) / area
    };
    let ws = [w(b, c), w(c, a), w(a, b)];
    ws.iter().all(|w| *w >= -1e-12).then_some(ws)
}

/// A projection point placed on a part's textured front: the front-most
/// such point over textured-front triangles, in model space.
pub fn on_front(mesh: &Mesh, p: [f64; 2]) -> Option<V3> {
    let mut best: Option<V3> = None;
    for (t, front) in mesh.triangles.iter().zip(&mesh.textured_front) {
        if !front {
            continue;
        }
        let Some(w) = weights(mesh, *t, p) else {
            continue;
        };
        let [a, b, c] = t.map(|i| mesh.positions[i as usize]);
        let q = [0, 1, 2].map(|k| w[0] * a[k] + w[1] * b[k] + w[2] * c[k]);
        if best.is_none_or(|b| q[2] > b[2]) {
            best = Some(q);
        }
    }
    best
}

/// The first triangle a ray from `from` toward `to` meets: its part, its
/// triangle and the hit point. Möller–Trumbore, both faces.
fn first_hit(robot: &Robot, from: V3, to: V3) -> Option<(usize, usize, V3)> {
    let d = sub(to, from);
    let mut best: Option<(f64, usize, usize)> = None;
    for (pi, part) in robot.parts.iter().enumerate() {
        let m = &part.mesh;
        for (ti, t) in m.triangles.iter().enumerate() {
            let [a, b, c] = t.map(|i| m.positions[i as usize]);
            let (e1, e2) = (sub(b, a), sub(c, a));
            let p = cross(d, e2);
            let det = dot(e1, p);
            if det.abs() < 1e-18 {
                continue;
            }
            let s = sub(from, a);
            let u = dot(s, p) / det;
            if !(-1e-12..=1.0 + 1e-12).contains(&u) {
                continue;
            }
            let q = cross(s, e1);
            let v = dot(d, q) / det;
            if v < -1e-12 || u + v > 1.0 + 1e-12 {
                continue;
            }
            let t = dot(e2, q) / det;
            if t > 1e-9 && best.is_none_or(|b| t < b.0) {
                best = Some((t, pi, ti));
            }
        }
    }
    best.map(|(t, pi, ti)| (pi, ti, [0, 1, 2].map(|k| from[k] + d[k] * t)))
}

/// Each face target with the part that carries it: every non-stalk eye
/// centre and the mouth centre on the head, and each stalk eye's centre
/// on its own pod, all in model space on their textured fronts.
fn targets(robot: &Robot, layout: &Layout, label: &str) -> Vec<(String, usize, V3)> {
    let head = robot.parts.iter().position(|p| p.name == "head").unwrap();
    let mut points: Vec<(String, usize, [f64; 2])> = layout
        .face
        .eyes
        .iter()
        .enumerate()
        .map(|(i, e)| (format!("eye {i}"), head, e.centre))
        .collect();
    points.push(("mouth".into(), head, layout.face.mouth_centre));
    for s in &layout.top.stalks {
        let name = if s.side < 0 {
            "stalk.L.eye"
        } else {
            "stalk.R.eye"
        };
        let pod = robot.parts.iter().position(|p| p.name == name).unwrap();
        points.push((name.into(), pod, s.rest.eye_centre));
    }
    points
        .into_iter()
        .map(|(name, part, p)| {
            let q = on_front(&robot.parts[part].mesh, p).unwrap_or_else(|| {
                panic!("{label}: {name} at {p:?} is off its part's textured front")
            });
            (name, part, q)
        })
        .collect()
}

#[test]
fn every_face_target_is_seen_first_from_the_stage_camera() {
    for (label, genome) in cases::cases(cases::committed_prefix()) {
        let layout = Layout::new(&decode(&genome));
        let robot = build_with(&genome, BuildOptions::untextured());
        for (name, part, target) in targets(&robot, &layout, &label) {
            let (hit_part, tri, at) = first_hit(&robot, robot.camera.eye, target)
                .unwrap_or_else(|| panic!("{label}: the ray to {name} meets nothing"));
            assert_eq!(
                robot.parts[hit_part].name, robot.parts[part].name,
                "{label}: the ray to {name} first meets {}",
                robot.parts[hit_part].name
            );
            assert!(
                robot.parts[hit_part].mesh.textured_front[tri],
                "{label}: the ray to {name} first meets an untextured triangle"
            );
            let miss = sub(at, target);
            assert!(
                dot(miss, miss).sqrt() <= HIT_TOLERANCE,
                "{label}: {name} is hit {miss:?} away"
            );
        }
    }
}

#[test]
fn every_face_element_lies_on_the_heads_textured_front() {
    for (label, genome) in cases::cases(cases::committed_prefix()) {
        let layout = Layout::new(&decode(&genome));
        let robot = build_with(&genome, BuildOptions::untextured());
        let head = &robot.parts.iter().find(|p| p.name == "head").unwrap().mesh;
        let f = &layout.face;
        let mut elements: Vec<(&str, &[[f64; 2]])> = f
            .eyes
            .iter()
            .map(|e| ("eye housing", e.outline.as_slice()))
            .collect();
        elements.extend(
            f.mouth_outlines
                .iter()
                .map(|c| ("mouth", c.points.as_slice())),
        );
        if let Some(bezel) = &f.bezel_outline {
            elements.push(("bezel", bezel.as_slice()));
        }
        for (name, points) in elements {
            for p in points {
                let over = head
                    .triangles
                    .iter()
                    .zip(&head.textured_front)
                    .any(|(t, front)| *front && weights(head, *t, *p).is_some());
                assert!(
                    over,
                    "{label}: a {name} point {p:?} is off the head's textured front"
                );
            }
        }
        // The stated transform puts the face on the +z side.
        let oy = layout.head.origin[1] - layout.rise;
        let mouth = to_model(f.mouth_centre, 0.0, oy);
        let on = on_front(head, f.mouth_centre).unwrap();
        assert!(
            (on[0] - mouth[0]).abs() < 1e-12 && (on[1] - mouth[1]).abs() < 1e-12 && on[2] > 0.0
        );
    }
}
