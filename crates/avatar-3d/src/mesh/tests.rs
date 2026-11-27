//! Checks of the mesh kit: closed and open solids, areas, normals, the
//! footprint of concave extrusions, lathe radii, textured-front tags,
//! projection coordinates, domain errors and pinned digests.

use std::collections::{HashMap, HashSet};

use sha2::{Digest, Sha256};

use super::*;

/// A box two wide and three tall, standing on y = 0.
const BOX: [[f64; 2]; 4] = [[-1.0, 0.0], [1.0, 0.0], [1.0, 3.0], [-1.0, 3.0]];

/// The corners of the small robots' `tee` head outline at a typical size,
/// with y increasing downward and the base on y = 0, and each corner's
/// rounding radius.
fn tee_corners() -> Vec<([f64; 2], f64)> {
    let (w, h) = (30.0, 50.0);
    let (wj, hj) = (w * 0.67, h * 0.32);
    vec![
        ([-wj, 0.0], 7.0),
        ([-wj, -hj], 4.0),
        ([-w, -hj], 7.0),
        ([-w, -h], 13.0),
        ([w, -h], 13.0),
        ([w, -hj], 7.0),
        ([wj, -hj], 4.0),
        ([wj, 0.0], 7.0),
    ]
}

fn tee_sharp() -> Vec<[f64; 2]> {
    tee_corners().into_iter().map(|(p, _)| p).collect()
}

/// The tee with each corner rounded by a quadratic curve sampled in `steps`,
/// its radius clamped to half of each neighbouring edge as hulls clamp it.
fn tee_round(steps: u32) -> Vec<[f64; 2]> {
    let corners = tee_corners();
    let n = corners.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (p, r) = corners[i];
        let a = corners[(i + n - 1) % n].0;
        let b = corners[(i + 1) % n].0;
        let towards = |q: [f64; 2]| {
            let d = sub2(q, p);
            let l = dot2(d, d).sqrt();
            let k = r.min(0.5 * l) / l;
            [p[0] + d[0] * k, p[1] + d[1] * k]
        };
        let (start, end) = (towards(a), towards(b));
        for k in 0..=steps {
            let t = f64::from(k) / f64::from(steps);
            let u = 1.0 - t;
            out.push([
                u * u * start[0] + 2.0 * u * t * p[0] + t * t * end[0],
                u * u * start[1] + 2.0 * u * t * p[1] + t * t * end[1],
            ]);
        }
    }
    out
}

fn extruded(points: &[[f64; 2]], depth: f64, bevel: f64, bevel_steps: u32) -> Mesh {
    let outline = Outline::new(points).expect("a valid outline");
    extrude(
        &outline,
        &Extrusion {
            depth,
            bevel,
            bevel_steps,
        },
    )
    .expect("an extrusion in the domain")
}

/// Half outlines for lathes: a frustum with a flat top, a cone whose tip
/// needs a normal per wedge, and a smooth bulb.
const FRUSTUM: [[f64; 2]; 4] = [[0.0, 0.0], [2.0, 0.0], [1.0, 2.0], [0.0, 2.0]];
const CONE: [[f64; 2]; 3] = [[0.0, 0.0], [1.0, 0.0], [0.0, 2.0]];
const BULB: [[f64; 2]; 7] = [
    [0.0, 0.0],
    [1.0, 0.2],
    [1.6, 1.0],
    [1.8, 2.0],
    [1.4, 3.0],
    [0.6, 3.6],
    [0.0, 3.7],
];
/// A bell open at its rim (y = 0) and closed at its top.
const BELL: [[f64; 2]; 3] = [[1.0, 0.0], [1.2, 1.0], [0.0, 1.5]];
/// A band open at both ends, given top first.
const BAND: [[f64; 2]; 2] = [[1.0, 1.0], [1.0, 0.0]];
const TUBE_PATH: [[f64; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.5, 2.0, 0.3],
    [1.5, 2.4, 0.3],
];

fn track() -> Belt {
    Belt {
        radius: 1.0,
        span: 3.0,
        thickness: 0.3,
        width: 0.8,
        axis: Axis::X,
        segments: 6,
    }
}

