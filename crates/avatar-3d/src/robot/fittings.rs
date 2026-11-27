//! Fittings on the hulls: the neck and waist, stalk eyes, antennae and their
//! tips, side fittings, the chest and what trails behind.

use std::f64::consts::PI;

use avatar_small_robots::layout::{BackLayout, Frame, MastLayout, StalkLayout};
use avatar_svg::wild::math;

use super::build::{Builder, NewPart, SkinSpec};
use super::hulls::front_z;
use super::materials::LightKind;
use super::solids::{
    self, add, ball, block, drum, ellipsoid, from_drawing, hose, merge, or_ball, quadratic, rod,
    rods, scale, sub, unit, Affine, V3,
};
use super::UNIT;
use crate::mesh::{self, Axis, Extrusion, Mesh, Outline};

/// The radius of antenna rods and stalks in robot units: the 2D stroke's
/// half width, thickened a little because a 3D rod has no ink outline
/// round it to give it the weight it has in the 2D drawing.
pub const ROD_RADIUS: f64 = 1.9;

/// A 2D drawing direction as a robot-space direction.
pub(super) fn dir3(d: [f64; 2]) -> V3 {
    [d[0], -d[1], 0.0]
}

/// The point `across` and `along` from `p` in a 2D direction `u`, as the
/// 2D type places points along a stem: `along` follows `u` and `across` is
/// square to it.
pub(super) fn place(p: [f64; 2], u: [f64; 2], across: f64, along: f64) -> [f64; 2] {
    [
        p[0] - u[1] * across + u[0] * along,
        p[1] + u[0] * across + u[1] * along,
    ]
}

/// The tangent of an angle in degrees.
pub(super) fn tan_deg(degrees: f64) -> f64 {
    let a = degrees * PI / 180.0;
    math::sin(a) / math::cos(a)
}

/// The neck and, on a stack, the waist plate between chassis and pelvis.
pub(super) fn joints(b: &mut Builder) {
    let metal = b.mats.metal();
    if let Some(waist) = b.layout.joints.waist {
        let body = b.find("body");
        let mesh = drum(
            b.at(Frame::UpperBody, waist.base, 0.0),
            b.at(Frame::UpperBody, waist.end, 0.0),
            waist.half_width,
        );
        b.rigid(
            "waist",
            ("plan", b.layout.plan),
            "waist",
            "rb-waist",
            Frame::UpperBody,
            waist.base,
            metal,
            body,
            mesh,
        );
    }
    let Some(neck) = b.layout.joints.neck else {
        return;
    };
    if neck.half_width <= 0.0 || neck.kind == "flush" {
        return;
    }
    let (lo, hi) = (
        b.at(Frame::UpperBody, neck.base, 0.0),
        b.at(Frame::UpperBody, neck.end, 0.0),
    );
    let hw = neck.half_width;
    let neck_bone = b.find("neck");
    let head_bone = b.find("head");
    let entry = ("neck", neck.kind);
    let part = |mesh, skin| NewPart {
        name: "neck".into(),
        entry,
        role: "neck",
        class: "rb-neck",
        frame: Frame::UpperBody,
        placement: neck.base,
        material: metal,
        mesh,
        skin,
    };
    match neck.kind {
        "collar" => {
            let p = part(drum(lo, hi, hw), SkinSpec::Rigid(neck_bone));
            b.add(p);
        }
        "piston" => {
            let sleeve = add(lo, scale(sub(hi, lo), 0.55));
            let mesh = merge(vec![drum(lo, hi, hw * 0.5), drum(lo, sleeve, hw)]);
            let p = part(mesh, SkinSpec::Rigid(neck_bone));
            b.add(p);
        }
        "ball" => {
            let p = part(ball(solids::mid(lo, hi), hw), SkinSpec::Rigid(neck_bone));
            b.add(p);
        }
        _ => {
            // Bellows and springs bend: each vertex blends from the neck's
            // bone at the bottom to the head's at the top.
            let mesh = if neck.kind == "bellows" {
                bellows(lo, hi, hw)
            } else {
                helix(lo, hi, hw * 0.8, (hw * 0.22).max(0.8), 4.0)
            };
            let skin = blend(&mesh, lo[1], hi[1], neck_bone, head_bone);
            let p = part(mesh, skin);
            b.add(p);
        }
    }
}

