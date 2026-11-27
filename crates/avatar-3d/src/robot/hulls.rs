//! The hulls: head, chassis and pelvis, each the 2D outline extruded with a
//! bevelled edge or, for round kinds, its right half turned on a lathe.

use avatar_small_robots::layout::{Frame, HullLayout};

use super::build::Builder;
use super::depth::Depth;
use super::mapping::{self, Kind, Solid};
use super::solids::{self, from_drawing, Affine, V3};
use crate::mesh::{self, Extrusion, Mesh, Outline};

/// Straight steps in each quarter-round bevel of a hull.
pub const BEVEL_STEPS: u32 = 3;
/// Steps round a lathed hull.
pub const LATHE_SEGMENTS: u32 = 32;

/// Builds the hulls and their parts. Each hull's projection coordinates are
/// in the 2D frame its marks layer uses: head-local for the head, and the
/// upper-body frame for the chassis and pelvis.
pub(super) fn build(b: &mut Builder) {
    let layout = b.layout;
    let d = b.depth;
    let paint = b.mats.paint();
    let head_bone = b.find("head");
    let body_bone = b.find("body");
    let elements = face_elements(b);
    let head = hull(
        &layout.head,
        0.0,
        "head",
        d.head_depth,
        0.76,
        d.head_round,
        &elements,
    );
    b.head_half_depth = head.half_depth;
    let oy = b.oy(Frame::Head);
    b.rigid(
        "head",
        ("head", layout.head.kind),
        "head",
        "rb-head",
        Frame::Head,
        [0.0, 0.0],
        paint,
        head_bone,
        solids::transform(head.mesh, &from_drawing(oy)),
    );
    let oy = b.oy(Frame::UpperBody);
    if let Some(chassis) = &layout.chassis {
        let made = hull(
            chassis,
            chassis.origin[1],
            "chassis",
            d.chassis_depth,
            0.72,
            d.chassis_round,
            &[],
        );
        b.chassis_half_depth = Some(made.half_depth);
        b.rigid(
            "chassis",
            ("chassis", chassis.kind),
            "chassis",
            "rb-chassis",
            Frame::UpperBody,
            chassis.origin,
            paint,
            body_bone,
            solids::transform(made.mesh, &from_drawing(oy)),
        );
    }
    if let Some(pelvis) = &layout.pelvis {
        let made = hull(
            pelvis,
            pelvis.origin[1],
            "pelvis",
            d.chassis_depth,
            0.72,
            d.chassis_round,
            &[],
        );
        b.rigid(
            "pelvis",
            ("pelvis", pelvis.kind),
            "pelvis",
            "rb-pelvis",
            Frame::UpperBody,
            pelvis.origin,
            paint,
            body_bone,
            solids::transform(made.mesh, &from_drawing(oy)),
        );
    }
}

/// A hull's mesh in its 2D drawing frame (x, y down, z), and half its depth.
pub(super) struct Made {
    pub mesh: Mesh,
    pub half_depth: f64,
}

/// Outline points closer than this, in robot units, to the point kept before
/// them are dropped: two rounded corners that meet give nearly equal points,
/// and an edge that short would leave no room for a bevel.
pub const MERGE: f64 = 0.01;

/// The outline of a hull in its texture frame: shifted down by `dy`, with
/// points within [`MERGE`] of the point before them (and a closing point
/// within it of the first) dropped.
pub(super) fn outline_points(h: &HullLayout, dy: f64) -> Vec<[f64; 2]> {
    let near = |a: &[f64; 2], b: &[f64; 2]| {
        let d = [a[0] - b[0], a[1] - b[1]];
        (d[0] * d[0] + d[1] * d[1]).sqrt() < MERGE
    };
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(h.outline.len());
    for p in &h.outline {
        let q = [p[0], p[1] + dy];
        if !pts.last().is_some_and(|last| near(last, &q)) {
            pts.push(q);
        }
    }
    while pts.len() > 1 && near(&pts[0], &pts[pts.len() - 1]) {
        pts.pop();
    }
    pts
}

/// Whether the mapping table lathes this hull.
pub(super) fn lathed(field: &str, kind: &str) -> bool {
    matches!(
        mapping::entry(field, kind).map(|e| e.kind),
        Some(Kind::Drawable {
            solid: Solid::Lathe,
            ..
        })
    )
}