/// Every closed fixture, by name.
fn closed_solids() -> Vec<(&'static str, Mesh)> {
    vec![
        ("box", extruded(&BOX, 1.0, 0.0, 0)),
        ("box bevelled", extruded(&BOX, 1.0, 0.25, 2)),
        ("tee", extruded(&tee_sharp(), 8.0, 1.5, 3)),
        ("tee rounded", extruded(&tee_round(6), 8.0, 1.0, 2)),
        ("lathe frustum", lathe(&FRUSTUM, 16).unwrap()),
        ("lathe cone", lathe(&CONE, 12).unwrap()),
        ("lathe bulb", lathe(&BULB, 24).unwrap()),
        ("cylinder x", cylinder(0.5, 2.0, Axis::X, 12).unwrap()),
        ("cylinder y", cylinder(0.5, 2.0, Axis::Y, 12).unwrap()),
        ("cylinder z", cylinder(0.5, 2.0, Axis::Z, 12).unwrap()),
        ("capsule", capsule(0.4, 1.2, Axis::X, 12, 3).unwrap()),
        ("capsule short", capsule(0.4, 0.0, Axis::Z, 8, 2).unwrap()),
        ("sphere", sphere(1.5, 16, 8).unwrap()),
        ("torus", torus(2.0, 0.5, Axis::Z, 16, 8).unwrap()),
        ("belt", belt(&track()).unwrap()),
        (
            "belt ring",
            belt(&Belt {
                span: 0.0,
                axis: Axis::Z,
                ..track()
            })
            .unwrap(),
        ),
        (
            "belt flat",
            belt(&Belt {
                axis: Axis::Y,
                ..track()
            })
            .unwrap(),
        ),
    ]
}

/// Every open fixture, by name.
fn open_solids() -> Vec<(&'static str, Mesh)> {
    vec![
        ("dome", dome(1.0, 16, 4).unwrap()),
        ("tube", tube(&TUBE_PATH, 0.2, 8).unwrap()),
        ("lathe bell", lathe(&BELL, 12).unwrap()),
        ("lathe band", lathe(&BAND, 8).unwrap()),
    ]
}

fn all_solids() -> Vec<(&'static str, Mesh)> {
    let mut all = closed_solids();
    all.extend(open_solids());
    all
}

/// Each vertex's index among the distinct positions, so vertices split for
/// normal seams become one.
fn weld(mesh: &Mesh) -> Vec<usize> {
    let mut ids = HashMap::new();
    mesh.positions
        .iter()
        .map(|p| {
            let next = ids.len();
            *ids.entry(p.map(f64::to_bits)).or_insert(next)
        })
        .collect()
}

/// Directed welded edges with how many triangles use each.
fn directed_edges(name: &str, mesh: &Mesh) -> HashMap<(usize, usize), usize> {
    let ids = weld(mesh);
    let mut edges = HashMap::new();
    for t in &mesh.triangles {
        let [a, b, c] = t.map(|i| ids[i as usize]);
        assert!(a != b && b != c && c != a, "{name}: a triangle collapses");
        for edge in [(a, b), (b, c), (c, a)] {
            *edges.entry(edge).or_insert(0) += 1;
        }
    }
    for (edge, uses) in &edges {
        assert_eq!(
            *uses, 1,
            "{name}: edge {edge:?} is wound the same way twice"
        );
    }
    edges
}

/// The loops of edges used by one triangle only, as welded vertex ids.
fn boundary_loops(name: &str, mesh: &Mesh) -> Vec<Vec<usize>> {
    let edges = directed_edges(name, mesh);
    let mut next: HashMap<usize, usize> = HashMap::new();
    for &(a, b) in edges.keys() {
        if !edges.contains_key(&(b, a)) {
            assert!(next.insert(a, b).is_none(), "{name}: boundary branches");
        }
    }
    let mut loops = Vec::new();
    let mut seen = HashSet::new();
    let mut starts: Vec<usize> = next.keys().copied().collect();
    starts.sort_unstable();
    for start in starts {
        if seen.contains(&start) {
            continue;
        }
        let mut cycle = vec![start];
        seen.insert(start);
        let mut at = next[&start];
        while at != start {
            assert!(seen.insert(at), "{name}: boundary loops cross");
            cycle.push(at);
            at = next[&at];
        }
        loops.push(cycle);
    }
    loops
}

fn welded_position(mesh: &Mesh, id: usize) -> [f64; 3] {
    let ids = weld(mesh);
    mesh.positions[ids.iter().position(|i| *i == id).unwrap()]
}

fn signed_volume(mesh: &Mesh) -> f64 {
    mesh.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| mesh.positions[i as usize]);
            dot3(a, cross3(b, c)) / 6.0
        })
        .sum()
}

fn close3(a: [f64; 3], b: [f64; 3], tolerance: f64) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() <= tolerance)
}

#[test]
fn closed_solids_are_watertight_with_consistent_winding_and_positive_volume() {
    for (name, mesh) in closed_solids() {
        let loops = boundary_loops(name, &mesh);
        assert!(loops.is_empty(), "{name}: open edges {loops:?}");
        let volume = signed_volume(&mesh);
        assert!(volume > 0.0, "{name}: volume {volume}");
    }
    // Hand-computed volumes: a square-edged box, and the sharp tee without a
    // bevel, whose volume is its outline's area times its depth.
    let volume = signed_volume(&extruded(&BOX, 1.0, 0.0, 0));
    assert!((volume - 6.0).abs() < 1e-12, "box volume {volume}");
    let tee = extruded(&tee_sharp(), 8.0, 0.0, 0);
    let area = Outline::new(&tee_sharp()).unwrap().area();
    assert!((signed_volume(&tee) - 8.0 * area).abs() < 1e-9 * area);
}

