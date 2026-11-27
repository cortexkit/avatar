//! Running gear: what carries the robot. It is drawn in ground coordinates,
//! from where it is fixed under the lowest module (which a mood moves) down
//! to the ground (which stays where it is).

use crate::arms::joint2;
use crate::body::Core;
use crate::draw::{
    bone, disc, hose, plate, plate_k, rod, seam, shell, Ctx, DiscOpts, El, Out, ShellOpts, NO_DOT,
};
use crate::geom::{
    apply_m, cp, disc_pts, line_path, place, poly_path, rrect, unit_of, up_dir, Path,
};
use crate::kit::{hyp, js_round, lerp, max, min, mix, Material, Palette, Reader, P2};
use crate::mood::MoodSpec;
use crate::robot::{Pose, Robot};

const GEAR: usize = 1080;

/// The kinds of running gear, in the order their gene picks them.
pub const GEAR_NAMES: [&str; 16] = [
    "biped", "piston", "spring", "quad", "hexa", "tripod", "treads", "track", "wheels", "discs",
    "mono", "ball", "jet", "rings", "pogo", "hoses",
];

pub(crate) fn gear_weights(plan: &str) -> [u32; 16] {
    match plan {
        "pod" => [8, 3, 3, 9, 4, 4, 3, 3, 3, 4, 4, 5, 5, 3, 2, 1],
        "neck" => [5, 3, 2, 5, 2, 2, 7, 9, 4, 6, 5, 5, 4, 3, 2, 0],
        _ => [8, 3, 3, 6, 2, 3, 4, 5, 4, 5, 6, 6, 5, 3, 1, 0],
    }
}

/// Gear that is legs. A pod whose face is a screen always stands on legs or on a pogo spring.
pub(crate) fn legged(kind: &str) -> bool {
    matches!(
        kind,
        "biped" | "piston" | "spring" | "quad" | "hexa" | "tripod" | "hoses"
    )
}

/// Legs that spread out to the sides.
pub(crate) fn splayed(kind: &str) -> bool {
    matches!(kind, "quad" | "hexa" | "tripod")
}

const FOOT_NAMES: [&str; 7] = ["pad", "dome", "block", "toes", "ball", "roller", "sucker"];
const FOOT_WEIGHTS: [u32; 7] = [7, 6, 5, 5, 3, 3, 3];

/// How high above the ground the leg meets each kind of foot.
fn ankle(kind: &str) -> f64 {
    match kind {
        "pad" => 5.5,
        "dome" => 8.0,
        "block" => 10.0,
        "toes" => 6.0,
        "ball" => 10.5,
        "roller" => 11.5,
        _ => 8.5,
    }
}

/// A foot standing at (x, y), where y is the ground or, in a hop, above it.
fn foot(out: &mut Out, ctx: &Ctx, c: &Robot, kind: &str, x: f64, y: f64, fs: f64) {
    let p = &c.p;
    let met = &p.metal;
    match kind {
        "dome" => {
            let mut d = Path::new();
            d.move_to([x - 11.0 * fs, y]);
            d.cubic_to(
                [x - 11.0 * fs, y - 13.5 * fs],
                [x + 11.0 * fs, y - 13.5 * fs],
                [x + 11.0 * fs, y],
            );
            d.close();
            out.put(
                El::path(&d)
                    .fill(met.base)
                    .stroke(met.line, ctx.line_w * 0.85)
                    .round_join(),
            );
        }
        "block" => shell(
            out,
            ctx,
            &rrect(x - 8.5 * fs, y - 11.5 * fs, x + 8.5 * fs, y, 2.5),
            &p.paint,
            ShellOpts {
                light: false,
                line_k: 0.85,
                ..ShellOpts::default()
            },
        ),
        "toes" => {
            let mut d = line_path(
                &[
                    [x - 11.0 * fs, y - 1.8],
                    [x, y - 6.5 * fs],
                    [x + 11.0 * fs, y - 1.8],
                ],
                false,
            );
            d.append(line_path(&[[x, y - 6.5 * fs], [x, y - 1.8]], false));
            rod(out, ctx, &d, 3.6 * fs, met, true);
            disc(
                out,
                ctx,
                [x, y - 6.5 * fs],
                3.4 * fs,
                met,
                DiscOpts::default(),
            );
        }
        "ball" => disc(
            out,
            ctx,
            [x, y - 7.2 * fs],
            7.2 * fs,
            met,
            DiscOpts {
                keep: true,
                ..DiscOpts::default()
            },
        ),
        "roller" => {
            disc(
                out,
                ctx,
                [x, y - 7.8 * fs],
                7.8 * fs,
                &p.rubber,
                DiscOpts {
                    keep: true,
                    dot: false,
                    ..DiscOpts::default()
                },
            );
            disc(out, ctx, [x, y - 7.8 * fs], 3.2 * fs, met, NO_DOT);
        }
        "sucker" => plate_k(
            out,
            ctx,
            &[
                cp(x - 10.5 * fs, y, 2.0),
                cp(x - 3.5 * fs, y - 9.5 * fs, 1.5),
                cp(x + 3.5 * fs, y - 9.5 * fs, 1.5),
                cp(x + 10.5 * fs, y, 2.0),
            ],
            &p.rubber,
            0.85,
        ),
        _ => plate_k(
            out,
            ctx,
            &rrect(x - 10.5 * fs, y - 6.8 * fs, x + 10.5 * fs, y, 3.0),
            met,
            0.85,
        ),
    }
}