/// Weights that hand each vertex from bone `a` at height `y0` to bone `b`
/// at `y1`, linearly in height.
pub(super) fn blend(mesh: &Mesh, y0: f64, y1: f64, a: usize, b: usize) -> SkinSpec {
    let mut joints = Vec::with_capacity(mesh.positions.len());
    let mut weights = Vec::with_capacity(mesh.positions.len());
    for p in &mesh.positions {
        let t = ((p[1] - y0) / (y1 - y0)).clamp(0.0, 1.0);
        joints.push([a as u32, b as u32, 0, 0]);
        weights.push([1.0 - t, t, 0.0, 0.0]);
    }
    SkinSpec::Skinned(joints, weights)
}

/// A ribbed lathe from `lo` up to `hi`: a bellows.
fn bellows(lo: V3, hi: V3, hw: f64) -> Mesh {
    let h = hi[1] - lo[1];
    let ribs = 4;
    let mut profile = vec![[0.0, 0.0]];
    for i in 0..=2 * ribs {
        let r = if i % 2 == 0 { hw } else { hw * 0.72 };
        profile.push([r, h * i as f64 / (2 * ribs) as f64]);
    }
    profile.push([0.0, h]);
    let mesh = or_ball(mesh::lathe(&profile, 16), [0.0, 0.5 * h, 0.0], hw);
    solids::transform(mesh, &Affine::translate(lo))
}

/// A coil spring from `a` to `b`: a tube of `wire` radius winding `turns`
/// times round the axis at `radius`.
pub(super) fn helix(a: V3, b: V3, radius: f64, wire: f64, turns: f64) -> Mesh {
    let steps = (turns * 12.0) as usize;
    let frame = Affine::along(sub(b, a), a);
    let length = solids::dot(sub(b, a), sub(b, a)).sqrt();
    let points: Vec<V3> = (0..=steps)
        .map(|i| {
            let s = i as f64 / steps as f64;
            let phi = 2.0 * PI * turns * s;
            frame.apply([radius * math::cos(phi), length * s, radius * math::sin(phi)])
        })
        .collect();
    hose(&points, wire)
}

/// A rounded rectangle centred on `c` with half extents `half` and
/// elliptical corners of `corner` radii, in a 2D drawing frame.
pub(super) fn rounded_rect(
    c: [f64; 2],
    half: [f64; 2],
    corner: [f64; 2],
    steps: usize,
) -> Vec<[f64; 2]> {
    let q = [
        corner[0].clamp(0.0, half[0] * 0.999),
        corner[1].clamp(0.0, half[1] * 0.999),
    ];
    let mut pts = Vec::with_capacity(4 * (steps + 1));
    // Corner centres in turn round the outline, each with the angle its
    // quarter arc starts at; angles turn from +x toward +y (down).
    let centres = [
        ([c[0] + half[0] - q[0], c[1] + half[1] - q[1]], 0.0),
        ([c[0] - half[0] + q[0], c[1] + half[1] - q[1]], 0.5 * PI),
        ([c[0] - half[0] + q[0], c[1] - half[1] + q[1]], PI),
        ([c[0] + half[0] - q[0], c[1] - half[1] + q[1]], 1.5 * PI),
    ];
    for (centre, start) in centres {
        for k in 0..=steps {
            let a = start + 0.5 * PI * k as f64 / steps as f64;
            pts.push([
                centre[0] + q[0] * math::cos(a),
                centre[1] + q[1] * math::sin(a),
            ]);
        }
    }
    pts.dedup();
    if pts.first() == pts.last() {
        pts.pop();
    }
    pts
}

