//! Arms: two parts and a hand. A mood names where the hand should be; the
//! elbow is then wherever the two parts meet, found with a square root.

use crate::draw::{bone, disc, hose, plate, rod, seam, Ctx, DiscOpts, El, Out, NO_DOT};
use crate::geom::{cpp, line_path, place, unit_of, up_dir};
use crate::kit::{clamp, hyp, lerp, max, min, mix, Material, Reader, MAGNET, MAGNET_TIP, P2};
use crate::mood::{Gesture, MoodSpec};
use crate::robot::{Frame, Layer, Part, PartKind, Pose, Robot, Trim};

const ARMS: usize = 1072;

/// How arms are built, in the order their gene picks them; "none" is never picked.
pub const ARM_NAMES: [&str; 6] = ["none", "rod", "shell", "hose", "telescope", "stub"];
/// What a hand is.
pub const HAND_NAMES: [&str; 8] = [
    "clamp", "pincer", "claw", "mitt", "ball", "magnet", "sucker", "fork",
];
const HAND_WEIGHTS: [u32; 8] = [8, 4, 5, 4, 3, 2, 3, 3];

/// The middle joint of a two-part limb from `s` to `t` with parts `a` and
/// `b` long. A target out of reach is brought in along the line from `s`;
/// of the two places for the joint, the one `prefer` scores higher wins.
pub(crate) fn joint2(s: P2, t: P2, a: f64, b: f64, prefer: impl Fn(P2) -> f64) -> (P2, P2) {
    let dx = t[0] - s[0];
    let dy = t[1] - s[1];
    let mut d = hyp(dx, dy);
    if d == 0.0 || d.is_nan() {
        d = 1e-6;
    }
    let dc = clamp(d, (a - b).abs() + 0.5, (a + b) * 0.998);
    let ux = dx / d;
    let uy = dy / d;
    let tt = [s[0] + ux * dc, s[1] + uy * dc];
    let along = (a * a - b * b + dc * dc) / (2.0 * dc);
    let h = max(0.0, a * a - along * along).sqrt();
    let e1 = [s[0] + ux * along - uy * h, s[1] + uy * along + ux * h];
    let e2 = [s[0] + ux * along + uy * h, s[1] + uy * along - ux * h];
    (if prefer(e1) >= prefer(e2) { e1 } else { e2 }, tt)
}

/// The arms of a robot: how they are built and mounted, their tools, size,
/// length and resting pose.
#[derive(Clone, Debug)]
pub(crate) struct Arms {
    pub(crate) style: &'static str,
    s: P2,
    pub(crate) hand: &'static str,
    pub(crate) hand_l: &'static str,
    hs: f64,
    len: f64,
    l1: f64,
    l2: f64,
    w: f64,
    mat: Material,
    pub(crate) rest: Gesture,
    joint: f64,
    odd: f64,
}

impl Arms {
    pub(crate) fn none(mat: Material) -> Self {
        Self {
            style: "none",
            s: [0.0, 0.0],
            hand: "",
            hand_l: "",
            hs: 0.0,
            len: 0.0,
            l1: 0.0,
            l2: 0.0,
            w: 0.0,
            mat,
            rest: Gesture::Hang,
            joint: 0.0,
            odd: 0.0,
        }
    }
}

/// The joints of one arm in one mood: shoulder, elbow, wrist, and the
/// direction the hand points in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ArmPose {
    s: P2,
    e: P2,
    t: P2,
    u: P2,
}

