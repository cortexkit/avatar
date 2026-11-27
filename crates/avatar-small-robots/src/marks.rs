//! Markings and what is set into a hull: the chest fitting, and the paint
//! scheme, seams, rivets, markings and wear of the head.

use crate::draw::{disc, lamp, plate_k, rays, rivets, seam, Ctx, DiscOpts, El, Out, NO_DOT};
use crate::face::Face;
use crate::geom::{line_path, poly_path, rrect, up_dir, Path};
use crate::hull::Hull;
use crate::kit::{hex, lerp, max, min, min3, Reader, Status, P2};
use crate::mood::MoodSpec;
use crate::robot::Robot;

const CHEST: usize = 1092;
const DECOR: usize = 1096;
const WEAR: usize = 1108;

/// Seven-segment digits: which of the segments a..g each digit lights.
const DIGITS: [&str; 10] = [
    "abcdef", "bc", "abged", "abgcd", "fgbc", "afgcd", "afgecd", "abc", "abcdefg", "abfgcd",
];

fn segment(s: char) -> [f64; 4] {
    match s {
        'a' => [0.0, 0.0, 1.0, 0.0],
        'b' => [1.0, 0.0, 1.0, 1.0],
        'c' => [1.0, 1.0, 1.0, 2.0],
        'd' => [0.0, 2.0, 1.0, 2.0],
        'e' => [0.0, 1.0, 0.0, 2.0],
        'f' => [0.0, 0.0, 0.0, 1.0],
        _ => [0.0, 1.0, 1.0, 1.0],
    }
}

/// A number stencilled in seven-segment digits `h` tall, centred on (x, y).
fn digits_path(text: &[u32], x: f64, y: f64, h: f64) -> Path {
    let w = h * 0.5;
    let gap = h * 0.26;
    let n = text.len() as f64;
    let total = n * w + (n - 1.0) * gap;
    let mut d = Path::new();
    let mut x0 = x - total / 2.0;
    for digit in text {
        for s in DIGITS[*digit as usize].chars() {
            let g = segment(s);
            d.move_to([x0 + g[0] * w, y - h / 2.0 + g[1] * h / 2.0]);
            d.line_to([x0 + g[2] * w, y - h / 2.0 + g[3] * h / 2.0]);
        }
        x0 += w + gap;
    }
    d
}

/// Small shapes painted on a hull, as outlines in a square from -1 to 1.
fn decal_outline(kind: &str) -> &'static [P2] {
    match kind {
        "star" => &[
            [0.0, -1.0],
            [0.235, -0.324],
            [0.951, -0.309],
            [0.38, 0.124],
            [0.588, 0.809],
            [0.0, 0.4],
            [-0.588, 0.809],
            [-0.38, 0.124],
            [-0.951, -0.309],
            [-0.235, -0.324],
        ],
        "bolt" => &[
            [0.25, -1.0],
            [-0.55, 0.12],
            [-0.05, 0.12],
            [-0.3, 1.0],
            [0.55, -0.22],
            [0.05, -0.22],
        ],
        "arrow" => &[
            [0.0, -1.0],
            [0.75, -0.1],
            [0.28, -0.1],
            [0.28, 1.0],
            [-0.28, 1.0],
            [-0.28, -0.1],
            [-0.75, -0.1],
        ],
        _ => &[[0.0, -0.9], [0.95, 0.75], [-0.95, 0.75]],
    }
}

const DECAL_NAMES: [&str; 8] = [
    "none", "none", "number", "star", "bars", "arrow", "bolt", "delta",
];

/// What is set into the chassis, or into a pod's hull under its face.
pub const CHEST_NAMES: [&str; 10] = [
    "none", "light", "panel", "vent", "gauge", "leds", "hatch", "stencil", "hazard", "grille",
];
const CHEST_WEIGHTS: [u32; 10] = [4, 5, 4, 3, 3, 4, 3, 3, 1, 2];