/// The stalks and their lens pods. Each pod is its lens housing extruded,
/// built in head-local 2D units so its texture's binding applies directly;
/// its front cap carries the stalk-eye texture.
pub(super) fn stalks(b: &mut Builder) {
    let stalks: Vec<StalkLayout> = b.layout.top.stalks.clone();
    let metal = b.mats.metal();
    let head = b.find("head");
    let oy = b.oy(Frame::Head);
    for s in &stalks {
        let name = if s.side < 0 { "stalk.L" } else { "stalk.R" };
        let stem = s.rest.stem;
        let bone = b.bone(name, Some(head), b.at(Frame::Head, stem.base, 0.0));
        let path: Vec<V3> = [stem.base, stem.bend, stem.end]
            .iter()
            .map(|p| b.at(Frame::Head, *p, 0.0))
            .collect();
        let mesh = if stem.straight {
            rod(path[0], path[2], ROD_RADIUS)
        } else {
            rods(&path, ROD_RADIUS)
        };
        b.rigid(
            format!("{name}.stem"),
            ("face", "stalk"),
            "stalk",
            s.class,
            Frame::Head,
            stem.base,
            metal,
            bone,
            mesh,
        );
        let mesh = stalk_eye(s, b.depth.housing_depth);
        b.rigid(
            format!("{name}.eye"),
            ("face", "stalk"),
            "stalk-eye",
            s.class,
            Frame::Head,
            s.rest.eye_centre,
            metal,
            bone,
            solids::transform(mesh, &from_drawing(oy)),
        );
    }
}

/// One lens pod in head-local 2D units (x, y down, z).
fn stalk_eye(s: &StalkLayout, housing: f64) -> Mesh {
    let pts = rounded_rect(s.rest.eye_centre, s.radii, s.corner_radii, 6);
    let r = s.radii[0].max(s.radii[1]);
    let depth = 2.0 * r * (0.5 + housing);
    let Ok(outline) = Outline::new(&pts) else {
        solids::stand_in("an ellipsoid for a stalk eye the mesh module refused");
        let c = s.rest.eye_centre;
        return ellipsoid([c[0], c[1], 0.0], [s.radii[0], s.radii[1], 0.5 * depth]);
    };
    let bevel = (0.2 * r).min(outline.max_bevel(2) * 0.99).min(0.45 * depth);
    mesh::extrude(
        &outline,
        &Extrusion {
            depth,
            bevel,
            bevel_steps: 2,
        },
    )
    .or_else(|_| {
        mesh::extrude(
            &outline,
            &Extrusion {
                depth,
                bevel: 0.0,
                bevel_steps: 1,
            },
        )
    })
    .expect("an unbevelled rounded rectangle is in the mesh module's domain")
}