pub(crate) fn plug_arms(r: &Reader, c: &mut Robot) {
    let o = ARMS;
    let p = &c.p;
    let style = ARM_NAMES[r.pick(o, 0, &[0, 9, 8, 7, 5, 3], 0)];
    if style == "none" {
        c.arms = Arms::none(p.metal);
        return;
    }
    // The shoulders: on the chassis, or low on the sides of a pod.
    let (host, y0, has_body) = match &c.body {
        Some(b) => (b, c.body_y, true),
        None => (&c.head, 0.0, false),
    };
    let ys = y0
        - host.h
            * if has_body {
                r.lin(o + 1, 0, 2, 0.6, 0.8)
            } else {
                r.lin(o + 1, 0, 2, 0.26, 0.46)
            };
    let s = [host.hw_at(ys - y0) + if has_body { 1.5 } else { -1.5 }, ys];
    let hand_i = r.pick(o + 2, 0, &HAND_WEIGHTS, 0);
    let hand = HAND_NAMES[hand_i];
    // One robot in eight has a different tool on its left arm.
    let hand_l = if r.raw(o + 3, 4, 3) == 5 {
        HAND_NAMES[(hand_i + 1 + r.raw(o + 2, 5, 2) as usize) % HAND_NAMES.len()]
    } else {
        hand
    };
    let hs = r.lin(o + 3, 0, 2, 0.84, 1.24) * c.scale;
    // Hanging, an arm ends above the ground.
    let drop = c.rise - ys - 21.0 * hs - 2.0;
    let len = clamp(drop * r.lin(o + 1, 2, 3, 0.62, 1.0), 15.0, 80.0)
        * c.trim_arms
        * if style == "stub" { 0.62 } else { 1.0 };
    let l1 = len
        * if style == "stub" || style == "telescope" {
            0.5
        } else {
            r.lin(o + 1, 5, 2, 0.44, 0.56)
        };
    let wide = r.unit(o + 3, 2, 2);
    let w = match style {
        "rod" => lerp(6.0, 7.6, wide),
        "shell" => lerp(10.0, 13.5, wide),
        "hose" => lerp(8.0, 10.5, wide),
        "telescope" => lerp(10.0, 13.0, wide),
        _ => lerp(11.0, 14.5, wide),
    } * c.scale;
    let mat = match style {
        "rod" => p.metal,
        "hose" => {
            if r.flag(o + 3, 7) {
                p.rubber
            } else {
                p.metal
            }
        }
        _ => p.paint,
    };
    let rest =
        [Gesture::Hang, Gesture::Out, Gesture::Ready, Gesture::Hang][r.raw(o + 4, 0, 2) as usize];
    c.arms = Arms {
        style,
        s,
        hand,
        hand_l,
        hs,
        len,
        l1,
        l2: len - l1,
        w,
        mat,
        rest,
        joint: r.lin(o + 4, 2, 2, 0.84, 1.26),
        // One arm in two hangs a little further out than the other.
        odd: [-1.0, 0.0, 0.0, 1.0][r.raw(o + 4, 4, 2) as usize],
    };
    for (side, class) in [(-1.0, "rb-arm rb-arm-l"), (1.0, "rb-arm rb-arm-r")] {
        c.parts.push(Part {
            kind: PartKind::Arm(side),
            class,
            layer: Layer::Arms,
            frame: Frame::Upper,
            trim: Some(Trim::Arms),
        });
    }
}

pub(crate) fn arm_pts(c: &Robot, side: f64, pose: &Pose) -> Vec<P2> {
    let a = pose.arm(side);
    let arms = &c.arms;
    let mut v = Vec::new();
    v.extend(crate::geom::disc_pts(a.s[0], a.s[1], arms.w));
    v.extend(crate::geom::disc_pts(a.e[0], a.e[1], arms.w));
    v.extend(crate::geom::disc_pts(
        a.t[0] + a.u[0] * 11.0 * arms.hs,
        a.t[1] + a.u[1] * 11.0 * arms.hs,
        13.0 * arms.hs,
    ));
    v
}

/// Where a hand goes for a named gesture, in the coordinates of the
/// chassis: x to the right of the axis, y down from the base of the lowest
/// module.
fn arm_target(c: &Robot, name: Gesture, side: f64, drop: f64) -> P2 {
    let a = &c.arms;
    let s = [side * a.s[0], a.s[1]];
    let l = a.len;
    let head = &c.head;
    let hand = 10.0 * a.hs;
    let from = |deg: f64, k: f64| {
        let u = up_dir(180.0 - side * (deg + if a.odd == side { 11.0 } else { 0.0 }));
        [s[0] + u[0] * l * k, s[1] + u[1] * l * k]
    };
    // The middle of the head and how wide it is there, as the arms see it.
    let hy = c.head_y + if c.body.is_some() { drop } else { 0.0 };
    let hw = |y: f64| head.hw_at(y - c.head_y);
    let front_y = match &c.body {
        Some(b) => c.body_y - b.h * 0.4,
        None => -head.h * 0.1,
    };
    match name {
        Gesture::Out => from(24.0, 0.985),
        Gesture::Ready => [s[0] + side * l * 0.36, s[1] + l * 0.62],
        Gesture::Limp => from(3.0, 0.99),
        Gesture::Fists => from(30.0, 0.9),
        Gesture::Front => [side * (hand * 0.75 + 2.0), front_y],
        Gesture::FrontHigh => [side * (hand * 0.75 + 2.0), front_y - 9.0],
        Gesture::Chin => [
            side * (hand + 4.0),
            if c.body.is_some() {
                hy + 3.0
            } else {
                -head.h * 0.14
            },
        ],
        Gesture::Hip => {
            let host = c.body.as_ref().unwrap_or(head);
            let y0 = if c.body.is_some() { c.body_y } else { 0.0 };
            [side * (host.hw_at(-host.h * 0.2) + 3.0), y0 - host.h * 0.2]
        }
        Gesture::Up => {
            let y = hy - head.h * 0.46;
            [side * (max(hw(y), hw(y + 12.0)) + hand + 5.0), y]
        }
        Gesture::High => {
            let y = hy - head.h * 0.72;
            [side * (max(hw(y), hw(y + 14.0)) + hand + 7.0), y]
        }
        Gesture::Shrug => [s[0] + side * l * 0.8, s[1] - l * 0.28],
        Gesture::Hang => from(10.0, 0.985),
    }
}

