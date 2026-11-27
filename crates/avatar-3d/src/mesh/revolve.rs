//! Solids of revolution: lathes of a half outline, and the cylinders,
//! capsules, spheres, domes and tori built the same way.

use super::outline::simple_area;
use super::CREASE_COS;
use super::{count, dot2, length, unit2, unit_circle, Axis, Builder, Front, Mesh, MeshError};

/// A point of a profile in (radius, height), with the outward profile
/// normals of the segments before and after it. They are equal where the
/// profile is smooth there.
struct Section {
    p: [f64; 2],
    before: [f64; 2],
    after: [f64; 2],
}

/// The vertices of one profile point round the turn.
enum Ring {
    /// One vertex per angle step.
    Circle(Vec<u32>),
    /// A point on the axis: one vertex shared by every wedge.
    Pole(u32),
    /// A point on the axis with a sloped normal: one vertex per wedge, its
    /// normal turned to the middle of the wedge.
    Fan(Vec<u32>),
}

impl Ring {
    fn pole(&self, wedge: usize) -> u32 {
        match self {
            Self::Pole(v) => *v,
            Self::Fan(v) | Self::Circle(v) => v[wedge],
        }
    }
}

/// Turns a profile round the y axis. Angle 0 points to +z and a quarter turn
/// to +x, so the front of the solid is centred on +z and its x extremes lie
/// exactly in the profile's plane. A point at radius 0 becomes a pole; one at
/// a larger radius at either end leaves that ring open.
fn revolve(profile: &[Section], segments: usize) -> Builder {
    // Even entries are the ring angles, odd entries the middles of wedges.
    let angles = unit_circle(2 * segments);
    let mut mesh = Builder::new();
    let last = profile.len() - 1;
    let mut rings: Vec<(Ring, Ring)> = Vec::with_capacity(profile.len());
    for (i, section) in profile.iter().enumerate() {
        let [r, y] = section.p;
        if r == 0.0 {
            let n = if i == 0 {
                section.after
            } else {
                section.before
            };
            let ring = if n[0] == 0.0 {
                Ring::Pole(mesh.vertex([0.0, y, 0.0], [0.0, n[1], 0.0]))
            } else {
                Ring::Fan(
                    (0..segments)
                        .map(|j| {
                            let [s, c] = angles[2 * j + 1];
                            mesh.vertex([0.0, y, 0.0], [n[0] * s, n[1], n[0] * c])
                        })
                        .collect(),
                )
            };
            let copy = match &ring {
                Ring::Pole(v) => Ring::Pole(*v),
                Ring::Fan(v) | Ring::Circle(v) => Ring::Fan(v.clone()),
            };
            rings.push((copy, ring));
            continue;
        }
        let positions: Vec<[f64; 3]> = (0..segments)
            .map(|j| {
                let [s, c] = angles[2 * j];
                [r * s, y, r * c]
            })
            .collect();
        let mut circle = |n: [f64; 2]| {
            Ring::Circle(
                positions
                    .iter()
                    .enumerate()
                    .map(|(j, p)| {
                        let [s, c] = angles[2 * j];
                        mesh.vertex(*p, [n[0] * s, n[1], n[0] * c])
                    })
                    .collect(),
            )
        };
        let pair = if i == 0 {
            (Ring::Circle(Vec::new()), circle(section.after))
        } else if i == last {
            (circle(section.before), Ring::Circle(Vec::new()))
        } else if section.before == section.after {
            let ring = circle(section.after);
            let copy = match &ring {
                Ring::Circle(v) => Ring::Circle(v.clone()),
                _ => unreachable!("a circle ring"),
            };
            (copy, ring)
        } else {
            (circle(section.before), circle(section.after))
        };
        rings.push(pair);
    }
    for i in 0..last {
        let (low, high) = (&rings[i].1, &rings[i + 1].0);
        for j in 0..segments {
            let k = (j + 1) % segments;
            match (low, high) {
                (Ring::Circle(a), Ring::Circle(b)) => {
                    mesh.triangle([a[j], a[k], b[k]], false);
                    mesh.triangle([a[j], b[k], b[j]], false);
                }
                (pole, Ring::Circle(b)) => mesh.triangle([pole.pole(j), b[k], b[j]], false),
                (Ring::Circle(a), pole) => mesh.triangle([a[j], a[k], pole.pole(j)], false),
                _ => unreachable!("a validated profile never joins two poles"),
            }
        }
    }
    mesh
}