/// A zigzag between two points: a spring.
pub(crate) fn zigzag(a: P2, b: P2, n: u32, amp: f64) -> Path {
    let u = unit_of(a, b);
    let len = hyp(b[0] - a[0], b[1] - a[1]);
    let mut pts = vec![a];
    for i in 1..n {
        pts.push(place(
            a,
            u,
            if i % 2 == 1 { amp } else { -amp },
            len * f64::from(i) / f64::from(n),
        ));
    }
    pts.push(b);
    line_path(&pts, false)
}

/// The ankle of a foot that stands at (x, 0): on the ground while the leg
/// reaches, lifted with the hip when it cannot.
fn ankle_of(hip: P2, x: f64, ah: f64, reach: f64) -> P2 {
    let a = [x, -ah];
    let d = hyp(a[0] - hip[0], a[1] - hip[1]);
    if d <= reach {
        a
    } else {
        mix(hip, a, reach / d)
    }
}

/// How a two-part leg is built: all metal rods, painted shells, or a boot.
#[derive(Clone, Copy, Debug)]
struct Limb {
    build: &'static str,
    w: f64,
    joint: f64,
}

#[derive(Clone, Debug)]
struct MultiLeg {
    yh: f64,
    hx: f64,
    fx: f64,
    l1: f64,
    l2: f64,
}

/// The measurements each kind of gear is drawn from.
#[derive(Clone, Debug)]
enum Shape {
    Legs {
        ah: f64,
        hx: f64,
        fx: f64,
        lm: Limb,
        rest: f64,
        l: f64,
        amp: f64,
    },
    Multi {
        legs: Vec<MultiLeg>,
        lw: f64,
        jk: f64,
        tip: bool,
    },
    Tripod {
        ah: f64,
        jk: f64,
        lw: f64,
        hx: f64,
        fx: f64,
        l: f64,
    },
    Treads {
        th: f64,
        tw: f64,
        tx: f64,
        gap: f64,
    },
    Track {
        th: f64,
        tw: f64,
        wheels: u32,
        wr: f64,
        gap: f64,
    },
    Wheels {
        r: f64,
        tw: f64,
        wx: f64,
        gap: f64,
    },
    Discs {
        r: f64,
        wx: f64,
        spokes: u32,
    },
    Mono {
        r: f64,
        hx: f64,
        band: bool,
    },
    Hover {
        top: f64,
        bot: f64,
        nh: f64,
        rings: u32,
    },
    Pogo {
        ah: f64,
        amp: f64,
        n: u32,
        l: f64,
    },
    Hoses {
        w: f64,
        mat: Material,
        legs: [[f64; 2]; 2],
    },
}

/// The running gear: how high it holds the lowest module, how far a mood
/// may lower or raise it, whether it hovers, and how wide its footprint is.
#[derive(Clone, Debug)]
pub(crate) struct Gear {
    pub(crate) kind: &'static str,
    pub(crate) rise: f64,
    pub(crate) squat: f64,
    pub(crate) lift_max: f64,
    pub(crate) hover: bool,
    pub(crate) foot: f64,
    pub(crate) land: f64,
    foot_kind: &'static str,
    fs: f64,
    wl: f64,
    shape: Shape,
}

