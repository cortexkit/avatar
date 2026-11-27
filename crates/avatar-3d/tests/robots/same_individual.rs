//! Same individual: the 3D robot agrees with its 2D layout in its parts,
//! colours, silhouettes and depth.

use avatar_3d::robot::{build_with, BuildOptions, Emission, Robot, Skin, UNIT};
use avatar_small_robots::layout::{BackLayout, Frame, GearShape, Layout, Status};
use avatar_small_robots::{decode, render_tree, Rgb, MOODS};
use avatar_svg::Paint;

use crate::cases;

/// One part as the inventory compares it: its entry, role, 2D class, frame,
/// placement and the bones it follows.
#[derive(Clone, Debug, PartialEq, PartialOrd)]
struct Item {
    entry: (String, String),
    role: String,
    class: String,
    frame: String,
    placement: [f64; 2],
    bones: String,
}

fn item(
    entry: (&str, &str),
    role: &str,
    class: &str,
    frame: Frame,
    placement: [f64; 2],
    bones: &str,
) -> Item {
    Item {
        entry: (entry.0.into(), entry.1.into()),
        role: role.into(),
        class: class.into(),
        frame: format!("{frame:?}"),
        placement,
        bones: bones.into(),
    }
}

fn actual(robot: &Robot) -> Vec<Item> {
    let mut out: Vec<Item> = robot
        .parts
        .iter()
        .map(|p| {
            let bones = match &p.skin {
                Skin::Rigid { bone } => robot.bones[*bone].name.clone(),
                Skin::Skinned { joints, .. } => {
                    let mut names: Vec<&str> = joints
                        .iter()
                        .flat_map(|j| j.iter().take(2))
                        .map(|j| robot.bones[*j as usize].name.as_str())
                        .collect();
                    names.sort_unstable();
                    names.dedup();
                    format!("skinned:{}", names.join("+"))
                }
                _ => unreachable!(),
            };
            item(p.entry, p.role, p.class, p.frame, p.placement, &bones)
        })
        .collect();
    out.sort_by(|a, b| a.partial_cmp(b).expect("placements are finite"));
    out
}