/// Antennae: each mast on its own bone at its socket, leaning toward or
/// away from the viewer by the antenna-tilt trait.
pub(super) fn masts(b: &mut Builder) {
    let masts: Vec<MastLayout> = b.layout.top.masts.clone();
    let top = b.layout.top.kind;
    let head = b.find("head");
    let metal = b.mats.metal();
    let lean = tan_deg(b.depth.antenna_tilt);
    for (i, m) in masts.iter().enumerate() {
        let g = m.rest;
        let bone = b.bone(
            format!("antenna.{i}"),
            Some(head),
            b.at(Frame::Head, g.base, 0.0),
        );
        // A point of the mast in robot space, leaning along z with height.
        let at = |b: &Builder, p: [f64; 2]| b.at(Frame::Head, p, (g.base[1] - p[1]) * lean);
        let u = g.base_dir;
        let entry = ("top", top);
        let stem = |b: &mut Builder, role: &'static str, material, mesh| {
            b.rigid(
                format!("antenna.{i}.{role}"),
                entry,
                role,
                m.class,
                Frame::Head,
                g.base,
                material,
                bone,
                mesh,
            );
        };
        let mut end = g.end;
        let mut end_dir = g.end_dir;
        match m.kind {
            "zigzag" | "coil" => {
                let coil = m.kind == "coil";
                end = place(g.base, u, 0.0, g.length);
                end_dir = u;
                let mesh = if coil {
                    let a = at(b, g.base);
                    let e = at(b, end);
                    helix(a, e, 4.2, 1.3, 4.5)
                } else {
                    let pts: Vec<V3> = (0..=4)
                        .map(|k| {
                            let across = match k {
                                0 | 4 => 0.0,
                                k if k % 2 == 1 => 5.4,
                                _ => -5.4,
                            };
                            at(b, place(g.base, u, across, g.length * k as f64 / 4.0))
                        })
                        .collect();
                    rods(&pts, ROD_RADIUS)
                };
                stem(b, "mast", metal, mesh);
            }
            "loop" => {
                let e = place(g.base, u, 0.0, g.length * 0.46);
                let r = g.length * 0.27;
                let centre = place(e, u, 0.0, r);
                stem(b, "mast", metal, rod(at(b, g.base), at(b, e), ROD_RADIUS));
                let ring = or_ball(
                    mesh::torus(r, ROD_RADIUS.min(0.45 * r), Axis::Z, 24, 8),
                    [0.0; 3],
                    r,
                );
                let c = at(b, centre);
                stem(
                    b,
                    "loop",
                    metal,
                    solids::transform(ring, &Affine::translate(c)),
                );
                continue;
            }
            "dish" => {
                let e = place(g.base, u, 0.0, g.length * 0.5);
                let r = g.length * 0.36;
                stem(b, "mast", metal, rod(at(b, g.base), at(b, e), ROD_RADIUS));
                // The bowl looks up its stem and toward the viewer.
                let face = unit(add(dir3(u), [0.0, 0.0, 0.8]));
                let c = at(b, e);
                let bowl = solids::transform(
                    ellipsoid([0.0; 3], [r, r * 0.35, r]),
                    &Affine::along(face, add(c, scale(face, r * 0.2))),
                );
                stem(b, "dish", metal, bowl);
                let lamp = b.mats.light("lamp", LightKind::Lamp);
                stem(b, "lamp", lamp, ball(add(c, scale(face, r * 0.75)), 2.6));
                continue;
            }
            "bulb" => {
                let r = (g.length * 0.3).clamp(7.5, 11.0);
                let base = block(
                    Affine::along(dir3(u), at(b, place(g.base, u, 0.0, 2.75))),
                    [4.6, 3.75, 4.6],
                    0.8,
                );
                stem(b, "mast", metal, base);
                let bulb = b.mats.light("bulb", LightKind::Bulb);
                stem(
                    b,
                    "bulb",
                    bulb,
                    ball(at(b, place(g.base, u, 0.0, 5.0 + r * 0.9)), r),
                );
                continue;
            }
            "beacon" => {
                let plate = block(
                    Affine::along(dir3(u), at(b, place(g.base, u, 0.0, 1.5))),
                    [10.5, 2.5, 10.5],
                    1.0,
                );
                stem(b, "mast", metal, plate);
                let beacon = b.mats.light("beacon", LightKind::Beacon);
                let dome = solids::transform(
                    ellipsoid([0.0; 3], [8.5, 9.4, 8.5]),
                    &Affine::along(dir3(u), at(b, place(g.base, u, 0.0, 3.5))),
                );
                stem(b, "beacon", beacon, dome);
                continue;
            }
            _ => {
                let pts: Vec<V3> = if g.kinked {
                    [g.base, g.bend, g.end].iter().map(|p| at(b, *p)).collect()
                } else if g.straight {
                    vec![at(b, g.base), at(b, g.end)]
                } else {
                    quadratic(at(b, g.base), at(b, g.bend), at(b, g.end), 8)
                };
                stem(
                    b,
                    "mast",
                    metal,
                    if pts.len() == 2 {
                        rod(pts[0], pts[1], ROD_RADIUS)
                    } else {
                        hose(&pts, ROD_RADIUS)
                    },
                );
            }
        }
        let Some(tip) = m.tip else { continue };
        let t = at(b, end);
        let d = unit(add(dir3(end_dir), [0.0, 0.0, lean * dir3(end_dir)[1]]));
        let entry = ("tip", tip.kind);
        let (material, mesh) = match tip.kind {
            "ball" => (
                b.mats.light("lamp", LightKind::Lamp),
                ball(t, tip.ball_radius),
            ),
            "fork" => {
                let frame = Affine::along(d, t);
                let mesh = merge(vec![
                    rod(t, frame.apply([-4.6, 7.5, 0.0]), 1.2),
                    rod(t, frame.apply([4.6, 7.5, 0.0]), 1.2),
                ]);
                (metal, mesh)
            }
            "arrow" => {
                let cone = or_ball(
                    mesh::lathe(&[[0.0, -1.5], [5.0, -1.5], [0.0, 8.5]], 16),
                    [0.0; 3],
                    3.0,
                );
                (
                    b.mats.light("lamp", LightKind::Lamp),
                    solids::transform(cone, &Affine::along(d, t)),
                )
            }
            _ => continue,
        };
        b.rigid(
            format!("antenna.{i}.tip"),
            entry,
            "tip",
            m.class,
            Frame::Head,
            g.end,
            material,
            bone,
            mesh,
        );
    }
}

