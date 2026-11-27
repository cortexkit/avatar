//! Extrusions of an outline along z, with rounded bevels round both caps.

use super::outline::{bevel_rings, ring_offsets, triangulate, Insets};
use super::{count, length, unit2, Builder, Front, Mesh, MeshError, Outline};

/// How [`extrude`] shapes an outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extrusion {
    /// Thickness along z. The solid spans z = -depth/2 to depth/2.
    pub depth: f64,
    /// Radius of the rounded edge round each cap, in input units: each cap
    /// is the outline inset by this much, and the rounded edge reaches this
    /// far along z before the straight side begins. Zero leaves square
    /// edges. Twice the bevel must stay below the depth, and the bevel must
    /// fit the outline ([`Outline::max_bevel`]).
    pub bevel: f64,
    /// Straight segments in each quarter-round bevel, at least 1 when the
    /// bevel is not zero and at most [`MAX_STEPS`](super::MAX_STEPS). Ignored
    /// when the bevel is zero.
    pub bevel_steps: u32,
}

/// One ring of outline vertices at a stage of the profile from front to back.
struct Ring {
    /// How far the ring is inset from the outline.
    offset: f64,
    z: f64,
    /// Weight of the outline's in-plane normal in the vertex normal.
    side: f64,
    /// Weight of +z in the vertex normal.
    front: f64,
}

/// Extrudes an outline along z with a rounded bevel round both caps.
///
/// The solid is closed. Its front cap faces +z and is the only textured
/// front surface. Rings run from the front cap through the front bevel, down
/// the straight side, through the back bevel to the back cap. Bevel normals
/// follow the quarter-round, so the bevel meets each cap and the side
/// smoothly; with no bevel, caps and side meet at a crease. Along the
/// outline, corners sharper than [`CREASE_COS`](super::CREASE_COS) allows get
/// one vertex per side. Each cap is the outline inset by the bevel and is
/// triangulated by ear clipping.
pub fn extrude(outline: &Outline, spec: &Extrusion) -> Result<Mesh, MeshError> {
    let depth = length(spec.depth, "depth", 0.0, false)?;
    let bevel = length(spec.bevel, "bevel", 0.0, true)?;
    if 2.0 * bevel >= depth {
        return Err(MeshError::OutOfRange { parameter: "bevel" });
    }
    let half = 0.5 * depth;
    let mut rings = Vec::new();
    if bevel == 0.0 {
        // Front cap, top of the side, bottom of the side, back cap.
        for (z, side, front) in [
            (half, 0.0, 1.0),
            (half, 1.0, 0.0),
            (-half, 1.0, 0.0),
            (-half, 0.0, -1.0),
        ] {
            rings.push(Ring {
                offset: 0.0,
                z,
                side,
                front,
            });
        }
    } else {
        let steps = count(spec.bevel_steps, "bevel steps", 1, 1)?;
        let angles = bevel_rings(steps);
        if !outline.fits(&ring_offsets(&angles, bevel)) {
            return Err(MeshError::BevelTooWide);
        }
        let quarter = |&[s, c]: &[f64; 2], sign: f64| Ring {
            offset: bevel * (1.0 - s),
            z: sign * (half - bevel * (1.0 - c)),
            side: s,
            front: sign * c,
        };
        rings.extend(angles.iter().map(|a| quarter(a, 1.0)));
        rings.extend(angles.iter().rev().map(|a| quarter(a, -1.0)));
    }

    let points = outline.points();
    let n = points.len();
    let insets = Insets::new(points);
    let mut mesh = Builder::new();
    // Per ring, the vertex each outline point uses for the edge before it
    // and for the edge after it; the same vertex where the corner is smooth.
    let mut before = Vec::with_capacity(rings.len());
    let mut after = Vec::with_capacity(rings.len());
    for ring in &rings {
        let placed = insets.at(ring.offset);
        let (mut b, mut a) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for i in 0..n {
            let position = [placed[i][0], placed[i][1], ring.z];
            let normal = |m: [f64; 2]| [ring.side * m[0], ring.side * m[1], ring.front];
            let (n_before, n_after) = (insets.normals[(i + n - 1) % n], insets.normals[i]);
            if ring.side == 0.0 {
                let v = mesh.vertex(position, [0.0, 0.0, ring.front]);
                b.push(v);
                a.push(v);
            } else if insets.smooth(i) {
                let shared = unit2([n_before[0] + n_after[0], n_before[1] + n_after[1]]);
                let v = mesh.vertex(position, normal(shared));
                b.push(v);
                a.push(v);
            } else {
                b.push(mesh.vertex(position, normal(n_before)));
                a.push(mesh.vertex(position, normal(n_after)));
            }
        }
        before.push(b);
        after.push(a);
    }

    for r in 0..rings.len() - 1 {
        let (front, back) = (&rings[r], &rings[r + 1]);
        if front.offset == back.offset && front.z == back.z {
            // A crease between a cap and an unbevelled side: no band.
            continue;
        }
        for i in 0..n {
            let j = (i + 1) % n;
            let (fi, fj) = (after[r][i], before[r][j]);
            let (bi, bj) = (after[r + 1][i], before[r + 1][j]);
            mesh.triangle([fi, bi, bj], false);
            mesh.triangle([fi, bj, fj], false);
        }
    }

    let cap = triangulate(&insets.at(rings[0].offset))?;
    let last = rings.len() - 1;
    for [a, b, c] in &cap {
        mesh.triangle([after[0][*a], after[0][*b], after[0][*c]], true);
    }
    for [a, b, c] in &cap {
        mesh.triangle([after[last][*a], after[last][*c], after[last][*b]], false);
    }
    mesh.finish(Front::Given, |p| p)
}