/// The parts the layout calls for, worked out from the layout alone.
fn expected(layout: &Layout) -> Vec<Item> {
    use Frame::{Ground, Head, UpperBody as Upper};
    let mut v = vec![item(
        ("head", layout.head.kind),
        "head",
        "rb-head",
        Head,
        [0.0, 0.0],
        "head",
    )];
    if let Some(h) = &layout.chassis {
        v.push(item(
            ("chassis", h.kind),
            "chassis",
            "rb-chassis",
            Upper,
            h.origin,
            "body",
        ));
    }
    if let Some(h) = &layout.pelvis {
        v.push(item(
            ("pelvis", h.kind),
            "pelvis",
            "rb-pelvis",
            Upper,
            h.origin,
            "body",
        ));
    }
    if let Some(w) = layout.joints.waist {
        v.push(item(
            ("plan", layout.plan),
            "waist",
            "rb-waist",
            Upper,
            w.base,
            "body",
        ));
    }
    if let Some(n) = layout.joints.neck {
        if n.kind != "flush" && n.half_width > 0.0 {
            let bones = if matches!(n.kind, "bellows" | "spring") {
                "skinned:head+neck"
            } else {
                "neck"
            };
            v.push(item(
                ("neck", n.kind),
                "neck",
                "rb-neck",
                Upper,
                n.base,
                bones,
            ));
        }
    }
    for s in &layout.top.stalks {
        let bone = if s.side < 0 { "stalk.L" } else { "stalk.R" };
        v.push(item(
            ("face", "stalk"),
            "stalk",
            s.class,
            Head,
            s.rest.stem.base,
            bone,
        ));
        v.push(item(
            ("face", "stalk"),
            "stalk-eye",
            s.class,
            Head,
            s.rest.eye_centre,
            bone,
        ));
    }
    for (i, m) in layout.top.masts.iter().enumerate() {
        let bone = format!("antenna.{i}");
        let roles: &[&str] = match m.kind {
            "loop" => &["mast", "loop"],
            "dish" => &["mast", "dish", "lamp"],
            "bulb" => &["mast", "bulb"],
            "beacon" => &["mast", "beacon"],
            _ => &["mast"],
        };
        for role in roles {
            v.push(item(
                ("top", layout.top.kind),
                role,
                m.class,
                Head,
                m.rest.base,
                &bone,
            ));
        }
        if let Some(tip) = m.tip.filter(|t| t.kind != "bare") {
            v.push(item(
                ("tip", tip.kind),
                "tip",
                m.class,
                Head,
                m.rest.end,
                &bone,
            ));
        }
    }
    for f in &layout.sides.fittings {
        if matches!(f.kind, "vents" | "none") {
            continue;
        }
        v.push(item(
            ("side", f.kind),
            "side",
            f.class,
            Head,
            f.mount,
            "head",
        ));
        if f.kind == "rods" {
            v.push(item(
                ("side", f.kind),
                "side-cap",
                f.class,
                Head,
                f.mount,
                "head",
            ));
        }
    }
    for (side, arm) in layout.arms.iter().enumerate() {
        let Some(arm) = arm else { continue };
        let x = ["L", "R"][side];
        let (upper, fore, wrist, grip) = (
            format!("upperarm.{x}"),
            format!("forearm.{x}"),
            format!("wrist.{x}"),
            format!("grip.{x}"),
        );
        let hose = format!("skinned:{fore}+{upper}+{wrist}");
        let roles: Vec<(&str, &str)> = match arm.kind {
            "rod" | "shell" => vec![
                ("arm-upper", &upper),
                ("arm-lower", &fore),
                ("elbow", &fore),
            ],
            "telescope" => vec![("arm-upper", &upper), ("arm-lower", &fore)],
            "stub" => vec![("arm", &upper)],
            _ => vec![("arm", &hose)],
        };
        for (role, bones) in roles {
            v.push(item(
                ("arms", arm.kind),
                role,
                arm.class,
                Upper,
                arm.rest.shoulder,
                bones,
            ));
        }
        let roles: Vec<(&str, &str)> = match arm.hand {
            "mitt" | "ball" | "fork" => vec![("hand", &wrist)],
            "magnet" => vec![("hand", &wrist), ("hand-tip", &wrist)],
            "sucker" => vec![("hand", &wrist), ("hand-cup", &wrist)],
            _ => vec![("hand", &wrist), ("jaw", &grip)],
        };
        for (role, bones) in roles {
            v.push(item(
                ("hand", arm.hand),
                role,
                arm.class,
                Upper,
                arm.rest.wrist,
                bones,
            ));
        }
    }
    let ch = &layout.chest;
    let host = if ch.frame == Upper { "body" } else { "head" };
    let roles: Vec<(&str, &str)> = match ch.kind {
        "none" => vec![],
        "light" => vec![("chest-rim", host), ("lamp", host)],
        "gauge" => vec![("dial", host), ("needle", "needle")],
        "leds" => (0..ch.count).map(|_| ("led", host)).collect(),
        _ => vec![("chest", host)],
    };
    for (role, bones) in roles {
        v.push(item(
            ("chest", ch.kind),
            role,
            "rb-chest",
            ch.frame,
            ch.centre,
            bones,
        ));
    }
    match layout.back {
        Some(BackLayout::Cable { class, rest, .. }) => {
            v.push(item(
                ("back", "cable"),
                "cable",
                class,
                Ground,
                rest.base,
                "skinned:cable+root",
            ));
            v.push(item(
                ("back", "cable"),
                "plug",
                class,
                Ground,
                rest.tip,
                "root",
            ));
        }
        Some(BackLayout::Pipe { class, base, .. }) => {
            v.push(item(("back", "pipe"), "pipe", class, Head, base, "pipe"));
        }
        _ => {}
    }
    let g = &layout.gear;
    let entry = ("gear", g.kind);
    let mut wheels = 0;
    for (i, s) in g.supports.iter().enumerate() {
        let (c, f, base) = (s.class, s.frame, s.base);
        match &g.shape {
            GearShape::Treads { .. } => v.push(item(entry, "tread", c, f, base, "root")),
            GearShape::Track { .. } if c == "rb-track" => {
                v.push(item(entry, "track", c, f, base, "root"))
            }
            GearShape::Track { .. } => {
                v.push(item(entry, "wheel", c, f, base, &format!("wheel.{wheels}")));
                wheels += 1;
            }
            GearShape::Wheels { .. } | GearShape::Discs { .. } => {
                let x = if base[0] < 0.0 { "L" } else { "R" };
                v.push(item(entry, "wheel", c, f, base, &format!("wheel.{x}")));
            }
            GearShape::Mono { .. } => {
                v.push(item(entry, "wheel", c, f, base, "wheel.M"));
                let other = if g.kind == "ball" { "cup" } else { "fork" };
                v.push(item(entry, other, c, f, base, "root"));
            }
            GearShape::Hover { .. } => {
                // The nozzle hangs on the jet; the flame or rings below it on
                // the exhaust, which a pose shrinks toward the nozzle.
                v.push(item(entry, "nozzle", c, f, base, "jet.0"));
                if g.kind == "rings" {
                    if let GearShape::Hover { rings, .. } = &g.shape {
                        for _ in 0..(*rings).max(1) {
                            v.push(item(entry, "ring", c, f, base, "exhaust.0"));
                        }
                    }
                } else {
                    v.push(item(entry, "flame", c, f, base, "exhaust.0"));
                }
            }
            shape => {
                let pogo = matches!(shape, GearShape::Pogo { .. });
                let x = if c.ends_with("-c") || pogo {
                    "M".to_string()
                } else {
                    let side = if base[0] < 0.0 { "L" } else { "R" };
                    match shape {
                        GearShape::Multi { .. } | GearShape::Hoses { .. } => {
                            format!("{side}{}", i / 2 + 1)
                        }
                        _ => side.to_string(),
                    }
                };
                let leg = if pogo {
                    "spring.0".to_string()
                } else {
                    format!("leg.{x}")
                };
                v.push(item(entry, "leg", c, f, base, &leg));
                if let Some(foot) = s.foot {
                    v.push(item(
                        ("foot", foot.kind),
                        "foot",
                        c,
                        Ground,
                        foot.sole,
                        &format!("foot.{x}"),
                    ));
                }
            }
        }
    }
    v.sort_by(|a, b| a.partial_cmp(b).expect("placements are finite"));
    v
}

