//! The anatomy round the body: what grows on top of the head, arms or
//! wings, legs, the tail, and the coat along the body's edge.

use std::collections::BTreeSet;

use crate::genes::{is_ear, CoatKind, EarDetail, LegKind, SideKind, TailKind, TopKind, WingKind};
use crate::grid::{clamp, oval, rndi, running, spread, trail, Dir, Mask, Pt, AX, CH, CW, GY};
use crate::palette::{BELLY, GLOW, HORN, MARK, POINT, WING};
use crate::sprite::{Car, Ctx, PartOpts, Z_ARM, Z_FEET, Z_FRONT, Z_HEAD, Z_HIND, Z_TAIL, Z_WING};

use Dir::{Ne, Nw, Se, Sw, E, N, S, W};

/// A pointed flap from a tip cell down to a base that spans `x0..x0 + b` in
/// `base_row`. A bend of -1 makes an edge bulge (it moves fastest near the
/// tip), +1 makes it hollow. `bury` rows of the base's width are added
/// below, to sit inside the body.
#[allow(clippy::too_many_arguments)]
fn pointed(
    m: &mut Mask,
    x0: i32,
    b: i32,
    base_row: i32,
    tip_col: i32,
    tip_row: i32,
    left_bend: i32,
    right_bend: i32,
    bury: i32,
) {
    let n = 1.max(base_row - tip_row);
    let dl = tip_col - x0;
    let dr = x0 + b - 1 - tip_col;
    // An edge that runs back under a leaning tip moves in step with the
    // other edge, so no row is left empty.
    let ls = running(&spread(
        dl.abs(),
        n,
        if dl < 0 { right_bend } else { left_bend },
    ));
    let rs = running(&spread(
        dr.abs(),
        n,
        if dr < 0 { left_bend } else { right_bend },
    ));
    let (mut pl, mut pr) = (tip_col, tip_col);
    for j in 0..=n {
        let mut xl = tip_col - dl.signum() * ls[j as usize];
        let mut xr = tip_col + dr.signum() * rs[j as usize];
        if xl > xr {
            let mid = if dl > -dr { xr } else { xl };
            xl = mid;
            xr = mid;
        }
        // Each row touches the one above it.
        if xl > pr + 1 {
            xl = pr + 1;
        }
        if xr < pl - 1 {
            xr = pl - 1;
        }
        m.span(base_row - n + j, xl, xr);
        pl = xl;
        pr = xr;
    }
    for k in 1..=bury {
        m.span(base_row + k, x0, x0 + b - 1);
    }
}

/// What grows on one side of the head.
struct TopSide {
    m: Mask,
    knob: Option<Pt>,
    /// The ear hangs beside the head, behind it.
    behind: bool,
}