/// Side fittings, bolted to the head's sides at its middle depth.
pub(super) fn sides(b: &mut Builder) {
    let fittings = b.layout.sides.fittings.clone();
    let head = b.find("head");
    let metal = b.mats.metal();
    for f in &fittings {
        // Painted vents are drawn into the head's marks texture, not built as
        // side-fitting geometry.
        if matches!(f.kind, "vents" | "none") {
            continue;
        }
        let k = f.scale;
        let s = f64::from(f.side);
        let [x, y] = [f.mount[0] * s, f.mount[1]];
        // A point `out` units outward from the mount line and `down` below it.
        let at = |b: &Builder, out: f64, down: f64, z: f64| {
            b.at(Frame::Head, [s * (x + out), y + down], z)
        };
        let side_name = if f.side < 0 { "side.L" } else { "side.R" };
        let out_dir = [s, 0.0, 0.0];
        let slab = |b: &Builder, from: f64, to: f64, half_h: f64, half_d: f64| {
            let c = at(b, 0.5 * (from + to), 0.0, 0.0);
            block(
                Affine::along([0.0, 1.0, 0.0], c),
                [0.5 * (to - from), half_h, half_d],
                0.6,
            )
        };
        let mut extra = None;
        let mesh = match f.kind {
            "bolts" => merge(vec![
                slab(b, -3.0, 7.0 * k, 4.2 * k, 4.2 * k),
                slab(b, 6.0 * k, 10.5 * k, 6.6 * k, 6.6 * k),
            ]),
            "rods" => {
                let tip = at(b, 15.0 * k, -5.0 * k, 0.0);
                extra = Some(ball(tip, 4.2 * k));
                rod(at(b, -2.0, 0.0, 0.0), tip, 1.5)
            }
            "cones" => {
                let horn = or_ball(
                    mesh::lathe(
                        &[
                            [0.0, 0.0],
                            [4.0 * k, 0.0],
                            [10.0 * k, 15.0 * k],
                            [0.0, 15.0 * k],
                        ],
                        16,
                    ),
                    [0.0; 3],
                    5.0 * k,
                );
                solids::transform(horn, &Affine::along(out_dir, at(b, -2.0, 0.0, 0.0)))
            }
            "knobs" => merge(vec![
                slab(b, -2.0, 6.0 * k, 7.0 * k, 7.0 * k),
                ball(at(b, 8.0 * k, 0.0, 0.0), 3.5 * k),
            ]),
            _ => slab(b, -2.0, 12.0 * k, 9.0 * k, 1.2),
        };
        let entry = ("side", f.kind);
        b.rigid(
            side_name,
            entry,
            "side",
            f.class,
            Frame::Head,
            f.mount,
            metal,
            head,
            mesh,
        );
        if let Some(cap) = extra {
            let accent = b.mats.accent();
            b.rigid(
                format!("{side_name}.cap"),
                entry,
                "side-cap",
                f.class,
                Frame::Head,
                f.mount,
                accent,
                head,
                cap,
            );
        }
    }
}

