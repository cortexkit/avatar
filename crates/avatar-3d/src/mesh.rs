//! A kit of deterministic solids: extrusions of a 2D outline with bevelled
//! edges, lathes of a half outline, and cylinders, capsules, tubes along a
//! path, domes, spheres, tori and belts.
//!
//! Every solid is a pure function of its inputs and returns a [`Mesh`] in the
//! inputs' own units, or a [`MeshError`] when an input lies outside the
//! stated domain. Inputs are never repaired. Coordinates are right-handed and
//! the front of a solid faces +z. Triangles wind counter-clockwise seen from
//! outside, so their right-handed face normals point outward. A transform that
//! mirrors a mesh must reverse each triangle's winding to keep that true.
//!
//! Each vertex carries an outward unit normal and a projection coordinate: its
//! rest-local (x, y), so a texture projected along the depth axis is addressed
//! through a per-texture binding rather than per-mesh texture coordinates.
//! Because projection is continuous, the only split vertices are normal
//! seams: where neighbouring faces turn by more than [`CREASE_COS`] allows,
//! each side gets its own vertex at the same position.
//!
//! Each triangle carries a textured-front tag. On an extrusion it marks the
//! front cap and nothing of the bevel or sides; on every other solid it marks
//! each triangle whose outward face normal has z > 0.
//!
//! Each solid's documentation states whether it is closed or which boundary
//! loops it leaves open. No triangle has an area below [`AREA_EPSILON`]: an
//! input that would need one is rejected.
#![deny(missing_docs)]

mod extrude;
mod outline;
mod revolve;
mod sweep;
#[cfg(test)]
mod tests;

pub use extrude::{extrude, Extrusion};
pub use outline::Outline;
pub use revolve::{capsule, cylinder, dome, lathe, sphere, torus};
pub use sweep::{belt, tube, Belt};

use std::f64::consts::FRAC_PI_2;
use std::fmt;

use avatar_svg::wild::math;

/// The smallest triangle area any solid emits, in square input units.
pub const AREA_EPSILON: f64 = 1e-9;

/// Faces that meet at a turn whose cosine is at least this share smooth
/// normals; a sharper turn (more than about 41.4 degrees) splits the vertex
/// into one per side. Applies along outlines, lathe profiles and at a lathe's
/// poles.
pub const CREASE_COS: f64 = 0.75;

/// The largest segment, ring, side or bevel step count any solid accepts.
pub const MAX_STEPS: u32 = 1_024;

/// A triangle mesh as plain data. All per-vertex arrays have one entry per
/// vertex and the per-triangle arrays one entry per triangle.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// Vertex positions in the solid's input units.
    pub positions: Vec<[f64; 3]>,
    /// Outward unit normals, one per vertex.
    pub normals: Vec<[f64; 3]>,
    /// Each vertex's (x, y) as the solid was built, before any transform moves it, in the solid's input units.
    pub projection: Vec<[f64; 2]>,
    /// Vertex indices, three per triangle, wound counter-clockwise seen from
    /// outside.
    pub triangles: Vec<[u32; 3]>,
    /// Whether each triangle belongs to the textured front surface.
    pub textured_front: Vec<bool>,
}

impl Mesh {
    /// The mesh's canonical bytes for hashing: positions, normals and
    /// projection coordinates as f64 little-endian with -0.0 written as +0.0,
    /// then the triangle indices and one textured-front flag per triangle as
    /// u32 little-endian. A non-finite value is an error.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, MeshError> {
        let mut out = Vec::new();
        for value in self
            .positions
            .iter()
            .chain(&self.normals)
            .flatten()
            .chain(self.projection.iter().flatten())
        {
            if !value.is_finite() {
                return Err(MeshError::NonFinite);
            }
            let value = if *value == 0.0 { 0.0 } else { *value };
            out.extend_from_slice(&value.to_le_bytes());
        }
        for index in self.triangles.iter().flatten() {
            out.extend_from_slice(&index.to_le_bytes());
        }
        for front in &self.textured_front {
            out.extend_from_slice(&u32::from(*front).to_le_bytes());
        }
        Ok(out)
    }
}