/// The fittings that would make a hull with a screen read as a monitor or
/// a television; the hull that carries a screen never carries them.
fn under_screen(kind: &str) -> bool {
    matches!(kind, "panel" | "vent" | "leds" | "hatch" | "grille")
}

#[derive(Clone, Debug)]
pub(crate) struct Chest {
    pub(crate) kind: &'static str,
    y: f64,
    w: f64,
    h: f64,
    n: u32,
    text: Vec<u32>,
}

impl Chest {
    pub(crate) fn none() -> Self {
        Self {
            kind: "none",
            y: 0.0,
            w: 0.0,
            h: 0.0,
            n: 0,
            text: Vec::new(),
        }
    }
}

pub(crate) fn chest_draw(out: &mut Out, ctx: &Ctx, c: &Robot, md: &MoodSpec) {
    let ch = &c.chest;
    if ch.kind == "none" {
        return;
    }
    let p = &c.p;
    let (y, w, h) = (ch.y, ch.w, ch.h);
    let col = p.status(md.status);
    let line = if p.dark { p.paint.lite } else { p.paint.line };
    let class = match ch.kind {
        "light" => "rb-chest rb-chest-light",
        "panel" => "rb-chest rb-chest-panel",
        "vent" => "rb-chest rb-chest-vent",
        "gauge" => "rb-chest rb-chest-gauge",
        "leds" => "rb-chest rb-chest-leds",
        "hatch" => "rb-chest rb-chest-hatch",
        "stencil" => "rb-chest rb-chest-stencil",
        "hazard" => "rb-chest rb-chest-hazard",
        _ => "rb-chest rb-chest-grille",
    };
    out.group(class, &[], |out| match ch.kind {
        "light" => {
            let r = min3(w, h, 9.0);
            if ctx.fine {
                disc(out, ctx, [0.0, y], r + 2.4, &p.metal, NO_DOT);
            }
            lamp(out, ctx, [0.0, y], r, col, &p.metal);
            if md.rays {
                rays(
                    out,
                    ctx,
                    [0.0, y],
                    r + 2.0,
                    col,
                    &[-120.0, -60.0, 0.0, 60.0, 120.0],
                );
            }
        }
        "panel" => {
            seam(
                out,
                ctx,
                &poly_path(&rrect(-w, y - h, w, y + h, 3.0)),
                line,
                1.4,
                None,
            );
            rivets(
                out,
                ctx,
                &[
                    [-w + 3.5, y - h + 3.5],
                    [w - 3.5, y - h + 3.5],
                    [-w + 3.5, y + h - 3.5],
                    [w - 3.5, y + h - 3.5],
                ],
                &p.metal,
            );
        }
        "vent" => {
            let mut d = Path::new();
            for i in 0..4 {
                let yy = y - h + (f64::from(i) + 0.5) * h / 2.0;
                d.move_to([-w + 2.0, yy]);
                d.line_to([w - 2.0, yy]);
            }
            out.put(
                El::path(&d)
                    .class("rb-vent")
                    .stroke(
                        if p.dark { p.paint.line } else { p.paint.dark },
                        max(2.2, ctx.line_w * 0.6),
                    )
                    .round_cap(),
            );
            seam(out, ctx, &d, p.paint.line, 0.9, Some(0.7));
        }
        "gauge" => {
            let r = min3(w, h + 2.0, 10.0);
            let u = up_dir(md.needle * 62.0);
            disc(
                out,
                ctx,
                [0.0, y],
                r,
                &p.metal,
                DiscOpts {
                    keep: true,
                    fill: Some(p.white),
                    dot: false,
                    ..DiscOpts::default()
                },
            );
            if ctx.size >= 48.0 {
                out.put(
                    El::path(&line_path(
                        &[
                            [0.0, y + r * 0.25],
                            [u[0] * r * 0.8, y + r * 0.25 + u[1] * r * 0.8],
                        ],
                        false,
                    ))
                    .stroke(p.status(Status::Err), max(1.6, ctx.line_w * 0.5))
                    .round_cap(),
                );
            }
        }
        "leds" => {
            let n = f64::from(ch.n);
            let r = min(3.4, (w - 2.0) / n);
            for i in 0..ch.n {
                let key = md.leds[(i % 4) as usize];
                let x = (f64::from(i) - (n - 1.0) / 2.0) * (2.0 * w - 2.0 * r - 2.0)
                    / max(1.0, n - 1.0);
                lamp(out, ctx, [x, y], r, p.status(key), &p.metal);
            }
        }
        "hatch" => {
            seam(
                out,
                ctx,
                &poly_path(&rrect(-w, y - h, w, y + h, 2.0)),
                line,
                1.4,
                None,
            );
            seam(
                out,
                ctx,
                &line_path(&[[w - 5.0, y - 2.5], [w - 5.0, y + 2.5]], false),
                line,
                2.0,
                None,
            );
            rivets(
                out,
                ctx,
                &[[-w - 0.5, y - h * 0.5], [-w - 0.5, y + h * 0.5]],
                &p.metal,
            );
        }
        "stencil" => seam(
            out,
            ctx,
            &digits_path(&ch.text, 0.0, y, min(2.0 * h, 15.0)),
            if p.dark { p.paint.lite } else { p.paint.line },
            1.9,
            None,
        ),
        "hazard" => {
            let d = poly_path(&rrect(-w, y - h * 0.62, w, y + h * 0.62, 2.0));
            out.put(El::path(&d).fill(hex(0xf6c531)));
            if ctx.fine {
                let mut st = Path::new();
                let mut x = -w - h;
                while x < w + h {
                    st.move_to([x, y + h]);
                    st.line_to([x + h * 1.6, y - h]);
                    x += 7.0;
                }
                out.put(
                    El::path(&st)
                        .stroke(hex(0x2a262d), 2.6)
                        .clip(crate::draw::clip_of(&d)),
                );
            }
            out.put(
                El::path(&d)
                    .stroke(hex(0x2a262d), min(ctx.line_w * 0.6, 2.4))
                    .round_join(),
            );
        }
        _ => {
            if !ctx.fine {
                return;
            }
            for i in 0..3 {
                for j in 0..2 {
                    out.put(
                        El::circle(
                            [
                                (f64::from(i) - 1.0) * w * 0.6,
                                y + (f64::from(j) - 0.5) * h * 0.9,
                            ],
                            1.7,
                        )
                        .fill(if p.dark {
                            p.paint.line
                        } else {
                            p.paint.dark
                        }),
                    );
                }
            }
        }
    });
}

