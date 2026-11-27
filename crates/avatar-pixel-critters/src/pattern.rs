//! Patches of another ramp laid over the body: a muzzle, a belly, spots,
//! patches and darker points. They are kept to what reads as an animal's own
//! coat; bands, bars and stripes read as clothing and are not drawn.

use avatar_svg::Mood;

use crate::genes::{Belly, Marks, Muzzle, TopKind};
use crate::grid::{clamp, disc_rows, oval, rndi, Mask, AX, CH, CW};
use crate::palette::{BELLY, MARK, POINT};
use crate::sprite::{col_of, overlay, row_of, Ctx, LobeId, Marking};

/// The cells of a disc of diameter `d` whose box has its top-left corner at
/// (x, y), as flat indices; a column off the canvas runs into the next row,
/// as in the reference.
fn disc_cells(x: i32, y: i32, d: i32) -> Vec<i32> {
    let rows = disc_rows(d);
    let mut out = Vec::new();
    for j in 0..d {
        let w = rows[j as usize];
        let off = (d - w) >> 1;
        for i in 0..w {
            out.push((y + j) * CW + x + off + i);
        }
    }
    out
}

/// The cell mirrored across the axis.
fn mirror_cell(c: i32) -> i32 {
    let x = c % CW;
    c - x + (CW - 1 - x)
}