#[test]
fn open_solids_leave_exactly_their_documented_boundaries() {
    let ring_at = |name: &str, mesh: &Mesh, size: usize, on: &dyn Fn([f64; 3]) -> bool| {
        let loops = boundary_loops(name, mesh);
        loops
            .iter()
            .filter(|l| l.len() == size && l.iter().all(|v| on(welded_position(mesh, *v))))
            .count()
    };
    let near = |a: f64, b: f64| (a - b).abs() < 1e-12;
    let radius = |p: [f64; 3]| (p[0] * p[0] + p[2] * p[2]).sqrt();

    // A dome's rim, at y = 0 and its radius, is its only boundary.
    let mesh = dome(1.0, 16, 4).unwrap();
    assert_eq!(boundary_loops("dome", &mesh).len(), 1);
    let rim = |p: [f64; 3]| p[1] == 0.0 && near(radius(p), 1.0);
    assert_eq!(ring_at("dome", &mesh, 16, &rim), 1);

    // A tube's two end rings, one round each end of the path.
    let mesh = tube(&TUBE_PATH, 0.2, 8).unwrap();
    assert_eq!(boundary_loops("tube", &mesh).len(), 2);
    for end in [TUBE_PATH[0], TUBE_PATH[3]] {
        let round = |p: [f64; 3]| near(dot3(sub3(p, end), sub3(p, end)).sqrt(), 0.2);
        assert_eq!(ring_at("tube", &mesh, 8, &round), 1, "end {end:?}");
    }

    // A lathe leaves a ring open at each end off the axis.
    let mesh = lathe(&BELL, 12).unwrap();
    assert_eq!(boundary_loops("bell", &mesh).len(), 1);
    let rim = |p: [f64; 3]| p[1] == 0.0 && near(radius(p), 1.0);
    assert_eq!(ring_at("bell", &mesh, 12, &rim), 1);
    let mesh = lathe(&BAND, 8).unwrap();
    assert_eq!(boundary_loops("band", &mesh).len(), 2);
    for y in [0.0, 1.0] {
        let rim = |p: [f64; 3]| p[1] == y && near(radius(p), 1.0);
        assert_eq!(ring_at("band", &mesh, 8, &rim), 1, "rim at {y}");
    }
}

#[test]
fn no_triangle_is_smaller_than_the_area_epsilon() {
    for (name, mesh) in all_solids() {
        for (i, t) in mesh.triangles.iter().enumerate() {
            let [a, b, c] = t.map(|i| mesh.positions[i as usize]);
            let n = cross3(sub3(b, a), sub3(c, a));
            let area = 0.5 * dot3(n, n).sqrt();
            assert!(area >= AREA_EPSILON, "{name}: triangle {i} has area {area}");
        }
    }
}

#[test]
fn normals_are_unit_length() {
    for (name, mesh) in all_solids() {
        assert_eq!(mesh.normals.len(), mesh.positions.len(), "{name}");
        for n in &mesh.normals {
            let l = dot3(*n, *n).sqrt();
            assert!((l - 1.0).abs() <= 1e-12, "{name}: normal {n:?}");
        }
    }
}

/// Asserts that the vertices at `p` carry exactly the `expected` normals.
fn assert_normals(name: &str, mesh: &Mesh, p: [f64; 3], expected: &[[f64; 3]]) {
    let got: Vec<[f64; 3]> = mesh
        .positions
        .iter()
        .zip(&mesh.normals)
        .filter(|(q, _)| close3(**q, p, 1e-12))
        .map(|(_, n)| *n)
        .collect();
    assert_eq!(got.len(), expected.len(), "{name} at {p:?}: {got:?}");
    for e in expected {
        assert!(
            got.iter().any(|g| close3(*g, *e, 1e-12)),
            "{name} at {p:?}: wanted {e:?}, got {got:?}"
        );
    }
}