/// The joints of one arm in one mood.
pub(crate) fn arm_pose(c: &Robot, side: f64, gesture: Gesture, drop: f64, ground: f64) -> ArmPose {
    let a = &c.arms;
    let s = [side * a.s[0], a.s[1]];
    let mut t = arm_target(c, gesture, side, drop);
    // A hand stays above the ground.
    t[1] = min(t[1], ground - 20.0 * a.hs);
    if a.style == "stub" || a.style == "telescope" {
        // No elbow: a stub keeps its length and only turns; a telescope slides in and out.
        let stub = a.style == "stub";
        let mut d = hyp(t[0] - s[0], t[1] - s[1]);
        if d == 0.0 || d.is_nan() {
            d = 1e-6;
        }
        let len = if stub {
            a.len
        } else {
            clamp(d, a.len * 0.6, a.len)
        };
        let t = [
            s[0] + (t[0] - s[0]) / d * len,
            s[1] + (t[1] - s[1]) / d * len,
        ];
        return ArmPose {
            s,
            e: mix(s, t, if stub { 0.5 } else { a.len * 0.56 / len }),
            t,
            u: unit_of(s, t),
        };
    }
    // Of the two places for the elbow, the lower and outer one.
    let (e, t) = joint2(s, t, a.l1, a.l2, |e| e[1] + 0.6 * side * e[0]);
    ArmPose {
        s,
        e,
        t,
        u: unit_of(e, t),
    }
}