/// What grows on top of the head, right-hand side. `car` is how it is
/// carried, `short` how many cells shorter than its genes ask the fit has
/// made it, and `quirk` one more cell that a one-sided quirk takes.
fn top_mask(ctx: &Ctx, car: Car, short: i32, quirk: i32) -> TopSide {
    let t = &ctx.g.top;
    let h = &ctx.l.head;
    let a = h.a;
    let less = short + quirk;
    let mut m = Mask::new();
    let mut out_knob = None;
    let mut behind = false;
    // The crown: what grows on a head grows where its top lies within a few
    // rows of its highest row.
    let crown = h.rows[((h.len() - 1).min(clamp(rndi(f64::from(h.len()) * 0.2), 1, 3))) as usize];
    let ac = a.min(crown + 1);
    let place_share = [0.3, 0.55, 0.8, 1.0][t.place as usize];
    // The column offset from the axis for something `b` cells wide at its base.
    let place_x = |b: i32| {
        let inner = (b >> 1) + 1;
        let outer = inner.max(ac - 1 - ((b + 1) >> 1));
        rndi(f64::from(inner) + f64::from(outer - inner) * place_share)
    };
    // Sizes are shares of the head's half-width.
    let scaled = |shares: [f64; 4]| 1.max(rndi(f64::from(a) * shares[t.size as usize]));
    let wide = if a >= 7 { t.wide } else { 0 };
    // Wool stands a cell proud of the head, so what grows through it starts higher.
    let wool = i32::from(ctx.g.coat.kind == CoatKind::Woolly);
    let top_at = |dx: i32| h.top_at(dx) - wool;
    let up = car == Car::Up;
    let droop = car == Car::Droop;
    let back = car == Car::Back;
    match t.kind {
        TopKind::Horns => {
            // A short horn three cells wide with a one-cell tip; a leaning
            // horn steps outward towards its tip.
            let hv = clamp(scaled([0.3, 0.42, 0.54, 0.66]) + 2 - less, 4, 7);
            let b = 3 + wide;
            let ex = place_x(b);
            let x0 = AX + ex - (b >> 1);
            let base = top_at(clamp(x0 - AX, 0, a - 1));
            for k in 0..hv {
                let wk = if k == hv - 1 {
                    1
                } else if k == hv - 2 && hv >= 3 {
                    2
                } else {
                    b
                };
                let xo = x0 + b - 1 + i32::from(t.lean != 0 && k >= hv - 2 && hv >= 3);
                m.span(base - 1 - k, xo - wk + 1, xo);
            }
            for x in x0..x0 + b {
                for y in base..=top_at((a - 1).min(x - AX)) + 1 {
                    m.put(x, y);
                }
            }
        }
        TopKind::Cat | TopKind::Lynx | TopKind::Bat => {
            let (mut hv, b);
            if t.kind == TopKind::Bat {
                b = clamp(rndi(f64::from(a) * 0.55) + t.wide, 3, 7);
                hv = scaled([0.4, 0.55, 0.7, 0.85]).min(b + 1);
            } else {
                // At least as wide at the root as it is tall, with sides that bulge.
                hv = 5.min(scaled([0.3, 0.42, 0.55, 0.68]) + i32::from(t.kind == TopKind::Lynx));
                b = clamp(hv + t.wide, 3, 3.max(6.min(a - 2)));
                hv = hv.min(b);
            }
            hv = 1.max(hv - less);
            if up {
                hv += 1;
            }
            if droop || back {
                hv = 1.max(hv - 1);
            }
            let ex = place_x(b);
            let x0 = AX + ex - (b >> 1);
            let mut tip_off = if t.kind == TopKind::Bat {
                b - 1 - t.lean
            } else {
                (b >> 1) + t.lean
            };
            tip_off += if droop {
                1
            } else if back {
                2
            } else {
                0
            };
            let tip_col = x0 + tip_off;
            let base_row = top_at(clamp(x0 - AX, 0, a - 1)) + 1;
            let tip_row = base_row - 1 - hv;
            pointed(&mut m, x0, b, base_row, tip_col, tip_row, -1, -1, 1);
            for x in x0..x0 + b {
                for y in base_row..=top_at((a - 1).min(x - AX)) + 1 {
                    m.put(x, y);
                }
            }
            if t.kind == TopKind::Lynx && hv >= 3 {
                m.put(tip_col + i32::from(t.lean != 0), tip_row - 1);
            }
        }
        TopKind::Round | TopKind::Mouse | TopKind::Nub => {
            let (d, top, ex);
            let carried = if up {
                -1
            } else if droop || back {
                1
            } else {
                0
            };
            if t.kind == TopKind::Round {
                d = clamp(scaled([0.5, 0.6, 0.7, 0.8]) + wide, 4, 8);
                ex = place_x(d);
                // Three rows above the head, as long as two rows still sit on it.
                let rooted = top_at(0.max(ex - (d >> 1) + 1)) + 2 - d;
                top = rooted.max(
                    (top_at((a - 1).min(ex)) + 1 + i32::from(d >= 6) - d).min(h.top - wool - 3),
                ) + less.min(2)
                    + carried;
            } else if t.kind == TopKind::Mouse {
                d = clamp(scaled([0.7, 0.8, 0.9, 1.0]), 4, 9);
                ex = clamp(ac - 3 + t.place, (d >> 1) + 1, ac);
                // Half of a mouse's ear stands above the head, as long as two
                // rows of it still sit on the head.
                let rooted = top_at(0.max(ex - (d >> 1) + 1)) + 2 - d;
                let stands = rooted
                    .max((top_at((a - 1).min(ex)) - (d >> 1)).min(h.top - wool - 2.max(d >> 1)));
                // A one-sided quirk sets this ear a row lower only where two
                // rows of it still stand above the head.
                top = stands + short + i32::from(quirk != 0 && stands < h.top - wool - 2)
                    - i32::from(up)
                    + i32::from(droop || back);
            } else {
                d = 3 + wide;
                ex = place_x(d);
                top = top_at((a - 1).min(ex)) - 1.max(1 + (t.size >> 1))
                    + 1.min(less)
                    + i32::from(droop);
            }
            m.disc(AX + ex - (d >> 1), top, d);
        }
        TopKind::Long => {
            // Four cells wide on a large head, so that there is ear inside the outline.
            let w = if a >= 8 { 4 } else { 3 + wide };
            let hv = clamp(scaled([0.6, 0.8, 1.0, 1.2]) - less + i32::from(up), 3, 11);
            // Long ears stand on the inner half of the head and lean apart.
            let ex = place_x(w).min(((w >> 1) + 1).max(rndi(f64::from(a) * 0.5)));
            let c = AX + ex;
            let yb = top_at((a - 1).min(ex + (w >> 1))) + 1;
            if droop {
                // A folded ear: it stands for a third of its length at the
                // upper corner of the head, folds outward and hangs beside
                // the head, behind it.
                let r = ex.max(a - 2);
                let y0 = top_at((a - 1).min(r)) + 1;
                let k = clamp(rndi(f64::from(hv) / 3.0), 1, 3);
                let hang = clamp(hv - k - 2, 2, 2.max(h.widest_row() + 1 - (y0 - k)));
                let pts = trail(
                    AX + r,
                    y0,
                    &[(N, k), (Ne, 1), (E, 0.max(a - r - 1)), (Se, 1), (S, hang)],
                );
                m.stroke(&pts, |_, _| 3);
                for &(x, y) in &pts {
                    m.span(y, AX + r, x);
                }
                behind = true;
            } else if back {
                // Ears laid back point up and out from the root.
                m.stroke(&trail(c, yb, &[(N, 1), (Ne, 1.max(hv - 2))]), |_, _| 3);
                m.block(c - ((w - 1) >> 1), yb, w, 2);
            } else {
                let tip_row = top_at((a - 1).min(ex)) - hv;
                let n = yb - tip_row;
                let lean_tot = 1.max(hv >> 2) + t.lean;
                let cum = running(&spread(lean_tot, n, 0));
                for j in 0..=n + 1 {
                    let shift = lean_tot - cum[j.min(n) as usize];
                    let x0 = c - ((w - 1) >> 1) + shift;
                    if j == 0 && w >= 3 {
                        m.span(tip_row, x0 + 1, x0 + w - 2);
                    } else {
                        m.span(tip_row + j, x0, x0 + w - 1);
                    }
                }
            }
        }
        TopKind::Goblin => {
            // Ears that point out sideways from the upper side of the head.
            let len = clamp(scaled([0.3, 0.42, 0.54, 0.66]) - less, 1, 6);
            let ys = h.top
                + 2.max(rndi(
                    f64::from(h.len()) * (0.24 + 0.06 * f64::from(t.place)),
                ));
            let xe = AX + h.width_at(ys) - 1;
            let moves: Vec<(Dir, i32)> = if up {
                vec![(Ne, 1 + (len >> 1)), (E, len >> 1)]
            } else if droop {
                vec![(E, 1), (Se, len)]
            } else if back {
                vec![(E, len + 1)]
            } else {
                vec![(E, 1), (Ne, 1), (E, 0.max(len - 1))]
            };
            m.stroke(&trail(xe - 1, ys, &moves), |u, _| {
                if u < 0.45 {
                    4 + wide
                } else if u < 0.85 {
                    3
                } else {
                    1
                }
            });
        }
        TopKind::Bull => {
            let ex = clamp(place_x(3) + 1, 2, 2.max(ac - 2));
            let rise = 1.max(scaled([0.15, 0.25, 0.35, 0.45]) - less);
            let pts = trail(
                AX + ex,
                top_at(ex) + 1,
                &[(Ne, 1), (E, if a >= 8 { 2 } else { 1 }), (Ne, 1), (N, rise)],
            );
            m.stroke(&pts, |u, _| {
                if u < 0.35 {
                    3 + wide
                } else if u < 0.8 {
                    3
                } else {
                    1
                }
            });
        }
        TopKind::Knobs => {
            // Two knobs that stand two rows clear of the top of the head, or
            // as near to that as leaves them standing on it.
            let d = 3 + wide;
            let ex = place_x(d);
            let at = top_at((a - 1).min(ex));
            m.disc(
                AX + ex - (d >> 1),
                (at - d).max(
                    (at - 1.max(1 + (t.size >> 1) - less)).min(h.top - wool - 2 + 1.min(less)),
                ),
                d,
            );
        }
        TopKind::Antlers => {
            let ex = clamp(place_x(1), 1, 1.max(ac - 2));
            let rise = 0.max(scaled([0.1, 0.2, 0.3, 0.4]) - less);
            let trunk = trail(AX + ex, top_at(ex), &[(N, 2), (Ne, 1), (N, rise), (Ne, 1)]);
            m.stroke(&trunk, |_, _| 1);
            m.stroke(
                &trail(trunk[2].0, trunk[2].1, &[(Ne, 1), (E, i32::from(a >= 8))]),
                |_, _| 1,
            );
            if rise >= 1 {
                m.stroke(
                    &trail(trunk[3].0, trunk[3].1, &[(Nw, 1), (N, t.wide)]),
                    |_, _| 1,
                );
            }
        }
        TopKind::Antennae => {
            let ex = clamp(place_x(1), 1, 1.max(ac - 2));
            let len = 2.max(scaled([0.4, 0.55, 0.7, 0.85]) - less);
            let (s1, s2) = ((len + 1) >> 1, len >> 1);
            let moves = if up {
                [(N, s1 + 1), (Ne, s2), (N, 1), (N, 0)]
            } else if droop {
                [(N, 1), (Ne, 1), (E, 1.max(s2)), (Se, 1)]
            } else if back {
                [(Ne, 1), (E, 1 + s2), (E, 0), (E, 0)]
            } else {
                [
                    (N, s1),
                    (Ne, s2),
                    (if t.lean != 0 { E } else { N }, 1),
                    (N, 0),
                ]
            };
            let pts = trail(AX + ex, top_at(ex), &moves);
            m.stroke(&pts[..pts.len() - 1], |_, _| 1);
            out_knob = Some(pts[pts.len() - 1]);
        }
        TopKind::Frills => {
            let mid = h.widest_row();
            for k in 0..3 {
                let y = clamp(mid - 3 + 3 * k, h.top + 2, h.bottom - 1);
                let xe = AX + h.width_at(y) - 1;
                let len =
                    1.max(1 + (t.size >> 1) + i32::from(k == 0) - less + i32::from(up || back));
                m.stroke(
                    &trail(xe - 1, y, &[(E, 1), (if droop { Se } else { Ne }, len)]),
                    |_, i| if i < 2 { 3 } else { 1 },
                );
            }
        }
        TopKind::None | TopKind::Unicorn => {}
    }
    TopSide {
        m,
        knob: out_knob,
        behind,
    }
}

