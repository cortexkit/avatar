//! Running gear: legs and feet, wheels, belts, and hover nozzles.

use avatar_small_robots::layout::{Frame, GearShape, SupportLayout};

use super::build::Builder;
use super::fittings::helix;
use super::solids::{self, ball, block, drum, hose, merge, or_ball, quadratic, rods, Affine, V3};
use crate::mesh::{self, Axis, Belt, Mesh};

/// The track belt's band thickness as a share of the belt's height.
const TRACK_BAND: f64 = 0.24;
/// The track's inner frame depth as a share of the belt's width, so its
/// faces sit back from the belt's.
const TRACK_FRAME_DEPTH: f64 = 0.8;
/// How far each road wheel stands out of the belt's faces, in robot units.
const WHEEL_PROUD: f64 = 1.5;
/// A road wheel's domed hub radius as a share of the wheel's.
const HUB: f64 = 0.4;

/// A slab whose outline is a stadium, `half_length` from its middle to
/// each end along x and `radius` round, `depth` deep along z and centred on
/// the origin.
fn stadium_slab(half_length: f64, radius: f64, depth: f64) -> Mesh {
    use avatar_svg::wild::math::{cos, sin};
    let straight = (half_length - radius).max(0.0);
    let steps: u32 = 8;
    let mut points = Vec::with_capacity(2 * steps as usize + 2);
    // The right half turn from the bottom up, then the left from the top
    // down; with no straight part between them they share their ends.
    for (end, start) in [(straight, -0.5), (-straight, 0.5)] {
        let shared = straight == 0.0 && start > 0.0;
        for k in 0..=steps {
            if shared && (k == 0 || k == steps) {
                continue;
            }
            let a = std::f64::consts::PI * (start + f64::from(k) / f64::from(steps));
            points.push([end + radius * cos(a), radius * sin(a)]);
        }
    }
    let flat = mesh::Extrusion {
        depth,
        bevel: 0.0,
        bevel_steps: 2,
    };
    let round = mesh::Extrusion {
        bevel: 0.4_f64.min(0.2 * radius),
        ..flat
    };
    mesh::Outline::new(&points)
        .and_then(|o| mesh::extrude(&o, &round).or_else(|_| mesh::extrude(&o, &flat)))
        .unwrap_or_else(|_| {
            solids::stand_in("a block for a track frame the mesh module refused");
            block(
                Affine::translate([0.0; 3]),
                [half_length, radius, 0.5 * depth],
                0.0,
            )
        })
}