/// The chest fitting, standing on the front of the chassis or a pod's head.
pub(super) fn chest(b: &mut Builder) {
    let ch = b.layout.chest.clone();
    if ch.kind == "none" {
        return;
    }
    let frame = ch.frame;
    let (bone, host) = if frame == Frame::UpperBody {
        (
            b.find("body"),
            b.parts.iter().position(|p| p.role == "chassis"),
        )
    } else {
        (b.find("head"), Some(0))
    };
    let [w, h] = ch.radii;
    let c = ch.centre;
    // The host's front over the chest centre, in robot units.
    let front = host
        .and_then(|i| front_z(&b.parts[i].mesh, c))
        .map_or(b.body_half_depth(), |z| z / UNIT);
    let entry = ("chest", ch.kind);
    let class = "rb-chest";
    let raise = b.depth.panel_raise;
    let at = |b: &Builder, p: [f64; 2], z: f64| b.at(frame, p, z);
    let put = |b: &mut Builder,
               name: String,
               role: &'static str,
               material: usize,
               the_bone: usize,
               mesh: Mesh| {
        b.rigid(name, entry, role, class, frame, c, material, the_bone, mesh);
    };
    let plate = |b: &Builder, [half_w, half_h]: [f64; 2]| {
        let thick = raise + 1.0;
        block(
            Affine::translate(at(b, c, front - 1.0 + 0.5 * thick)),
            [half_w, half_h, 0.5 * thick],
            0.5 * raise.min(1.0),
        )
    };
    match ch.kind {
        "light" => {
            let r = w.min(h).min(9.0);
            let metal = b.mats.metal();
            let rim = drum(at(b, c, front - 1.0), at(b, c, front + raise), r + 2.4);
            put(b, "chest.rim".into(), "chest-rim", metal, bone, rim);
            let lamp = b.mats.light("lamp", LightKind::Lamp);
            let mesh = ellipsoid(at(b, c, front + raise), [r, r, r * 0.6]);
            put(b, "chest.lamp".into(), "lamp", lamp, bone, mesh);
        }
        "gauge" => {
            let r = w.min(h + 2.0).min(10.0);
            let white = b.mats.white();
            let dial = drum(at(b, c, front - 1.0), at(b, c, front + raise), r);
            put(b, "chest.dial".into(), "dial", white, bone, dial);
            let centre = at(b, c, front + raise + 0.6);
            let needle_bone = b.bone("needle", Some(bone), centre);
            let n = b.layout.mood(avatar_small_robots::Mood::Neutral).needle;
            let a = n * 62.0 * PI / 180.0;
            let tip = add(
                centre,
                [r * 0.8 * math::sin(a), r * 0.8 * math::cos(a), 0.0],
            );
            let red = b
                .layout
                .palette
                .status(avatar_small_robots::layout::Status::Err)
                .status;
            let material = b.mats.named("needle", red);
            put(
                b,
                "chest.needle".into(),
                "needle",
                material,
                needle_bone,
                rod(centre, tip, 0.8),
            );
        }
        "leds" => {
            let n = f64::from(ch.count);
            let r = 3.4_f64.min((w - 2.0) / n);
            for i in 0..ch.count {
                let x = (f64::from(i) - (n - 1.0) / 2.0) * (2.0 * w - 2.0 * r - 2.0)
                    / (n - 1.0).max(1.0);
                let slot = (i % 4) as usize;
                let name = ["led.0", "led.1", "led.2", "led.3"][slot];
                let led = b.mats.light(name, LightKind::Led { slot });
                let mesh = ellipsoid(at(b, [x, c[1]], front + 0.2), [r, r, r * 0.7]);
                put(b, format!("chest.led.{i}"), "led", led, bone, mesh);
            }
        }
        kind => {
            // The 2D draws these on the hull's paint (a hazard plate in its
            // own yellow); the drawing is painted onto the plate from the
            // chest layer, so the plate is grown to hold all of it.
            let (material, half_h) = match kind {
                "hazard" => (b.mats.hazard(), h * 0.62),
                _ => (b.mats.paint(), h),
            };
            let mut half = [w, half_h];
            if let Some(layer) = b.creature.chest_layer() {
                let r = layer.region;
                let margin = avatar_small_robots::layout::LAYER_MARGIN;
                for k in 0..2 {
                    let lo = r.origin[k] + margin - c[k];
                    let hi = r.origin[k] + r.size[k] - margin - c[k];
                    half[k] = half[k].max(lo.abs()).max(hi.abs());
                }
            }
            // The front cap is the outline inset by the bevel, so the plate
            // grows by the bevel to keep the drawing on its cap.
            let bevel = 0.5 * raise.min(1.0);
            let mut mesh = plate(b, half.map(|v| v + bevel));
            // Projection coordinates in the chest's 2D frame, where the
            // chest layer's canvas is bound.
            let oy = b.oy(frame);
            mesh.projection = mesh
                .positions
                .iter()
                .map(|p| [p[0] + 0.0, -p[1] - oy + 0.0])
                .collect();
            put(b, "chest".into(), "chest", material, bone, mesh);
        }
    }
}