pub(crate) fn build_top(ctx: &mut Ctx) {
    let t = ctx.g.top.clone();
    if t.kind == TopKind::None || ctx.lim.top >= 9 {
        return;
    }
    let h = ctx.l.head.clone();
    if t.kind == TopKind::Unicorn {
        // One horn on the axis, two cells wide at the tip so that it stays
        // mirrored: a cone, not a stick.
        let hv = clamp(
            1.max(rndi(
                f64::from(h.a) * [0.35, 0.45, 0.55, 0.65][t.size as usize],
            )) - ctx.lim.top,
            2,
            6,
        );
        let mut m = Mask::new();
        let hw: Vec<i32> = (0..hv)
            .map(|j| {
                if j == 0 {
                    1
                } else {
                    2 + (j - 1) * (1 + t.wide) / 1.max(hv - 1)
                }
            })
            .collect();
        m.sym_rows(h.top - hv, &hw);
        let last = hw[hw.len() - 1];
        m.sym_rows(h.top, &[last, last]);
        let o = PartOpts {
            ramp: Some(HORN),
            ..PartOpts::default()
        };
        let p = ctx.add("horn", "horns", Z_HEAD, m, o, true);
        ctx.parts[p].ord -= 1000;
        return;
    }
    let mut car_r = ctx.pose.ears;
    let mut car_l = ctx.pose.ears;
    let side = ctx.g.temper.side;
    if car_r == Car::Cocked {
        car_r = if side > 0 { Car::Up } else { Car::Droop };
        car_l = if side > 0 { Car::Droop } else { Car::Up };
    }
    let (less_r, less_l) = (ctx.lim.top, ctx.lim.top);
    let (mut quirk_r, mut quirk_l) = (0, 0);
    match t.quirk {
        1 => quirk_r = 1,
        2 => quirk_l = 1,
        3 => {
            // One ear flopped: only a long ear is long enough to flop; the
            // rest come a cell short.
            if t.kind == TopKind::Long {
                if car_r == Car::Rest {
                    car_r = Car::Droop;
                }
            } else {
                quirk_l = 1;
            }
        }
        _ => {}
    }
    let mut right = top_mask(ctx, car_r, less_r, quirk_r);
    let mut left = top_mask(ctx, car_l, less_l, quirk_l);
    // A pit one cell wide between an ear and the head, or within an ear,
    // is filled in: it would be a dent in the outline.
    if t.kind != TopKind::Antlers && t.kind != TopKind::Antennae {
        let head_mask = ctx.parts[ctx.head].mask.clone();
        let mirrored_head = head_mask.mirrored();
        for (side_mask, hm) in [(&mut right.m, &head_mask), (&mut left.m, &mirrored_head)] {
            for y in 1..CH - 1 {
                for x in 1..CW - 1 {
                    let c = y * CW + x;
                    let on = |m: &Mask, c: i32| m.at(c) || hm.at(c);
                    if !on(side_mask, c)
                        && on(side_mask, c - 1)
                        && on(side_mask, c + 1)
                        && (on(side_mask, c + CW) || on(side_mask, c - CW))
                        && (side_mask.at(c - 1) || side_mask.at(c + 1))
                    {
                        side_mask.set_at(c);
                    }
                }
            }
        }
    }
    let ear = is_ear(t.kind);
    let horn = matches!(t.kind, TopKind::Horns | TopKind::Bull | TopKind::Knobs);
    let group: &'static str = if ear {
        "ears"
    } else if horn {
        "horns"
    } else if t.kind == TopKind::Antlers {
        "antlers"
    } else if t.kind == TopKind::Antennae {
        "antennae"
    } else {
        "frills"
    };
    let name = if ear {
        "ear"
    } else if horn {
        "horn"
    } else if t.kind == TopKind::Antlers {
        "antler"
    } else if t.kind == TopKind::Antennae {
        "antenna"
    } else {
        "frill"
    };
    let o = PartOpts {
        ramp: Some(if horn { HORN } else { crate::palette::BODY }),
        line_only: t.kind == TopKind::Antlers || t.kind == TopKind::Antennae,
        ..PartOpts::default()
    };
    let right_mask = right.m.clone();
    let pair = ctx.add_pair(
        name,
        group,
        Z_HEAD,
        right.m.clone(),
        o,
        true,
        Some(left.m.mirrored()),
    );
    for &p in &pair {
        ctx.parts[p].ord -= 1000;
    }
    // An ear that hangs beside the head lies behind it.
    if right.behind {
        ctx.parts[pair[0]].z = Z_HEAD - 1;
    }
    if left.behind {
        ctx.parts[pair[1]].z = Z_HEAD - 1;
    }
    if let (Some(rk), Some(lk)) = (right.knob, left.knob) {
        for (k, sfx) in [(rk, "-r"), ((CW - 1 - lk.0, lk.1), "-l")] {
            let mut m = Mask::new();
            m.dot(k.0, k.1, 3);
            let o = PartOpts {
                ramp: Some(GLOW),
                single: true,
                flat: true,
                ..PartOpts::default()
            };
            let p = ctx.add(&format!("antenna-knob{sfx}"), group, Z_HEAD, m, o, true);
            ctx.parts[p].ord -= 900;
        }
    }
    if ear && t.kind != TopKind::Nub {
        for &p in &pair {
            match t.detail {
                Some(EarDetail::Inner) => {
                    // The inside of the ear: the cells with ear all round
                    // them, above the head.
                    let mut cells = Vec::new();
                    let part_mask = &ctx.parts[p].mask;
                    let head_mask = &ctx.parts[ctx.head].mask;
                    for y in 1..CH - 1 {
                        for x in 1..CW - 1 {
                            let c = y * CW + x;
                            if !part_mask.at(c) || head_mask.at(c) {
                                continue;
                            }
                            if crate::compose::NB8
                                .iter()
                                .all(|&(dx, dy)| part_mask.at(c + dy * CW + dx))
                            {
                                cells.push(c);
                            }
                        }
                    }
                    crate::sprite::overlay(&mut ctx.parts[p], &cells, BELLY);
                }
                Some(EarDetail::Dark) => {
                    // Ears a shade darker than the body, whole.
                    ctx.parts[p].ramp = if t.kind == TopKind::Long { POINT } else { MARK };
                }
                _ => {}
            }
        }
        if t.detail == Some(EarDetail::Knobs) {
            // Two small horn knobs between the ears, where the ears leave a
            // clear cell on each side of them.
            let mut m = Mask::new();
            let ex = 2;
            m.disc(
                AX + ex - 1,
                h.top_at(ex) - 1 - i32::from(ctx.g.coat.kind == CoatKind::Woolly),
                3,
            );
            let mut room = true;
            let mut c = CW;
            while c < CW * (CH - 1) && room {
                if m.at(c) {
                    for d in [0, 1, -1, CW, -CW] {
                        let e = c + d;
                        let mirror = CW * (e / CW) + CW - 1 - (e % CW);
                        if right_mask.at(e) || left.m.at(mirror) {
                            room = false;
                        }
                    }
                }
                c += 1;
            }
            if room {
                let o = PartOpts {
                    ramp: Some(HORN),
                    single: true,
                    ..PartOpts::default()
                };
                for p in ctx.add_pair("horn", "horns", Z_HEAD, m, o, true, None) {
                    ctx.parts[p].ord -= 950;
                }
            }
        }
    }
}