/// Chooses what is set into the chassis, given the room there is: a middle
/// height and the half-width and half-height of the room.
pub(crate) fn plug_chest(r: &Reader, pod: bool, f: &Face, y: f64, w: f64, h: f64) -> Chest {
    let o = CHEST;
    if w < 6.0 || h < 4.5 {
        return Chest::none();
    }
    let mut kind = CHEST_NAMES[r.pick(o, 0, &CHEST_WEIGHTS, 0)];
    // A pod's hull is its head, so what is set into it sits right under its face.
    if pod && f.screen.is_some() && under_screen(kind) {
        kind = "none";
    }
    if kind == "none" {
        return Chest::none();
    }
    let k = r.lin(o + 2, 0, 2, 0.76, 1.0);
    let mut chest = Chest {
        kind,
        y,
        w: min(w, 20.0) * k,
        h: min(h, 13.0) * k,
        ..Chest::none()
    };
    if kind == "leds" {
        chest.n = 3 + r.raw(o + 1, 0, 1);
    }
    if kind == "stencil" {
        chest.text = vec![r.raw(o + 1, 1, 3) + 1, r.raw(o + 1, 4, 3)];
    }
    chest
}

#[derive(Clone, Debug)]
struct Row {
    y: f64,
    seam: bool,
    rivets: bool,
}

/// A marking or a mark of wear: what it is, where, and how large.
#[derive(Clone, Debug)]
struct Spot {
    kind: &'static str,
    x: f64,
    y: f64,
    s: f64,
    text: Vec<u32>,
}

