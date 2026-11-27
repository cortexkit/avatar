//! Solids swept along a path: tubes and belts.

use super::{
    add3, count, cross3, dot3, length, scale3, sub3, unit3, unit_circle, Axis, Builder, Front,
    Mesh, MeshError,
};

/// A tube of `radius` along a path of 3D points.
///
/// The domain: at least two finite points, no two consecutive points equal,
/// a turn of at most 90 degrees at each inner point, and each segment longer
/// than the room its end rings take on the inside of their bends (the radius
/// times the tangent of half the turn, at each end), so consecutive rings
/// never cross. `radius` is above zero; `sides` (steps round the tube) is a
/// multiple of 4 from 4 to [`MAX_STEPS`](super::MAX_STEPS).
///
/// Each path point carries one circular ring, square to the path's mean
/// direction there and oriented by a rotation-minimising frame, so the tube
/// does not twist. Vertices run ring by ring along the path, `sides` per
/// ring, which lets a caller weight each ring to its own joints. It is open:
/// the first and last rings are its two boundary loops. It is smooth
/// everywhere. Every triangle whose outward normal has z > 0 is textured
/// front.
pub fn tube(path: &[[f64; 3]], radius: f64, sides: u32) -> Result<Mesh, MeshError> {
    let sides = count(sides, "sides", 4, 4)?;
    let radius = length(radius, "radius", 0.0, false)?;
    if path.iter().flatten().any(|v| !v.is_finite()) {
        return Err(MeshError::NonFinite);
    }
    if path.len() < 2 {
        return Err(MeshError::TooFewPoints { needed: 2 });
    }
    for i in 1..path.len() {
        if path[i] == path[i - 1] {
            return Err(MeshError::RepeatedPoint { index: i });
        }
    }
    let n = path.len();
    let segments: Vec<[f64; 3]> = path.windows(2).map(|w| sub3(w[1], w[0])).collect();
    let directions: Vec<[f64; 3]> = segments.iter().map(|s| unit3(*s)).collect();
    // The tangent of half the turn at each point; zero at the ends.
    let mut bend = vec![0.0; n];
    for i in 1..n - 1 {
        let (a, b) = (directions[i - 1], directions[i]);
        let cos = dot3(a, b);
        if cos < 0.0 {
            return Err(MeshError::PathTooSharp { index: i });
        }
        let sin = dot3(cross3(a, b), cross3(a, b)).sqrt();
        bend[i] = sin / (1.0 + cos);
    }
    for (i, segment) in segments.iter().enumerate() {
        if dot3(*segment, *segment).sqrt() <= radius * (bend[i] + bend[i + 1]) {
            return Err(MeshError::PathTooSharp { index: i });
        }
    }
    let tangents: Vec<[f64; 3]> = (0..n)
        .map(|i| match i {
            0 => directions[0],
            i if i == n - 1 => directions[n - 2],
            i => unit3(add3(directions[i - 1], directions[i])),
        })
        .collect();

    // The first frame starts from the world axis least aligned with the path,
    // the first such in x, y, z order on a tie.
    let t0 = tangents[0];
    let mut seed = [1.0, 0.0, 0.0];
    if t0[1].abs() < t0[0].abs() && t0[1].abs() <= t0[2].abs() {
        seed = [0.0, 1.0, 0.0];
    } else if t0[2].abs() < t0[0].abs() && t0[2].abs() < t0[1].abs() {
        seed = [0.0, 0.0, 1.0];
    }
    let mut normals = vec![square(seed, t0)];
    // Each later frame follows by the double reflection method, which
    // rotates the frame as little as possible between points.
    for i in 0..n - 1 {
        let v1 = segments[i];
        let c1 = dot3(v1, v1);
        let reflect = |a: [f64; 3], v: [f64; 3], c: f64| sub3(a, scale3(v, 2.0 / c * dot3(v, a)));
        let r = reflect(normals[i], v1, c1);
        let t = reflect(tangents[i], v1, c1);
        let v2 = sub3(tangents[i + 1], t);
        let c2 = dot3(v2, v2);
        let r = if c2 == 0.0 { r } else { reflect(r, v2, c2) };
        normals.push(square(r, tangents[i + 1]));
    }

    let circle = unit_circle(sides);
    let mut mesh = Builder::new();
    for i in 0..n {
        let binormal = cross3(tangents[i], normals[i]);
        for &[s, c] in &circle {
            let out = add3(scale3(normals[i], c), scale3(binormal, s));
            mesh.vertex(add3(path[i], scale3(out, radius)), out);
        }
    }
    let index = |ring: usize, k: usize| (ring * sides + k) as u32;
    for i in 0..n - 1 {
        for k in 0..sides {
            let k1 = (k + 1) % sides;
            mesh.triangle([index(i, k), index(i, k1), index(i + 1, k1)], false);
            mesh.triangle([index(i, k), index(i + 1, k1), index(i + 1, k)], false);
        }
    }
    mesh.finish(Front::FacingZ, |p| p)
}