/// The row an arm or a wing leaves the body at, and the body's edge there.
fn shoulder(ctx: &Ctx, high: i32) -> (i32, i32) {
    let t = ctx.trunk();
    let h = &ctx.l.head;
    let ys = if ctx.l.split {
        clamp(h.bottom + 1 + (high >> 1), t.top + 1, t.bottom - 1)
    } else if ctx.l.welded {
        // Where the belly first shows wider than the head: the crease.
        let mut crease = t.top;
        while crease < t.bottom && t.width_at(crease) <= h.width_at(crease) {
            crease += 1;
        }
        // An arm leaves the belly two rows under the crease: at the crease
        // itself, a pair of arms is a scarf.
        clamp(crease + 2 + (high >> 1), t.top + 1, t.bottom - 2)
    } else {
        clamp(
            t.widest_row() + [1, 0, 2, 3][high as usize],
            t.top + 3,
            t.bottom - 2,
        )
    };
    (ys, AX + t.width_at(ys) - 1)
}

/// One arm, right-hand side, carried as `car` says. An arm starts on the
/// outline and lies outside the body; only crossed arms and hands held in
/// front lie over a torso.
fn arm_mask(ctx: &Ctx, car: Car) -> Mask {
    let s = &ctx.g.side;
    let mut m = Mask::new();
    let t = ctx.trunk();
    let (ys, xe) = shoulder(ctx, s.high);
    let less = ctx.lim.arm;
    let g = GY - 1;
    let reach_in = xe - AX;
    let scaled = |shares: [f64; 4]| 1.max(rndi(f64::from(t.a) * shares[s.len as usize]));
    let thick = if t.a >= 6 { s.thick } else { 0 };
    match s.kind {
        SideKind::Nub => {
            let d = 3 + thick + if t.a >= 8 { s.len >> 1 } else { 0 };
            let dy = match car {
                Car::Up => -2,
                Car::Half | Car::Front => -1,
                Car::Limp => 1,
                _ => 0,
            };
            m.dot(xe + i32::from(d < 5), ys + dy, d);
        }
        SideKind::Short | SideKind::Long => {
            let d = 3 + thick;
            let mut n = 1.max(
                scaled([0.2, 0.3, 0.4, 0.5]) + if s.kind == SideKind::Long { 2 } else { 0 } - less,
            );
            // A hanging arm ends above the bottom of the body, or it is a leg.
            if car == Car::Rest || car == Car::Limp {
                n = clamp(n, 1, 1.max((g - 3).min(t.bottom - 2) - ys));
            }
            let moves: Vec<(Dir, i32)> = match car {
                Car::Up => vec![(E, 1), (Ne, n)],
                Car::Half => vec![(E, 1), (Ne, 1), (N, 0.max(n - 2))],
                Car::Front => vec![(S, 1), (Sw, 1), (W, clamp(n - 1, 0, 0.max(reach_in - 3)))],
                Car::Crossed => vec![(Sw, 1), (W, clamp(n, 1, 1.max(reach_in - 1)))],
                Car::Limp => vec![(S, 1), (Se, 1), (S, n - 1)],
                Car::Out => vec![(E, n)],
                _ => vec![(Se, 1), (S, n - 1)],
            };
            // An arm never reaches below the ground.
            let pts: Vec<Pt> = trail(xe, ys, &moves)
                .into_iter()
                .filter(|p| p.1 + (d >> 1) <= g)
                .collect();
            m.stroke(&pts, |_, _| d);
        }
        SideKind::Claw => {
            let n = 0.max((s.len >> 1) - less);
            let d = 5 + thick;
            let moves: Vec<(Dir, i32)> = match car {
                Car::Up => vec![(E, 1), (Ne, 1 + n)],
                Car::Half | Car::Front => vec![(E, 1), (Ne, 1)],
                Car::Limp => vec![(Se, 1), (S, n)],
                Car::Out | Car::Crossed => vec![(E, 1 + n)],
                _ => vec![(E, 1), (Se, n)],
            };
            let pts = trail(xe, ys, &moves);
            let end = pts[pts.len() - 1];
            m.stroke(&pts, |_, _| 3);
            // The pincer: a disc with a notch cut from its outer side. It
            // shuts when the creature is blocked.
            let x0 = end.0;
            let y0 = (g - d + 1).min(end.1 - (d >> 1));
            m.disc(x0, y0, d);
            if ctx.mood != avatar_svg::Mood::Blocked {
                if car == Car::Up {
                    m.unput(x0 + (d >> 1), y0);
                    m.unput(x0 + (d >> 1), y0 + 1);
                    if d == 6 {
                        m.unput(x0 + 2, y0);
                        m.unput(x0 + 2, y0 + 1);
                    }
                } else {
                    m.unput(x0 + d - 1, y0 + (d >> 1));
                    m.unput(x0 + d - 2, y0 + (d >> 1));
                    if d == 6 {
                        m.unput(x0 + d - 1, y0 + 2);
                        m.unput(x0 + d - 2, y0 + 2);
                    }
                }
            }
        }
        SideKind::Flipper => {
            let n = 1.max(scaled([0.25, 0.35, 0.45, 0.55]) - less);
            // At rest a flipper hangs down along the body, as a penguin's does.
            let moves: Vec<(Dir, i32)> = match car {
                Car::Up => vec![(E, 1), (Ne, n)],
                Car::Half | Car::Out => vec![(E, 1), (Ne, 1.max(n - 1))],
                Car::Limp => vec![(S, n + 1)],
                Car::Front | Car::Crossed => vec![(S, 1), (Sw, 1.max(n - 1))],
                _ => vec![(Se, 1), (S, 1.max(n - 1))],
            };
            let pts: Vec<Pt> = trail(xe, ys, &moves)
                .into_iter()
                .filter(|p| p.1 < g)
                .collect();
            m.stroke(&pts, |u, _| {
                if u < 0.5 {
                    4 + thick
                } else if u < 0.9 {
                    3
                } else {
                    1
                }
            });
        }
        SideKind::Tentacle => {
            let curl = s.len & 1 != 0;
            let mut n = 1.max(scaled([0.2, 0.3, 0.4, 0.5]) - less);
            if car == Car::Rest || car == Car::Limp {
                n = clamp(n, 1, 1.max((g - 5).min(t.bottom - 4) - ys));
            }
            let moves: Vec<(Dir, i32)> = match car {
                Car::Up => vec![(E, 1), (Ne, n), (N, 1), (if curl { Nw } else { Ne }, 1)],
                Car::Half | Car::Out => vec![(E, n), (Ne, 1), (if curl { N } else { E }, 1)],
                Car::Front | Car::Crossed => {
                    vec![(Sw, 1), (W, clamp(n, 1, 1.max(reach_in - 2))), (Nw, 1)]
                }
                Car::Limp => vec![(Se, 1), (S, n + 1), (if curl { Sw } else { Se }, 1)],
                _ => vec![
                    (Se, 1),
                    (S, n),
                    (if curl { Sw } else { Se }, 1),
                    (if curl { W } else { E }, 1),
                ],
            };
            let pts: Vec<Pt> = trail(xe, ys, &moves)
                .into_iter()
                .filter(|p| p.1 < g)
                .collect();
            m.stroke(&pts, |u, _| {
                if u < 0.6 {
                    3 + thick
                } else if u < 0.9 {
                    3
                } else {
                    1
                }
            });
        }
        SideKind::Wings | SideKind::None => {}
    }
    m
}