/// The seams, rivets, markings and wear of a head.
#[derive(Clone, Debug)]
pub(crate) struct Decor {
    rows: Vec<Row>,
    scheme: &'static str,
    top: f64,
    stripe_w: f64,
    stripe_clear: f64,
    dip_y: f64,
    six: bool,
    decal: Option<Spot>,
    wear: Option<Spot>,
}

impl Decor {
    pub(crate) fn plain() -> Self {
        Self {
            rows: Vec::new(),
            scheme: "plain",
            top: 0.0,
            stripe_w: 0.0,
            stripe_clear: 0.0,
            dip_y: 0.0,
            six: false,
            decal: None,
            wear: None,
        }
    }
}

/// The seams, rivets and markings of a head, drawn between its paint and
/// its outline. All but the paint scheme is for the large sizes.
pub(crate) fn head_marks(out: &mut Out, ctx: &Ctx, c: &Robot) {
    let d = &c.decor;
    let p = &c.p;
    let hull = &c.head;
    let line = if p.dark { p.paint.lite } else { p.paint.line };
    let clip = || crate::draw::clip_of(&poly_path(&hull.pts));
    // A stripe of the second paint down the middle of the crown, or a dipped foot.
    if d.scheme == "stripe" {
        out.put(
            El::rect(
                -d.stripe_w,
                -hull.h - 2.0,
                2.0 * d.stripe_w,
                hull.h + 2.0 + d.top - 1.0,
            )
            .class("rb-stripe")
            .fill(p.accent.base)
            .clip(clip()),
        );
    }
    if d.scheme == "dip" {
        out.put(
            El::rect(-hull.w - 2.0, d.dip_y, 2.0 * hull.w + 4.0, -d.dip_y + 3.0)
                .class("rb-dip")
                .fill(p.accent.base)
                .clip(clip()),
        );
    }
    if !ctx.fine {
        return;
    }
    if hull.kind == "can" {
        let e = lerp(6.0, 10.0, hull.a);
        out.put(
            El::ellipse(0.0, -hull.h + e, hull.w - 1.5, e - 1.0)
                .class("rb-lid")
                .fill(p.paint.lite)
                .stroke(p.paint.line, 1.4),
        );
    }
    for row in &d.rows {
        let hw = hull.hw_at(row.y) - 5.0;
        if row.seam {
            seam(
                out,
                ctx,
                &line_path(&[[-hw - 3.5, row.y], [hw + 3.5, row.y]], false),
                line,
                1.3,
                Some(0.75),
            );
        }
        if row.rivets {
            let ks: &[f64] = if d.six {
                &[-0.9, -0.62, -0.34, 0.34, 0.62, 0.9]
            } else {
                &[-0.86, -0.5, 0.5, 0.86]
            };
            let dy = if row.seam {
                if row.y < d.top {
                    4.5
                } else {
                    -4.5
                }
            } else {
                0.0
            };
            let list: Vec<P2> = ks
                .iter()
                .map(|k| [k * hw, row.y + dy])
                .filter(|q| d.scheme != "stripe" || q[0].abs() > d.stripe_clear + 3.0)
                .collect();
            rivets(out, ctx, &list, &p.metal);
        }
    }
    if let Some(v) = c.side_vents {
        let [x, y, k] = v;
        let mut path = Path::new();
        for side in [-1.0, 1.0] {
            for i in -1..=1 {
                let yy = y + f64::from(i) * 4.5 * k;
                path.move_to([side * (x - 5.0), yy]);
                path.line_to([side * (x - 5.0 - 8.0 * k), yy]);
            }
        }
        seam(out, ctx, &path, p.paint.line, 2.2, None);
    }
    if let Some(m) = &d.decal {
        let ink = p.accent.base;
        match m.kind {
            "number" => seam(
                out,
                ctx,
                &digits_path(&m.text, m.x, m.y, m.s * 1.7),
                line,
                1.7,
                None,
            ),
            "bars" => {
                let mut path = Path::new();
                for k in [-0.6, 0.0, 0.6] {
                    path.move_to([m.x - m.s, m.y + k * m.s]);
                    path.line_to([m.x + m.s, m.y + k * m.s]);
                }
                seam(out, ctx, &path, ink, 2.4, None);
            }
            kind => {
                let pts: Vec<P2> = decal_outline(kind)
                    .iter()
                    .map(|q| [m.x + q[0] * m.s, m.y + q[1] * m.s])
                    .collect();
                out.put(
                    El::path(&line_path(&pts, true))
                        .class("rb-decal")
                        .fill(ink)
                        .stroke(p.accent.line, 1.0)
                        .round_join(),
                );
            }
        }
    }
    if let Some(m) = &d.wear {
        let (x, y) = (m.x, m.y);
        match m.kind {
            "dent" => {
                let mut a = Path::new();
                a.move_to([x - 5.0, y - 2.0]);
                a.quad_to([x - 5.0, y - 2.0], [x, y + 5.0], [x + 5.0, y - 2.0]);
                a.move_to([x - 2.5, y + 3.5]);
                a.quad_to([x - 2.5, y + 3.5], [x + 1.0, y + 6.0], [x + 4.0, y + 3.0]);
                seam(out, ctx, &a, p.paint.dark, 1.8, None);
                let mut b = Path::new();
                b.move_to([x - 4.0, y - 4.0]);
                b.quad_to([x - 4.0, y - 4.0], [x, y - 1.5], [x + 4.0, y - 4.0]);
                seam(out, ctx, &b, p.paint.lite, 1.3, None);
            }
            "patch" => {
                plate_k(
                    out,
                    ctx,
                    &rrect(x - 6.5, y - 4.5, x + 6.5, y + 4.5, 1.5),
                    &p.metal,
                    0.6,
                );
                rivets(
                    out,
                    ctx,
                    &[
                        [x - 4.3, y - 2.3],
                        [x + 4.3, y - 2.3],
                        [x - 4.3, y + 2.3],
                        [x + 4.3, y + 2.3],
                    ],
                    &p.metal,
                );
            }
            _ => {
                let mut a = Path::new();
                a.move_to([x - 6.0, y + 3.0]);
                a.line_to([x + 2.0, y - 4.0]);
                a.move_to([x - 1.0, y + 4.5]);
                a.line_to([x + 6.0, y - 2.0]);
                a.move_to([x - 5.0, y - 2.0]);
                a.line_to([x - 2.0, y - 4.5]);
                seam(out, ctx, &a, p.paint.lite, 1.2, Some(0.9));
            }
        }
    }
}

