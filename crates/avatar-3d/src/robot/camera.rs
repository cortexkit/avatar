//! The stage camera: the 3D lead's three-quarter view, framing one robot.

use std::f64::consts::PI;

use avatar_svg::wild::math;

use super::Part;

/// Degrees the stage view is turned toward +x, the viewer's right.
pub const AZIMUTH: f64 = 11.0;
/// Degrees the stage view looks down from.
pub const ELEVATION: f64 = 12.0;
/// The vertical field of view in degrees: a long lens, so the front reads
/// as the 2D drawing.
pub const FOV_Y: f64 = 20.0;
/// The frame's height in pixels; the frame is square.
pub const FRAME_PX: u32 = 288;
/// How far from the frame's centre, as a share of the half frame, the rest
/// bounds may reach.
pub const FILL: f64 = 0.9;

/// A perspective camera as plain data, in model units.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Camera {
    /// Where the camera is.
    pub eye: [f64; 3],
    /// The point it looks at.
    pub target: [f64; 3],
    /// The up direction, +y.
    pub up: [f64; 3],
    /// The vertical field of view in degrees.
    pub fov_y_degrees: f64,
    /// Width over height of the frame.
    pub aspect: f64,
    /// The frame's height in pixels.
    pub height_px: u32,
}

impl Camera {
    /// A point's normalised device x and y (−1 to 1 across the frame) and
    /// its distance in front of the camera along the view direction.
    pub fn project(&self, p: [f64; 3]) -> [f64; 3] {
        let f = unit(sub(self.target, self.eye));
        let r = unit(cross(f, self.up));
        let u = cross(r, f);
        let v = sub(p, self.eye);
        let depth = dot(v, f);
        let k = tan(0.5 * self.fov_y_degrees);
        [
            dot(v, r) / (depth * k * self.aspect),
            dot(v, u) / (depth * k),
            depth,
        ]
    }
}

/// The stage camera for a robot: it looks at the centre of the rest bounds
/// from [`AZIMUTH`] and [`ELEVATION`], backed off until every corner of the
/// bounds lies within [`FILL`] of the square frame.
pub(super) fn stage(parts: &[Part]) -> Camera {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in parts.iter().flat_map(|p| p.mesh.positions.iter()) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let target = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.0];
    let (az, el) = (AZIMUTH * PI / 180.0, ELEVATION * PI / 180.0);
    let direction = [
        math::sin(az) * math::cos(el),
        math::sin(el),
        math::cos(az) * math::cos(el),
    ];
    let extent = (hi[0] - lo[0]).max(hi[1] - lo[1]);
    let mut distance = 0.5 * extent / tan(0.5 * FOV_Y);
    let mut camera = Camera {
        eye: [0.0; 3],
        target,
        up: [0.0, 1.0, 0.0],
        fov_y_degrees: FOV_Y,
        aspect: 1.0,
        height_px: FRAME_PX,
    };
    for _ in 0..16 {
        camera.eye = [0, 1, 2].map(|k| target[k] + direction[k] * distance);
        let mut over: f64 = 0.0;
        for i in 0..8 {
            let corner = [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] });
            let q = camera.project(corner);
            over = over.max(q[0].abs()).max(q[1].abs());
        }
        if over <= FILL {
            break;
        }
        distance *= over / FILL + 0.01;
    }
    camera
}

/// The tangent of an angle in degrees, within the portable tangent's
/// domain for the half fields of view used here.
fn tan(degrees: f64) -> f64 {
    let a = degrees * PI / 180.0;
    math::sin(a) / math::cos(a)
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