pub(crate) fn build_pattern(ctx: &mut Ctx, marks0: &mut Vec<Marking>) {
    let g = ctx.g;
    let pt = &g.pattern;
    let fz = &g.face;
    let face = ctx.f.clone();
    let hd = ctx.l.head.clone();
    let trunk = ctx.trunk().clone();
    let (fdx, fdy) = (ctx.fdx, ctx.fdy);
    let split = ctx.l.split;
    let welded = ctx.l.welded;
    // The face keeps a clear patch round it, which moves with the face.
    let in_clear = |x: i32, y: i32, margin: i32| {
        let (x, y) = (x - fdx, y - fdy);
        let c = &face.clear;
        x >= c.x0 - margin && x <= c.x1 + margin && y >= c.y0 - margin && y <= c.y1 + margin
    };
    // Where a second eye would be, beside one eye alone.
    let beside = |x: i32, y: i32| {
        let (x, y) = (x - fdx, y - fdy);
        face.beside
            .is_some_and(|b| x >= b.x0 && x <= b.x1 && y >= b.y0 && y <= b.y1)
    };
    let mut used = Mask::new();

    // ---- muzzle: a small rounded patch round the mouth ----
    if (fz.muzzle == Muzzle::Muzzle || fz.muzzle == Muzzle::MuzzleAndNose)
        && hd.a >= 7
        && face.bumps.is_none()
    {
        let m = face.mouth;
        let mwz = (m.w + 2).min(8).min(2 * (hd.width_at(m.y) - 3));
        let y0 = (face.eye_bottom + 1).max(m.y - 1);
        let y1 = (hd.bottom - 2).min(m.y + 2);
        // A mouth that moved up for want of a nose has a muzzle only if it
        // had room for one where it was.
        let had = (hd.bottom - 2).min(m.with + 2) - (face.eye_bottom + 1).max(m.with - 1) >= 2
            && (m.w + 2).min(8).min(2 * (hd.width_at(m.with) - 3)) >= 4;
        if mwz >= 4 && y1 - y0 >= 2 && had {
            let mut mask = Mask::new();
            oval(&mut mask, AX - mwz / 2 + fdx, y0 + fdy, mwz, y1 - y0 + 1);
            let head = ctx.head;
            overlay(&mut ctx.parts[head], &mask.cells(), BELLY);
            used.union(&mask);
        }
    }

    // ---- belly: one soft rounded patch on the lower body ----
    let mut belly_top = CH;
    let mut belly_lobe: Option<LobeId> = None;
    if pt.belly != Belly::None {
        let size = pt.belly_size as usize;
        let y1 = trunk.bottom - 1;
        let y0 = if split {
            (hd.bottom + 1).max(trunk.top + 1)
        } else {
            (face.clear.y1 + 1).max(trunk.top + rndi(f64::from(trunk.len()) * 0.5))
        };
        let a_low = trunk.width_at(clamp(y1 - 1, trunk.top, trunk.bottom));
        let rows = y1 - y0 + 1;
        let mut m = Mask::new();
        if rows >= 3 && a_low >= 4 {
            let al = f64::from(a_low);
            match pt.belly {
                Belly::Round => {
                    let bw = 2 * clamp(rndi(al * [0.4, 0.48, 0.56, 0.64][size]), 2, a_low - 2);
                    oval(&mut m, AX - bw / 2, y0, bw, rows);
                }
                Belly::Low => {
                    let bw = 2 * clamp(rndi(al * [0.5, 0.58, 0.66, 0.74][size]), 2, a_low - 2);
                    let hh = rows.min(3 + (size as i32 & 1));
                    oval(&mut m, AX - bw / 2, y1 - hh + 1, bw, hh);
                }
                _ => {
                    let bw = if a_low >= 6 && size >= 2 { 6 } else { 4 };
                    let hh = rows.min(3 + (size as i32 & 1));
                    let raise = i32::from(rows > hh + 1);
                    oval(&mut m, AX - bw / 2, y1 - hh + 1 - raise, bw, hh);
                }
            }
            // The belly keeps two cells of the body's own colour at each side
            // in every row: a pale patch from edge to edge is a band.
            let part = ctx.trunk_part;
            let pm = &ctx.parts[part].mask;
            let cells: Vec<i32> = m
                .cells()
                .into_iter()
                .filter(|&c| {
                    !in_clear(col_of(c), row_of(c), 0)
                        && pm.at(c - 2)
                        && pm.at(c + 2)
                        && pm.at(c - 3)
                        && pm.at(c + 3)
                })
                .collect();
            overlay(&mut ctx.parts[part], &cells, BELLY);
            used.union(&m);
            if let Some(b) = m.bounds() {
                belly_top = b.y0;
                belly_lobe = Some(if split { LobeId::Torso } else { LobeId::Head });
            }
        }
    }

    // ---- markings ----
    if pt.marks == Marks::None {
        return;
    }
    let head = ctx.head;
    let body_parts: Vec<usize> = if split {
        vec![head, ctx.torso.expect("a split body has a torso")]
    } else {
        vec![head]
    };
    let lobe_of = |id: LobeId| if id == LobeId::Torso { &trunk } else { &hd };
    let (spots, patches, points) = match pt.marks {
        Marks::Spots | Marks::PaleSpots => (true, false, false),
        Marks::Patches | Marks::OnePatch => (false, true, false),
        Marks::Points => (false, false, true),
        Marks::SpotsAndPoints => (true, false, true),
        Marks::PatchesAndPoints => (false, true, true),
        Marks::None => (false, false, false),
    };
    // Spots and patches are placed once, on the neutral pose, and kept by
    // where they lie on their lobe: a mood moves the creature, not its marks.
    if ctx.mood == Mood::Neutral {
        marks0.clear();
        let reader = crate::genes::Reader::new(ctx.bytes);
        let mut bit_at = pt.at * 8;
        let mut next = |n: u32| {
            let mut v = 0;
            for i in 0..n {
                v |= reader.bit_at(bit_at + i as usize) << i;
            }
            bit_at += n as usize;
            v as i32
        };
        let parts = &ctx.parts;
        let inside2 = |c: i32| {
            body_parts.iter().any(|&p| {
                let m = &parts[p].mask;
                m.at(c) && m.at(c - 1) && m.at(c + 1) && m.at(c - CW) && m.at(c + CW)
            })
        };
        let clear_of = |used: &Mask, c: i32| {
            !used.at(c)
                && !used.at(c - 1)
                && !used.at(c + 1)
                && !used.at(c - CW)
                && !used.at(c + CW)
        };
        let free = |used: &Mask, c: i32| {
            let (x, y) = (col_of(c), row_of(c));
            inside2(c) && !in_clear(x, y, 0) && !beside(x, y) && clear_of(used, c)
        };
        let mut keep = |used: &mut Mask, cells: &[i32], ramp: u8, lobe: LobeId| {
            let lb = lobe_of(lobe);
            marks0.push(Marking {
                lobe,
                ramp,
                cells: cells
                    .iter()
                    .map(|&c| (c % CW - AX, row_of(c) - lb.bottom))
                    .collect(),
            });
            for &c in cells {
                used.set_at(c);
            }
        };
        if spots {
            // A few whole spots: two cells square, or a rounded four.
            let n = 1 + pt.count;
            let d = if pt.big != 0 { 4 } else { 2 };
            let pale = pt.marks == Marks::PaleSpots;
            let ramp = if pale { BELLY } else { MARK };
            let mut spots_at = Mask::new();
            // Spots may take about a tenth of the body between them.
            let mut budget = 6.0;
            for &p in &body_parts {
                for _ in parts[p].mask.cells() {
                    budget += 0.1;
                }
            }
            for k in 0..n {
                // No bits are read for a spot the budget cannot pay for.
                if budget < if pt.mirror { 8.0 } else { 4.0 } {
                    break;
                }
                // Two bits across the half body, two down it, and one for the
                // side if it is not one of a mirrored pair.
                let fx = next(2);
                let fy = next(2);
                let left = if pt.mirror { 0 } else { next(1) };
                let lobe = if split && (k & 1) != 0 {
                    LobeId::Torso
                } else {
                    LobeId::Head
                };
                let lb = lobe_of(lobe);
                let y0 = lb.top + 1 + rndi(f64::from((lb.len() - 2 - d) * fy) / 3.0);
                let x0 = AX + rndi(f64::from(0.max(lb.width_at(y0) - 2 - d) * fx) / 3.0);
                // The free place nearest to that; a rounded four that finds
                // none tries again as a two.
                let mut done = false;
                let sizes: &[i32] = if d == 4 { &[4, 2] } else { &[2] };
                for &dd in sizes {
                    let mut r: i32 = 0;
                    while r <= 9 && !done {
                        let mut dy = -r;
                        while dy <= r && !done {
                            let mut dx = -r;
                            while dx <= r && !done {
                                if dx.abs().max(dy.abs()) == r {
                                    let (x, y) = (x0 + dx, y0 + dy);
                                    // A mirrored pair keeps off the axis.
                                    let fits = !(x < AX + i32::from(pt.mirror)
                                        || y < lb.top + 1
                                        || y + dd > lb.bottom + 1
                                        // A pale spot keeps above the belly.
                                        || (pale && Some(lobe) == belly_lobe && y + dd > belly_top - 1));
                                    if fits {
                                        let mut cells = disc_cells(x, y, dd);
                                        // Two spots side by side above the
                                        // mouth are a second pair of eyes.
                                        let above = lobe == LobeId::Head && y < face.mouth.with;
                                        let pair_up = pt.mirror && !above;
                                        if pair_up {
                                            let mirrored: Vec<i32> =
                                                cells.iter().map(|&c| mirror_cell(c)).collect();
                                            cells.extend(mirrored);
                                        } else if left != 0 {
                                            cells = cells.iter().map(|&c| mirror_cell(c)).collect();
                                        }
                                        let ok = cells.len() as f64 <= budget
                                            && cells.iter().all(|&c| free(&used, c))
                                            // Nor may a lone spot land where it
                                            // mirrors one already there.
                                            && !(!pair_up
                                                && above
                                                && cells.iter().any(|&c| {
                                                    let q = mirror_cell(c);
                                                    [0, -1, 1, -CW, CW, -2, 2]
                                                        .iter()
                                                        .any(|&o| spots_at.at(q + o))
                                                }));
                                        if ok {
                                            budget -= cells.len() as f64;
                                            for &c in &cells {
                                                spots_at.set_at(c);
                                            }
                                            keep(&mut used, &cells, ramp, lobe);
                                            done = true;
                                        }
                                    }
                                }
                                dx += 1;
                            }
                            dy += 1;
                        }
                        r += 1;
                    }
                }
            }
        }
        if patches {
            // Large patches that wrap round the edge of the body, never
            // mirrored, clipped where they meet the face: the first in the
            // upper half of the body, the second on the other side, lower.
            let n = if pt.marks == Marks::OnePatch { 1 } else { 2 };
            let top = hd.top;
            let rows = if welded { trunk.bottom } else { hd.bottom } - top + 1;
            let wide_at = |y: i32| {
                hd.width_at(y)
                    .max(if welded { trunk.width_at(y) } else { 0 })
            };
            let side = if next(1) != 0 { -1 } else { 1 };
            let head_mask = &parts[head].mask;
            for k in 0..n {
                let fy = next(3);
                let grow = next(1);
                let d = 9.min(if n == 1 { 6 } else { 5 } + pt.big + grow + (pt.count >> 1));
                let y = if n == 1 {
                    top + rndi(f64::from((rows - 1) * fy) / 7.0)
                } else {
                    let half = if k != 0 { rows >> 1 } else { 0 };
                    top + half + rndi(f64::from(((rows >> 1) - 1) * fy) / 7.0)
                };
                let toward = if k != 0 { -side } else { side };
                let x = AX + toward * (wide_at(clamp(y, top, top + rows - 1)) - 1 - (pt.count & 1));
                let cells: Vec<i32> = disc_cells(x - ((d - 1) >> 1), y - ((d - 1) >> 1), d)
                    .into_iter()
                    .filter(|&c| {
                        c > CW
                            && c < CW * (CH - 1)
                            && head_mask.at(c)
                            && !in_clear(col_of(c), row_of(c), 1)
                            && clear_of(&used, c)
                    })
                    .collect();
                if cells.len() >= 6 {
                    let lobe = if welded && y > hd.bottom {
                        LobeId::Torso
                    } else {
                        LobeId::Head
                    };
                    keep(&mut used, &cells, MARK, lobe);
                }
            }
        }
    }
    for mk in marks0.iter() {
        let lb = lobe_of(mk.lobe);
        let cells: Vec<i32> = mk
            .cells
            .iter()
            .map(|&(dx, dy)| (lb.bottom + dy) * CW + AX + dx)
            .filter(|&c| c > 0 && c < CW * CH)
            .collect();
        for &p in &body_parts {
            overlay(&mut ctx.parts[p], &cells, mk.ramp);
        }
    }
    if points {
        // Points, as on a Siamese cat: the ears and the tail a shade darker
        // than the body, whole.
        let long = g.top.kind == TopKind::Long;
        for p in ctx.parts.iter_mut() {
            if !p.line_only && (p.group == "ears" || p.group == "tail") {
                p.ramp = if p.group == "ears" && long {
                    POINT
                } else {
                    MARK
                };
            }
        }
    }
}
