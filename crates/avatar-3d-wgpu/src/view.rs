//! Cameras: the view and projection matrices a draw uses, and points
//! projected to pixels for labels.
//!
//! Matrices are column-major, right-handed, looking down −z in view space,
//! with wgpu's clip depth from 0 at the near plane to 1 at the far plane.

use avatar_3d::motion::math::{cos, sin};
use avatar_3d::robot::Camera;

/// A perspective camera in model units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Where the camera is.
    pub eye: [f64; 3],
    /// The point it looks at, which lands in the frame's centre.
    pub target: [f64; 3],
    /// The up direction.
    pub up: [f64; 3],
    /// The vertical field of view in degrees.
    pub fov_y_degrees: f64,
    /// Width over height of the frame.
    pub aspect: f64,
}

impl From<&Camera> for View {
    /// The exported camera, unchanged.
    fn from(c: &Camera) -> Self {
        Self {
            eye: c.eye,
            target: c.target,
            up: c.up,
            fov_y_degrees: c.fov_y_degrees,
            aspect: c.aspect,
        }
    }
}

impl View {
    /// The distance from the eye to the target.
    pub fn distance(&self) -> f64 {
        length(sub(self.target, self.eye))
    }

    /// The near and far clip distances: a fiftieth of the distance to the
    /// target, and four times it plus ten model units, which hold every
    /// scene the stage and plaza frame.
    pub fn clip_range(&self) -> (f64, f64) {
        let d = self.distance();
        (d / 50.0, 4.0 * d + 10.0)
    }

    /// The camera's right, up and forward directions, each of unit length.
    pub fn basis(&self) -> [[f64; 3]; 3] {
        let f = unit(sub(self.target, self.eye));
        let r = unit(cross(f, self.up));
        let u = cross(r, f);
        [r, u, f]
    }

    /// The view matrix: model space to view space, where the camera sits at
    /// the origin looking down −z with +y up.
    pub fn view_matrix(&self) -> [f64; 16] {
        let [r, u, f] = self.basis();
        let e = self.eye;
        [
            r[0],
            u[0],
            -f[0],
            0.0,
            r[1],
            u[1],
            -f[1],
            0.0,
            r[2],
            u[2],
            -f[2],
            0.0,
            -dot(r, e),
            -dot(u, e),
            dot(f, e),
            1.0,
        ]
    }

    /// The projection matrix: view space to clip space, with depth 0 at
    /// `near` and 1 at `far`.
    pub fn projection_matrix(&self, near: f64, far: f64) -> [f64; 16] {
        let k = 1.0 / tan_degrees(0.5 * self.fov_y_degrees);
        let mut m = [0.0; 16];
        m[0] = k / self.aspect;
        m[5] = k;
        m[10] = far / (near - far);
        m[11] = -1.0;
        m[14] = near * far / (near - far);
        m
    }

    /// A point's normalised device x and y (−1 to 1 across the frame, +y
    /// up) and its distance in front of the camera, or `None` when it is
    /// not in front.
    pub fn project(&self, p: [f64; 3]) -> Option<[f64; 3]> {
        let [r, u, f] = self.basis();
        let v = sub(p, self.eye);
        let depth = dot(v, f);
        if depth <= 0.0 {
            return None;
        }
        let k = tan_degrees(0.5 * self.fov_y_degrees);
        Some([
            dot(v, r) / (depth * k * self.aspect),
            dot(v, u) / (depth * k),
            depth,
        ])
    }

    /// Where a point lands in a `width` × `height` frame, in pixels from
    /// the top-left corner, or `None` when it is behind the camera.
    pub fn to_screen(&self, p: [f64; 3], width: u32, height: u32) -> Option<[f64; 2]> {
        let [x, y, _] = self.project(p)?;
        Some([
            (x + 1.0) * 0.5 * f64::from(width),
            (1.0 - y) * 0.5 * f64::from(height),
        ])
    }

    /// The same camera at the same distance from the same target, seen from
    /// `azimuth` degrees round toward +x from the front (+z) and
    /// `elevation` degrees above the floor's plane.
    pub fn orbit(&self, azimuth: f64, elevation: f64) -> Self {
        let d = self.distance();
        let dir = direction(azimuth, elevation);
        Self {
            eye: [0, 1, 2].map(|k| self.target[k] + dir[k] * d),
            ..*self
        }
    }

    /// The same camera with another frame shape.
    pub fn with_aspect(self, aspect: f64) -> Self {
        Self { aspect, ..self }
    }
}

/// A camera at `azimuth` and `elevation` degrees (as [`View::orbit`]) that
/// looks at the middle of a box and stands back until every corner of the
/// box lies within `fill` of the frame's half width and half height.
pub fn framing(
    lo: [f64; 3],
    hi: [f64; 3],
    azimuth: f64,
    elevation: f64,
    fov_y_degrees: f64,
    aspect: f64,
    fill: f64,
) -> View {
    let target = [0, 1, 2].map(|k| 0.5 * (lo[k] + hi[k]));
    let dir = direction(azimuth, elevation);
    let radius = 0.5 * length(sub(hi, lo));
    let mut distance = radius / tan_degrees(0.5 * fov_y_degrees) + radius;
    let mut view = View {
        eye: [0.0; 3],
        target,
        up: [0.0, 1.0, 0.0],
        fov_y_degrees,
        aspect,
    };
    // Walk in from a distance where the box surely fits, then back off by
    // the overshoot until it does.
    for _ in 0..64 {
        view.eye = [0, 1, 2].map(|k| target[k] + dir[k] * distance);
        let mut over: f64 = 0.0;
        for i in 0..8 {
            let corner = [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] });
            match view.project(corner) {
                Some(q) => over = over.max(q[0].abs()).max(q[1].abs()),
                None => over = f64::INFINITY,
            }
        }
        if over > fill {
            distance *= 1.02;
        } else if over < 0.97 * fill {
            distance *= 0.99;
        } else {
            break;
        }
    }
    view
}

/// The unit direction from a target to a camera at `azimuth` degrees round
/// toward +x from +z and `elevation` degrees up.
fn direction(azimuth: f64, elevation: f64) -> [f64; 3] {
    let (az, el) = (radians(azimuth), radians(elevation));
    [sin(az) * cos(el), sin(el), cos(az) * cos(el)]
}

fn radians(degrees: f64) -> f64 {
    degrees * std::f64::consts::PI / 180.0
}

fn tan_degrees(degrees: f64) -> f64 {
    let a = radians(degrees);
    sin(a) / cos(a)
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

fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = length(a);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// Multiplies two column-major 4 × 4 matrices, `a · b`.
pub fn mul(a: &[f64; 16], b: &[f64; 16]) -> [f64; 16] {
    let mut out = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            out[c * 4 + r] = (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
        }
    }
    out
}

/// A column-major 4 × 4 matrix applied to a point.
pub fn apply(m: &[f64; 16], p: [f64; 3]) -> [f64; 4] {
    [0, 1, 2, 3].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}
