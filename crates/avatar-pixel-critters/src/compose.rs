//! Stacking the parts and colouring every cell: outline, seams, shading
//! from the upper left, and the clean-up that keeps patches whole and
//! single stray pixels out of the picture.

use crate::genes::CoatKind;
use crate::grid::{inside, Mask, CELLS, CH, CW};
use crate::palette::{
    ramp_colour, BASE, DEEP, GLINT, INNER, LIGHT, OUTLINE_DARK, OUTLINE_LIT, SHADE,
};
use crate::sprite::Part;

pub(crate) const NB4: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
pub(crate) const NB8: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (-1, -1),
    (1, -1),
    (-1, 1),
];

/// Per cell of the canvas: the colour index (0 for empty), the index of the
/// part that owns it (-1 for none), and whether it is outline (1), a seam
/// between two depths (2) or neither (0).
pub(crate) struct Comp {
    pub(crate) col: Vec<u8>,
    pub(crate) own: Vec<i16>,
    pub(crate) line: Vec<u8>,
}

/// How many steps one can take from (x, y) along (dx, dy) without leaving
/// the mask, up to `cap`.
fn reach(m: &Mask, x: i32, y: i32, dx: i32, dy: i32, cap: i32) -> i32 {
    let mut k = 0;
    while k < cap && m.has(x + (k + 1) * dx, y + (k + 1) * dy) {
        k += 1;
    }
    k
}

/// The shading level of each cell of a part. Light falls from the upper
/// left; the bands are measured along the diagonal, so on a round part they
/// are crescents that fade out at the upper right and the lower left.
fn shade_part(p: &Part) -> Vec<u8> {
    let mut lv = vec![BASE; CELLS];
    let Some(b) = p.mask.bounds() else {
        return lv;
    };
    if p.flat || p.line_only {
        return lv;
    }
    let darken = |lv: &mut Vec<u8>| {
        if p.darker != 0 {
            for (i, level) in lv.iter_mut().enumerate() {
                if p.mask.at(i as i32) {
                    *level = DEEP.min(*level + p.darker);
                }
            }
        }
    };
    let thick = b.w.min(b.h);
    // A part three cells thick has one row inside its outline: too little
    // to shade.
    if thick <= 3 {
        darken(&mut lv);
        return lv;
    }
    let shade_band = if thick <= 11 { 2 } else { 3 };
    let light_band = if p.style == CoatKind::Flat {
        0
    } else if thick >= 6 {
        2
    } else if thick >= 4 {
        1
    } else {
        0
    };
    let mut glint = -1;
    let mut glint_score = 1e9;
    let (bw, bh) = (f64::from(b.w), f64::from(b.h));
    for y in b.y0..=b.y1 {
        for x in b.x0..=b.x1 {
            if !p.mask.has(x, y) {
                continue;
            }
            let br = reach(&p.mask, x, y, 1, 1, 3);
            let tl = reach(&p.mask, x, y, -1, -1, 3);
            // How far towards the lower right the cell lies in the box, 0..2.
            let diag = (f64::from(x - b.x0) + 0.5) / bw + (f64::from(y - b.y0) + 0.5) / bh;
            let sb = if diag > 1.45 {
                shade_band
            } else if diag > 1.08 {
                shade_band - 1
            } else {
                0
            };
            let lb = if diag < 0.55 {
                light_band
            } else if diag < 0.92 {
                light_band - 1
            } else {
                0
            };
            let level = if p.style == CoatKind::Sleek && thick >= 8 && br == 0 && diag > 1.08 {
                DEEP
            } else if br < sb {
                SHADE
            } else if tl < lb {
                LIGHT
            } else {
                BASE
            };
            let c = y * CW + x;
            lv[c as usize] = level;
            if p.style == CoatKind::Glossy
                && thick >= 8
                && tl == 2
                && br >= 3
                && p.mask.has(x + 1, y)
                && reach(&p.mask, x + 1, y, -1, -1, 3) >= 2
            {
                let score = diag * 100.0
                    + (f64::from(x - b.x0) / bw - f64::from(y - b.y0) / bh).abs() * 10.0;
                if score < glint_score {
                    glint_score = score;
                    glint = c;
                }
            }
        }
    }
    if glint >= 0 {
        lv[glint as usize] = GLINT;
        lv[glint as usize + 1] = GLINT;
    }
    darken(&mut lv);
    lv
}