/// One wing, right-hand side: columns outward from the body, each with a
/// top and a bottom row.
fn wing_mask(ctx: &Ctx, car: Car) -> Mask {
    let s = &ctx.g.side;
    let mut m = Mask::new();
    let t = ctx.trunk();
    let (sh_ys, _) = shoulder(ctx, if s.high != 0 { 0 } else { 2 });
    let mut yw = sh_ys - if ctx.l.torso.is_some() { 0 } else { 2 };
    let mut span0 = 2.max(4 + s.span - ctx.lim.wing);
    let deep = s.deep;
    let fold = car == Car::Fold;
    let up = car == Car::Up;
    if up {
        yw -= 1;
    }
    if fold {
        span0 = 2.max(span0 - 2);
        yw += 1;
    }
    if car == Car::Spread {
        span0 += 1;
    }
    if !ctx.l.split {
        yw = clamp(yw, t.top + 1, t.bottom - 1);
    }
    // Under a head wider than its torso a wing at the shoulder would show
    // only as a skirt at the hips; there it rides higher and further out.
    if ctx.l.split {
        yw = yw.min(ctx.l.head.bottom - 2);
    }
    let mut xs = AX + t.width_at(yw) - 2; // two columns inside the body
    if ctx.l.split {
        xs = xs.max(AX + ctx.l.head.width_at(yw) - 2);
    }
    let n = span0 + 2;
    match s.wing {
        Some(WingKind::Bat) => {
            let rise = 2 + deep + i32::from(up) - if fold { 2 } else { 0 };
            for i in 0..n {
                let top = yw - i.min(rise);
                let mut bottom = yw + 2 + deep - 0.max(i - (n - 3)) * 2;
                // The trailing edge is scalloped: every other column is a cell shorter.
                if i > 1 && i < n - 1 && (i & 1) != 0 {
                    bottom -= 1;
                }
                if fold {
                    bottom += 1;
                }
                for y in top..=top.max(bottom) {
                    m.put(xs + i, y);
                }
            }
        }
        Some(WingKind::Feather) => {
            let h = 3 + deep + i32::from(fold);
            let mut arc = running(&spread(2, 2, -1));
            arc.push(2);
            for i in 0..n {
                let top =
                    yw - if fold {
                        0
                    } else {
                        2.min(arc[i.min(3) as usize])
                    } + i32::from(i == n - 1);
                // The trailing edge steps up towards the tip: one step per feather.
                let bottom = yw + h - i * (h + 1) / n;
                for y in top..=top.max(bottom) {
                    m.put(xs + i, y);
                }
            }
        }
        Some(WingKind::Insect) => {
            // Two lobes: the upper one larger and carried higher.
            let (w1, h1, w2, h2) = (span0 + 1, 3 + deep, 2.max(span0 - 1), 3);
            let lift = if fold {
                2
            } else if up {
                0
            } else {
                1
            };
            oval(&mut m, xs, yw - h1 + lift, w1 + 1, h1 + 1);
            // Under a head on a small torso the lower lobe would hang at the
            // hips like a skirt: there the wing is its upper lobe alone.
            if !ctx.l.split {
                oval(&mut m, xs, yw + 1 + i32::from(fold), w2 + 1, h2);
            }
        }
        _ => {
            // Tiny: a stub of a wing with a pointed tip.
            let d = clamp(3 + s.span - ctx.lim.wing, 3, 5);
            let y0 = yw - (d >> 1)
                + if up {
                    -1
                } else if fold {
                    1
                } else {
                    0
                };
            m.disc(xs + 1, y0, d);
            if !fold {
                m.put(xs + d, y0);
                m.put(xs + d + 1, y0 - 1);
            }
        }
    }
    m
}

pub(crate) fn build_side(ctx: &mut Ctx) {
    let s = ctx.g.side.clone();
    if s.kind == SideKind::None || ctx.lim.arm >= 9 {
        return;
    }
    if s.kind == SideKind::Wings {
        let m = wing_mask(ctx, ctx.pose.wings);
        let ramp = if s.wing == Some(WingKind::Insect) {
            BELLY
        } else {
            WING
        };
        let o = PartOpts {
            ramp: Some(ramp),
            ..PartOpts::default()
        };
        ctx.add_pair("wing", "wings", Z_WING, m, o, false, None);
        return;
    }
    let side = ctx.g.temper.side;
    // Only a torso has room for hands held in front of it or crossed arms.
    let on_torso = ctx.l.split;
    let crossed = ctx.g.temper.crossed;
    let settle = |car: Car| {
        if car == Car::Front && !on_torso {
            Car::Half
        } else if car == Car::Crossed && (!on_torso || !crossed) {
            Car::Out
        } else {
            car
        }
    };
    let mut car_r = settle(ctx.pose.arms);
    let mut car_l = settle(ctx.pose.arms);
    // Waiting, it lifts the arm on the side it looks to.
    if car_r == Car::One {
        car_r = if side > 0 { Car::Half } else { Car::Rest };
        car_l = if side > 0 { Car::Rest } else { Car::Half };
    }
    let welded = s.kind == SideKind::Nub;
    let z = if welded {
        ctx.parts[ctx.trunk_part].z
    } else if ctx.l.split {
        Z_ARM
    } else {
        Z_FRONT
    };
    let right = arm_mask(ctx, car_r);
    let left = arm_mask(ctx, car_l).mirrored();
    let pair = ctx.add_pair(
        "arm",
        "arms",
        z,
        right,
        PartOpts::default(),
        false,
        Some(left),
    );
    if welded {
        for p in pair {
            ctx.parts[p].ord -= 1000;
        }
    }
}