#[test]
fn normals_match_hand_computed_outward_directions() {
    let h = std::f64::consts::FRAC_1_SQRT_2;
    let (r5, r20) = (1.0 / 5.0_f64.sqrt(), 2.0 / 5.0_f64.sqrt());

    // A box bevelled by 0.25 in two steps, 1 deep. The bottom-right corner
    // (1, 0) is mitred inward along (-1, 1); each step of the quarter-round
    // turns the normal 45 degrees from +z toward the side.
    let mesh = extruded(&BOX, 1.0, 0.25, 2);
    let name = "box bevelled";
    assert_normals(name, &mesh, [0.75, 0.25, 0.5], &[[0.0, 0.0, 1.0]]);
    let o = 0.25 * (1.0 - h);
    let z = 0.5 - 0.25 * (1.0 - h);
    let corner = [1.0 - o, o, z];
    assert_normals(name, &mesh, corner, &[[h, 0.0, h], [0.0, -h, h]]);
    assert_normals(
        name,
        &mesh,
        [1.0, 0.0, 0.25],
        &[[1.0, 0.0, 0.0], [0.0, -1.0, 0.0]],
    );
    assert_normals(
        name,
        &mesh,
        [1.0, 0.0, -0.25],
        &[[1.0, 0.0, 0.0], [0.0, -1.0, 0.0]],
    );
    assert_normals(name, &mesh, [-0.75, 2.75, -0.5], &[[0.0, 0.0, -1.0]]);
    // Without a bevel the cap and the sides meet at a crease.
    let mesh = extruded(&BOX, 1.0, 0.0, 0);
    let expected = [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]];
    assert_normals("box", &mesh, [1.0, 0.0, 0.5], &expected);

    // The frustum's side slopes inward by 1 over a rise of 2, so its normal
    // is (2, 1) / sqrt(5) in (radius, height), turned to each angle.
    let mesh = lathe(&FRUSTUM, 16).unwrap();
    let name = "lathe frustum";
    assert_normals(
        name,
        &mesh,
        [0.0, 0.0, 2.0],
        &[[0.0, -1.0, 0.0], [0.0, r5, r20]],
    );
    assert_normals(
        name,
        &mesh,
        [2.0, 0.0, 0.0],
        &[[0.0, -1.0, 0.0], [r20, r5, 0.0]],
    );
    assert_normals(
        name,
        &mesh,
        [0.0, 2.0, -1.0],
        &[[0.0, 1.0, 0.0], [0.0, r5, -r20]],
    );
    assert_normals(name, &mesh, [0.0, 2.0, 0.0], &[[0.0, 1.0, 0.0]]);
    assert_normals(name, &mesh, [0.0, 0.0, 0.0], &[[0.0, -1.0, 0.0]]);

    // A cylinder along x: flat ends face along x, the side faces outward.
    let mesh = cylinder(0.5, 2.0, Axis::X, 12).unwrap();
    let name = "cylinder x";
    assert_normals(
        name,
        &mesh,
        [1.0, 0.0, 0.5],
        &[[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
    );
    let expected = [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0]];
    assert_normals(name, &mesh, [-1.0, -0.5, 0.0], &expected);
    assert_normals(name, &mesh, [1.0, 0.0, 0.0], &[[1.0, 0.0, 0.0]]);

    // A sphere's normal is its position over its radius.
    let mesh = sphere(1.5, 16, 8).unwrap();
    assert_normals("sphere", &mesh, [1.5, 0.0, 0.0], &[[1.0, 0.0, 0.0]]);
    assert_normals("sphere", &mesh, [0.0, 1.5, 0.0], &[[0.0, 1.0, 0.0]]);
    let p = [0.0, -1.5 * h, 1.5 * h];
    assert_normals("sphere", &mesh, p, &[[0.0, -h, h]]);
}

/// Whether `p` lies inside the polygon or within `tolerance` of its edges.
fn inside_or_on(polygon: &[[f64; 2]], p: [f64; 2], tolerance: f64) -> bool {
    let n = polygon.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + 1) % n]);
        let ab = sub2(b, a);
        let t = (dot2(sub2(p, a), ab) / dot2(ab, ab)).clamp(0.0, 1.0);
        let d = sub2(p, [a[0] + ab[0] * t, a[1] + ab[1] * t]);
        if dot2(d, d).sqrt() <= tolerance {
            return true;
        }
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0])
        {
            inside = !inside;
        }
    }
    inside
}

/// Whether segments ab and cd cross at a point inside both, beyond rounding.
fn cross_properly(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let tolerance = 1e-9;
    let apart =
        |x: f64, y: f64| (x > tolerance && y < -tolerance) || (x < -tolerance && y > tolerance);
    apart(orient(c, d, a), orient(c, d, b)) && apart(orient(a, b, c), orient(a, b, d))
}

#[test]
fn concave_extrusion_footprints_stay_inside_the_outline() {
    for (name, points, bevel, steps) in [
        ("tee", tee_sharp(), 1.5, 3),
        ("tee rounded", tee_round(6), 1.0, 2),
        ("tee rounded flat", tee_round(6), 0.0, 0),
    ] {
        let mesh = extruded(&points, 8.0, bevel, steps);
        for t in &mesh.triangles {
            let corners = t.map(|i| mesh.projection[i as usize]);
            let centroid = [
                (corners[0][0] + corners[1][0] + corners[2][0]) / 3.0,
                (corners[0][1] + corners[1][1] + corners[2][1]) / 3.0,
            ];
            for p in corners.iter().chain([&centroid]) {
                assert!(inside_or_on(&points, *p, 1e-9), "{name}: {p:?} outside");
            }
            for k in 0..3 {
                let (a, b) = (corners[k], corners[(k + 1) % 3]);
                for i in 0..points.len() {
                    let (c, d) = (points[i], points[(i + 1) % points.len()]);
                    assert!(!cross_properly(a, b, c, d), "{name}: {a:?}-{b:?} leaves");
                }
            }
        }
    }
}