/// Stacks the parts by depth and colours every cell.
pub(crate) fn compose(parts: &[Part]) -> Comp {
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by_key(|&i| (parts[i].z, parts[i].ord));
    let mut own = vec![-1i16; CELLS];
    for &i in &order {
        for c in parts[i].mask.cells() {
            own[c as usize] = i as i16;
        }
    }
    let levels: Vec<Vec<u8>> = parts.iter().map(shade_part).collect();
    let mut col = vec![0u8; CELLS];
    let mut line = vec![0u8; CELLS];
    let mut ramp_at = vec![255u8; CELLS];
    let mut level_at = vec![0u8; CELLS];
    let owner = |x: i32, y: i32, own: &[i16]| -> i16 {
        if inside(x, y) {
            own[(y * CW + x) as usize]
        } else {
            -1
        }
    };
    for y in 0..CH {
        for x in 0..CW {
            let c = (y * CW + x) as usize;
            let o = own[c];
            if o < 0 {
                continue;
            }
            let part = &parts[o as usize];
            let (mut outer, mut seam) = (false, false);
            for (dx, dy) in NB4 {
                let n = owner(x + dx, y + dy, &own);
                if n < 0 {
                    outer = true;
                } else if parts[n as usize].z < part.z {
                    seam = true;
                }
            }
            if part.line_only {
                line[c] = 1;
                col[c] = OUTLINE_DARK;
                continue;
            }
            if outer {
                // Which way the outline faces: an outline that faces up or
                // to the left catches the light; one that faces neither way
                // takes after its upper side.
                let (mut fx, mut fy) = (0, 0);
                for (dx, dy) in NB8 {
                    if owner(x + dx, y + dy, &own) < 0 {
                        fx += dx;
                        fy += dy;
                    }
                }
                let lit = -(fx + fy);
                line[c] = 1;
                col[c] = if lit > 0 || (lit == 0 && fy < 0) {
                    OUTLINE_LIT
                } else {
                    OUTLINE_DARK
                };
                continue;
            }
            if seam {
                line[c] = 2;
                col[c] = INNER;
                continue;
            }
            let mut level = levels[o as usize][c];
            // A part in front casts a one-cell shadow down and to the right.
            for (dx, dy) in [(-1, -1), (0, -1)] {
                let (xx, yy) = (x + dx, y + dy);
                if !inside(xx, yy) {
                    continue;
                }
                let n = own[(yy * CW + xx) as usize];
                if n >= 0
                    && parts[n as usize].z > part.z
                    && !parts[n as usize].line_only
                    && level < SHADE
                {
                    level = SHADE;
                }
            }
            let ci = c as i32;
            let ramp = part
                .over
                .as_ref()
                .and_then(|over| over.get(&ci).copied())
                .unwrap_or(part.ramp);
            if part.lift.as_ref().is_some_and(|lift| lift.contains(&ci)) && level > GLINT {
                level -= 1;
            }
            ramp_at[c] = ramp;
            level_at[c] = level;
            col[c] = ramp_colour(ramp, level);
        }
    }
    // A belly, a muzzle or a marking is a whole patch: a cell of one that is
    // not part of a 2 × 2 block of the same kind goes back to the colour of
    // the part it lies on.
    let cw = CW as usize;
    for _ in 0..3 {
        let mut drop = Vec::new();
        for y in 1..CH as usize - 1 {
            for x in 1..cw - 1 {
                let c = y * cw + x;
                if own[c] < 0 || line[c] != 0 {
                    continue;
                }
                let part = &parts[own[c] as usize];
                if ramp_at[c] == part.ramp || part.single {
                    continue;
                }
                let r = ramp_at[c];
                let whole = [(0, 0), (1, 0), (0, cw), (1, cw)].iter().any(|&(ox, oy)| {
                    let a = c - oy - ox;
                    ramp_at[a] == r
                        && ramp_at[a + 1] == r
                        && ramp_at[a + cw] == r
                        && ramp_at[a + cw + 1] == r
                });
                if !whole {
                    drop.push(c);
                }
            }
        }
        if drop.is_empty() {
            break;
        }
        for c in drop {
            ramp_at[c] = parts[own[c] as usize].ramp;
            col[c] = ramp_colour(ramp_at[c], level_at[c]);
        }
    }
    // No stray single pixels: a shaded cell with no neighbour of its own
    // colour takes the colour most of its neighbours have. The one cell
    // inside the outline of a small part is all the colour that part has,
    // and it keeps it.
    let mut fill = vec![0u32; parts.len()];
    for c in 0..CELLS {
        if own[c] >= 0 && line[c] == 0 {
            fill[own[c] as usize] += 1;
        }
    }
    let one_cell: Vec<bool> = fill.iter().map(|&n| n == 1).collect();
    for _ in 0..2 {
        for y in 1..CH - 1 {
            for x in 1..CW - 1 {
                let c = (y * CW + x) as usize;
                if own[c] < 0 || line[c] != 0 {
                    continue;
                }
                let o = own[c] as usize;
                if parts[o].single || one_cell[o] {
                    continue;
                }
                let neighbour = |dx: i32, dy: i32| (c as i32 + dy * CW + dx) as usize;
                let alone = !NB8.iter().any(|&(dx, dy)| {
                    let n = neighbour(dx, dy);
                    own[n] >= 0 && line[n] == 0 && col[n] == col[c]
                });
                if !alone {
                    continue;
                }
                let mut seen = [0u32; 256];
                let (mut best, mut best_count) = (0u8, 0u32);
                // Its direct neighbours decide; a cell that only touches
                // others at its corners asks those.
                for ring in [&NB4[..], &NB8[..]] {
                    for &(dx, dy) in ring {
                        let n = neighbour(dx, dy);
                        if own[n] < 0 || line[n] != 0 {
                            continue;
                        }
                        let k = &mut seen[col[n] as usize];
                        *k += 1;
                        if *k > best_count {
                            best_count = *k;
                            best = col[n];
                        }
                    }
                    if best != 0 {
                        break;
                    }
                }
                if best != 0 {
                    col[c] = best;
                }
            }
        }
    }
    Comp { col, own, line }
}