pub(crate) fn build_gear(r: &Reader, c: &Core, pal: &Palette, kind: &'static str) -> Gear {
    let o = GEAR;
    let h = c.head.h;
    let low = c.low();
    let pod = c.plan == "pod";
    let k = if pod { 1.0 } else { 0.84 };
    let sc = c.scale;
    let tall = || r.unit(o + 1, 0, 3);
    let wide = || r.unit(o + 1, 3, 3);
    let build = || r.unit(o + 2, 0, 3);
    let wl = max(low.hw_at(-low.h * 0.2), low.w * 0.5);
    let under = |x: f64| [x, low.bot_at(x) - 2.5];
    let knee = || r.lin(o + 5, 0, 2, 0.84, 1.26);
    let mut g = Gear {
        kind,
        rise: 0.0,
        squat: 3.0,
        lift_max: 3.0,
        hover: false,
        foot: 0.0,
        land: 0.0,
        foot_kind: "",
        fs: 0.0,
        wl,
        shape: Shape::Mono {
            r: 0.0,
            hx: 0.0,
            band: false,
        },
    };
    let pick_foot = |g: &mut Gear| {
        g.foot_kind = FOOT_NAMES[r.pick(o + 3, 0, &FOOT_WEIGHTS, 0)];
        g.fs = r.lin(o + 3, 5, 2, 0.92, 1.26) * sc;
        ankle(g.foot_kind) * g.fs
    };
    match kind {
        "biped" | "piston" | "spring" => {
            let ah = pick_foot(&mut g);
            g.rise = max(ah + 12.0, h * lerp(0.3, 0.56, tall()) * k);
            let hx = wl * lerp(0.42, 0.72, wide());
            let fx = hx + lerp(0.0, 9.0, build()) * sc;
            let biped = kind == "biped";
            // A pod is heavy: under one, the thin all-metal leg gives way to a shell.
            let mut lm = if biped {
                let b = r.pick(o + 4, 0, if pod { &[0, 5, 3] } else { &[3, 3, 2] }, 0);
                Limb {
                    build: ["rod", "shell", "boot"][b],
                    w: if b == 0 {
                        r.lin(o + 4, 2, 2, 6.6, 8.4)
                    } else {
                        r.lin(o + 4, 2, 2, 10.0, 13.5)
                    } * sc,
                    joint: 0.0,
                }
            } else {
                Limb {
                    build: "",
                    w: r.lin(o + 4, 2, 2, 10.0, 13.5) * sc,
                    joint: 0.0,
                }
            };
            if biped {
                lm.joint = knee();
            }
            let rest = hyp(fx - hx, g.rise - under(hx)[1] - ah);
            let l = rest
                * if biped {
                    r.lin(o + 4, 4, 2, 1.06, 1.22)
                } else {
                    1.12
                };
            let amp = r.lin(o + 4, 4, 2, 4.6, 7.0) * sc;
            g.squat = g.rise - ah - 9.0;
            g.lift_max = 30.0;
            g.foot = fx + 11.0 * g.fs;
            g.shape = Shape::Legs {
                ah,
                hx,
                fx,
                lm,
                rest,
                l,
                amp,
            };
        }
        "quad" | "hexa" => {
            let n = if kind == "quad" { 2 } else { 3 };
            let lw = if kind == "quad" {
                r.lin(o + 4, 2, 2, 8.4, 10.4)
            } else {
                r.lin(o + 4, 2, 2, 7.0, 8.6)
            } * sc;
            g.rise = h * lerp(0.14, 0.32, tall()) * k + 4.0;
            let jk = knee();
            let reach = h * lerp(0.3, 0.54, wide());
            let up = r.lin(o + 4, 4, 2, 7.0, 19.0) * sc;
            let tip = r.flag(o + 4, 6);
            let mut legs = Vec::new();
            for i in 0..n {
                // A short thigh up and out to a knee, and a long shin down to the ground.
                let fi = f64::from(i);
                let yh = -low.h * (0.14 + (if n == 2 { 0.2 } else { 0.14 }) * fi);
                let hx = low.hw_at(yh) * 0.9;
                let fx = wl + reach * (0.42 + 0.58 * (fi + 1.0) / f64::from(n));
                let kn = [hx + (fx - hx) * 0.42, -g.rise + yh - up * (1.0 + 0.25 * fi)];
                legs.push(MultiLeg {
                    yh,
                    hx,
                    fx,
                    l1: hyp(kn[0] - hx, up * (1.0 + 0.25 * fi)),
                    l2: hyp(fx - kn[0], kn[1] + 3.0),
                });
            }
            g.squat = max(2.0, g.rise - 7.0);
            g.lift_max = 26.0;
            g.foot = legs[legs.len() - 1].fx + 5.0;
            g.shape = Shape::Multi { legs, lw, jk, tip };
        }
        "tripod" => {
            let ah = pick_foot(&mut g);
            let jk = knee();
            let lw = r.lin(o + 4, 2, 2, 6.6, 8.4) * sc;
            g.rise = max(ah + 12.0, h * lerp(0.3, 0.55, tall()) * k);
            let hx = wl * 0.56;
            let fx = hx + lerp(13.0, 27.0, wide()) * sc;
            let l = hyp(fx - hx, g.rise - under(hx)[1] - ah) * r.lin(o + 4, 4, 2, 1.03, 1.14);
            g.squat = g.rise - ah - 9.0;
            g.lift_max = 26.0;
            g.foot = fx + 11.0 * g.fs;
            g.shape = Shape::Tripod {
                ah,
                jk,
                lw,
                hx,
                fx,
                l,
            };
        }
        "treads" => {
            // Two track pods seen from the front, a beam between them and a post up to the hull.
            let th = h * lerp(0.3, 0.42, tall()) * k;
            let tw = lerp(13.0, 21.0, build()) * sc;
            let tx = max(wl * lerp(0.82, 1.1, wide()), tw / 2.0 + 11.0);
            g.rise = th * r.lin(o + 4, 0, 2, 0.7, 0.98);
            g.foot = tx + tw / 2.0 + 1.0;
            let gap = if r.flag(o + 4, 2) { 6.6 } else { 4.6 };
            g.shape = Shape::Treads { th, tw, tx, gap };
        }
        "track" => {
            // One track seen from the side: a belt round a row of wheels.
            let th = h * lerp(0.24, 0.34, tall()) * k;
            let tw = max(wl * lerp(0.95, 1.35, wide()), th * 1.15);
            g.rise = th
                + (if pod {
                    lerp(8.0, 17.0, build())
                } else {
                    lerp(4.0, 10.0, build())
                }) * sc;
            g.foot = tw;
            let wheels = max(2.0, min(4.0, js_round((2.0 * tw - th) / (th * 0.95)) + 1.0)) as u32;
            let wr = th / 2.0 - 5.2;
            let gap = if r.flag(o + 4, 2) { 8.4 } else { 5.6 };
            g.shape = Shape::Track {
                th,
                tw,
                wheels,
                wr,
                gap,
            };
        }
        "wheels" => {
            // Two tyres seen from the front, on an axle.
            let rr = h * lerp(0.16, 0.26, tall()) * k;
            let tw = lerp(10.0, 16.0, build()) * sc;
            let wx = wl * lerp(0.86, 1.1, wide()) + tw / 2.0;
            g.rise = rr + r.lin(o + 4, 0, 2, 3.0, 11.0) * sc;
            g.foot = wx + tw / 2.0;
            let gap = if r.flag(o + 4, 2) { 6.4 } else { 4.4 };
            g.shape = Shape::Wheels { r: rr, tw, wx, gap };
        }
        "discs" => {
            // Two wheels turned to face us, as a child draws a cart.
            let rr = h * lerp(0.16, 0.27, tall()) * k;
            let wx = max(wl * lerp(0.8, 1.05, wide()), rr + 6.0);
            let spokes = 3 + r.raw(o + 4, 0, 2);
            g.rise = rr * lerp(0.92, 1.5, build()) + 2.0;
            g.foot = wx + rr * 0.7;
            g.shape = Shape::Discs { r: rr, wx, spokes };
        }
        "mono" | "ball" => {
            // One wheel in a fork, or a ball in a cup: the robot balances on it.
            let mono = kind == "mono";
            let rr =
                h * (if mono {
                    lerp(0.2, 0.3, tall())
                } else {
                    lerp(0.2, 0.29, tall())
                }) * k;
            let hx = min(wl * 0.62, rr * 0.84);
            let band = !mono && r.flag(o + 4, 0);
            g.rise = rr * if mono { 1.72 } else { 1.62 };
            g.foot = rr * 0.86;
            g.squat = 2.0;
            g.lift_max = 2.0;
            g.shape = Shape::Mono { r: rr, hx, band };
        }
        "jet" | "rings" => {
            // It hovers: a nozzle with a flame, or rings of light under an emitter.
            let jet = kind == "jet";
            g.rise = h * lerp(0.3, if jet { 0.5 } else { 0.46 }, tall()) * k;
            g.hover = true;
            g.lift_max = 16.0;
            let top = wl * lerp(0.3, 0.52, wide());
            let bot = top * if jet { lerp(0.6, 0.86, build()) } else { 1.25 };
            let nh = (if jet { lerp(7.0, 12.0, build()) } else { 5.0 }) * sc;
            let rings = if jet { 0 } else { 2 + r.raw(o + 4, 0, 1) };
            g.squat = g.rise - nh - 1.0;
            g.foot = max(top, wl * 0.62);
            g.land = nh + 1.0;
            g.shape = Shape::Hover {
                top,
                bot,
                nh,
                rings,
            };
        }
        "pogo" => {
            let ah = pick_foot(&mut g);
            g.rise = max(ah + 16.0, h * lerp(0.34, 0.56, tall()) * k);
            let amp = lerp(8.0, 12.5, wide()) * sc;
            let n = 5 + js_round(2.0 * build()) as u32 * 2;
            let l = (g.rise - ah) * 1.18;
            g.squat = (g.rise - ah) * 0.5;
            g.lift_max = 30.0;
            g.foot = 12.0 * g.fs;
            g.shape = Shape::Pogo { ah, amp, n, l };
        }
        _ => {
            // Hoses for legs, two a side, each ending in a cup that grips the ground.
            g.rise = h * lerp(0.26, 0.4, tall()) * k;
            let w = lerp(7.5, 10.0, build()) * sc;
            let mat = if r.flag(o + 4, 0) {
                pal.rubber
            } else {
                pal.metal
            };
            let spread = lerp(10.0, 22.0, wide()) * sc;
            let mut legs = [[0.0; 2]; 2];
            for (i, leg) in legs.iter_mut().enumerate() {
                let fi = i as f64;
                let hx = wl * (0.28 + 0.4 * fi);
                *leg = [hx, hx + spread * (0.6 + 0.7 * fi)];
            }
            g.squat = g.rise - 10.0;
            g.lift_max = 10.0;
            g.foot = legs[1][1] + 10.0;
            g.shape = Shape::Hoses { w, mat, legs };
        }
    }
    g
}