#[test]
fn lathe_radii_never_exceed_the_profile_at_their_height() {
    let check = |name: &str, profile: &[[f64; 2]], mesh: &Mesh| {
        for p in &mesh.positions {
            let y = p[1];
            let mut reach: Option<f64> = None;
            for w in profile.windows(2) {
                let ([r0, y0], [r1, y1]) = (w[0], w[1]);
                if y < y0.min(y1) || y > y0.max(y1) {
                    continue;
                }
                let r = if y0 == y1 {
                    r0.max(r1)
                } else {
                    r0 + (r1 - r0) * (y - y0) / (y1 - y0)
                };
                reach = Some(reach.map_or(r, |m: f64| m.max(r)));
            }
            let reach = reach.unwrap_or_else(|| panic!("{name}: {p:?} beyond the profile"));
            let radius = (p[0] * p[0] + p[2] * p[2]).sqrt();
            assert!(
                radius <= reach * (1.0 + 1e-12) + 1e-12,
                "{name}: {p:?} reaches {radius} over {reach}"
            );
        }
    };
    for (name, profile, segments) in [
        ("frustum", &FRUSTUM[..], 16),
        ("cone", &CONE[..], 12),
        ("bulb", &BULB[..], 24),
        ("bell", &BELL[..], 12),
        ("band", &BAND[..], 8),
    ] {
        check(name, profile, &lathe(profile, segments).unwrap());
    }
}

#[test]
fn textured_front_is_the_extrusion_front_cap() {
    for (name, points, depth, bevel, steps, inset_area) in [
        ("box", &BOX[..], 1.0, 0.0, 0, 6.0),
        ("box bevelled", &BOX[..], 1.0, 0.25, 2, 1.5 * 2.5),
        ("tee", &tee_sharp()[..], 8.0, 1.5, 3, f64::NAN),
    ] {
        let mesh = extruded(points, depth, bevel, steps);
        let mut area = 0.0;
        for (t, front) in mesh.triangles.iter().zip(&mesh.textured_front) {
            let on_cap = t.iter().all(|i| {
                let i = *i as usize;
                mesh.positions[i][2] == 0.5 * depth && mesh.normals[i] == [0.0, 0.0, 1.0]
            });
            assert_eq!(*front, on_cap, "{name}: {t:?}");
            if *front {
                let [a, b, c] = t.map(|i| mesh.positions[i as usize]);
                area += 0.5 * cross3(sub3(b, a), sub3(c, a))[2];
            }
        }
        let expected = if inset_area.is_nan() {
            Outline::new(points).unwrap().inset(bevel).unwrap().area()
        } else {
            inset_area
        };
        assert!((area - expected).abs() < 1e-9, "{name}: front area {area}");
    }
}

#[test]
fn textured_front_elsewhere_is_every_triangle_facing_plus_z() {
    for (name, mesh) in all_solids() {
        if name.starts_with("box") || name.starts_with("tee") {
            continue;
        }
        for (t, front) in mesh.triangles.iter().zip(&mesh.textured_front) {
            let [a, b, c] = t.map(|i| mesh.positions[i as usize]);
            let facing = cross3(sub3(b, a), sub3(c, a))[2] > 0.0;
            assert_eq!(*front, facing, "{name}: {t:?}");
        }
        assert!(mesh.textured_front.iter().any(|f| *f), "{name}: no front");
    }
    // On a lathe with a multiple of 4 segments no triangle straddles z = 0:
    // front triangles lie at z >= 0 and the rest at z <= 0.
    let mesh = sphere(1.5, 16, 8).unwrap();
    for (t, front) in mesh.triangles.iter().zip(&mesh.textured_front) {
        let z = t.map(|i| mesh.positions[i as usize][2]);
        if *front {
            assert!(z.iter().all(|z| *z >= 0.0) && z.iter().any(|z| *z > 0.0));
        } else {
            assert!(z.iter().all(|z| *z <= 0.0));
        }
    }
}

#[test]
fn projection_coordinates_are_the_rest_local_xy() {
    for (name, mesh) in all_solids() {
        assert_eq!(mesh.projection.len(), mesh.positions.len(), "{name}");
        assert_eq!(mesh.textured_front.len(), mesh.triangles.len(), "{name}");
        for (q, p) in mesh.projection.iter().zip(&mesh.positions) {
            assert_eq!(
                q.map(f64::to_bits),
                [p[0], p[1]].map(f64::to_bits),
                "{name}"
            );
        }
    }
    // The front cap of an unbevelled extrusion is addressed by the outline's
    // own coordinates.
    let mesh = extruded(&BOX, 1.0, 0.0, 0);
    for corner in BOX {
        assert!(mesh.projection.contains(&corner));
    }
}