/// How far up and down the head the face reaches, with a little room round it.
pub(crate) struct Extent {
    pub(crate) top: f64,
    pub(crate) bot: f64,
}

pub(crate) fn face_extent(f: &Face) -> Extent {
    let b = f.bx;
    if f.screen.is_some() {
        return Extent {
            top: b.cy - b.fh - 5.0,
            bot: b.cy + b.fh + 5.0,
        };
    }
    let mouth = f.mouth.kind != crate::face::MouthKind::None;
    let mut top = if mouth { f.mouth.y - 12.0 } else { b.cy };
    let mut bot = if mouth { f.mouth.y + 11.0 } else { b.cy };
    for e in &f.eyes {
        top = min(top, e.y - e.ry * 1.16 - if f.brow_room { 9.0 } else { 2.0 });
        bot = max(bot, e.y + e.ry * 1.16 + 2.0);
    }
    Extent { top, bot }
}

/// Chooses the seams, rivets, markings and wear of a head from the room
/// its face leaves.
pub(crate) fn plug_decor(
    r: &Reader,
    hull: &Hull,
    f: &Face,
    pod: bool,
    has_body: bool,
    chest: &Chest,
    ext: &Extent,
) -> Decor {
    let o = DECOR;
    let mut d = Decor::plain();
    let face_top = ext.top;
    let face_bot = ext.bot;
    // A pod with something set into it under the face has no free band there.
    let top_room = face_top - hull.top_at(0.0);
    let bot_room = if !has_body && chest.kind != "none" {
        0.0
    } else {
        -face_bot
    };
    d.top = face_top;
    let can_stripe = top_room >= 9.0;
    let can_dip = pod && bot_room >= 12.0;
    if can_stripe || can_dip {
        let scheme = r.pick(o, 0, &[2, 1, 1], 0);
        if scheme == 1 && can_stripe {
            d.scheme = "stripe";
            d.stripe_clear = r.lin(o, 2, 2, 5.0, 11.0);
            d.stripe_w = d.stripe_clear;
            // Under eyes on stalks the stripe is at least three times as long as wide.
            if f.stalk.is_some() {
                d.stripe_w = min(d.stripe_w, (top_room - 1.0) / 6.0);
            }
        }
        if scheme == 2 && can_dip {
            d.scheme = "dip";
            d.dip_y = -bot_room * r.lin(o, 2, 2, 0.5, 0.85);
        }
    }
    // A row over the face and a row under it: a seam, rivets, both or neither.
    if top_room >= 11.0 {
        let v = r.raw(o + 1, 0, 2);
        if v != 0 {
            d.rows.push(Row {
                y: hull.top_at(0.0) + top_room * 0.55,
                seam: v & 1 != 0,
                rivets: v & 2 != 0,
            });
        }
    }
    if bot_room >= 11.0 && d.scheme != "dip" {
        let v = r.raw(o + 1, 2, 2);
        if v != 0 {
            d.rows.push(Row {
                y: -bot_room * 0.45,
                seam: v & 1 != 0,
                rivets: v & 2 != 0,
            });
        }
    }
    if d.rows.iter().any(|row| row.rivets) {
        d.six = r.flag(o + 4, 0);
    }
    // A marking and a mark of wear, each to one side of a free band.
    let stripe = d.scheme == "stripe";
    let clear = d.stripe_clear;
    let spot = |side: f64, top_band: bool| -> Option<Spot> {
        let (y, room) = if top_band {
            (hull.top_at(0.0) + top_room * 0.5, top_room)
        } else {
            (-bot_room * 0.5, bot_room)
        };
        let x = side * min(hull.hw_at(y - room * 0.3), hull.hw_at(y + room * 0.3)) * 0.56;
        let need = if stripe { clear + 9.0 } else { 12.0 };
        (room >= 13.0 && x.abs() >= need).then(|| Spot {
            kind: "none",
            x,
            y,
            s: min(room * 0.3, 6.5),
            text: Vec::new(),
        })
    };
    let top = spot(1.0, true).is_some();
    let bottom = spot(1.0, false).is_some();
    let mark_side = || if r.flag(o + 2, 3) { 1.0 } else { -1.0 };
    if top || (bottom && d.scheme != "dip") {
        let mut dk = DECAL_NAMES[r.raw(o + 2, 0, 3) as usize];
        // Beside a screen, three short bars are a vent.
        if dk == "bars" && f.screen.is_some() {
            dk = "none";
        }
        if dk != "none" {
            let mut decal = spot(mark_side(), top).expect("the band has room for a marking");
            decal.kind = dk;
            if dk == "number" {
                decal.text = vec![r.raw(o + 2, 4, 3) + 1, r.raw(o + 3, 0, 3)];
            }
            d.decal = Some(decal);
        }
    }
    if top || bottom {
        let mut wk = ["none", "dent", "patch", "scratch"][r.raw(WEAR, 0, 2) as usize];
        // Beside a screen, a riveted patch is a drive slot or a panel of buttons.
        if wk == "patch" && f.screen.is_some() {
            wk = "none";
        }
        if wk != "none" {
            let band_top = if top && bottom { r.flag(WEAR, 2) } else { top };
            let mut at = spot(-mark_side(), band_top).expect("the band has room for wear");
            if !(d.scheme == "dip" && at.y > d.dip_y - 6.0) {
                at.kind = wk;
                d.wear = Some(at);
            }
        }
    }
    d
}