fn to_g(pose: &Pose, p: P2) -> P2 {
    apply_m(pose.u, p)
}

fn under(c: &Robot, x: f64) -> P2 {
    [x, c.low().bot_at(x) - 2.5]
}

/// A two-part leg from a hip to an ankle, with the knee where `prefer` likes it.
#[allow(clippy::too_many_arguments)]
fn leg(
    out: &mut Out,
    ctx: &Ctx,
    c: &Robot,
    hip: P2,
    ankle: P2,
    l: f64,
    lm: Limb,
    prefer: impl Fn(P2) -> f64,
) {
    let (e, t) = joint2(hip, ankle, l / 2.0, l / 2.0, prefer);
    let jk = lm.joint;
    let p = &c.p;
    let sc = c.scale;
    match lm.build {
        "rod" => {
            bone(out, ctx, hip, e, lm.w, &p.metal);
            bone(out, ctx, e, t, lm.w, &p.metal);
            disc(out, ctx, e, lm.w * 0.92 * jk, &p.metal, DiscOpts::default());
        }
        "shell" => {
            bone(out, ctx, e, t, lm.w * 0.9, &p.paint);
            bone(out, ctx, hip, e, lm.w, &p.paint);
            disc(out, ctx, e, lm.w * 0.46 * jk, &p.metal, DiscOpts::default());
        }
        _ => {
            bone(out, ctx, hip, e, 7.0 * sc, &p.metal);
            bone(out, ctx, e, t, lm.w * 1.05, &p.paint);
            disc(out, ctx, e, 5.4 * sc * jk, &p.metal, DiscOpts::default());
        }
    }
}