/// Why a solid could not be built from its inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MeshError {
    /// An input number is NaN or infinite, or a result overflowed.
    NonFinite,
    /// The input has fewer points than the solid needs.
    TooFewPoints {
        /// The least number of points accepted.
        needed: usize,
    },
    /// The point at this index equals the one before it.
    RepeatedPoint {
        /// Index of the second of the two equal points.
        index: usize,
    },
    /// The outline is not a simple polygon: two edges cross or touch, or an
    /// edge doubles back along the one before it.
    NotSimple,
    /// The outline encloses no area.
    ZeroArea,
    /// A parameter lies outside its stated range.
    OutOfRange {
        /// The parameter's name.
        parameter: &'static str,
    },
    /// The bevel or inset is wider than the outline allows.
    BevelTooWide,
    /// A tube path turns by more than 90 degrees at a point, or a segment is
    /// too short for the rings at its ends not to overlap.
    PathTooSharp {
        /// Index of the path point or segment at fault.
        index: usize,
    },
    /// Meshing the input would make a triangle smaller than [`AREA_EPSILON`].
    Degenerate,
    /// The mesh would need more vertices than u32 indices can address.
    TooLarge,
}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => write!(f, "a value is NaN or infinite"),
            Self::TooFewPoints { needed } => write!(f, "at least {needed} points are needed"),
            Self::RepeatedPoint { index } => {
                write!(f, "point {index} repeats the point before it")
            }
            Self::NotSimple => write!(f, "the outline crosses or touches itself"),
            Self::ZeroArea => write!(f, "the outline encloses no area"),
            Self::OutOfRange { parameter } => write!(f, "{parameter} is out of range"),
            Self::BevelTooWide => write!(f, "the bevel is wider than the outline allows"),
            Self::PathTooSharp { index } => {
                write!(f, "the path turns too sharply at {index}")
            }
            Self::Degenerate => write!(f, "a triangle would be smaller than the area epsilon"),
            Self::TooLarge => write!(f, "too many vertices for u32 indices"),
        }
    }
}

impl std::error::Error for MeshError {}

/// A coordinate axis, used to orient the solids that have one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    /// The x axis: right, as seen from the front.
    X,
    /// The y axis.
    Y,
    /// The z axis: toward the viewer at the front.
    Z,
}

impl Axis {
    /// Turns a point or direction built around the y axis onto this axis by
    /// an exact quarter turn (no rounding), keeping handedness.
    fn turn_from_y(self, p: [f64; 3]) -> [f64; 3] {
        match self {
            Self::X => [p[1], -p[0], p[2]],
            Self::Y => p,
            Self::Z => [p[0], -p[2], p[1]],
        }
    }

    /// Turns a point or direction built around the z axis onto this axis by
    /// an exact quarter turn, keeping handedness.
    fn turn_from_z(self, p: [f64; 3]) -> [f64; 3] {
        match self {
            Self::X => [p[2], p[1], -p[0]],
            Self::Y => [p[0], p[2], -p[1]],
            Self::Z => p,
        }
    }
}

/// Which triangles a finished mesh tags as textured front.
#[derive(Clone, Copy)]
enum Front {
    /// The tags given with each triangle.
    Given,
    /// Every triangle whose outward face normal has z > 0.
    FacingZ,
}

/// Collects vertices and triangles, then checks and finishes a mesh.
struct Builder {
    positions: Vec<[f64; 3]>,
    normals: Vec<[f64; 3]>,
    triangles: Vec<[u32; 3]>,
    front: Vec<bool>,
    too_large: bool,
}