#[test]
fn outlines_are_normalised_without_repair() {
    let clockwise = [BOX[0], BOX[3], BOX[2], BOX[1]];
    let ccw = Outline::new(&BOX).unwrap();
    assert_eq!(Outline::new(&clockwise).unwrap(), ccw);
    assert_eq!(ccw.points(), &BOX);
    let mut closed = BOX.to_vec();
    closed.push(BOX[0]);
    assert_eq!(Outline::new(&closed).unwrap(), ccw);
    let spec = Extrusion {
        depth: 1.0,
        bevel: 0.25,
        bevel_steps: 2,
    };
    let reversed = Outline::new(&clockwise).unwrap();
    assert_eq!(extrude(&reversed, &spec), extrude(&ccw, &spec));
    // A point between two collinear neighbours is kept, not merged.
    let with_midpoint = [BOX[0], [0.0, 0.0], BOX[1], BOX[2], BOX[3]];
    let outline = Outline::new(&with_midpoint).unwrap();
    assert_eq!(outline.points().len(), 5);
    extrude(&outline, &spec).unwrap();
    // A lathe profile given top first turns the same way.
    let reversed: Vec<[f64; 2]> = FRUSTUM.iter().rev().copied().collect();
    let mesh = lathe(&reversed, 16).unwrap();
    assert!(signed_volume(&mesh) > 0.0);
}

#[test]
fn the_widest_accepted_bevel_is_found_and_no_wider_one_is_accepted() {
    for (name, points) in [("tee", tee_sharp()), ("tee rounded", tee_round(6))] {
        let outline = Outline::new(&points).unwrap();
        let widest = outline.max_bevel(3);
        assert!(widest > 0.5, "{name}: {widest}");
        let spec = |bevel| Extrusion {
            depth: 100.0,
            bevel,
            bevel_steps: 3,
        };
        extrude(&outline, &spec(widest)).unwrap();
        assert_eq!(
            extrude(&outline, &spec(widest * 1.5)),
            Err(MeshError::BevelTooWide),
            "{name}"
        );
        assert!(outline.inset(widest).is_ok());
    }
    assert_eq!(Outline::new(&BOX).unwrap().max_bevel(0), 0.0);
}