fn side_class(side: f64) -> &'static str {
    if side < 0.0 {
        "rb-leg rb-leg-l"
    } else {
        "rb-leg rb-leg-r"
    }
}

impl Gear {
    /// The points one mood's gear reaches, in ground coordinates.
    pub(crate) fn pts(&self, c: &Robot, md: &MoodSpec, pose: &Pose) -> Vec<P2> {
        let mut out = Vec::new();
        match &self.shape {
            Shape::Legs { ah, hx, fx, l, .. } => {
                for side in [-1.0, 1.0] {
                    let hip = to_g(pose, under(c, side * hx));
                    let a = ankle_of(hip, side * fx, *ah, l * 0.995);
                    out.extend(disc_pts(a[0], a[1] + ah * 0.5, 12.0 * self.fs));
                    out.push([hip[0] + side * (l * 0.3 + 6.0), (hip[1] + a[1]) / 2.0]);
                }
            }
            Shape::Multi { legs, lw, .. } => {
                for side in [-1.0, 1.0] {
                    for lg in legs {
                        let (_, a, e, _) = multi_parts(pose, side, lg);
                        out.extend(disc_pts(e[0], e[1], *lw));
                        out.extend(disc_pts(a[0], a[1], 5.0));
                    }
                }
            }
            Shape::Tripod { ah, hx, fx, l, .. } => {
                for side in [-1.0, 1.0] {
                    let hip = to_g(pose, under(c, side * hx));
                    let a = ankle_of(hip, side * fx, *ah, l * 0.995);
                    out.extend(disc_pts(a[0], a[1] + ah * 0.5, 12.0 * self.fs));
                }
            }
            Shape::Treads { th, tw, tx, .. } => {
                out.push([-tx - tw / 2.0, -th]);
                out.push([tx + tw / 2.0, 0.0]);
            }
            Shape::Track { th, tw, .. } => {
                out.push([-tw, -th]);
                out.push([*tw, 0.0]);
            }
            Shape::Wheels { r, tw, wx, .. } => {
                out.push([-wx - tw / 2.0, -2.0 * r]);
                out.push([wx + tw / 2.0, 0.0]);
            }
            Shape::Discs { r, wx, .. } => {
                out.push([-wx - r, -2.0 * r]);
                out.push([wx + r, 0.0]);
            }
            Shape::Mono { r, .. } => {
                out.push([-r, -2.0 * r]);
                out.push([*r, 0.0]);
            }
            Shape::Hover { nh, .. } => {
                let flame = md.thrust
                    * if self.kind == "jet" {
                        self.rise * 0.62
                    } else {
                        self.rise * 0.7
                    };
                out.push(to_g(pose, [-self.wl * 0.8, 0.0]));
                out.push(to_g(pose, [self.wl * 0.8, nh + flame]));
            }
            Shape::Pogo { ah, amp, l, .. } => {
                let hip = to_g(pose, [0.0, -1.0]);
                let a = ankle_of(hip, hip[0] * 0.4, *ah, *l);
                out.extend(disc_pts(a[0], a[1] + ah * 0.5, 12.0 * self.fs));
                out.push([a[0] - amp - 3.0, a[1] - 6.0]);
                out.push([a[0] + amp + 3.0, a[1] - 6.0]);
            }
            Shape::Hoses { legs, .. } => {
                out.push([-legs[1][1] - 11.0, -8.0]);
                out.push([legs[1][1] + 11.0, 0.0]);
            }
        }
        out
    }