impl Builder {
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            triangles: Vec::new(),
            front: Vec::new(),
            too_large: false,
        }
    }

    /// Adds a vertex; the normal is scaled to unit length.
    fn vertex(&mut self, position: [f64; 3], normal: [f64; 3]) -> u32 {
        let index = match u32::try_from(self.positions.len()) {
            Ok(index) => index,
            Err(_) => {
                self.too_large = true;
                0
            }
        };
        self.positions.push(position);
        self.normals.push(unit3(normal));
        index
    }

    fn triangle(&mut self, corners: [u32; 3], front: bool) {
        self.triangles.push(corners);
        self.front.push(front);
    }

    /// Turns every position and normal with `turn`, sets projection
    /// coordinates and front tags, and checks that every value is finite and
    /// every triangle is at least [`AREA_EPSILON`] in area.
    fn finish(self, front: Front, turn: impl Fn([f64; 3]) -> [f64; 3]) -> Result<Mesh, MeshError> {
        if self.too_large {
            return Err(MeshError::TooLarge);
        }
        let positions: Vec<[f64; 3]> = self.positions.iter().map(|&p| plus_zero(turn(p))).collect();
        let normals: Vec<[f64; 3]> = self.normals.iter().map(|&n| plus_zero(turn(n))).collect();
        if positions
            .iter()
            .chain(&normals)
            .flatten()
            .any(|v| !v.is_finite())
        {
            return Err(MeshError::NonFinite);
        }
        let mut textured_front = Vec::with_capacity(self.triangles.len());
        for (corners, given) in self.triangles.iter().zip(&self.front) {
            let n = face_cross(&positions, *corners);
            let area = 0.5 * dot3(n, n).sqrt();
            if area.is_nan() || area < AREA_EPSILON {
                return Err(MeshError::Degenerate);
            }
            textured_front.push(match front {
                Front::Given => *given,
                Front::FacingZ => n[2] > 0.0,
            });
        }
        Ok(Mesh {
            projection: positions.iter().map(|p| [p[0], p[1]]).collect(),
            positions,
            normals,
            triangles: self.triangles,
            textured_front,
        })
    }
}

/// The cross product of a triangle's two edges from its first corner: its
/// outward face normal scaled by twice its area.
fn face_cross(positions: &[[f64; 3]], corners: [u32; 3]) -> [f64; 3] {
    let [a, b, c] = corners.map(|i| positions[i as usize]);
    cross3(sub3(b, a), sub3(c, a))
}

/// Sine and cosine of `2πk/n` for k in 0..n, with `n` a multiple of 4. Only
/// the first quadrant is evaluated; the rest follow by exact symmetry, so the
/// quarter turns are exactly 0 and ±1 and mirrored angles match bit for bit.
fn unit_circle(n: usize) -> Vec<[f64; 2]> {
    let q = n / 4;
    let quarter: Vec<f64> = (0..=q)
        .map(|j| match j {
            0 => 0.0,
            j if j == q => 1.0,
            j => math::sin(FRAC_PI_2 * j as f64 / q as f64),
        })
        .collect();
    (0..n)
        .map(|k| {
            let (s, c) = (quarter[k % q], quarter[q - k % q]);
            let [sin, cos] = match k / q {
                0 => [s, c],
                1 => [c, -s],
                2 => [-s, -c],
                _ => [-c, s],
            };
            // Adding +0.0 turns a negated zero into +0.0.
            [sin + 0.0, cos + 0.0]
        })
        .collect()
}

/// Checks a count parameter: within `min..=MAX_STEPS` and a multiple of
/// `multiple`.
fn count(value: u32, parameter: &'static str, min: u32, multiple: u32) -> Result<usize, MeshError> {
    if value < min || value > MAX_STEPS || !value.is_multiple_of(multiple) {
        return Err(MeshError::OutOfRange { parameter });
    }
    Ok(value as usize)
}

/// Checks a length parameter: finite and above `min` (or equal to it when
/// `inclusive`).
fn length(
    value: f64,
    parameter: &'static str,
    min: f64,
    inclusive: bool,
) -> Result<f64, MeshError> {
    if !value.is_finite() {
        return Err(MeshError::NonFinite);
    }
    if value < min || (!inclusive && value == min) {
        return Err(MeshError::OutOfRange { parameter });
    }
    Ok(value)
}

fn plus_zero(p: [f64; 3]) -> [f64; 3] {
    p.map(|v| v + 0.0)
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale3(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit3(a: [f64; 3]) -> [f64; 3] {
    let l = dot3(a, a).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}

fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn dot2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn unit2(a: [f64; 2]) -> [f64; 2] {
    let l = dot2(a, a).sqrt();
    [a[0] / l, a[1] / l]
}

/// Twice the signed area of triangle (a, b, c): positive when it turns
/// counter-clockwise.
fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
