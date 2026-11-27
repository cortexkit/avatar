//! Placing the mesh kit's solids in robot space: robot units with +y up, the
//! floor at y = 0 and the front toward +z. Model space is robot space scaled
//! by [`UNIT`](super::UNIT), applied once when a part is finished.

use crate::mesh::{self, Axis, Mesh, MeshError};

pub(super) type V3 = [f64; 3];

/// Steps round every round solid.
pub(super) const ROUND: u32 = 16;
/// Steps round thin rods and tubes.
pub(super) const ROD: u32 = 12;

/// A linear map followed by a translation. `m` holds rows.
#[derive(Clone, Copy, Debug)]
pub(super) struct Affine {
    pub m: [[f64; 3]; 3],
    pub t: V3,
}

impl Affine {
    pub fn translate(t: V3) -> Self {
        Self {
            m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            t,
        }
    }

    pub fn scale(s: V3) -> Self {
        Self {
            m: [[s[0], 0.0, 0.0], [0.0, s[1], 0.0], [0.0, 0.0, s[2]]],
            t: [0.0; 3],
        }
    }

    /// A rotation taking +y to the unit direction `dir` and keeping the
    /// rest of the frame as square to it as it can, then a move to `origin`.
    pub fn along(dir: V3, origin: V3) -> Self {
        let y = unit(dir);
        // The world axis least aligned with the direction seeds the frame.
        let seed = if y[2].abs() <= y[0].abs() && y[2].abs() <= y[1].abs() {
            [0.0, 0.0, 1.0]
        } else if y[0].abs() <= y[1].abs() {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let x = unit(cross(y, seed));
        let z = cross(x, y);
        // Columns are the images of the x, y and z axes.
        Self {
            m: [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]],
            t: origin,
        }
    }

    /// This map followed by `next`.
    pub fn then(self, next: Self) -> Self {
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| next.m[i][k] * self.m[k][j]).sum();
            }
        }
        Self {
            m,
            t: add(next.apply_linear(self.t), next.t),
        }
    }

    pub fn apply(&self, p: V3) -> V3 {
        add(self.apply_linear(p), self.t)
    }

    fn apply_linear(&self, p: V3) -> V3 {
        [0, 1, 2].map(|i| self.m[i][0] * p[0] + self.m[i][1] * p[1] + self.m[i][2] * p[2])
    }

    fn det(&self) -> f64 {
        let m = &self.m;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    /// The cofactor matrix's rows: normals map by it, up to a positive
    /// factor when the determinant is positive.
    fn cofactor(&self) -> [[f64; 3]; 3] {
        let m = &self.m;
        let c = |r0: usize, r1: usize, c0: usize, c1: usize| {
            m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
        };
        [
            [c(1, 2, 1, 2), -c(1, 2, 0, 2), c(1, 2, 0, 1)],
            [-c(0, 2, 1, 2), c(0, 2, 0, 2), -c(0, 2, 0, 1)],
            [c(0, 1, 1, 2), -c(0, 1, 0, 2), c(0, 1, 0, 1)],
        ]
    }
}

/// The map from a 2D drawing frame, read as (x, y down, z) in robot units,
/// to robot space; `oy` is the frame's ground offset at rest.
pub(super) fn from_drawing(oy: f64) -> Affine {
    Affine {
        m: [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]],
        t: [0.0, -oy, 0.0],
    }
}

/// Moves a mesh by an affine map. Normals follow the inverse transpose and
/// stay unit length; a mirroring map reverses the winding so triangles stay
/// counter-clockwise seen from outside. Projection coordinates are kept.
pub(super) fn transform(mut mesh: Mesh, a: &Affine) -> Mesh {
    let det = a.det();
    let cof = a.cofactor();
    let sign = if det < 0.0 { -1.0 } else { 1.0 };
    for p in &mut mesh.positions {
        *p = a.apply(*p).map(|v| v + 0.0);
    }
    for n in &mut mesh.normals {
        let m = [0, 1, 2].map(|i| sign * (cof[i][0] * n[0] + cof[i][1] * n[1] + cof[i][2] * n[2]));
        *n = unit(m).map(|v| v + 0.0);
    }
    if det < 0.0 {
        for t in &mut mesh.triangles {
            t.swap(1, 2);
        }
    }
    mesh
}

/// Joins meshes into one, keeping each triangle's tag.
pub(super) fn merge(meshes: Vec<Mesh>) -> Mesh {
    let mut out = Mesh {
        positions: Vec::new(),
        normals: Vec::new(),
        projection: Vec::new(),
        triangles: Vec::new(),
        textured_front: Vec::new(),
    };
    for m in meshes {
        let base = out.positions.len() as u32;
        out.positions.extend(m.positions);
        out.normals.extend(m.normals);
        out.projection.extend(m.projection);
        out.triangles
            .extend(m.triangles.iter().map(|t| t.map(|i| i + base)));
        out.textured_front.extend(m.textured_front);
    }
    out
}