/// Turns a half outline round the y axis.
///
/// Each profile point is (radius, height): its distance from the axis and
/// its y. The domain: at least two finite points, no radius below zero, no
/// point but the first and last on the axis (radius 0), no two consecutive
/// points equal, and the profile, closed along the axis and by straight
/// segments from any end off the axis to the axis, a simple polygon with
/// area; either direction is accepted. `segments` (angle steps round the
/// turn) is a multiple of 4 from 4 to [`MAX_STEPS`](super::MAX_STEPS).
///
/// An end on the axis becomes a pole and closes the solid there; an end off
/// the axis leaves its ring of `segments` edges open. So the lathe is closed
/// when both ends lie on the axis and otherwise has one open boundary loop
/// per end off the axis. Profile corners sharper than [`CREASE_COS`] allows
/// get one ring per side; at a pole, the normal is the axis direction when
/// the profile meets the axis within that limit of square, and otherwise each
/// wedge gets its own vertex. Every triangle whose outward normal has z > 0
/// is textured front. No vertex lies farther from the axis than the profile
/// at its height.
pub fn lathe(profile: &[[f64; 2]], segments: u32) -> Result<Mesh, MeshError> {
    let segments = count(segments, "segments", 4, 4)?;
    if profile.iter().flatten().any(|v| !v.is_finite()) {
        return Err(MeshError::NonFinite);
    }
    if profile.len() < 2 {
        return Err(MeshError::TooFewPoints { needed: 2 });
    }
    let last = profile.len() - 1;
    for (i, p) in profile.iter().enumerate() {
        if p[0] < 0.0 || (p[0] == 0.0 && i != 0 && i != last) {
            return Err(MeshError::OutOfRange {
                parameter: "profile radius",
            });
        }
        if i > 0 && profile[i - 1] == *p {
            return Err(MeshError::RepeatedPoint { index: i });
        }
    }
    let mut closed = profile.to_vec();
    if profile[last][0] > 0.0 {
        closed.push([0.0, profile[last][1]]);
    }
    if profile[0][0] > 0.0 {
        closed.push([0.0, profile[0][1]]);
    }
    let mut points = profile.to_vec();
    if simple_area(&closed)? < 0.0 {
        points.reverse();
    }
    // With the closed profile counter-clockwise in (radius, height), the
    // outside lies to the right of each segment.
    let normals: Vec<[f64; 2]> = points
        .windows(2)
        .map(|w| unit2([w[1][1] - w[0][1], w[0][0] - w[1][0]]))
        .collect();
    let sections: Vec<Section> = points
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let before = normals[i.saturating_sub(1)];
            let after = normals[i.min(last - 1)];
            let (before, after) = if i == 0 || i == last {
                let n = if i == 0 { after } else { before };
                let n = if p[0] == 0.0 && n[1] * n[1] - n[0] * n[0] >= CREASE_COS {
                    [0.0, n[1].signum()]
                } else {
                    n
                };
                (n, n)
            } else if dot2(before, after) >= CREASE_COS {
                let shared = unit2([before[0] + after[0], before[1] + after[1]]);
                (shared, shared)
            } else {
                (before, after)
            };
            Section { p, before, after }
        })
        .collect();
    revolve(&sections, segments).finish(Front::FacingZ, |p| p)
}

/// A closed cylinder of `radius` round `axis`, `length` long and centred on
/// the origin, with flat ends.
///
/// Both ends are capped, so it is closed; the ends meet the side at a
/// crease. `segments` is a multiple of 4 from 4 to [`MAX_STEPS`](super::MAX_STEPS); radius and
/// length are above zero. Every triangle whose outward normal has z > 0 is
/// textured front.
pub fn cylinder(radius: f64, length: f64, axis: Axis, segments: u32) -> Result<Mesh, MeshError> {
    let segments = count(segments, "segments", 4, 4)?;
    let r = self::length(radius, "radius", 0.0, false)?;
    let h = 0.5 * self::length(length, "length", 0.0, false)?;
    let (down, out, up) = ([0.0, -1.0], [1.0, 0.0], [0.0, 1.0]);
    let profile = [
        Section {
            p: [0.0, -h],
            before: down,
            after: down,
        },
        Section {
            p: [r, -h],
            before: down,
            after: out,
        },
        Section {
            p: [r, h],
            before: out,
            after: up,
        },
        Section {
            p: [0.0, h],
            before: up,
            after: up,
        },
    ];
    revolve(&profile, segments).finish(Front::FacingZ, |p| axis.turn_from_y(p))
}