#[test]
fn inventory_equals_the_layout() {
    for (label, genome) in cases::cases(cases::committed_prefix()) {
        let layout = Layout::new(&decode(&genome));
        let robot = build_with(&genome, BuildOptions::untextured());
        let (got, want) = (actual(&robot), expected(&layout));
        if got != want {
            let extra: Vec<_> = got.iter().filter(|i| !want.contains(i)).collect();
            let missing: Vec<_> = want.iter().filter(|i| !got.contains(i)).collect();
            panic!("{label}: extra parts {extra:#?}\nmissing parts {missing:#?}");
        }
    }
}

/// The 2D colour each material name stands for.
fn colour_2d(layout: &Layout, name: &str) -> Option<Rgb> {
    let p = &layout.palette;
    Some(match name {
        "paint" => p.paint.base,
        "metal" => p.metal.base,
        "accent" => p.accent.base,
        "rubber" => p.rubber.base,
        "magnet" => p.magnet.base,
        "magnet-tip" => p.magnet_tip.base,
        "white" => p.white,
        "flame" => p.glow,
        "hazard" => Rgb {
            r: 0xf6,
            g: 0xc5,
            b: 0x31,
        },
        "needle" => p.status(Status::Err).status,
        "arm" => layout.arms.iter().flatten().next()?.material.base,
        "hose" => match &layout.gear.shape {
            GearShape::Hoses { mat, .. } => mat.base,
            _ => return None,
        },
        _ => return None,
    })
}