pub(crate) fn build_legs(ctx: &mut Ctx) {
    let l = ctx.g.legs.clone();
    let g = GY - 1;
    let bottom = ctx.l.bottom;
    if l.kind == LegKind::None || ctx.lim.leg >= 9 {
        return;
    }
    let t = ctx.trunk().clone();
    let trunk_z = ctx.parts[ctx.trunk_part].z;
    // Half-width of the body two rows above its lowest row: how far out a
    // leg can stand.
    let room = t.width_at(t.top.max(bottom - 2));
    let inner = clamp(1 + l.spread, 1, 1.max(room - 3));
    let o = PartOpts::default();
    match l.kind {
        LegKind::Feet => {
            let w = 4 + l.big;
            let mut m = Mask::new();
            let x0 = AX + inner.min(1.max(room - w + 1));
            m.span(g - 2, x0 + 1, x0 + w - 2);
            m.span(g - 1, x0, x0 + w - 1);
            m.span(g, x0, x0 + w - 1);
            ctx.add_pair("foot", "legs", Z_FEET, m, o, false, None);
        }
        LegKind::Legs => {
            // Two legs with a gap of at least two cells between them.
            let w = (3 + l.big).min(2.max(room - 1));
            let mut m = Mask::new();
            let x0 = AX + clamp(inner, 1, 1.max(room - w));
            m.block(x0, bottom - 1, w, g - bottom + 2);
            // The foot: one cell longer than the leg is wide, pointing outward.
            m.span(g, x0, x0 + w);
            m.span(g - 1, x0, x0 + w);
            for p in ctx.add_pair("leg", "legs", trunk_z, m, o, false, None) {
                ctx.parts[p].ord -= 1000;
            }
        }
        LegKind::Bird => {
            let mut m = Mask::new();
            let x = AX + (inner + 1).min(1.max(room - 2));
            for y in bottom..=g {
                m.put(x, y);
            }
            m.span(g, x - 1, x + 1 + l.big);
            let o = PartOpts {
                line_only: true,
                ..o
            };
            ctx.add_pair("leg", "legs", Z_HIND, m, o, false, None);
        }
        LegKind::Quad => {
            // Fore legs in front with a gap between them; hind legs outside
            // and behind, a shade darker.
            let w = 3;
            let (mut front, mut hind) = (Mask::new(), Mask::new());
            let x0 = AX + clamp(inner, 1, 1.max(room - w - 2));
            front.block(x0, bottom - 1, w, g - bottom + 2);
            let x1 = (x0 + w).min(AX + room - 2);
            hind.block(x1, bottom - 2, w + l.big, g - bottom + 2);
            for p in ctx.add_pair("leg-fore", "legs", trunk_z, front, o, false, None) {
                ctx.parts[p].ord -= 1000;
            }
            let o = PartOpts { darker: 1, ..o };
            ctx.add_pair("leg-hind", "legs", Z_HIND, hind, o, false, None);
        }
        LegKind::Tentacles => {
            let mut m = Mask::new();
            let count = clamp((room + 1) / 4, 1, 3);
            for k in 0..count {
                let x = AX + 1 + 4 * k;
                let outer = k == count - 1;
                // Lengths alternate; the outermost curls outward at its tip.
                let down = g - bottom - i32::from((k + l.spread) & 1 != 0) - i32::from(outer);
                let mut moves = vec![(S, 1.max(down + 1))];
                if outer {
                    moves.push((if l.big != 0 { E } else { Se }, 1));
                }
                let pts: Vec<Pt> = trail(x, bottom - 1, &moves)
                    .into_iter()
                    .filter(|p| p.1 < g)
                    .collect();
                m.stroke(&pts, |_, _| 3);
            }
            for p in ctx.add_pair("tentacle", "legs", trunk_z, m, o, false, None) {
                ctx.parts[p].ord -= 1000;
            }
        }
        LegKind::Wisp => {
            // The body runs out into a tail that curls to one side: a
            // creature that hovers.
            let mut m = Mask::new();
            let n = g - bottom;
            let side = if l.side != 0 { 1 } else { -1 };
            let start_hw = 2.max(t.width_at(bottom) - 1);
            let hws = running(&spread(start_hw - 1, n, 1));
            let shift = running(&spread(2 + l.big + (n >> 1), n, 1));
            for j in 0..=n {
                let hw = start_hw - hws[j as usize];
                let c = AX + side * shift[j as usize];
                if bottom + j <= g {
                    m.span(bottom + j, c - hw, c + hw - 1);
                }
            }
            let p = ctx.add("wisp", "legs", trunk_z, m, o, false);
            ctx.parts[p].ord -= 1000;
        }
        LegKind::Crab => {
            // Two short legs a side, splayed: one under the body, one
            // reaching out from its flank.
            let mut m = Mask::new();
            let ya = bottom - 1;
            let xa = AX + 1.max(t.width_at(ya) - 2);
            let pts: Vec<Pt> = trail(xa, ya, &[(Se, g - ya)])
                .into_iter()
                .filter(|p| p.1 <= g)
                .collect();
            m.stroke(&pts, |_, _| 1);
            let yb = clamp(bottom - 3 - l.big, t.top + 2, bottom);
            let xb = AX + t.width_at(yb) - 1;
            let pts: Vec<Pt> = trail(xb, yb, &[(E, 1 + (l.spread & 1)), (Se, g - yb)])
                .into_iter()
                .filter(|p| p.1 <= g)
                .collect();
            m.stroke(&pts, |_, _| 1);
            let o = PartOpts {
                line_only: true,
                ..o
            };
            ctx.add_pair("leg", "legs", Z_HIND, m, o, false, None);
        }
        LegKind::None => {}
    }
}