/// Builds one hull. `ratio` is its depth trait and `low` the bottom of that
/// trait's range; `round` its rounding trait. The head's front cap must
/// contain every face element.
#[allow(clippy::too_many_arguments)]
pub(super) fn hull(
    h: &HullLayout,
    dy: f64,
    field: &str,
    ratio: f64,
    low: f64,
    round: f64,
    elements: &[(Vec<[f64; 2]>, bool)],
) -> Made {
    let pts = outline_points(h, dy);
    let width = 2.0 * h.half_width;
    if lathed(field, h.kind) {
        if let Some(made) = lathe(&pts, width, Depth::lathe_ratio(ratio, low)) {
            return made;
        }
    }
    extrusion(&pts, width, h.height, width * ratio, round, elements)
}

/// The largest bevel, up to the rounding trait's share of the hull's
/// smaller side, that the outline allows and that keeps every face element
/// on the front cap. Zero always qualifies, because the 2D type fits every
/// face element inside the outline with a margin.
pub(super) fn clamp_bevel(
    outline: &Outline,
    width: f64,
    height: f64,
    depth: f64,
    round: f64,
    elements: &[(Vec<[f64; 2]>, bool)],
) -> f64 {
    let m = width.min(height);
    let wanted = (m * 0.1 * round).max(m * 0.03);
    let limit = (outline.max_bevel(BEVEL_STEPS) * 0.999).min(depth * 0.49);
    let fits = |bevel: f64| {
        bevel == 0.0
            || outline.inset(bevel).is_ok_and(|cap| {
                elements
                    .iter()
                    .all(|(e, closed)| inside(cap.points(), e, *closed))
            })
    };
    let mut bevel = wanted.min(limit).max(0.0);
    if fits(bevel) {
        return bevel;
    }
    let mut low = 0.0;
    for _ in 0..40 {
        let middle = 0.5 * (low + bevel);
        if fits(middle) {
            low = middle;
        } else {
            bevel = middle;
        }
    }
    low
}

fn extrusion(
    pts: &[[f64; 2]],
    width: f64,
    height: f64,
    depth: f64,
    round: f64,
    elements: &[(Vec<[f64; 2]>, bool)],
) -> Made {
    let Ok(outline) = Outline::new(pts) else {
        return boxed(pts, depth);
    };
    let bevel = clamp_bevel(&outline, width, height, depth, round, elements);
    let spec = |bevel| Extrusion {
        depth,
        bevel,
        bevel_steps: BEVEL_STEPS,
    };
    match mesh::extrude(&outline, &spec(bevel)) {
        Ok(mesh) => Made {
            mesh,
            half_depth: 0.5 * depth,
        },
        Err(_) => match mesh::extrude(&outline, &spec(0.0)) {
            Ok(mesh) => Made {
                mesh,
                half_depth: 0.5 * depth,
            },
            Err(_) => boxed(pts, depth),
        },
    }
}