#[test]
fn colours_and_status_lights_equal_the_2d_fills() {
    for (label, genome) in cases::cases(cases::committed_prefix()) {
        let layout = Layout::new(&decode(&genome));
        let robot = build_with(&genome, BuildOptions::untextured());
        for m in &robot.materials {
            if let Emission::Status { .. } = m.emission {
                continue;
            }
            assert_eq!(
                Some(m.base),
                colour_2d(&layout, m.name),
                "{label}: material {}",
                m.name
            );
        }
        for mood in MOODS {
            let md = layout.mood(mood);
            // Every status-lit part's colour in this mood, read from the
            // table with the mood's status, lit and rays.
            let mut lit3: Vec<Rgb> = Vec::new();
            for part in &robot.parts {
                let Emission::Status { light } = robot.materials[part.material].emission else {
                    continue;
                };
                let entry = &robot.status.lights[light];
                let status = match entry.kind {
                    avatar_3d::robot::LightKind::Led { slot } => md.leds[slot],
                    _ => md.status,
                };
                let colour = robot.status.colour(light, status, md.lit, md.rays);
                if part.entry == ("tip", "arrow") {
                    // The 2D arrow tip is a filled path, not a light; its
                    // fill is the status colour.
                    assert_eq!(
                        colour,
                        layout.palette.status(md.status).status,
                        "{label}: arrow tip"
                    );
                    continue;
                }
                lit3.push(colour);
            }
            let tree = render_tree(&genome, mood, None);
            let mut lit2: Vec<Rgb> = tree
                .primitives
                .iter()
                .filter(|p| p.label == "rb-light")
                .map(|p| match p.paint {
                    Paint::Solid { color } => color,
                    _ => panic!("{label}: a light without a solid fill"),
                })
                .collect();
            let key = |c: &Rgb| (c.r, c.g, c.b);
            lit3.sort_by_key(key);
            lit2.sort_by_key(key);
            assert_eq!(lit3, lit2, "{label} {mood:?}: status lights");
        }
    }
}

/// How far, in robot units, the builder may move an outline: it drops points
/// this close to the point before them.
const MERGE: f64 = 0.01;

/// Whether `p` lies inside `poly` or within `tol` of its edges.
fn inside_or_near(poly: &[[f64; 2]], p: [f64; 2], tol: f64) -> bool {
    let n = poly.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let ab = [b[0] - a[0], b[1] - a[1]];
        let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]))
            .clamp(0.0, 1.0);
        let d = [a[0] + ab[0] * t - p[0], a[1] + ab[1] * t - p[1]];
        if (d[0] * d[0] + d[1] * d[1]).sqrt() <= tol {
            return true;
        }
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) * ab[0] / ab[1] {
            inside = !inside;
        }
    }
    inside
}