    pub(crate) fn draw(&self, out: &mut Out, ctx: &Ctx, c: &Robot, md: &MoodSpec, pose: &Pose) {
        let p = &c.p;
        let sc = c.scale;
        let wl = self.wl;
        match &self.shape {
            Shape::Legs {
                ah,
                hx,
                fx,
                lm,
                rest,
                l,
                amp,
            } => {
                for side in [-1.0, 1.0] {
                    let hip = to_g(pose, under(c, side * hx));
                    let a = ankle_of(hip, side * fx, *ah, l * 0.995);
                    out.group(side_class(side), &[], |out| {
                        match self.kind {
                            "biped" => leg(out, ctx, c, hip, a, *l, *lm, |e| side * e[0]),
                            "piston" => {
                                let d = hyp(a[0] - hip[0], a[1] - hip[1]);
                                bone(out, ctx, hip, a, lm.w * 0.46, &p.metal);
                                bone(
                                    out,
                                    ctx,
                                    hip,
                                    mix(hip, a, min(0.86, rest * 0.56 / d)),
                                    lm.w,
                                    &p.paint,
                                );
                            }
                            _ => rod(out, ctx, &zigzag(hip, a, 6, *amp), 3.3 * sc, &p.metal, true),
                        }
                        foot(out, ctx, c, self.foot_kind, a[0], a[1] + ah, self.fs);
                    });
                }
            }
            Shape::Multi { legs, lw, jk, tip } => {
                // The legs mounted highest are furthest back, so they are drawn first.
                for i in (0..legs.len()).rev() {
                    for side in [-1.0, 1.0] {
                        let (hip, _, e, t) = multi_parts(pose, side, &legs[i]);
                        let class = match (side < 0.0, i) {
                            (true, 0) => "rb-leg rb-leg-l1",
                            (true, 1) => "rb-leg rb-leg-l2",
                            (true, _) => "rb-leg rb-leg-l3",
                            (false, 0) => "rb-leg rb-leg-r1",
                            (false, 1) => "rb-leg rb-leg-r2",
                            (false, _) => "rb-leg rb-leg-r3",
                        };
                        out.group(class, &[], |out| {
                            bone(out, ctx, hip, e, *lw, &p.metal);
                            bone(out, ctx, e, t, lw * 0.86, &p.metal);
                            disc(out, ctx, e, lw * 0.9 * jk, &p.metal, DiscOpts::default());
                            if *tip {
                                plate_k(
                                    out,
                                    ctx,
                                    &rrect(
                                        t[0] - 6.0 * sc,
                                        t[1] - 1.0,
                                        t[0] + 6.0 * sc,
                                        t[1] + 3.0,
                                        1.8,
                                    ),
                                    &p.metal,
                                    0.8,
                                );
                            } else {
                                disc(out, ctx, [t[0], t[1] + 0.5], 3.3 * sc, &p.metal, NO_DOT);
                            }
                        });
                    }
                }
            }
            Shape::Tripod {
                ah,
                jk,
                lw,
                hx,
                fx,
                l,
            } => {
                for side in [-1.0, 1.0] {
                    let hip = to_g(pose, under(c, side * hx));
                    let a = ankle_of(hip, side * fx, *ah, l * 0.995);
                    out.group(side_class(side), &[], |out| {
                        let lm = Limb {
                            build: "rod",
                            w: *lw,
                            joint: *jk,
                        };
                        leg(out, ctx, c, hip, a, *l, lm, |e| side * e[0]);
                        foot(out, ctx, c, self.foot_kind, a[0], a[1] + ah, self.fs * 0.9);
                    });
                }
                let hip = to_g(pose, [0.0, -1.0]);
                let a = ankle_of(hip, hip[0] * 0.5, *ah, self.rise - ah + 6.0);
                out.group("rb-leg rb-leg-c", &[], |out| {
                    bone(out, ctx, hip, a, lw * 1.5, &p.metal);
                    foot(out, ctx, c, self.foot_kind, a[0], a[1] + ah, self.fs);
                });
            }
            Shape::Treads { th, tw, tx, gap } => {
                let hip = to_g(pose, [0.0, -3.0]);
                bone(
                    out,
                    ctx,
                    [-tx, -th * 0.5],
                    [*tx, -th * 0.5],
                    7.0 * sc,
                    &p.metal,
                );
                bone(
                    out,
                    ctx,
                    [0.0, -th * 0.5],
                    hip,
                    max(11.0, wl * 0.36),
                    &p.metal,
                );
                for side in [-1.0, 1.0] {
                    let x0 = side * tx - tw / 2.0;
                    let pts = rrect(x0, -th, x0 + tw, 0.0, min(tw * 0.42, 8.0));
                    let mut lugs = Path::new();
                    let mut y = -th + 5.0;
                    while y < -3.0 {
                        lugs.move_to([x0 + 1.5, y]);
                        lugs.line_to([x0 + tw - 1.5, y]);
                        y += gap;
                    }
                    let class = if side < 0.0 {
                        "rb-tread rb-tread-l"
                    } else {
                        "rb-tread rb-tread-r"
                    };
                    out.group(class, &[], |out| {
                        plate(out, ctx, &pts, &p.rubber);
                        seam(out, ctx, &lugs, p.rubber.lite, 1.5, None);
                    });
                }
            }
            Shape::Track {
                th,
                tw,
                wheels,
                wr,
                gap,
            } => {
                let hip = to_g(pose, [0.0, -3.0]);
                out.group("rb-track", &[], |out| {
                    bone(
                        out,
                        ctx,
                        [0.0, -th + 2.0],
                        hip,
                        max(12.0, wl * 0.5),
                        &p.metal,
                    );
                    plate(
                        out,
                        ctx,
                        &rrect(-tw * 0.72, -th - 3.5, tw * 0.72, -th + 3.0, 2.0),
                        &p.metal,
                    );
                    plate(out, ctx, &rrect(-tw, -th, *tw, 0.0, th / 2.0), &p.rubber);
                    if ctx.shade {
                        out.put(
                            El::path(&poly_path(&rrect(
                                -tw + 4.6,
                                -th + 4.6,
                                tw - 4.6,
                                -4.6,
                                th / 2.0 - 4.6,
                            )))
                            .fill(p.metal.dark),
                        );
                    }
                    for i in 0..*wheels {
                        let x = if *wheels == 1 {
                            0.0
                        } else {
                            lerp(
                                -tw + th / 2.0,
                                tw - th / 2.0,
                                f64::from(i) / f64::from(wheels - 1),
                            )
                        };
                        disc(
                            out,
                            ctx,
                            [x, -th / 2.0],
                            *wr,
                            &p.metal,
                            DiscOpts {
                                line_k: 0.7,
                                ..DiscOpts::default()
                            },
                        );
                    }
                    if ctx.fine {
                        let mut lugs = Path::new();
                        let mut x = -tw + th / 2.0;
                        while x <= tw - th / 2.0 + 0.1 {
                            lugs.move_to([x, -th + 0.6]);
                            lugs.line_to([x, -th + 3.6]);
                            lugs.move_to([x, -0.6]);
                            lugs.line_to([x, -3.6]);
                            x += gap;
                        }
                        seam(out, ctx, &lugs, p.rubber.lite, 1.6, None);
                    }
                });
            }
            Shape::Wheels { r, tw, wx, gap } => {
                let hip = to_g(pose, [0.0, -3.0]);
                bone(out, ctx, [-wx, -r], [*wx, -r], 6.5 * sc, &p.metal);
                bone(out, ctx, [0.0, -r], hip, max(11.0, wl * 0.36), &p.metal);
                for side in [-1.0, 1.0] {
                    let x0 = side * wx - tw / 2.0;
                    let mut tread = Path::new();
                    let mut y = -2.0 * r + 4.5;
                    while y < -3.0 {
                        tread.move_to([x0 + 1.5, y]);
                        tread.line_to([x0 + tw - 1.5, y]);
                        y += gap;
                    }
                    let class = if side < 0.0 {
                        "rb-wheel rb-wheel-l"
                    } else {
                        "rb-wheel rb-wheel-r"
                    };
                    out.group(class, &[], |out| {
                        plate(
                            out,
                            ctx,
                            &rrect(x0, -2.0 * r, x0 + tw, 0.0, min(tw * 0.45, 7.0)),
                            &p.rubber,
                        );
                        seam(out, ctx, &tread, p.rubber.lite, 1.4, None);
                    });
                }
            }
            Shape::Discs { r, wx, spokes } => {
                let hip = to_g(pose, [0.0, -3.0]);
                bone(out, ctx, [-wx, -r], [*wx, -r], 6.5 * sc, &p.metal);
                if self.rise > r + 5.0 {
                    bone(out, ctx, [0.0, -r], hip, max(11.0, wl * 0.36), &p.metal);
                }
                for side in [-1.0, 1.0] {
                    let ctr = [side * wx, -r];
                    let class = if side < 0.0 {
                        "rb-wheel rb-wheel-l"
                    } else {
                        "rb-wheel rb-wheel-r"
                    };
                    out.group(class, &[], |out| {
                        disc(
                            out,
                            ctx,
                            ctr,
                            *r,
                            &p.rubber,
                            DiscOpts {
                                keep: true,
                                dot: false,
                                line_k: 1.0,
                                fill: None,
                            },
                        );
                        disc(
                            out,
                            ctx,
                            ctr,
                            r * 0.56,
                            &p.metal,
                            DiscOpts {
                                keep: true,
                                dot: false,
                                ..DiscOpts::default()
                            },
                        );
                        if ctx.fine {
                            for i in 0..*spokes {
                                let u = up_dir(f64::from(i) * 360.0 / f64::from(*spokes));
                                out.put(
                                    El::circle(
                                        [ctr[0] + u[0] * r * 0.34, ctr[1] + u[1] * r * 0.34],
                                        max(1.3, r * 0.08),
                                    )
                                    .fill(p.metal.dark),
                                );
                            }
                        }
                    });
                }
            }
            Shape::Mono { r, hx, band } => {
                let ctr = [0.0, -r];
                let keep = DiscOpts {
                    keep: true,
                    dot: false,
                    ..DiscOpts::default()
                };
                if self.kind == "ball" {
                    out.group("rb-ball", &[], |out| {
                        disc(
                            out,
                            ctx,
                            ctr,
                            *r,
                            &p.metal,
                            DiscOpts {
                                line_k: 1.0,
                                ..keep
                            },
                        );
                        if ctx.shade {
                            let mut d = Path::new();
                            d.move_to([-r * 0.86, -r * 0.5]);
                            d.quad_to(
                                [-r * 0.86, -r * 0.5],
                                [0.0, -r * 0.05],
                                [r * 0.86, -r * 0.5],
                            );
                            out.put(
                                El::path(&d)
                                    .stroke(
                                        if *band { p.accent.base } else { p.metal.dark },
                                        if *band { r * 0.2 } else { 1.6 },
                                    )
                                    .round_cap(),
                            );
                        }
                    });
                    let a = to_g(pose, under(c, -hx * 1.2));
                    let b = to_g(pose, under(c, hx * 1.2));
                    bone(out, ctx, a, b, 5.5 * sc, &p.metal);
                    return;
                }
                out.group("rb-wheel rb-wheel-c", &[], |out| {
                    disc(
                        out,
                        ctx,
                        ctr,
                        *r,
                        &p.rubber,
                        DiscOpts {
                            line_k: 1.0,
                            ..keep
                        },
                    );
                    disc(out, ctx, ctr, r * 0.6, &p.metal, keep);
                    for side in [-1.0, 1.0] {
                        bone(
                            out,
                            ctx,
                            to_g(pose, under(c, side * hx)),
                            ctr,
                            5.2 * sc,
                            &p.metal,
                        );
                    }
                    disc(
                        out,
                        ctx,
                        ctr,
                        max(3.4, r * 0.2),
                        &p.metal,
                        DiscOpts::default(),
                    );
                });
            }
            Shape::Hover {
                top,
                bot,
                nh,
                rings,
            } => {
                let under_top = under(c, *top)[1];
                out.group("rb-hover", &pose.uops, |out| {
                    if self.kind == "jet" {
                        let len = md.thrust * self.rise * 0.62;
                        if len > 2.0 {
                            let mut d = Path::new();
                            d.move_to([-bot * 0.92, nh - 1.0]);
                            d.quad_to(
                                [-bot * 0.92, nh - 1.0],
                                [-bot * 0.5, nh + len * 0.6],
                                [0.0, nh + len],
                            );
                            d.quad_to(
                                [0.0, nh + len],
                                [bot * 0.5, nh + len * 0.6],
                                [bot * 0.92, nh - 1.0],
                            );
                            d.close();
                            out.put(El::path(&d).class("rb-flame").fill(p.glow).opacity(0.92));
                            let mut core = Path::new();
                            core.move_to([-bot * 0.5, nh - 1.0]);
                            core.quad_to(
                                [-bot * 0.5, nh - 1.0],
                                [-bot * 0.26, nh + len * 0.34],
                                [0.0, nh + len * 0.58],
                            );
                            core.quad_to(
                                [0.0, nh + len * 0.58],
                                [bot * 0.26, nh + len * 0.34],
                                [bot * 0.5, nh - 1.0],
                            );
                            core.close();
                            out.put(El::path(&core).class("rb-flame-core").fill(p.glow_lite));
                        }
                        plate(
                            out,
                            ctx,
                            &[
                                cp(-bot, *nh, 1.5),
                                cp(-top, under_top, 1.5),
                                cp(*top, under_top, 1.5),
                                cp(*bot, *nh, 1.5),
                            ],
                            &p.metal,
                        );
                    } else {
                        if md.thrust > 0.05 {
                            for i in 0..*rings {
                                let fi = f64::from(i);
                                out.put(
                                    El::ellipse(
                                        0.0,
                                        nh + 7.0 + fi * 9.5 * md.thrust,
                                        bot * (1.0 - 0.2 * fi),
                                        3.6 - 0.5 * fi,
                                    )
                                    .class("rb-ring")
                                    .stroke(p.glow, max(2.6, ctx.line_w * 0.8))
                                    .opacity(0.95 - 0.28 * fi),
                                );
                            }
                        }
                        plate(
                            out,
                            ctx,
                            &[
                                cp(-bot, *nh, 2.0),
                                cp(-top, under_top, 1.5),
                                cp(*top, under_top, 1.5),
                                cp(*bot, *nh, 2.0),
                            ],
                            &p.metal,
                        );
                    }
                });
            }
            Shape::Pogo { ah, amp, n, l } => {
                let hip = to_g(pose, [0.0, -1.0]);
                let a = ankle_of(hip, hip[0] * 0.4, *ah, *l);
                out.group("rb-leg rb-leg-c", &[], |out| {
                    rod(
                        out,
                        ctx,
                        &zigzag(hip, a, *n, *amp),
                        4.0 * sc,
                        &p.metal,
                        true,
                    );
                    foot(out, ctx, c, self.foot_kind, a[0], a[1] + ah, self.fs * 1.15);
                });
            }
            Shape::Hoses { w, mat, legs } => {
                for i in (0..2).rev() {
                    for side in [-1.0, 1.0] {
                        let hip = to_g(pose, under(c, side * legs[i][0]));
                        let a = [side * legs[i][1], -7.0 * sc];
                        let class = match (side < 0.0, i) {
                            (true, 0) => "rb-leg rb-leg-l1",
                            (true, _) => "rb-leg rb-leg-l2",
                            (false, 0) => "rb-leg rb-leg-r1",
                            (false, _) => "rb-leg rb-leg-r2",
                        };
                        out.group(class, &[], |out| {
                            hose(
                                out,
                                ctx,
                                hip,
                                [hip[0] + side * 2.0, a[1] - 1.0],
                                a,
                                *w,
                                mat,
                                7,
                            );
                            plate_k(
                                out,
                                ctx,
                                &[
                                    cp(a[0] - 9.5 * sc, 0.0, 2.0),
                                    cp(a[0] - 3.5 * sc, -8.0 * sc, 1.5),
                                    cp(a[0] + 3.5 * sc, -8.0 * sc, 1.5),
                                    cp(a[0] + 9.5 * sc, 0.0, 2.0),
                                ],
                                &p.rubber,
                                0.85,
                            );
                        });
                    }
                }
            }
        }
    }
}

/// One leg of four or six: its hip, its ankle, its knee and its foot.
fn multi_parts(pose: &Pose, side: f64, lg: &MultiLeg) -> (P2, P2, P2, P2) {
    let hip = to_g(pose, [side * lg.hx, lg.yh]);
    let a = ankle_of(hip, side * lg.fx, 3.0, (lg.l1 + lg.l2) * 0.995);
    let (e, t) = joint2(hip, a, lg.l1, lg.l2, |e| side * e[0] - e[1]);
    (hip, a, e, t)
}
