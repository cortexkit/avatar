//! The modules of the hull and where they stand: the head, a chassis, a
//! lower segment, and what joins the head to the chassis.

use crate::draw::{disc, plate, rivets, rod, seam, shell, Ctx, DiscOpts, Out, ShellOpts};
use crate::face::Tech;
use crate::gear::{gear_weights, legged, splayed, zigzag, GEAR_NAMES};
use crate::geom::{cp, line_path, rrect, shift_pts, Cp};
use crate::hull::{hull_aspect, make_hull, Hull, HULL_NAMES};
use crate::kit::{clamp, js_round, lerp, max, min, Reader};
use crate::marks::{chest_draw, head_marks};
use crate::mood::MoodSpec;
use crate::robot::{Pose, Robot};

const PLAN: usize = 1024;
const HEAD: usize = 1032;
const BODY: usize = 1040;
const NECK: usize = 1046;

/// The assembly plans: a pod that is all head, a head on a small chassis,
/// a head on a neck, a head on two stacked segments.
pub const PLAN_NAMES: [&str; 4] = ["pod", "chibi", "neck", "stack"];
const HULL_WEIGHTS: [u32; 15] = [7, 5, 4, 6, 3, 5, 4, 5, 3, 5, 4, 3, 4, 3, 3];
/// The chassis shapes, in the order their gene picks them.
pub const BODY_NAMES: [&str; 8] = [
    "box", "barrel", "vee", "bell", "ball", "can", "octagon", "capsule",
];
const BODY_WEIGHTS: [u32; 8] = [3, 5, 5, 4, 5, 3, 3, 4];
/// How the head is joined on.
pub const NECK_NAMES: [&str; 6] = ["flush", "collar", "piston", "bellows", "ball", "spring"];

/// The hull a chassis of each kind is drawn as.
fn body_hull(kind: &str) -> &'static str {
    match kind {
        "barrel" => "soft",
        "vee" => "bucket",
        "bell" => "pyramid",
        "ball" => "ball",
        "can" => "can",
        "octagon" => "octagon",
        "capsule" => "capsule",
        _ => "box",
    }
}

/// Hulls with no flat front, on which a screen is rarer.
fn round_hull(kind: &str) -> bool {
    matches!(
        kind,
        "ball" | "egg" | "acorn" | "dome" | "soft" | "capsule" | "cup"
    )
}

/// Hulls that are round all over.
fn rounded(kind: &str) -> bool {
    matches!(kind, "ball" | "egg" | "acorn" | "soft" | "capsule")
}

#[derive(Clone, Debug)]
pub(crate) struct Neck {
    pub(crate) kind: &'static str,
    w: f64,
}

/// What the plan genes build before anything is plugged in. Heights are
/// measured from the base of the lowest module, upwards negative: `body_y`
/// is the base of the chassis, `body_top` its top, `head_y` the base of the head.
#[derive(Clone, Debug)]
pub(crate) struct Core {
    pub(crate) plan: &'static str,
    pub(crate) scale: f64,
    pub(crate) head: Hull,
    pub(crate) gear_kind: &'static str,
    pub(crate) face_tech: Tech,
    pub(crate) body: Option<Hull>,
    pub(crate) body_kind: Option<&'static str>,
    pub(crate) pelvis: Option<Hull>,
    pub(crate) neck: Option<Neck>,
    pub(crate) body_y: f64,
    pub(crate) body_top: f64,
    pub(crate) head_y: f64,
    pub(crate) body_rivets: bool,
}

impl Core {
    /// The lowest module: the lower segment, the chassis, or the head.
    pub(crate) fn low(&self) -> &Hull {
        self.pelvis
            .as_ref()
            .or(self.body.as_ref())
            .unwrap_or(&self.head)
    }
}