#[test]
fn hull_silhouettes_match_the_reference_outlines_unmirrored() {
    for (label, genome) in cases::cases(cases::committed_prefix()) {
        let layout = Layout::new(&decode(&genome));
        let robot = build_with(&genome, BuildOptions::untextured());
        let rise = layout.rise;
        let hulls = [
            (
                "head",
                Some(&layout.head),
                layout.head.origin[1] - rise,
                0.0,
            ),
            (
                "chassis",
                layout.chassis.as_ref(),
                -rise,
                layout.chassis.as_ref().map_or(0.0, |h| h.origin[1]),
            ),
            ("pelvis", layout.pelvis.as_ref(), -rise, 0.0),
        ];
        for (name, hull, oy, dy) in hulls {
            let Some(hull) = hull else { continue };
            let part = robot
                .parts
                .iter()
                .find(|p| p.name == name)
                .expect("a hull part");
            let outline: Vec<[f64; 2]> = hull.outline.iter().map(|p| [p[0], p[1] + dy]).collect();
            // Back through the stated transform: x = X / UNIT, y = -Y / UNIT - oy.
            let back: Vec<[f64; 2]> = part
                .mesh
                .positions
                .iter()
                .map(|p| [p[0] / UNIT, -p[1] / UNIT - oy])
                .collect();
            for (b, q) in back.iter().zip(&part.mesh.projection) {
                // Projection coordinates are the 2D point itself: neither
                // axis is mirrored.
                assert!(
                    (b[0] - q[0]).abs() < 1e-9 && (b[1] - q[1]).abs() < 1e-9,
                    "{label} {name}: vertex {b:?} projects to {q:?}"
                );
                assert!(
                    inside_or_near(&outline, *b, MERGE),
                    "{label} {name}: {b:?} is outside the outline"
                );
            }
            for p in &outline {
                let tol = MERGE;
                assert!(
                    back.iter()
                        .any(|b| (b[0] - p[0]).abs() <= tol && (b[1] - p[1]).abs() <= tol),
                    "{label} {name}: the silhouette misses the outline point {p:?}"
                );
            }
            let top = outline.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
            let highest = back.iter().map(|b| b[1]).fold(f64::INFINITY, f64::min);
            assert!(
                (top - highest).abs() <= MERGE,
                "{label} {name}: upside down"
            );
        }
    }
}

/// The x and z extents of a part, in robot units.
fn extents(robot: &Robot, name: &str) -> Option<[f64; 2]> {
    let part = robot.parts.iter().find(|p| p.name == name)?;
    let span = |k: usize| {
        let vs = part.mesh.positions.iter().map(|p| p[k]);
        (vs.clone().fold(f64::NEG_INFINITY, f64::max) - vs.fold(f64::INFINITY, f64::min)) / UNIT
    };
    Some([span(0), span(2)])
}

#[test]
fn depth_bytes_change_their_hulls_and_not_the_2d_tree() {
    use avatar_3d::robot::mapping::{entry, Kind, Solid};
    for (label, genome) in cases::cases(cases::committed_prefix()) {
        let layout = Layout::new(&decode(&genome));
        let trees: Vec<_> = MOODS
            .iter()
            .map(|m| render_tree(&genome, *m, None))
            .collect();
        // Bits 0-3 of byte 6145 set the head's depth over its width, 0.76
        // to 1, and of byte 6144 the chassis's, 0.72 to 1. A lathed hull maps
        // the same bits to 0.86 to 1 of its widest turn.
        let hulls = [
            (6145, "head", Some(&layout.head), 0.76),
            (6144, "chassis", layout.chassis.as_ref(), 0.72),
        ];
        for (byte, name, hull, low) in hulls {
            let Some(hull) = hull else { continue };
            let lathed = matches!(
                entry(name, hull.kind).map(|e| e.kind),
                Some(Kind::Drawable {
                    solid: Solid::Lathe,
                    ..
                })
            );
            let mut depths = Vec::new();
            for (nibble, ratio) in [(0_u8, low), (15, 1.0)] {
                let mut g = genome.clone();
                g[byte] = (g[byte] & 0xf0) | nibble;
                let robot = build_with(&g, BuildOptions::untextured());
                let [x, z] = extents(&robot, name).expect("the hull is built");
                let want = if lathed {
                    x * if nibble == 0 { 0.86 } else { 1.0 }
                } else {
                    2.0 * hull.half_width * ratio
                };
                assert!(
                    (z - want).abs() < 1e-9 * want,
                    "{label}: {name} depth {z}, not {want}"
                );
                let changed: Vec<_> = MOODS.iter().map(|m| render_tree(&g, *m, None)).collect();
                assert!(changed == trees, "{label}: byte {byte} changed the 2D tree");
                depths.push(z);
            }
            assert!(
                depths[0] < depths[1],
                "{label}: byte {byte} did not change the {name}'s depth"
            );
        }
    }
}