/// Unwraps a solid that the inputs guarantee; the fallback stands in where a
/// degenerate input slipped through, so the builder never fails. Each use
/// is recorded ([`stand_in`]).
pub(super) fn or_ball(result: Result<Mesh, MeshError>, centre: V3, radius: f64) -> Mesh {
    result.unwrap_or_else(|_| {
        stand_in("a ball for a solid the kit refused");
        ball(centre, radius.max(0.5))
    })
}

thread_local! {
    static STAND_INS: std::cell::RefCell<Vec<&'static str>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Records that a simpler solid stood in for one the kit refused.
pub(super) fn stand_in(what: &'static str) {
    STAND_INS.with(|s| s.borrow_mut().push(what));
}

/// The stand-ins recorded on this thread since the last call.
pub(super) fn take_stand_ins() -> Vec<&'static str> {
    STAND_INS.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

/// A sphere.
pub(super) fn ball(centre: V3, radius: f64) -> Mesh {
    let mesh = mesh::sphere(radius.max(0.05), ROUND, ROUND / 2)
        .expect("a sphere of positive radius is in the kit's domain");
    transform(mesh, &Affine::translate(centre))
}

/// An ellipsoid: a unit sphere scaled by its three radii.
pub(super) fn ellipsoid(centre: V3, radii: V3) -> Mesh {
    let mesh = mesh::sphere(1.0, ROUND, ROUND / 2).expect("the unit sphere is in the kit's domain");
    transform(
        mesh,
        &Affine::scale(radii.map(|r| r.max(0.05))).then(Affine::translate(centre)),
    )
}

/// A rod with round ends from `a` to `b`; a ball when they meet.
pub(super) fn rod(a: V3, b: V3, radius: f64) -> Mesh {
    let d = sub(b, a);
    let length = dot(d, d).sqrt();
    let radius = radius.max(0.05);
    if length < 1e-6 {
        return ball(a, radius);
    }
    let mesh = or_ball(
        mesh::capsule(radius, length, Axis::Y, ROD, 2),
        [0.0; 3],
        radius,
    );
    transform(mesh, &Affine::along(d, mid(a, b)))
}

/// Rods along a polyline, one per segment.
pub(super) fn rods(points: &[V3], radius: f64) -> Mesh {
    merge(points.windows(2).map(|w| rod(w[0], w[1], radius)).collect())
}

/// A flat-ended cylinder of `radius` from `a` to `b`.
pub(super) fn drum(a: V3, b: V3, radius: f64) -> Mesh {
    let d = sub(b, a);
    let length = dot(d, d).sqrt();
    if length < 1e-6 {
        return ball(a, radius);
    }
    let mesh = or_ball(
        mesh::cylinder(radius.max(0.05), length, Axis::Y, ROUND),
        [0.0; 3],
        radius,
    );
    transform(mesh, &Affine::along(d, mid(a, b)))
}

/// A box of half extents `half` with rounded edges of `bevel`, centred on
/// the origin of `frame`.
pub(super) fn block(frame: Affine, half: V3, bevel: f64) -> Mesh {
    let [hx, hy, hz] = half.map(|v| v.max(0.1));
    let outline = mesh::Outline::new(&[[-hx, -hy], [hx, -hy], [hx, hy], [-hx, hy]])
        .expect("a rectangle is a simple outline");
    let limit = outline.max_bevel(2).min(0.45 * 2.0 * hz);
    let spec = mesh::Extrusion {
        depth: 2.0 * hz,
        bevel: bevel.clamp(0.0, limit * 0.99),
        bevel_steps: 2,
    };
    let flat = mesh::Extrusion { bevel: 0.0, ..spec };
    let mesh = mesh::extrude(&outline, &spec)
        .or_else(|_| mesh::extrude(&outline, &flat))
        .expect("an unbevelled box is in the kit's domain");
    transform(mesh, &frame)
}

/// A tube through `points` with ball ends; falls back to rods where the
/// path bends too sharply for one tube.
pub(super) fn hose(points: &[V3], radius: f64) -> Mesh {
    let radius = radius.max(0.05);
    match mesh::tube(points, radius, ROD) {
        Ok(tube) => merge(vec![
            tube,
            ball(points[0], radius),
            ball(points[points.len() - 1], radius),
        ]),
        Err(_) => rods(points, radius),
    }
}

/// Points of the quadratic Bézier `a`, `c`, `b`, `n` segments, ends included.
pub(super) fn quadratic(a: V3, c: V3, b: V3, n: usize) -> Vec<V3> {
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            [0, 1, 2].map(|k| u * u * a[k] + 2.0 * u * t * c[k] + t * t * b[k])
        })
        .collect()
}

pub(super) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(super) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn scale(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub(super) fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(super) fn unit(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l == 0.0 {
        return [0.0, 1.0, 0.0];
    }
    [a[0] / l, a[1] / l, a[2] / l]
}

pub(super) fn mid(a: V3, b: V3) -> V3 {
    [0, 1, 2].map(|k| 0.5 * (a[k] + b[k]))
}