/// The cable or exhaust pipe at the robot's back.
pub(super) fn back(b: &mut Builder) {
    let Some(back) = b.layout.back else { return };
    let behind = -b.body_half_depth();
    match back {
        BackLayout::Cable { class, rest, .. } => {
            let body = b.find("body");
            let root = b.find("root");
            let base = b.at(Frame::Ground, rest.base, behind * 0.5);
            let cable_bone = b.bone("cable", Some(body), base);
            let tip_z = behind * 1.1;
            let control = b.at(Frame::Ground, rest.control, behind * 0.9);
            let mut tip = b.at(Frame::Ground, rest.tip, tip_z);
            let plug_half = [2.6, 2.6, 4.0];
            tip[1] = tip[1].max(plug_half[1]);
            let points = quadratic(base, control, tip, 12);
            let mesh = hose(&points, 1.8);
            // Each vertex blends from the cable's bone at the body to the
            // root at the plug, along the cable's height.
            let skin = blend(&mesh, base[1], tip[1], cable_bone, root);
            let rubber = b.mats.rubber();
            b.add(NewPart {
                name: "cable".into(),
                entry: ("back", "cable"),
                role: "cable",
                class,
                frame: Frame::Ground,
                placement: rest.base,
                material: rubber,
                mesh,
                skin,
            });
            let metal = b.mats.metal();
            let plug = block(Affine::along(dir3(rest.plug_dir), tip), plug_half, 0.8);
            b.rigid(
                "cable.plug",
                ("back", "cable"),
                "plug",
                class,
                Frame::Ground,
                rest.tip,
                metal,
                root,
                plug,
            );
        }
        BackLayout::Pipe {
            class,
            base,
            outlet,
            scale: k,
            ..
        } => {
            let head = b.find("head");
            let z = -b.head_half_depth * 0.5;
            let a = b.at(Frame::Head, base, z);
            let e = b.at(Frame::Head, outlet, z);
            let bone = b.bone("pipe", Some(head), a);
            let metal = b.mats.metal();
            let mesh = merge(vec![
                rod(a, e, 2.6 * k),
                drum(sub(e, scale(unit(sub(e, a)), 2.0)), e, 3.8 * k),
            ]);
            b.rigid(
                "pipe",
                ("back", "pipe"),
                "pipe",
                class,
                Frame::Head,
                base,
                metal,
                bone,
                mesh,
            );
        }
        // The 2D type has no other back fitting.
        _ => {}
    }
}
