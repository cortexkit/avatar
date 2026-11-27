//! Moods change posture as well as the face, and the toy is fitted into its
//! frame once over the poses of all seven moods.

use crate::build::{Pose, Rig, Toy, MOODS};
use crate::geom::{bbox_of, clamp, lerp, min, rot, Pt, PI};

/// One mood's posture. Angles are degrees; for the right side a positive
/// arm angle raises the arm and a positive ear or stalk angle swings it
/// outwards. `sx` and `sy` scale the toy about the middle of its base, `hop`
/// lifts it by a share of its height, and `look` swings both eye stalks the
/// same way.
struct MoodPose {
    arm: f64,
    ear: f64,
    stalk: f64,
    look: f64,
    head: f64,
    head_dy: f64,
    tail: f64,
    lean: f64,
    sx: f64,
    sy: f64,
    hop: f64,
}

const POSES: [MoodPose; 7] = [
    MoodPose {
        arm: 0.0,
        ear: 0.0,
        stalk: 0.0,
        look: 0.0,
        head: 0.0,
        head_dy: 0.0,
        tail: 0.0,
        lean: 0.0,
        sx: 1.0,
        sy: 1.0,
        hop: 0.0,
    },
    MoodPose {
        arm: -16.0,
        ear: -6.0,
        stalk: -5.0,
        look: 0.0,
        head: 0.0,
        head_dy: 0.03,
        tail: 0.0,
        lean: 0.0,
        sx: 1.0,
        sy: 0.985,
        hop: 0.0,
    },
    MoodPose {
        arm: 1.0,
        ear: -12.0,
        stalk: -7.0,
        look: 0.0,
        head: 0.0,
        head_dy: -0.02,
        tail: -18.0,
        lean: 0.0,
        sx: 0.985,
        sy: 1.03,
        hop: 0.03,
    },
    MoodPose {
        arm: 8.0,
        ear: 7.0,
        stalk: 0.0,
        look: 10.0,
        head: -9.0,
        head_dy: 0.0,
        tail: 8.0,
        lean: -4.0,
        sx: 1.0,
        sy: 1.0,
        hop: 0.0,
    },
    MoodPose {
        arm: -10.0,
        ear: 18.0,
        stalk: 9.0,
        look: 0.0,
        head: 0.0,
        head_dy: 0.02,
        tail: 0.0,
        lean: 0.0,
        sx: 1.03,
        sy: 0.955,
        hop: 0.0,
    },
    MoodPose {
        arm: -18.0,
        ear: 36.0,
        stalk: 24.0,
        look: 0.0,
        head: 0.0,
        head_dy: 0.06,
        tail: 22.0,
        lean: 0.0,
        sx: 1.01,
        sy: 0.975,
        hop: 0.0,
    },
    MoodPose {
        arm: -8.0,
        ear: 24.0,
        stalk: 17.0,
        look: 0.0,
        head: 11.0,
        head_dy: 0.04,
        tail: 10.0,
        lean: 5.0,
        sx: 1.02,
        sy: 0.96,
        hop: 0.0,
    },
];

/// How far each arm kind rises when happy, at most.
fn happy_arm_max(kind: &str) -> f64 {
    match kind {
        "long" => 82.0,
        "nub" | "four" => 46.0,
        "wing" => 58.0,
        _ => 62.0,
    }
}

fn rest_box(g: &Toy) -> [f64; 4] {
    bbox_of(g.parts.iter().map(|p| p.outline.as_slice()))
}