/// A hand at the wrist `t`, pointing along `u`. `grip` runs from 0 (open)
/// to 1 (shut).
#[allow(clippy::too_many_arguments)]
fn hand(out: &mut Out, ctx: &Ctx, c: &Robot, kind: &str, t: P2, u: P2, hs: f64, grip: f64) {
    let pal = &c.p;
    let met = &pal.metal;
    let p = |across: f64, along: f64| place(t, u, across * hs, along * hs);
    let w = max(2.6, 3.3 * hs);
    let lp = |pts: &[P2]| line_path(pts, false);
    match kind {
        "pincer" => {
            let g = 1.0 - grip;
            rod(
                out,
                ctx,
                &lp(&[
                    p(-3.0, 3.0),
                    p(-10.0, 9.0),
                    p(-8.5, 18.0),
                    p(-(2.0 + 4.5 * g), 22.5),
                ]),
                w * 1.45,
                met,
                true,
            );
            rod(
                out,
                ctx,
                &lp(&[
                    p(3.0, 3.0),
                    p(8.0, 8.5),
                    p(6.5, 15.0),
                    p(2.0 + 3.0 * g, 19.0),
                ]),
                w * 1.1,
                met,
                true,
            );
        }
        "claw" => {
            rod(
                out,
                ctx,
                &lp(&[p(-2.5, 2.5), p(-8.5, 9.0), p(-7.5 + 3.5 * grip, 17.5)]),
                w,
                met,
                true,
            );
            rod(
                out,
                ctx,
                &lp(&[p(2.5, 2.5), p(8.5, 9.0), p(7.5 - 3.5 * grip, 17.5)]),
                w,
                met,
                true,
            );
            rod(out, ctx, &lp(&[p(0.0, 3.0), p(0.0, 19.0)]), w, met, true);
        }
        "mitt" => {
            plate(
                out,
                ctx,
                &[
                    cpp(p(-7.0, 1.5), 2.0),
                    cpp(p(-8.5, 12.5), 5.0),
                    cpp(p(-4.0, 18.0), 4.0),
                    cpp(p(4.0, 18.0), 4.0),
                    cpp(p(8.5, 12.5), 5.0),
                    cpp(p(7.0, 1.5), 2.0),
                ],
                if pal.accent_kind == 3 {
                    &pal.rubber
                } else {
                    met
                },
            );
            seam(
                out,
                ctx,
                &lp(&[p(-7.0, 5.5), p(7.0, 5.5)]),
                met.line,
                1.2,
                None,
            );
        }
        "ball" => disc(
            out,
            ctx,
            p(0.0, 8.5),
            8.2 * hs,
            met,
            DiscOpts {
                keep: true,
                dot: false,
                ..DiscOpts::default()
            },
        ),
        "magnet" => {
            rod(
                out,
                ctx,
                &lp(&[
                    p(-6.5, 16.5),
                    p(-6.5, 7.0),
                    p(-4.0, 3.5),
                    p(4.0, 3.5),
                    p(6.5, 7.0),
                    p(6.5, 16.5),
                ]),
                w * 1.5,
                &MAGNET,
                false,
            );
            if ctx.fine {
                let mut d = lp(&[p(-6.5, 14.0), p(-6.5, 16.5)]);
                d.append(lp(&[p(6.5, 14.0), p(6.5, 16.5)]));
                out.put(El::path(&d).stroke(MAGNET_TIP.base, w * 1.5).round_cap());
            }
        }
        "sucker" => {
            bone(out, ctx, p(0.0, 0.0), p(0.0, 5.0), w * 1.1, met);
            plate(
                out,
                ctx,
                &[
                    cpp(p(-3.5, 4.0), 1.5),
                    cpp(p(-10.5, 14.5), 2.5),
                    cpp(p(10.5, 14.5), 2.5),
                    cpp(p(3.5, 4.0), 1.5),
                ],
                &pal.rubber,
            );
        }
        "fork" => {
            rod(
                out,
                ctx,
                &lp(&[p(-6.0, 17.5), p(-6.0, 5.5), p(6.0, 5.5), p(6.0, 17.5)]),
                w,
                met,
                true,
            );
            bone(out, ctx, p(0.0, 0.0), p(0.0, 5.5), w * 1.1, met);
        }
        _ => {
            let g = lerp(6.5, 2.4, grip);
            rod(
                out,
                ctx,
                &lp(&[p(-3.5, 4.0), p(-9.0, 9.0), p(-9.0, 14.5), p(-g, 18.5)]),
                w,
                met,
                true,
            );
            rod(
                out,
                ctx,
                &lp(&[p(3.5, 4.0), p(9.0, 9.0), p(9.0, 14.5), p(g, 18.5)]),
                w,
                met,
                true,
            );
            plate(
                out,
                ctx,
                &[
                    cpp(p(-4.6, -1.0), 1.2),
                    cpp(p(-4.6, 4.6), 1.2),
                    cpp(p(4.6, 4.6), 1.2),
                    cpp(p(4.6, -1.0), 1.2),
                ],
                met,
            );
        }
    }
}

pub(crate) fn arm_draw(out: &mut Out, ctx: &Ctx, c: &Robot, side: f64, md: &MoodSpec, pose: &Pose) {
    let a = &c.arms;
    let p = &c.p;
    let q = pose.arm(side);
    let w = a.w;
    let mat = &a.mat;
    match a.style {
        "rod" => {
            bone(out, ctx, q.s, q.e, w, mat);
            bone(out, ctx, q.e, q.t, w, mat);
            disc(
                out,
                ctx,
                q.e,
                w * 0.95 * a.joint,
                &p.metal,
                DiscOpts::default(),
            );
        }
        "shell" => {
            bone(out, ctx, q.e, q.t, w * 0.9, mat);
            bone(out, ctx, q.s, q.e, w, mat);
            disc(
                out,
                ctx,
                q.e,
                w * 0.46 * a.joint,
                &p.metal,
                DiscOpts::default(),
            );
        }
        "hose" => hose(
            out,
            ctx,
            q.s,
            [
                2.0 * q.e[0] - (q.s[0] + q.t[0]) / 2.0,
                2.0 * q.e[1] - (q.s[1] + q.t[1]) / 2.0,
            ],
            q.t,
            w,
            mat,
            8,
        ),
        "telescope" => {
            bone(out, ctx, q.e, q.t, w * 0.5, &p.metal);
            bone(out, ctx, q.s, q.e, w, mat);
        }
        _ => bone(out, ctx, q.s, q.t, w, mat),
    }
    hand(
        out,
        ctx,
        c,
        if side < 0.0 { a.hand_l } else { a.hand },
        q.t,
        q.u,
        a.hs,
        md.grip,
    );
    if a.style != "hose" {
        disc(out, ctx, q.t, max(3.0, w * 0.36), &p.metal, NO_DOT);
    }
    disc(
        out,
        ctx,
        q.s,
        (w * 0.5 + 2.6) * a.joint,
        &p.metal,
        DiscOpts::default(),
    );
}