/// Builds the gear and returns the indices of the parts that touch the
/// floor at rest: every foot, tread, track belt, wheel standing on the floor
/// and ball. Hover gear touches nothing.
pub(super) fn build(b: &mut Builder) -> Vec<usize> {
    let g = b.layout.gear.clone();
    let root = b.find("root");
    let depth = 2.0 * b.body_half_depth();
    let entry = ("gear", g.kind);
    let metal = b.mats.metal();
    let rubber = b.mats.rubber();
    let mut grounded = Vec::new();
    let leg_width = match &g.shape {
        GearShape::Legs { lm, .. } => lm.w,
        GearShape::Multi { lw, .. } | GearShape::Tripod { lw, .. } => *lw,
        GearShape::Hoses { w, .. } => *w,
        _ => 6.0,
    };
    let multi = matches!(g.shape, GearShape::Multi { .. } | GearShape::Hoses { .. });
    let pairs = g.supports.len().div_ceil(2);
    for (i, s) in g.supports.iter().enumerate() {
        let class = s.class;
        let at = |b: &Builder, p: [f64; 2], z: f64| b.at(s.frame, p, z);
        match &g.shape {
            GearShape::Legs { .. }
            | GearShape::Tripod { .. }
            | GearShape::Multi { .. }
            | GearShape::Pogo { .. }
            | GearShape::Hoses { .. } => {
                let x = label(s, i, &g.shape);
                // Pairs of splayed legs and hoses spread from front to back;
                // a tripod's middle leg stands behind.
                let z = if class.ends_with("-c") && !matches!(g.shape, GearShape::Pogo { .. }) {
                    -0.3 * depth
                } else if multi && pairs > 1 {
                    0.3 * depth * (1.0 - 2.0 * (i / 2) as f64 / (pairs - 1) as f64)
                } else {
                    0.0
                };
                let hip = at(b, s.base, z);
                let end = s.end.map_or(hip, |e| at(b, e, z));
                let spring = matches!(g.shape, GearShape::Pogo { .. }) || g.kind == "spring";
                let leg_name = if matches!(g.shape, GearShape::Pogo { .. }) {
                    "spring.0".to_string()
                } else {
                    format!("leg.{x}")
                };
                let leg_bone = b.bone(leg_name.as_str(), Some(root), hip);
                let r = 0.5 * leg_width;
                // A leg ends at its ankle or tip, which can sit lower than
                // the leg's own radius: such ends are raised so the leg's
                // round end stays on the floor's top; the foot covers them.
                let coil = r.max(2.5);
                let clear = if spring { coil + (0.3 * r).max(0.9) } else { r };
                let lift = |p: V3| [p[0], p[1].max(clear), p[2]];
                let (hip_at, end_at) = (lift(hip), lift(end));
                let mesh = if spring {
                    helix(hip_at, end_at, coil, (0.3 * r).max(0.9), 5.0)
                } else if let GearShape::Hoses { .. } = g.shape {
                    let c = s.middle.map_or(hip, |m| at(b, m, z));
                    let path: Vec<V3> = quadratic(hip, c, end, 10).into_iter().map(lift).collect();
                    hose(&path, r)
                } else if g.kind == "piston" {
                    let sleeve = solids::mid(hip_at, end_at);
                    merge(vec![
                        drum(hip_at, sleeve, r * 1.2),
                        drum(hip_at, end_at, r * 0.6),
                    ])
                } else {
                    let mut pts = vec![hip_at];
                    if let Some(m) = s.middle {
                        pts.push(lift(at(b, m, z)));
                    }
                    pts.push(end_at);
                    rods(&pts, r)
                };
                let material = match &g.shape {
                    GearShape::Hoses { mat, .. } => b.mats.named("hose", mat.base),
                    _ => metal,
                };
                b.rigid(
                    format!("leg.{x}"),
                    entry,
                    "leg",
                    class,
                    s.frame,
                    s.base,
                    material,
                    leg_bone,
                    mesh,
                );
                if let Some(foot) = s.foot {
                    let foot_bone = b.bone(format!("foot.{x}"), Some(leg_bone), end);
                    let sole = b.at(Frame::Ground, [foot.sole[0], 0.0], z);
                    let (material, mesh) = foot_mesh(b, foot.kind, foot.scale, sole);
                    grounded.push(b.rigid(
                        format!("foot.{x}"),
                        ("foot", foot.kind),
                        "foot",
                        class,
                        Frame::Ground,
                        foot.sole,
                        material,
                        foot_bone,
                        mesh,
                    ));
                }
            }
            GearShape::Treads { th, tw, .. } => {
                let x = if s.base[0] < 0.0 { "L" } else { "R" };
                let length = b.depth.tread_width * depth;
                let thick = 0.3 * th;
                let belt = mesh::belt(&Belt {
                    radius: 0.5 * (th - thick),
                    span: (length - th).max(0.0),
                    thickness: thick,
                    width: *tw,
                    axis: Axis::X,
                    segments: 12,
                });
                let centre = at(b, s.base, 0.0);
                let mesh = solids::transform(
                    or_ball(belt, [0.0; 3], 0.5 * th),
                    &Affine::translate(centre),
                );
                grounded.push(b.rigid(
                    format!("tread.{x}"),
                    entry,
                    "tread",
                    class,
                    s.frame,
                    s.base,
                    rubber,
                    root,
                    mesh,
                ));
            }
            GearShape::Track { th, tw, wr, .. } => {
                let width = b.depth.tread_width * depth;
                let centre = at(b, s.base, 0.0);
                if class == "rb-track" {
                    let thick = TRACK_BAND * th;
                    let belt = mesh::belt(&Belt {
                        radius: 0.5 * (th - thick),
                        span: (2.0 * tw - th).max(0.0),
                        thickness: thick,
                        width,
                        axis: Axis::Z,
                        segments: 12,
                    });
                    let mesh = solids::transform(
                        or_ball(belt, [0.0; 3], 0.5 * th),
                        &Affine::translate(centre),
                    );
                    grounded.push(b.rigid(
                        "track", entry, "track", class, s.frame, s.base, rubber, root, mesh,
                    ));
                    // The dark metal the 2D fills the belt with, set a little
                    // back from the belt's faces so the loop reads round it
                    // and the road wheels stand out in front of it.
                    let inner = (0.5 * th - thick).max(0.5);
                    let frame =
                        stadium_slab((tw - thick).max(inner), inner, TRACK_FRAME_DEPTH * width);
                    let dark = b.mats.metal_dark();
                    b.rigid(
                        "track.frame",
                        entry,
                        "track-frame",
                        class,
                        s.frame,
                        s.base,
                        dark,
                        root,
                        solids::transform(frame, &Affine::translate(centre)),
                    );
                } else {
                    let n = b.parts.iter().filter(|p| p.role == "wheel").count();
                    let bone = b.bone(format!("wheel.{n}"), Some(root), centre);
                    // Each road wheel stands proud of both belt faces, with a
                    // domed hub on each side, as the 2D draws wheels in the
                    // belt.
                    let half = 0.5 * width + WHEEL_PROUD;
                    let hub = HUB * *wr;
                    let mesh = merge(vec![
                        drum(
                            [centre[0], centre[1], -half],
                            [centre[0], centre[1], half],
                            *wr,
                        ),
                        ball([centre[0], centre[1], half], hub),
                        ball([centre[0], centre[1], -half], hub),
                    ]);
                    b.rigid(
                        format!("wheel.{n}"),
                        entry,
                        "wheel",
                        class,
                        s.frame,
                        s.base,
                        metal,
                        bone,
                        mesh,
                    );
                }
            }
            GearShape::Wheels { r, tw, .. } => {
                let x = if s.base[0] < 0.0 { "L" } else { "R" };
                let c = at(b, s.base, 0.0);
                let bone = b.bone(format!("wheel.{x}"), Some(root), c);
                let mesh = drum(
                    [c[0] - 0.5 * tw, c[1], c[2]],
                    [c[0] + 0.5 * tw, c[1], c[2]],
                    *r,
                );
                grounded.push(b.rigid(
                    format!("wheel.{x}"),
                    entry,
                    "wheel",
                    class,
                    s.frame,
                    s.base,
                    rubber,
                    bone,
                    mesh,
                ));
            }
            GearShape::Discs { r, .. } => {
                let x = if s.base[0] < 0.0 { "L" } else { "R" };
                let c = at(b, s.base, 0.0);
                let bone = b.bone(format!("wheel.{x}"), Some(root), c);
                let half = 0.5 * b.depth.wheel_width * depth;
                let mesh = drum([c[0], c[1], c[2] - half], [c[0], c[1], c[2] + half], *r);
                grounded.push(b.rigid(
                    format!("wheel.{x}"),
                    entry,
                    "wheel",
                    class,
                    s.frame,
                    s.base,
                    rubber,
                    bone,
                    mesh,
                ));
            }
            GearShape::Mono { r, hx, .. } => {
                let c = at(b, s.base, 0.0);
                let bone = b.bone("wheel.M", Some(root), c);
                if g.kind == "ball" {
                    let top = b.at(Frame::UpperBody, [0.0, 0.0], 0.0)[1];
                    let cup = or_ball(
                        mesh::lathe(
                            &[
                                [0.0, c[1] + 0.3 * r],
                                [1.08 * r, c[1] + 0.3 * r],
                                [1.08 * r, top.max(c[1] + r + 1.0)],
                                [0.0, top.max(c[1] + r + 1.0)],
                            ],
                            solids::ROUND,
                        ),
                        c,
                        *r,
                    );
                    let cup = solids::transform(cup, &Affine::translate([c[0], 0.0, c[2]]));
                    b.rigid(
                        "cup", entry, "cup", class, s.frame, s.base, metal, root, cup,
                    );
                    grounded.push(b.rigid(
                        "ball",
                        entry,
                        "wheel",
                        class,
                        s.frame,
                        s.base,
                        metal,
                        bone,
                        ball(c, *r),
                    ));
                } else {
                    let half = (hx - 2.0).clamp(1.0, 0.4 * r);
                    let wheel = drum([c[0] - half, c[1], c[2]], [c[0] + half, c[1], c[2]], *r);
                    grounded.push(b.rigid(
                        "wheel.M", entry, "wheel", class, s.frame, s.base, rubber, bone, wheel,
                    ));
                    let top = b.at(Frame::UpperBody, [0.0, 0.0], 0.0)[1];
                    let fork = merge(
                        [-1.0, 1.0]
                            .iter()
                            .map(|k| {
                                let x = c[0] + k * (half + 1.6);
                                rods(&[[x, top, 0.0], [x, c[1], 0.0]], 1.4)
                            })
                            .collect(),
                    );
                    b.rigid(
                        "fork", entry, "fork", class, s.frame, s.base, metal, root, fork,
                    );
                }
            }
            GearShape::Hover {
                top,
                bot,
                nh,
                rings,
                ..
            } => {
                let body = b.find("body");
                let mount = at(b, s.base, 0.0);
                let outlet = at(b, [0.0, *nh], 0.0);
                let bone = b.bone("jet.0", Some(body), mount);
                let nozzle = or_ball(
                    mesh::lathe(
                        &[
                            [0.0, mount[1]],
                            [*top, mount[1]],
                            [*bot, outlet[1]],
                            [0.0, outlet[1]],
                        ],
                        solids::ROUND,
                    ),
                    mount,
                    *top,
                );
                b.rigid(
                    "nozzle", entry, "nozzle", class, s.frame, s.base, metal, bone, nozzle,
                );
                // The flame or rings hang on their own bone at the outlet, so
                // a pose can shorten them toward the nozzle as thrust drops
                // and keep them off the floor when the robot lands.
                let bone = b.bone("exhaust.0", Some(bone), outlet);
                if g.kind == "rings" {
                    // The 2D draws the rings as strokes of light in the glow
                    // colour, so they share the flame's material and shine
                    // with the thrust.
                    let light = b.mats.flame();
                    for k in 0..(*rings).max(1) {
                        let y = outlet[1] - 3.0 * (f64::from(k) + 1.0);
                        let r = bot * (1.0 - 0.15 * f64::from(k));
                        let minor = 1.2_f64.min(0.4 * r);
                        let ring = or_ball(
                            mesh::torus(r.max(2.0 * minor), minor, Axis::Y, 24, 8),
                            [0.0; 3],
                            1.0,
                        );
                        let ring = solids::transform(
                            ring,
                            &Affine::translate([0.0, y.max(minor + 0.5), 0.0]),
                        );
                        b.rigid(
                            format!("ring.{k}"),
                            entry,
                            "ring",
                            class,
                            s.frame,
                            s.base,
                            light,
                            bone,
                            ring,
                        );
                    }
                } else {
                    let length = 0.6 * outlet[1];
                    let flame = or_ball(
                        mesh::lathe(
                            &[
                                [0.0, outlet[1]],
                                [0.8 * bot, outlet[1]],
                                [0.0, outlet[1] - length],
                            ],
                            solids::ROUND,
                        ),
                        outlet,
                        0.5 * bot,
                    );
                    let material = b.mats.flame();
                    b.rigid(
                        "flame", entry, "flame", class, s.frame, s.base, material, bone, flame,
                    );
                }
            }
            // Every gear shape the 2D type builds is handled above.
            _ => {}
        }
    }
    grounded
}

