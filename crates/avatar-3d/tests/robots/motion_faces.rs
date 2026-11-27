//! Faces read under every mood's posture: from the stage camera, each face
//! target is still the first thing its ray meets, on its part's textured
//! front, with the robot's own arms and hands left out of the occluders (a
//! gesture such as a hand at the chin may cross the face).

use avatar_3d::motion::Actor;
use avatar_3d::robot::{build_with, BuildOptions, Robot, Skin};
use avatar_small_robots::layout::Layout;
use avatar_small_robots::{decode, MOODS};

use crate::cases;
use crate::faces::targets;
use crate::motion::{apply, local, name, posed, V3};

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

/// One posed occluder: its part, posed vertices and bounds.
struct Posed {
    part: usize,
    points: Vec<V3>,
    lo: V3,
    hi: V3,
}

/// Whether the ray `from + s·d`, s > 0, can meet the box at all.
fn may_hit(from: V3, d: V3, lo: V3, hi: V3) -> bool {
    let (mut near, mut far) = (0.0_f64, f64::INFINITY);
    for k in 0..3 {
        if d[k] == 0.0 {
            if from[k] < lo[k] - 1e-9 || from[k] > hi[k] + 1e-9 {
                return false;
            }
            continue;
        }
        let (a, b) = ((lo[k] - from[k]) / d[k], (hi[k] - from[k]) / d[k]);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    near <= far + 1e-9
}

/// A posed triangle that the segment from `from` to the target `to` meets
/// strictly before the target: its part and triangle. Möller–Trumbore, both
/// faces. Triangles meeting the segment at the target itself, such as the
/// target's own and its neighbours on the same surface, are not counted.
fn occluder(robot: &Robot, occluders: &[Posed], from: V3, to: V3) -> Option<(usize, usize, f64)> {
    let d = sub(to, from);
    for o in occluders {
        if !may_hit(from, d, o.lo, o.hi) {
            continue;
        }
        for (ti, t) in robot.parts[o.part].mesh.triangles.iter().enumerate() {
            let [a, b, c] = t.map(|i| o.points[i as usize]);
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
            if t > 1e-9 && t < 1.0 - 1e-9 {
                return Some((o.part, ti, t));
            }
        }
    }
    None
}

/// Each face target with the part and textured-front triangle carrying it
/// at rest: every non-stalk eye centre and the mouth centre on the head,
/// and each stalk eye's centre on its own pod, placed as the rest check
/// places them (the front-most textured-front point).
fn carried(robot: &Robot, layout: &Layout, label: &str) -> Vec<(String, usize, usize, V3)> {
    targets(robot, layout, label)
        .into_iter()
        .map(|(name, part, at)| {
            let mesh = &robot.parts[part].mesh;
            let tri = (0..mesh.triangles.len())
                .filter(|&t| mesh.textured_front[t])
                .find(|&t| {
                    let [a, b, c] = mesh.triangles[t].map(|i| mesh.positions[i as usize]);
                    let n = cross(sub(b, a), sub(c, a));
                    let w = [
                        cross(sub(b, at), sub(c, at)),
                        cross(sub(c, at), sub(a, at)),
                        cross(sub(a, at), sub(b, at)),
                    ];
                    let area = dot(n, n);
                    // On the triangle's plane and inside or on its edges.
                    dot(sub(at, a), n).abs() <= 1e-12 * area.sqrt()
                        && w.iter().all(|w| dot(*w, n) >= -1e-12 * area)
                })
                .unwrap_or_else(|| panic!("{label}: {name} lies on no textured-front triangle"));
            (name, part, tri, at)
        })
        .collect()
}

#[test]
fn every_face_target_is_seen_first_in_every_moods_posture() {
    for (label, genome) in cases::cases(cases::committed_prefix()) {
        let layout = Layout::new(&decode(&genome));
        let robot = build_with(&genome, BuildOptions::untextured());
        let rest_targets = carried(&robot, &layout, &label);
        for mood in MOODS {
            let label = format!("{label} {}", name(mood));
            let pose = Actor::new(&robot, mood).without_idle().evaluate(0.0);
            let occluders: Vec<Posed> = (0..robot.parts.len())
                .filter(|&i| !matches!(robot.parts[i].entry.0, "arms" | "hand"))
                .map(|i| {
                    let points = posed(&robot, &pose, i);
                    let mut lo = [f64::INFINITY; 3];
                    let mut hi = [f64::NEG_INFINITY; 3];
                    for p in &points {
                        for k in 0..3 {
                            lo[k] = lo[k].min(p[k]);
                            hi[k] = hi[k].max(p[k]);
                        }
                    }
                    Posed {
                        part: i,
                        points,
                        lo,
                        hi,
                    }
                })
                .collect();
            for (target_name, part, tri, rest) in &rest_targets {
                let Skin::Rigid { bone } = robot.parts[*part].skin else {
                    panic!("{label}: a face target's part is rigid");
                };
                let target = apply(&pose.bones[bone], local(&robot, bone, *rest));
                // The target's own triangle is textured front and faces the
                // camera, so the ray meets it from the front, at the target.
                let mesh = &robot.parts[*part].mesh;
                assert!(mesh.textured_front[*tri]);
                let corners = mesh.triangles[*tri].map(|i| mesh.positions[i as usize]);
                let winding = |[a, b, c]: [V3; 3]| cross(sub(b, a), sub(c, a));
                // Whether the triangle's winding turns with its outward
                // normal or against it.
                let outward = dot(
                    winding(corners),
                    mesh.normals[mesh.triangles[*tri][0] as usize],
                )
                .signum();
                let posed_corners =
                    corners.map(|p| apply(&pose.bones[bone], local(&robot, bone, p)));
                assert!(
                    outward * dot(winding(posed_corners), sub(robot.camera.eye, target)) > 0.0,
                    "{label}: {target_name}'s surface faces away from the camera"
                );
                // Nothing else lies between the camera and the target.
                if let Some((p, t, at)) = occluder(&robot, &occluders, robot.camera.eye, target) {
                    panic!(
                        "{label}: the ray to {target_name} first meets {} (triangle {t}, textured front {}) at {at} of the way",
                        robot.parts[p].name, robot.parts[p].mesh.textured_front[t]
                    );
                }
            }
        }
        // The face elements lie on the head's textured front in its
        // projection coordinates, which a pose cannot change: the head is
        // rigid, so the rest check holds in every posture.
        let head = robot.parts.iter().find(|p| p.name == "head").unwrap();
        assert!(
            matches!(head.skin, Skin::Rigid { .. }),
            "{label}: the head is rigid"
        );
    }
}