#[test]
fn inputs_outside_the_domain_are_errors() {
    use MeshError::*;
    let nan = f64::NAN;
    let outline = |p: &[[f64; 2]]| Outline::new(p).err();
    assert_eq!(outline(&[]), Some(TooFewPoints { needed: 3 }));
    assert_eq!(outline(&BOX[..2]), Some(TooFewPoints { needed: 3 }));
    assert_eq!(
        outline(&[[0.0, 0.0], [1.0, nan], [0.0, 1.0]]),
        Some(NonFinite)
    );
    let repeated = [BOX[0], BOX[1], BOX[1], BOX[2]];
    assert_eq!(outline(&repeated), Some(RepeatedPoint { index: 2 }));
    let bow_tie = [[0.0, 0.0], [1.0, 1.0], [1.0, 0.0], [0.0, 1.0]];
    assert_eq!(outline(&bow_tie), Some(NotSimple));
    let spike = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [2.0, 1.0]];
    assert_eq!(outline(&spike), Some(NotSimple));
    let touching = [
        [0.0, 0.0],
        [2.0, 0.0],
        [1.0, 1.0],
        [2.0, 2.0],
        [0.0, 2.0],
        [1.0, 1.0],
    ];
    assert_eq!(outline(&touching), Some(NotSimple));
    assert!(outline(&[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]]).is_some());

    let ok = Outline::new(&BOX).unwrap();
    let extrusion = |depth, bevel, bevel_steps| {
        extrude(
            &ok,
            &Extrusion {
                depth,
                bevel,
                bevel_steps,
            },
        )
        .err()
    };
    let parameter = |parameter| Some(OutOfRange { parameter });
    assert_eq!(extrusion(0.0, 0.0, 0), parameter("depth"));
    assert_eq!(extrusion(-1.0, 0.0, 0), parameter("depth"));
    assert_eq!(extrusion(nan, 0.0, 0), Some(NonFinite));
    assert_eq!(extrusion(1.0, -0.1, 1), parameter("bevel"));
    assert_eq!(extrusion(1.0, 0.5, 1), parameter("bevel"));
    assert_eq!(extrusion(1.0, 0.2, 0), parameter("bevel steps"));
    assert_eq!(extrusion(1.0, 0.2, MAX_STEPS + 1), parameter("bevel steps"));
    assert_eq!(extrusion(10.0, 1.2, 2), Some(BevelTooWide));
    assert_eq!(ok.inset(-0.1).err(), parameter("inset distance"));
    assert_eq!(ok.inset(1.2).err(), Some(BevelTooWide));

    assert_eq!(lathe(&FRUSTUM, 6).err(), parameter("segments"));
    assert_eq!(lathe(&FRUSTUM, 0).err(), parameter("segments"));
    assert_eq!(lathe(&FRUSTUM, 2048).err(), parameter("segments"));
    assert_eq!(
        lathe(&FRUSTUM[..1], 8).err(),
        Some(TooFewPoints { needed: 2 })
    );
    let pinched = [[0.0, 0.0], [1.0, 1.0], [0.0, 2.0], [1.0, 3.0], [0.0, 4.0]];
    assert_eq!(lathe(&pinched, 8).err(), parameter("profile radius"));
    let negative = [[0.0, 0.0], [-1.0, 1.0], [0.0, 2.0]];
    assert_eq!(lathe(&negative, 8).err(), parameter("profile radius"));
    let crossing = [[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [1.0, 3.0], [0.0, 3.0]];
    assert_eq!(lathe(&crossing, 8).err(), Some(NotSimple));
    let repeated = [[0.0, 0.0], [1.0, 1.0], [1.0, 1.0], [0.0, 2.0]];
    assert_eq!(lathe(&repeated, 8).err(), Some(RepeatedPoint { index: 2 }));
    assert_eq!(
        lathe(&[[0.0, 0.0], [0.0, 1.0]], 8).err(),
        Some(TooFewPoints { needed: 3 })
    );
    assert_eq!(lathe(&[[1.0, 0.0], [nan, 1.0]], 8).err(), Some(NonFinite));

    assert_eq!(cylinder(0.0, 1.0, Axis::Y, 8).err(), parameter("radius"));
    assert_eq!(cylinder(1.0, -1.0, Axis::Y, 8).err(), parameter("length"));
    assert_eq!(cylinder(1.0, 1.0, Axis::Y, 5).err(), parameter("segments"));
    assert_eq!(capsule(1.0, 1.0, Axis::Y, 8, 0).err(), parameter("rings"));
    assert_eq!(capsule(1.0, -0.1, Axis::Y, 8, 2).err(), parameter("length"));
    assert_eq!(sphere(1.0, 8, 3).err(), parameter("rings"));
    assert_eq!(sphere(-1.0, 8, 4).err(), parameter("radius"));
    assert_eq!(dome(nan, 8, 2).err(), Some(NonFinite));
    assert_eq!(
        torus(1.0, 1.0, Axis::Y, 8, 8).err(),
        parameter("major radius")
    );
    assert_eq!(
        torus(2.0, 0.0, Axis::Y, 8, 8).err(),
        parameter("minor radius")
    );
    assert_eq!(torus(2.0, 1.0, Axis::Y, 8, 6).err(), parameter("sides"));

    assert_eq!(
        tube(&TUBE_PATH[..1], 0.1, 8).err(),
        Some(TooFewPoints { needed: 2 })
    );
    assert_eq!(tube(&TUBE_PATH, 0.0, 8).err(), parameter("radius"));
    assert_eq!(tube(&TUBE_PATH, 0.1, 6).err(), parameter("sides"));
    let repeated = [TUBE_PATH[0], TUBE_PATH[1], TUBE_PATH[1]];
    assert_eq!(
        tube(&repeated, 0.1, 8).err(),
        Some(RepeatedPoint { index: 2 })
    );
    let doubling = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.1, 0.0]];
    assert_eq!(
        tube(&doubling, 0.1, 8).err(),
        Some(PathTooSharp { index: 1 })
    );
    // A right-angled turn takes one radius of room on each side of the
    // corner, so a segment of 0.15 cannot hold two rings of radius 0.1.
    let short = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.15, 0.0],
        [0.0, 0.15, 0.0],
    ];
    assert_eq!(tube(&short, 0.1, 8).err(), Some(PathTooSharp { index: 1 }));
    assert!(tube(&short, 0.07, 8).is_ok());
    assert_eq!(
        tube(&[[0.0, 0.0, nan], [1.0, 0.0, 0.0]], 0.1, 8).err(),
        Some(NonFinite)
    );

    let belt_error = |change: &dyn Fn(&mut Belt)| {
        let mut spec = track();
        change(&mut spec);
        belt(&spec).err()
    };
    assert_eq!(
        belt_error(&|b: &mut Belt| b.thickness = 2.0),
        parameter("thickness")
    );
    assert_eq!(
        belt_error(&|b: &mut Belt| b.segments = 3),
        parameter("segments")
    );
    assert_eq!(
        belt_error(&|b: &mut Belt| b.width = 0.0),
        parameter("width")
    );
    assert_eq!(belt_error(&|b: &mut Belt| b.span = -1.0), parameter("span"));
    assert_eq!(belt_error(&|b: &mut Belt| b.radius = nan), Some(NonFinite));
}

#[test]
fn canonical_bytes_write_zero_signs_alike_and_refuse_non_finite_values() {
    let mesh = Mesh {
        positions: vec![[-0.0, 1.0, 2.0]],
        normals: vec![[0.0, 0.0, 1.0]],
        projection: vec![[-0.0, 1.0]],
        triangles: vec![[0, 0, 0]],
        textured_front: vec![true],
    };
    let bytes = mesh.canonical_bytes().unwrap();
    assert_eq!(bytes.len(), 8 * 8 + 3 * 4 + 4);
    assert_eq!(bytes[..8], 0.0_f64.to_le_bytes());
    assert_eq!(bytes[8 * 6..8 * 7], 0.0_f64.to_le_bytes());
    assert_eq!(bytes[bytes.len() - 4..], 1_u32.to_le_bytes());
    let broken = Mesh {
        normals: vec![[f64::INFINITY, 0.0, 0.0]],
        ..mesh
    };
    assert_eq!(broken.canonical_bytes(), Err(MeshError::NonFinite));
}