pub(crate) fn build_core(r: &Reader) -> Core {
    let plan = PLAN_NAMES[r.pick(PLAN, 0, &[16, 8, 4, 4], 0)];
    // The smallest robot is about four fifths as tall as the largest of its plan.
    let scale = r.lin(PLAN + 1, 0, 3, 0.84, 1.0);
    let pod = plan == "pod";
    let kind = HULL_NAMES[r.pick(HEAD, 0, &HULL_WEIGHTS, 0)];
    let round = round_hull(kind);
    let gear_kind = GEAR_NAMES[r.pick(1080, 0, &gear_weights(plan), 0)];
    let face_weights: [u32; 7] = match (pod, round) {
        (true, false) => [8, 8, 4, 5, 2, 2, 3],
        (true, true) => [5, 9, 5, 6, 2, 2, 3],
        (false, false) => [12, 7, 4, 4, 2, 1, 2],
        (false, true) => [8, 9, 5, 5, 2, 1, 2],
    };
    let mut face_tech = Tech::ALL[r.pick(1048, 0, &face_weights, 0)];
    // A pod with a screen that rolls or floats is a television on a trolley.
    if face_tech == Tech::Screen && pod && !legged(gear_kind) && gear_kind != "pogo" {
        face_tech = Tech::Lens;
    }
    let lim = hull_aspect(kind);
    let mut aspect = r.lin(HEAD + 1, 0, 4, lim[0], lim[1]);
    let base_area = match plan {
        "pod" => 14500.0,
        "chibi" => 11600.0,
        "neck" => 9800.0,
        _ => 10200.0,
    };
    let mut area = base_area * scale * scale;
    // Eyes on stalks leave the hull only the mouth, so the hull under them is smaller.
    if face_tech == Tech::Stalk {
        aspect = max(0.6, aspect * 0.9);
        area *= if pod { 0.72 } else { 0.86 };
    }
    // A screen still has to hold two eyes and a mouth, so its head is larger.
    if face_tech == Tech::Screen {
        area *= if pod { 1.1 } else { 1.22 };
    }
    // A pod is never much wider than tall.
    if pod {
        aspect = max(aspect, 0.82);
    }
    // On splayed legs a flat pod is a table; under stalks a low one is a bread bin.
    if pod && splayed(gear_kind) && !rounded(kind) {
        aspect = max(aspect, 1.08);
    }
    if pod && face_tech == Tech::Stalk {
        aspect = max(aspect, 1.04);
        area *= 1.12;
    }
    let w = (area / (4.0 * aspect)).sqrt();
    let h = 2.0 * w * aspect;
    let head = make_hull(kind, w, h, r.unit(HEAD + 2, 0, 4), r.unit(HEAD + 2, 4, 4));
    let mut core = Core {
        plan,
        scale,
        head,
        gear_kind,
        face_tech,
        body: None,
        body_kind: None,
        pelvis: None,
        neck: None,
        body_y: 0.0,
        body_top: 0.0,
        head_y: 0.0,
        body_rivets: false,
    };
    if pod {
        return core;
    }
    let ob = BODY;
    let stack = plan == "stack";
    let mut bk = BODY_NAMES[r.pick(ob, 0, &BODY_WEIGHTS, 0)];
    // A chassis that widens downwards over a pair of legs reads as a skirt.
    if bk == "bell" && legged(gear_kind) {
        bk = "box";
    }
    let mut hb = h * r.lin(
        ob + 1,
        3,
        3,
        if stack { 0.26 } else { 0.34 },
        if stack { 0.34 } else { 0.5 },
    );
    let mut wb = w * r.lin(ob + 1, 0, 3, 0.5, 0.8);
    if bk == "ball" {
        wb = clamp(wb, hb * 0.5, hb * 0.68);
    }
    if bk == "capsule" {
        hb = min(hb, wb * 1.3);
    }
    core.body_kind = Some(bk);
    let body = make_hull(
        body_hull(bk),
        wb,
        hb,
        r.unit(ob + 2, 0, 3),
        r.unit(ob + 2, 3, 3),
    );
    if stack {
        // A second, smaller segment under the chassis, with a waist joint between the two.
        let pk = ["box", "soft", "ball", "bucket"][r.raw(ob + 3, 0, 2) as usize];
        let hp = h * r.lin(ob + 3, 2, 2, 0.22, 0.3);
        let mut wp = wb * r.lin(ob + 3, 4, 2, 0.62, 0.94);
        if pk == "ball" {
            wp = clamp(wp, hp * 0.5, hp * 0.7);
        }
        core.pelvis = Some(make_hull(pk, wp, hp, 0.4, 0.4));
        core.body_y = -(hp + 5.0);
    }
    core.body_top = core.body_y - hb;
    let long = plan == "neck";
    let weights: [u32; 6] = if long {
        [0, 0, 6, 4, 2, 4]
    } else {
        [6, 5, 2, 1, 2, 0]
    };
    let nk = NECK_NAMES[r.pick(NECK, 0, &weights, 0)];
    let flush = nk == "flush";
    let nh = if flush {
        0.0
    } else if long {
        h * r.lin(NECK + 1, 0, 3, 0.16, 0.32)
    } else {
        r.lin(NECK + 1, 0, 3, 4.5, 10.0) * scale
    };
    let neck = Neck {
        kind: nk,
        w: if flush {
            0.0
        } else {
            wb * r.lin(NECK + 1, 3, 2, 0.26, 0.42)
        },
    };
    // A head whose underside is round is set down until it covers what it
    // stands on, and a head with no neck sits a little way into the chassis.
    let at = if flush {
        min(wb * 0.62, core.head.w * 0.5)
    } else {
        neck.w
    };
    let sink = min(core.head.h * 0.14, -core.head.bot_at(at));
    core.head_y = core.body_top - nh + sink + if flush { 3.0 } else { 0.0 };
    core.body_rivets = r.flag(ob + 3, 6);
    core.neck = Some(neck);
    core.body = Some(body);
    core
}