pub(crate) fn build_tail(ctx: &mut Ctx) {
    let t = ctx.g.tail.clone();
    let g = GY - 1;
    if t.kind == TailKind::None || ctx.lim.tail >= 9 {
        return;
    }
    let trunk = ctx.trunk().clone();
    let car = ctx.pose.tail;
    let less = ctx.lim.tail;
    let up = car == Car::Up;
    let down = car == Car::Down;
    let flick = car == Car::Flick;
    let bristle = car == Car::Bristle;
    let yt = clamp(trunk.bottom - 1 - (t.curl & 1), trunk.top + 2, g - 1);
    let xe = AX + trunk.width_at(yt) - 1;
    let len = 0.max(t.len - less);
    let mut m = Mask::new();
    let mut tip: Option<Vec<Pt>> = None;
    let mut line_only = false;
    let mut tip_mask: Option<Mask> = None;
    match t.kind {
        TailKind::Stub => {
            let d = clamp(4 + (t.len >> 1) - less + i32::from(bristle), 3, 6);
            let rise = if up {
                2
            } else if down {
                0
            } else {
                1
            };
            m.dot(xe + 1, yt - rise, d);
        }
        TailKind::Curl => {
            let d = if bristle { 4 } else { 3 };
            let turned = t.curl & 2 != 0;
            let moves: Vec<(Dir, i32)> = if down {
                vec![(E, 1), (Se, 1.min(g - 1 - yt)), (E, 1 + len)]
            } else if up {
                vec![(Ne, 1), (N, 3 + len), (if turned { Nw } else { Ne }, 1)]
            } else {
                let back = turned ^ flick;
                vec![
                    (E, 1),
                    (Ne, 1),
                    (N, 1 + len),
                    (if back { Nw } else { Ne }, 1),
                    (if back { W } else { E }, 1),
                ]
            };
            m.stroke(&trail(xe, yt, &moves), |_, _| d);
        }
        TailKind::Bushy => {
            let big = 5 + i32::from(t.curl & 2 != 0) + i32::from(bristle);
            let moves: Vec<(Dir, i32)> = if down {
                vec![(E, 2), (Se, 1.min(g - 2 - yt)), (E, 1 + len)]
            } else {
                vec![
                    (E, 1),
                    (Ne, 2),
                    (N, if up { 2 } else { 1 } + len),
                    (if flick { Ne } else { Nw }, 1),
                ]
            };
            let pts = trail(xe, yt, &moves);
            m.stroke(&pts, |u, _| {
                if u < 0.12 {
                    3
                } else if u < 0.3 {
                    4
                } else if u < 0.8 {
                    big
                } else if u < 0.95 {
                    4
                } else {
                    3
                }
            });
            let keep = 2.max(pts.len() as i32 >> 2) as usize;
            tip = Some(pts[pts.len().saturating_sub(keep)..].to_vec());
        }
        TailKind::Devil => {
            let moves: Vec<(Dir, i32)> = if down {
                vec![(E, 2), (Se, 1.min(g - 1 - yt)), (E, 1 + len), (Ne, 1)]
            } else {
                vec![
                    (E, 2),
                    (Ne, 1),
                    (N, if up { 2 } else { 1 } + len),
                    (if flick { Nw } else { Ne }, 1),
                ]
            };
            let pts = trail(xe, yt, &moves);
            let end = pts[pts.len() - 1];
            let prev = pts[pts.len() - 2];
            m.stroke(&pts, |_, _| 1);
            line_only = true;
            // The spade at the tip, pointing the way the tail ends.
            let mut tm = Mask::new();
            let (dx, dy) = (end.0 - prev.0, end.1 - prev.1);
            tm.dot(end.0, end.1, 3);
            tm.put(end.0 + dx, end.1 + dy);
            if bristle {
                tm.dot(end.0 + dx, end.1 + dy, 3);
            }
            tip_mask = Some(tm);
        }
        TailKind::Lizard => {
            // Thick where it leaves the body, tapering to a point along the ground.
            let n = 3 + len + i32::from(bristle);
            let d0 = 4 + (t.curl >> 1);
            let ds: Vec<i32> = running(&spread(d0 - 1, n, 1))
                .into_iter()
                .map(|v| d0 - v)
                .collect();
            let lifted = if up {
                2
            } else if flick {
                1
            } else {
                0
            };
            for i in 0..=n {
                let d = 1.max(ds[i as usize]);
                let rise = if lifted != 0 {
                    i.min(lifted + (i >> 1))
                } else {
                    0
                };
                let yb = g.min(yt + 2) - rise;
                m.disc(xe - 1 + i, yb - d + 1, d);
            }
        }
        TailKind::Fluke => {
            let stem: Vec<(Dir, i32)> = if down {
                vec![(E, 2 + (len >> 1))]
            } else {
                vec![(E, 1), (Ne, 1 + (len >> 1)), (N, if up { 2 } else { 1 })]
            };
            let pts = trail(xe, yt, &stem);
            let end = pts[pts.len() - 1];
            m.stroke(&pts, |_, _| 3);
            let f = 1 + (len & 1) + i32::from(bristle);
            let flukes: [(Dir, i32); 2] = if down {
                [(Ne, f + 1), (Se, (f + 1).min(g - end.1))]
            } else {
                [(Nw, f + 1), (Ne, f + 1)]
            };
            for mv in flukes {
                m.stroke(&trail(end.0, end.1, &[mv]), |_, _| 1);
            }
        }
        TailKind::Rat => {
            let drop = 0.max(g - yt);
            let moves: Vec<(Dir, i32)> = if up {
                vec![(E, 1), (Ne, 2 + len), (N, 1)]
            } else {
                vec![
                    (Se, drop),
                    (E, 2 + len),
                    (if down { E } else { Ne }, 1),
                    (if t.curl & 2 != 0 { N } else { E }, i32::from(!down)),
                ]
            };
            m.stroke(&trail(xe, yt, &moves), |_, _| 1);
            line_only = true;
        }
        TailKind::None => {}
    }
    let flip = |mask: Mask| if t.side != 0 { mask.mirrored() } else { mask };
    let o = PartOpts {
        line_only,
        ..PartOpts::default()
    };
    let part = ctx.add("tail", "tail", Z_TAIL, flip(m), o, false);
    if let Some(tm) = tip_mask {
        let o = PartOpts {
            single: true,
            ..PartOpts::default()
        };
        ctx.add("tail-tip", "tail", Z_TAIL, flip(tm), o, false);
    }
    if let (Some(tip), true) = (tip, t.tipped) {
        // A pale tip, as on a fox: the end of the tail one step lighter
        // than the rest of it, in the tail's own colour.
        let mut tm = Mask::new();
        tm.stroke(&tip, |_, _| 6);
        let fm = flip(tm);
        let lift: BTreeSet<i32> = fm
            .cells()
            .into_iter()
            .filter(|&c| ctx.parts[part].mask.at(c))
            .collect();
        ctx.parts[part].lift = Some(lift);
    }
}

/// A cell on the right half of a lobe's outline with the direction that
/// points out of the body there.
#[derive(Clone, Copy)]
struct Rim {
    x: i32,
    y: i32,
    d: Dir,
}

/// The cells along the right half of a lobe's outline, from the top of the
/// axis round to the bottom.
fn rim_of(lb: &crate::sprite::Lobe) -> Vec<Rim> {
    let mut cells = Vec::new();
    let rows = &lb.rows;
    for x in 0..rows[0] {
        let d = if x == rows[0] - 1 && rows.len() > 1 && rows[1] > rows[0] {
            Ne
        } else {
            N
        };
        cells.push(Rim {
            x: AX + x,
            y: lb.top,
            d,
        });
    }
    for i in 1..rows.len() {
        let y = lb.top + i as i32;
        if rows[i] > rows[i - 1] {
            for x in rows[i - 1]..rows[i] {
                let d = if x == rows[i] - 1 { Ne } else { N };
                cells.push(Rim { x: AX + x, y, d });
            }
        } else if rows[i] == rows[i - 1] {
            cells.push(Rim {
                x: AX + rows[i] - 1,
                y,
                d: E,
            });
        } else {
            cells.push(Rim {
                x: AX + rows[i] - 1,
                y,
                d: Se,
            });
        }
    }
    cells
}