/// The outline's bounding box extruded: the stand-in for an outline the
/// `mesh` module cannot take.
fn boxed(pts: &[[f64; 2]], depth: f64) -> Made {
    solids::stand_in("a box for a hull outline the mesh module refused");
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in pts {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let rect = [
        [lo[0], lo[1]],
        [hi[0], lo[1]],
        [hi[0], hi[1]],
        [lo[0], hi[1]],
    ];
    let outline = Outline::new(&rect).expect("a bounding rectangle is simple");
    let mesh = mesh::extrude(
        &outline,
        &Extrusion {
            depth,
            bevel: 0.0,
            bevel_steps: 1,
        },
    )
    .expect("an unbevelled rectangle is in the mesh module's domain");
    Made {
        mesh,
        half_depth: 0.5 * depth,
    }
}

/// Turns the outline's right half round its centreline (x = 0), scaled
/// along z to `ratio` of the width. `None` when the outline does not cross
/// the centreline exactly twice.
fn lathe(pts: &[[f64; 2]], width: f64, ratio: f64) -> Option<Made> {
    let profile = right_half(pts)?;
    let mesh = mesh::lathe(&profile, LATHE_SEGMENTS).ok()?;
    let mesh = solids::transform(mesh, &Affine::scale([1.0, 1.0, ratio]));
    Some(Made {
        mesh,
        half_depth: 0.5 * width * ratio,
    })
}

/// The outline's part with x > 0, closed at both ends where it meets the
/// centreline, as (radius, y) lathe profile points. `None` unless those
/// points form one run round the outline.
pub(super) fn right_half(pts: &[[f64; 2]]) -> Option<Vec<[f64; 2]>> {
    let n = pts.len();
    let positive = pts.iter().filter(|p| p[0] > 0.0).count();
    let start = (0..n).find(|&s| pts[s][0] <= 0.0 && pts[(s + 1) % n][0] > 0.0)?;
    let mut run = Vec::with_capacity(positive);
    let mut i = (start + 1) % n;
    while pts[i][0] > 0.0 && run.len() < n {
        run.push(pts[i]);
        i = (i + 1) % n;
    }
    if run.len() != positive || run.len() < 2 {
        return None;
    }
    // Where the edge from `a` (x ≤ 0) to `b` (x > 0) meets the centreline.
    let meet = |a: [f64; 2], b: [f64; 2]| {
        if a[0] == 0.0 {
            a[1]
        } else {
            a[1] + (b[1] - a[1]) * (a[0] / (a[0] - b[0]))
        }
    };
    let first = meet(pts[start], run[0]);
    let last = meet(pts[i], run[run.len() - 1]);
    let mut profile = vec![[0.0, first]];
    profile.extend(run);
    profile.push([0.0, last]);
    profile.dedup();
    (profile.len() >= 4 && first != last).then_some(profile)
}

/// Every face element outline in head-local robot units: eye housings, the
/// mouth's contours and a screen's bezel, each with whether it is closed.
pub(super) fn face_elements(b: &Builder) -> Vec<(Vec<[f64; 2]>, bool)> {
    let f = &b.layout.face;
    let mut out: Vec<(Vec<[f64; 2]>, bool)> =
        f.eyes.iter().map(|e| (e.outline.clone(), true)).collect();
    out.extend(
        f.mouth_outlines
            .iter()
            .map(|c| (c.points.clone(), c.closed)),
    );
    if let Some(bezel) = &f.bezel_outline {
        out.push((bezel.clone(), true));
    }
    out
}

/// Whether every point of an element lies inside or on `cap`, and none of
/// its segments crosses one of `cap`'s edges.
pub(super) fn inside(cap: &[[f64; 2]], element: &[[f64; 2]], closed: bool) -> bool {
    if !element.iter().all(|p| contains(cap, *p)) {
        return false;
    }
    let n = cap.len();
    let m = element.len();
    let segments = if closed { m } else { m.saturating_sub(1) };
    (0..segments).all(|j| {
        let (p, q) = (element[j], element[(j + 1) % m]);
        (0..n).all(|i| !crosses(cap[i], cap[(i + 1) % n], p, q))
    })
}

/// Point in polygon by the even-odd rule; a point on an edge counts as in.
pub(super) fn contains(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    let n = poly.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let cross = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        let within = p[0] >= a[0].min(b[0])
            && p[0] <= a[0].max(b[0])
            && p[1] >= a[1].min(b[1])
            && p[1] <= a[1].max(b[1]);
        if cross == 0.0 && within {
            return true;
        }
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
            if p[0] < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Whether segments ab and pq cross at a point inside both.
fn crosses(a: [f64; 2], b: [f64; 2], p: [f64; 2], q: [f64; 2]) -> bool {
    let o = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    let (d1, d2) = (o(a, b, p), o(a, b, q));
    let (d3, d4) = (o(p, q, a), o(p, q, b));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// The model-space z of a mesh's textured front over a projection point:
/// the front-most textured-front triangle whose projection holds it.
pub(super) fn front_z(mesh: &Mesh, point: [f64; 2]) -> Option<f64> {
    let mut best: Option<f64> = None;
    for (t, front) in mesh.triangles.iter().zip(&mesh.textured_front) {
        if !front {
            continue;
        }
        let [a, b, c] = t.map(|i| mesh.projection[i as usize]);
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        if area == 0.0 {
            continue;
        }
        let w = |p: [f64; 2], q: [f64; 2]| {
            ((q[0] - p[0]) * (point[1] - p[1]) - (q[1] - p[1]) * (point[0] - p[0])) / area
        };
        let (wa, wb, wc) = (w(b, c), w(c, a), w(a, b));
        if wa < -1e-12 || wb < -1e-12 || wc < -1e-12 {
            continue;
        }
        let [za, zb, zc] = t.map(|i| mesh.positions[i as usize][2]);
        let z = wa * za + wb * zb + wc * zc;
        best = Some(best.map_or(z, |v: f64| v.max(z)));
    }
    best
}

/// The face region's centre on the head's front, in model space.
pub(super) fn face_centre(b: &Builder) -> V3 {
    let f = &b.layout.face;
    let centre = f.region.map_or(f.mouth_centre, |r| {
        [r.origin[0] + 0.5 * r.size[0], r.origin[1] + 0.5 * r.size[1]]
    });
    let head = &b.parts[0];
    let z = front_z(&head.mesh, centre).unwrap_or(b.head_half_depth * super::UNIT);
    let p = b.at(Frame::Head, centre, 0.0);
    [p[0] * super::UNIT + 0.0, p[1] * super::UNIT + 0.0, z]
}