/// A closed capsule of `radius` round `axis`: a cylinder `length` long,
/// centred on the origin, with a hemisphere on each end.
///
/// It is closed and smooth everywhere. `segments` is a multiple of 4 from 4
/// to [`MAX_STEPS`](super::MAX_STEPS); `rings` (steps in each quarter turn of a hemisphere)
/// runs from 1 to [`MAX_STEPS`](super::MAX_STEPS). The radius is above zero and the length is
/// zero or more; a zero length makes a sphere. Every triangle whose outward
/// normal has z > 0 is textured front.
pub fn capsule(
    radius: f64,
    length: f64,
    axis: Axis,
    segments: u32,
    rings: u32,
) -> Result<Mesh, MeshError> {
    let segments = count(segments, "segments", 4, 4)?;
    let rings = count(rings, "rings", 1, 1)?;
    let r = self::length(radius, "radius", 0.0, false)?;
    let h = 0.5 * self::length(length, "length", 0.0, true)?;
    let quarter = unit_circle(4 * rings);
    let mut profile: Vec<Section> = quarter[..=rings]
        .iter()
        .map(|&[s, c]| smooth([r * s, -h - r * c], [s, -c]))
        .collect();
    let first_top = usize::from(h == 0.0);
    profile.extend(
        quarter[first_top..=rings]
            .iter()
            .map(|&[s, c]| smooth([r * c, h + r * s], [c, s])),
    );
    revolve(&profile, segments).finish(Front::FacingZ, |p| axis.turn_from_y(p))
}

/// A closed sphere of `radius` centred on the origin, with its poles on the
/// y axis.
///
/// `segments` is a multiple of 4 from 4 to [`MAX_STEPS`](super::MAX_STEPS); `rings` (steps
/// from pole to pole) is even, from 2 to [`MAX_STEPS`](super::MAX_STEPS), so there is an
/// equator. The radius is above zero. Every triangle whose outward normal
/// has z > 0 is textured front.
pub fn sphere(radius: f64, segments: u32, rings: u32) -> Result<Mesh, MeshError> {
    let segments = count(segments, "segments", 4, 4)?;
    let rings = count(rings, "rings", 2, 2)?;
    let r = length(radius, "radius", 0.0, false)?;
    let half_turn = unit_circle(2 * rings);
    let profile: Vec<Section> = (0..=rings)
        .map(|k| {
            let [s, c] = half_turn[k];
            smooth([r * s, -r * c], [s, -c])
        })
        .collect();
    revolve(&profile, segments).finish(Front::FacingZ, |p| p)
}

/// A hemisphere of `radius` standing on y = 0 with its pole at y = radius.
///
/// It is open: its rim, the ring of `segments` edges at y = 0, is its only
/// boundary loop. `segments` is a multiple of 4 from 4 to [`MAX_STEPS`](super::MAX_STEPS);
/// `rings` (steps from rim to pole) runs from 1 to [`MAX_STEPS`](super::MAX_STEPS). The radius
/// is above zero. Every triangle whose outward normal has z > 0 is textured
/// front.
pub fn dome(radius: f64, segments: u32, rings: u32) -> Result<Mesh, MeshError> {
    let segments = count(segments, "segments", 4, 4)?;
    let rings = count(rings, "rings", 1, 1)?;
    let r = length(radius, "radius", 0.0, false)?;
    let quarter = unit_circle(4 * rings);
    let profile: Vec<Section> = quarter[..=rings]
        .iter()
        .map(|&[s, c]| smooth([r * c, r * s], [c, s]))
        .collect();
    revolve(&profile, segments).finish(Front::FacingZ, |p| p)
}

/// A closed torus round `axis`, centred on the origin: a tube of radius
/// `minor` whose centre circle has radius `major`.
///
/// It is closed and smooth everywhere. `segments` (steps round the axis)
/// and `sides` (steps round the tube) are multiples of 4 from 4 to
/// [`MAX_STEPS`](super::MAX_STEPS); `major` is above `minor`, and `minor` above zero. Every
/// triangle whose outward normal has z > 0 is textured front.
pub fn torus(
    major: f64,
    minor: f64,
    axis: Axis,
    segments: u32,
    sides: u32,
) -> Result<Mesh, MeshError> {
    let segments = count(segments, "segments", 4, 4)?;
    let sides = count(sides, "sides", 4, 4)?;
    let minor = length(minor, "minor radius", 0.0, false)?;
    let major = length(major, "major radius", minor, false)?;
    let around = unit_circle(segments);
    let tube = unit_circle(sides);
    let mut mesh = Builder::new();
    for &[st, ct] in &around {
        for &[sv, cv] in &tube {
            let reach = major + minor * cv;
            mesh.vertex([reach * st, minor * sv, reach * ct], [cv * st, sv, cv * ct]);
        }
    }
    let index = |j: usize, k: usize| (j * sides + k) as u32;
    for j in 0..segments {
        let j1 = (j + 1) % segments;
        for k in 0..sides {
            let k1 = (k + 1) % sides;
            mesh.triangle([index(j, k), index(j1, k), index(j1, k1)], false);
            mesh.triangle([index(j, k), index(j1, k1), index(j, k1)], false);
        }
    }
    mesh.finish(Front::FacingZ, |p| axis.turn_from_y(p))
}

fn smooth(p: [f64; 2], n: [f64; 2]) -> Section {
    Section {
        p,
        before: n,
        after: n,
    }
}