/// A support's side label for bone and part names: `L`, `R` or `M` for a
/// single middle support; pairs of splayed legs and hoses add their pair's
/// number from the front.
fn label(s: &SupportLayout, i: usize, shape: &GearShape) -> String {
    if s.class.ends_with("-c") || matches!(shape, GearShape::Pogo { .. }) {
        return "M".into();
    }
    let side = if s.base[0] < 0.0 { "L" } else { "R" };
    match shape {
        GearShape::Multi { .. } | GearShape::Hoses { .. } => format!("{side}{}", i / 2 + 1),
        _ => side.into(),
    }
}

/// A foot standing on the floor at `sole`: its lowest point is exactly at
/// y = 0. Its depth is its width times the foot-length trait.
fn foot_mesh(b: &mut Builder, kind: &str, scale: f64, sole: V3) -> (usize, Mesh) {
    let s = scale.max(0.2);
    let long = b.depth.foot_length;
    let lift = |m: Mesh| solids::transform(m, &Affine::translate(sole));
    match kind {
        "dome" => {
            let lathe = or_ball(
                mesh::lathe(
                    &[
                        [0.0, 0.0],
                        [7.0 * s, 0.0],
                        [5.0 * s, 4.0 * s],
                        [0.0, 5.5 * s],
                    ],
                    solids::ROUND,
                ),
                [0.0, 3.0 * s, 0.0],
                4.0 * s,
            );
            let mesh = solids::transform(
                lathe,
                &Affine::scale([1.0, 1.0, long]).then(Affine::translate(sole)),
            );
            (b.mats.paint(), mesh)
        }
        "block" => (
            b.mats.paint(),
            block(
                Affine::translate([sole[0], 4.0 * s, sole[2]]),
                [8.0 * s, 4.0 * s, 8.0 * s * long],
                1.2 * s,
            ),
        ),
        "toes" => {
            let mut parts = vec![block(
                Affine::translate([sole[0], 3.0 * s, sole[2]]),
                [7.0 * s, 3.0 * s, 6.0 * s * long],
                1.0 * s,
            )];
            for k in [-1.0, 0.0, 1.0] {
                parts.push(lift(ball([4.5 * s * k, 2.2 * s, 6.0 * s * long], 2.2 * s)));
            }
            (b.mats.metal(), merge(parts))
        }
        "ball" => (b.mats.metal(), lift(ball([0.0, 4.0 * s, 0.0], 4.0 * s))),
        "roller" => {
            let half = 5.0 * s * long;
            (
                b.mats.rubber(),
                lift(drum([0.0, 3.5 * s, -half], [0.0, 3.5 * s, half], 3.5 * s)),
            )
        }
        "sucker" => {
            let cup = or_ball(
                mesh::lathe(
                    &[
                        [0.0, 0.0],
                        [6.0 * s, 0.0],
                        [2.5 * s, 4.0 * s],
                        [0.0, 4.0 * s],
                    ],
                    solids::ROUND,
                ),
                [0.0, 2.0 * s, 0.0],
                3.0 * s,
            );
            (b.mats.rubber(), lift(cup))
        }
        _ => (
            b.mats.metal(),
            block(
                Affine::translate([sole[0], 2.5 * s, sole[2]]),
                [7.0 * s, 2.5 * s, 7.0 * s * long],
                1.0 * s,
            ),
        ),
    }
}