/// The neck, from the top of the chassis to the base of the head, which a
/// mood may have lowered.
pub(crate) fn neck_draw(out: &mut Out, ctx: &Ctx, c: &Robot, pose: &Pose) {
    let Some(n) = &c.neck else { return };
    if n.kind == "flush" {
        return;
    }
    let p = &c.p;
    let y0 = c.body_top + 3.0;
    let y1 = c.head_y + pose.drop - 3.0;
    let w = n.w;
    let h = y0 - y1;
    if h < 3.0 {
        return;
    }
    match n.kind {
        "collar" => {
            plate(out, ctx, &rrect(-w, y1, w, y0, 2.5), &p.metal);
            seam(
                out,
                ctx,
                &line_path(
                    &[[-w + 2.0, (y0 + y1) / 2.0], [w - 2.0, (y0 + y1) / 2.0]],
                    false,
                ),
                p.metal.line,
                1.1,
                None,
            );
        }
        "piston" => {
            plate(out, ctx, &rrect(-w * 0.5, y1, w * 0.5, y0, 1.5), &p.metal);
            plate(out, ctx, &rrect(-w, y1 + h * 0.5, w, y0, 2.5), &p.metal);
        }
        "bellows" => {
            let n = max(2.0, js_round(h / 5.0)) as u32;
            let mut left: Vec<Cp> = Vec::new();
            let mut right: Vec<Cp> = Vec::new();
            for i in 0..=n {
                let y = lerp(y0, y1, f64::from(i) / f64::from(n));
                let x = if i % 2 == 1 { w * 0.72 } else { w * 1.08 };
                left.push(cp(-x, y, 0.8));
                right.insert(0, cp(x, y, 0.8));
            }
            left.extend(right);
            plate(out, ctx, &left, &p.rubber);
        }
        "ball" => {
            let r = min(w, max(4.5, h * 0.5));
            let n = max(1.0, js_round((h - 4.0) / (r * 1.7))) as u32;
            for i in 0..n {
                let y = if n == 1 {
                    (y0 + y1) / 2.0
                } else {
                    lerp(
                        y0 - 2.0 - r * 0.6,
                        y1 + 2.0 + r * 0.6,
                        f64::from(i) / f64::from(n - 1),
                    )
                };
                disc(
                    out,
                    ctx,
                    [0.0, y],
                    r,
                    &p.metal,
                    DiscOpts {
                        keep: true,
                        ..DiscOpts::default()
                    },
                );
            }
        }
        _ => rod(
            out,
            ctx,
            &zigzag(
                [0.0, y0],
                [0.0, y1],
                max(4.0, js_round(h / 3.6)) as u32,
                w * 0.9,
            ),
            3.2,
            &p.metal,
            true,
        ),
    }
}

pub(crate) fn body_draw(out: &mut Out, ctx: &Ctx, c: &Robot, md: &MoodSpec) {
    let p = &c.p;
    let b = c
        .body
        .as_ref()
        .expect("a chassis is drawn only when there is one");
    if let Some(pelvis) = &c.pelvis {
        let ww = min(pelvis.hw_at(-pelvis.h + 2.0), b.hw_at(-2.0)) * 0.56;
        out.group("rb-waist", &[], |out| {
            plate(
                out,
                ctx,
                &rrect(-ww, -pelvis.h - 7.5, ww, -pelvis.h + 2.5, 2.5),
                &p.metal,
            );
        });
        out.group("rb-segment", &[], |out| {
            shell(out, ctx, &pelvis.pts, &p.paint, ShellOpts::default());
        });
    }
    out.group("rb-chassis-shell", &[], |out| {
        let mut over = out.sub();
        if c.body_rivets {
            let yt = c.body_top + 5.5;
            let yb = c.body_y - 5.5;
            rivets(
                &mut over,
                ctx,
                &[
                    [-b.hw_at(-b.h + 5.5) + 5.0, yt],
                    [b.hw_at(-b.h + 5.5) - 5.0, yt],
                    [-b.hw_at(-5.5) + 5.0, yb],
                    [b.hw_at(-5.5) - 5.0, yb],
                ],
                &p.metal,
            );
        }
        shell(
            out,
            ctx,
            &shift_pts(&b.pts, 0.0, c.body_y),
            &p.paint,
            ShellOpts {
                over: Some(over),
                ..ShellOpts::default()
            },
        );
    });
    chest_draw(out, ctx, c, md);
}

pub(crate) fn head_draw(out: &mut Out, ctx: &Ctx, c: &Robot, md: &MoodSpec) {
    out.group("rb-head-shell", &[], |out| {
        let mut over = out.sub();
        head_marks(&mut over, ctx, c);
        shell(
            out,
            ctx,
            &c.head.pts,
            &c.p.paint,
            ShellOpts {
                over: Some(over),
                ..ShellOpts::default()
            },
        );
    });
    if c.body.is_none() {
        chest_draw(out, ctx, c, md);
    }
    out.group("rb-face", &[], |out| {
        crate::face_draw::face(out, ctx, &c.f, &c.p, &c.head, md);
    });
}