/// The unit part of `a` square to the unit vector `t`.
fn square(a: [f64; 3], t: [f64; 3]) -> [f64; 3] {
    unit3(sub3(a, scale3(t, dot3(a, t))))
}

/// A belt: a band of rectangular section round a stadium, such as a track
/// round its wheels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Belt {
    /// Radius of each rounded end, measured to the middle of the band; above
    /// zero.
    pub radius: f64,
    /// Distance between the centres of the two rounded ends; zero or more.
    /// Zero makes a ring.
    pub span: f64,
    /// Thickness of the band across the loop; above zero and below twice the
    /// radius.
    pub thickness: f64,
    /// Extent of the band along `axis`; above zero.
    pub width: f64,
    /// The direction the band's width runs along. The loop lies in the plane
    /// square to it, with the span along x for [`Axis::Z`] and [`Axis::Y`]
    /// and along z for [`Axis::X`].
    pub axis: Axis,
    /// Steps in each rounded end's half turn; even, from 2 to
    /// [`MAX_STEPS`](super::MAX_STEPS).
    pub segments: u32,
}

/// A belt centred on the origin; see [`Belt`].
///
/// It is closed: an outer band, an inner band facing the hole, and two flat
/// sides, meeting at creases. The straight runs join the rounded ends
/// smoothly. Every triangle whose outward normal has z > 0 is textured front.
pub fn belt(spec: &Belt) -> Result<Mesh, MeshError> {
    let segments = count(spec.segments, "segments", 2, 2)?;
    let radius = length(spec.radius, "radius", 0.0, false)?;
    let span = length(spec.span, "span", 0.0, true)?;
    let thickness = length(spec.thickness, "thickness", 0.0, false)?;
    let width = length(spec.width, "width", 0.0, false)?;
    if thickness >= 2.0 * radius {
        return Err(MeshError::OutOfRange {
            parameter: "thickness",
        });
    }
    // Built with the width along z and the span along x, then turned.
    let turn = 2 * segments;
    let angles = unit_circle(turn);
    let quarter = segments / 2;
    let per_end = if span == 0.0 { segments } else { segments + 1 };
    let mut loop_points: Vec<(f64, [f64; 2])> = Vec::with_capacity(2 * per_end);
    for (centre, start) in [(0.5 * span, turn - quarter), (-0.5 * span, quarter)] {
        for k in 0..per_end {
            let [s, c] = angles[(start + k) % turn];
            loop_points.push((centre, [c, s]));
        }
    }
    let (outer, inner) = (radius + 0.5 * thickness, radius - 0.5 * thickness);
    let place =
        |(centre, [c, s]): (f64, [f64; 2]), reach: f64, z: f64| [centre + reach * c, reach * s, z];
    let half = 0.5 * width;
    let mut mesh = Builder::new();
    let mut surface = |reach: f64, z: f64, normal: &dyn Fn([f64; 2]) -> [f64; 3]| -> Vec<u32> {
        loop_points
            .iter()
            .map(|&(centre, u)| mesh.vertex(place((centre, u), reach, z), normal(u)))
            .collect()
    };
    let outward = |u: [f64; 2]| [u[0], u[1], 0.0];
    let inward = |u: [f64; 2]| [-u[0], -u[1], 0.0];
    let outer_front = surface(outer, half, &outward);
    let outer_back = surface(outer, -half, &outward);
    let inner_front = surface(inner, half, &inward);
    let inner_back = surface(inner, -half, &inward);
    let rim_front = surface(outer, half, &|_| [0.0, 0.0, 1.0]);
    let hole_front = surface(inner, half, &|_| [0.0, 0.0, 1.0]);
    let rim_back = surface(outer, -half, &|_| [0.0, 0.0, -1.0]);
    let hole_back = surface(inner, -half, &|_| [0.0, 0.0, -1.0]);
    let m = loop_points.len();
    for i in 0..m {
        let j = (i + 1) % m;
        let (a, b) = (&outer_front, &outer_back);
        mesh.triangle([a[i], b[i], b[j]], false);
        mesh.triangle([a[i], b[j], a[j]], false);
        let (a, b) = (&inner_front, &inner_back);
        mesh.triangle([a[i], b[j], b[i]], false);
        mesh.triangle([a[i], a[j], b[j]], false);
        let (o, h) = (&rim_front, &hole_front);
        mesh.triangle([h[i], o[i], o[j]], false);
        mesh.triangle([h[i], o[j], h[j]], false);
        let (o, h) = (&rim_back, &hole_back);
        mesh.triangle([h[i], o[j], o[i]], false);
        mesh.triangle([h[i], h[j], o[j]], false);
    }
    let axis = spec.axis;
    mesh.finish(Front::FacingZ, |p| axis.turn_from_z(p))
}