pub(crate) fn build_coat(ctx: &mut Ctx) {
    let c = ctx.g.coat.clone();
    if !matches!(
        c.kind,
        CoatKind::Furry | CoatKind::Spiky | CoatKind::Woolly | CoatKind::Warty
    ) {
        return;
    }
    let h = ctx.l.head.clone();
    // One bit per spine, tuft or wart, read as each is drawn.
    let mut bit_at = c.at * 8 + 8;
    let reader = crate::genes::Reader::new(ctx.bytes);
    let mut next_bit = || {
        let bit = reader.bit_at(bit_at);
        bit_at += 1;
        bit as i32
    };
    let rim = rim_of(&h);
    let head = ctx.head;
    let parts = &ctx.parts;
    let taken = |x: i32, y: i32| {
        parts
            .iter()
            .enumerate()
            .any(|(i, p)| i != head && p.mask.has(x, y))
    };
    let body = |x: i32, y: i32| parts[head].mask.has(x, y);
    // The rim above the widest row, where nothing else grows.
    let widest = h.widest_row();
    let upper: Vec<Rim> = rim
        .iter()
        .copied()
        .filter(|r| r.d != Se && r.y <= widest + 1)
        .collect();
    let single = PartOpts {
        single: true,
        ..PartOpts::default()
    };
    match c.kind {
        CoatKind::Spiky => {
            // Spines round the upper body, each a small triangle on the
            // outline with a cell of the body's colour inside it. They grow
            // a cell when the creature bristles.
            let step = 3 + (c.grain >> 1);
            let (mut right, mut left) = (Mask::new(), Mask::new());
            let bristle = i32::from(ctx.pose.tail == Car::Bristle);
            let mut k = 1 + (c.grain & 1);
            while (k as usize) < upper.len() {
                let r = upper[k as usize];
                let (x, y) = (r.x, r.y);
                let (cells, tip): (Option<Vec<Pt>>, Option<Pt>) =
                    if r.d == N && body(x - 1, y) && body(x + 1, y) {
                        (
                            Some(vec![(x - 1, y - 1), (x, y - 1), (x + 1, y - 1), (x, y - 2)]),
                            Some((0, -1)),
                        )
                    } else if r.d == E && body(x, y - 1) && body(x, y + 1) {
                        (
                            Some(vec![(x + 1, y - 1), (x + 1, y), (x + 1, y + 1), (x + 2, y)]),
                            Some((1, 0)),
                        )
                    } else if r.d == Ne {
                        (Some(vec![(x + 1, y), (x, y - 1), (x + 1, y - 1)]), None)
                    } else {
                        (None, None)
                    };
                if let Some(cells) = cells {
                    for (m, flip) in [(&mut right, false), (&mut left, true)] {
                        let tall = next_bit() + bristle;
                        let mut list = cells.clone();
                        if let Some(tip) = tip {
                            let last = cells[3];
                            for j in 1..=tall {
                                list.push((last.0 + tip.0 * j, last.1 + tip.1 * j));
                            }
                        }
                        if list.iter().any(|&(cx, cy)| {
                            taken(cx, cy) || taken(cx, cy - 1) || taken(cx + 1, cy)
                        }) {
                            continue;
                        }
                        for (cx, cy) in list {
                            m.put(if flip { CW - 1 - cx } else { cx }, cy);
                        }
                    }
                }
                k += step;
            }
            for (m, name) in [(right, "spines-r"), (left, "spines-l")] {
                let p = ctx.add(name, "coat", Z_HEAD, m, single, true);
                ctx.parts[p].ord -= 1000;
            }
        }
        CoatKind::Woolly => {
            // A scalloped edge: the outline steps a cell out for two or three
            // cells and back in for one, like wool, where it runs straight.
            let all: Vec<Rim> = rim
                .iter()
                .copied()
                .filter(|r| r.y <= h.bottom - 2)
                .collect();
            let mut wool = Mask::new();
            let mut k = (c.grain & 1) as usize;
            while k < all.len() {
                let len = 2 + next_bit() as usize;
                for r in &all[k..all.len().min(k + len)] {
                    if r.d == N || r.d == E {
                        let (dx, dy) = r.d.delta();
                        wool.put(r.x + dx, r.y + dy);
                    }
                }
                k += len + 1 + (c.grain >> 1) as usize;
            }
            let mirrored = wool.mirrored();
            let head_mask = &mut ctx.parts[ctx.head].mask;
            head_mask.union(&wool);
            head_mask.union(&mirrored);
        }
        CoatKind::Furry => {
            // Tufts at the cheeks: short points of fur that break the outline.
            let mid = widest;
            let (mut right, mut left) = (Mask::new(), Mask::new());
            let count = 2 + (c.grain & 1);
            for k in 0..count {
                let y = clamp(mid - 1 + 2 * k + (c.grain >> 1), h.top + 2, h.bottom - 1);
                let xe = AX + h.width_at(y) - 1;
                for (m, flip) in [(&mut right, false), (&mut left, true)] {
                    let long = next_bit() != 0;
                    let mut cells = vec![(xe + 1, y), (xe + 1, y + 1)];
                    if long {
                        cells.push((xe + 2, y + 1));
                    }
                    if cells.iter().any(|&(x, yy)| taken(x, yy)) {
                        continue;
                    }
                    for (x, yy) in cells {
                        m.put(if flip { CW - 1 - x } else { x }, yy);
                    }
                }
            }
            for (m, name) in [(right, "tufts-r"), (left, "tufts-l")] {
                let p = ctx.add(name, "coat", Z_HEAD, m, single, true);
                ctx.parts[p].ord -= 1000;
            }
        }
        CoatKind::Warty => {
            // A few warts on the flanks, not mirrored: each side has its own.
            let mut m = Mask::new();
            let step = 2 + (c.grain >> 1);
            let flanks: Vec<Rim> = upper
                .iter()
                .copied()
                .filter(|r| {
                    let (dx, dy) = r.d.delta();
                    r.d != N && !taken(r.x + dx, r.y + dy) && !taken(r.x + 2 * dx, r.y + 2 * dy)
                })
                .collect();
            let mut k = c.grain & 1;
            while (k as usize) < flanks.len() {
                for flip in [false, true] {
                    if next_bit() == 0 {
                        continue;
                    }
                    let r = flanks[k as usize];
                    let (dx, dy) = r.d.delta();
                    let mut tmp = Mask::new();
                    tmp.dot(r.x + dx, r.y + dy, 3);
                    m.union(&if flip { tmp.mirrored() } else { tmp });
                }
                k += step;
            }
            let p = ctx.add("warts", "coat", Z_HEAD, m, single, true);
            ctx.parts[p].ord -= 1000;
        }
        _ => {}
    }
}