pub fn build_poses(g: &mut Toy) {
    let side = g.t.mood_side;
    // A happy toy throws its arms up, but only while the raised arm's
    // bounding box stays within the resting toy's box, give or take 5 % of
    // its width to the side and 3 % of its height above.
    let mut happy_arm = 0.0;
    if let Some(arm_spec) = g.arm_spec {
        let rest = rest_box(g);
        let w = rest[2] - rest[0];
        let h = rest[3] - rest[1];
        let arm = g
            .parts
            .iter()
            .find(|p| p.id == "arm-r")
            .expect("a toy with arms has a right arm");
        let piv = g.rig("armR").expect("the right arm has a rig").pivot;
        let mut a = happy_arm_max(arm_spec.kind);
        while a >= 0.0 {
            let turned: Vec<Pt> = arm
                .outline
                .iter()
                .map(|&p| rot(p, -a * PI / 180.0, Some(piv)))
                .collect();
            let b = bbox_of([turned.as_slice()]);
            if b[2] <= rest[2] + w * 0.05 && b[1] >= rest[1] - h * 0.03 {
                happy_arm = a;
                break;
            }
            a -= 4.0;
        }
    }
    g.happy_arm = happy_arm;
    // Parts that swing out (ears, eye stalks) do so while their bounding box
    // stays within the resting toy's box, give or take 4 % of its width.
    let swing_max = |g: &Toy, rname: &str, most: f64| -> f64 {
        let rest = rest_box(g);
        let w = rest[2] - rest[0];
        let piv = g.rig(rname).expect("the rig exists").pivot;
        let own: Vec<&crate::build::Part> = g.parts.iter().filter(|p| p.rig == rname).collect();
        let mut a = most;
        while a > 4.0 {
            let turned: Vec<Vec<Pt>> = own
                .iter()
                .map(|p| {
                    p.outline
                        .iter()
                        .map(|&q| rot(q, a * PI / 180.0, Some(piv)))
                        .collect()
                })
                .collect();
            let b = bbox_of(turned.iter().map(Vec::as_slice));
            if b[2] <= rest[2] + w * 0.04 {
                break;
            }
            a -= 3.0;
        }
        a
    };
    let ear_max = match g.ear_spec {
        Some(es) if g.rig("earR").is_some() && es.rot_k > 0.0 => {
            swing_max(g, "earR", 36.0 * es.rot_k)
        }
        _ => f64::INFINITY,
    };
    let stalk_max = if g.rig("stalkR").is_some() {
        swing_max(
            g,
            "stalkR",
            24.0 * g.mount_spec.expect("stalks have a mount").rot_k,
        )
    } else {
        f64::INFINITY
    };
    let small = g
        .arm_spec
        .is_some_and(|a| a.kind == "nub" || a.kind == "four");
    let has_arms = g.arm_spec.is_some();
    let body_h = g.body.y_bot - g.head.as_ref().map_or(g.body.y_top, |h| h.y_top);
    let head_h = g.head.as_ref().map(|h| h.h);
    let tail_side = g.t.tail.side;
    let ear_k = g.ear_spec.map_or(0.0, |e| e.rot_k);
    let mount_k = g.mount_spec.map_or(0.0, |m| m.rot_k);
    for (mi, m) in MOODS.iter().enumerate() {
        let p = &POSES[mi];
        let arm_k = if has_arms {
            if *m == "happy" {
                happy_arm
            } else {
                p.arm * if small { 0.8 } else { 1.0 }
            }
        } else {
            0.0
        };
        let set = |g: &mut Toy, name: &str, pose: Pose| {
            if let Some(r) = g.rig_mut(name) {
                r.pose[mi] = Some(pose);
            }
        };
        let turn = |rot: f64| Pose {
            rot,
            ..Pose::default()
        };
        if g.rig("armR").is_some() {
            set(g, "armR", turn(-arm_k));
            set(g, "armL", turn(arm_k));
        }
        if g.rig("arm2R").is_some() {
            set(g, "arm2R", turn(-arm_k * 0.55));
            set(g, "arm2L", turn(arm_k * 0.55));
        }
        if g.rig("earR").is_some() {
            let e = min(p.ear * ear_k, ear_max);
            set(g, "earR", turn(e));
            set(g, "earL", turn(-e));
        }
        if g.rig("stalkR").is_some() {
            let e = min(p.stalk * mount_k, stalk_max);
            let look = p.look * mount_k * side;
            set(g, "stalkR", turn(e + look));
            set(g, "stalkL", turn(-e + look));
        }
        if g.rig("tail").is_some() {
            set(g, "tail", turn(p.tail * -tail_side));
        }
        if let Some(hh) = head_h {
            set(
                g,
                "head",
                Pose {
                    rot: p.head * side,
                    dy: p.head_dy * hh,
                    ..Pose::default()
                },
            );
        }
        set(
            g,
            "root",
            Pose {
                rot: if head_h.is_some() {
                    p.lean * side * 0.4
                } else {
                    p.lean * side
                },
                scale: Some((p.sx, p.sy)),
                dy: -p.hop * body_h,
                dx: 0.0,
            },
        );
    }
    let ground = g.ground_y;
    if let Some(root) = g.rig_mut("root") {
        root.pivot = [0.0, ground];
    }
}

/// Moves a point on a part through its rig and every rig above it, in one
/// mood.
pub fn apply_rig(rigs: &[Rig], name: &str, mood: usize, p: Pt) -> Pt {
    let mut q = p;
    let mut cur = rigs.iter().find(|r| r.name == name);
    while let Some(r) = cur {
        if let Some(ps) = r.pose[mood] {
            if let Some((sx, sy)) = ps.scale {
                if sx != 1.0 || sy != 1.0 {
                    q = [
                        r.pivot[0] + (q[0] - r.pivot[0]) * sx,
                        r.pivot[1] + (q[1] - r.pivot[1]) * sy,
                    ];
                }
            }
            if ps.rot != 0.0 {
                q = rot(q, ps.rot * PI / 180.0, Some(r.pivot));
            }
            if ps.dx != 0.0 || ps.dy != 0.0 {
                q = [q[0] + ps.dx, q[1] + ps.dy];
            }
        }
        cur = r
            .parent
            .and_then(|parent| rigs.iter().find(|r| r.name == parent));
    }
    q
}

/// The frame's edge in its own units.
pub const FRAME: f64 = 200.0;

/// Fits the toy into the frame once, over the poses of all seven moods.
/// Margins leave room for the ground shadow. The size gene then makes it up
/// to a sixth smaller, standing on the same ground line.
pub fn fit_frame(g: &mut Toy) {
    let mut lists: Vec<Vec<Pt>> = Vec::new();
    for mi in 0..MOODS.len() {
        for part in &g.parts {
            lists.push(
                part.outline
                    .iter()
                    .map(|&p| apply_rig(&g.rigs, part.rig, mi, p))
                    .collect(),
            );
        }
    }
    let [x0, y0, x1, y1] = bbox_of(lists.iter().map(Vec::as_slice));
    let (top, bottom, side) = (10.0, 186.0, 9.0);
    let full = min((FRAME - 2.0 * side) / (x1 - x0), (bottom - top) / (y1 - y0));
    // A toy whose box already leaves much of the frame empty is made less
    // small, or not at all, so that no face is lost in an icon.
    let box_share = (x1 - x0) * (y1 - y0) * full * full / ((FRAME - 2.0 * side) * (bottom - top));
    let s = full * lerp(1.0, g.t.size, clamp((box_share - 0.4) / 0.3, 0.0, 1.0));
    let ox = FRAME / 2.0 - s * (x0 + x1) / 2.0;
    // The smaller toy keeps the lower edge the full-size one would have.
    let oy = (top + bottom) / 2.0 + full * (y1 - y0) / 2.0 - s * y1;
    g.fit = crate::build::Fit {
        s,
        ox,
        oy,
        bbox: [x0, y0, x1, y1],
    };
}