#[test]
fn quarter_turns_are_exact() {
    let circle = unit_circle(16);
    assert_eq!(circle[0], [0.0, 1.0]);
    assert_eq!(circle[4], [1.0, 0.0]);
    assert_eq!(circle[8], [0.0, -1.0]);
    assert_eq!(circle[12], [-1.0, 0.0]);
    for k in 0..16 {
        let mirrored = circle[(16 - k) % 16];
        assert_eq!(mirrored, [(-circle[k][0]) + 0.0, circle[k][1]], "angle {k}");
    }
}

/// SHA-256 of each solid's canonical bytes, triangle tags included, for the
/// fixed inputs above. A change to any solid's vertices, normals,
/// projection, triangles or tags changes its digest.
const PINS: [(&str, &str); 21] = [
    (
        "box",
        "0d40081408f238e4a02ed6cfa7cbc67408ac80400b95857dc31b5765fb08c80c",
    ),
    (
        "box bevelled",
        "f6d1f664a69045000cda234fe927168b0eef5bde09abc880959da4e1e1842be7",
    ),
    (
        "tee",
        "19d0adbf6af89052a6fc4d987c178267cb007f541dcf16217f6d56ea68aaea37",
    ),
    (
        "tee rounded",
        "dcb29801c744aff07d325396a630741f967853281f17b3b52800e366365398ac",
    ),
    (
        "lathe frustum",
        "ae6dccbee4b987cda7b7260ba2f8fb5393795ece3bc0ad60bebc6c3931da35d6",
    ),
    (
        "lathe cone",
        "34892cbfcf867799e5e0388d87959a4ca114cc62acca811a38b4c161d88d94f0",
    ),
    (
        "lathe bulb",
        "2935cc31bfe5369203493af7923ed100b3eb934fc6e6d6c159bc93970998609a",
    ),
    (
        "cylinder x",
        "623779adcdc3f33a429fc2a5133094c0df5de47f1b9f89712889add497ee61af",
    ),
    (
        "cylinder y",
        "bbdce0ffc622da1454a36b714bb9dbc44339985910e6bf5a93892e1f2feaa451",
    ),
    (
        "cylinder z",
        "08ff3dfceb56cd9c0b4f278fe762169344feb3d1f03502b81b72a835e34c3b65",
    ),
    (
        "capsule",
        "cad0889b0a9b23f236eb1e1dd0dbd77a5606c0d35b68b93ba0ce46bd948eba88",
    ),
    (
        "capsule short",
        "d0b21040fb3abd9ac091628549974911a527d55c5ea71c02f90dfcc708c51e82",
    ),
    (
        "sphere",
        "e8ba4f3959a31d8fa4a3fa64811b486e0c83c50637963d7fee803a58ba76e211",
    ),
    (
        "torus",
        "d4d06a601f0efe0b22f52b8e911387bfc6ed22bb3392031547717af71ecc5fb0",
    ),
    (
        "belt",
        "cc08291559797d594cbe1e54945fc71bdc7e4f143e7e2b94ff253de12a1c9402",
    ),
    (
        "belt ring",
        "d7cc669243683d8b51a0a834a0a8a7979e4b0c1f24f8fbcd550306cc6a4cf212",
    ),
    (
        "belt flat",
        "c459df4ee063e9f5583caac334ab92d14482c213ef61df6f273b9af85e315500",
    ),
    (
        "dome",
        "965e776149bf5fce0aa3ac9f8fd174b67cc9b9094400fc2b62bc1b7933169f97",
    ),
    (
        "tube",
        "959b320e2ad69a2d808fad24a41475250a30df2e9a0e9b525b9ebbd3207e230d",
    ),
    (
        "lathe bell",
        "d45355eddc5ae1bf578de2e7ae964e81862d3780f5d7ef9683b24ac977b74360",
    ),
    (
        "lathe band",
        "a12712d0f53c2af1e2f4e0caa9582a2753ac0d4ba8f1d1bd8b7e35b83aa66524",
    ),
];

#[test]
fn every_solid_has_a_pinned_digest() {
    let solids = all_solids();
    let actual: Vec<(&str, String)> = solids
        .iter()
        .map(|(name, mesh)| {
            let digest = Sha256::digest(mesh.canonical_bytes().unwrap());
            let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
            (*name, hex)
        })
        .collect();
    let listing: String = actual
        .iter()
        .map(|(name, hex)| format!("    (\"{name}\", \"{hex}\"),\n"))
        .collect();
    let pinned: Vec<(&str, String)> = PINS.iter().map(|(n, h)| (*n, h.to_string())).collect();
    assert_eq!(actual, pinned, "current digests:\n{listing}");
}
