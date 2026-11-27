//! The face: where the eyes and the mouth go, and how each mood draws them.

use avatar_svg::Mood;

use crate::compose::Comp;
use crate::genes::{
    BlockedFace, BrowGene, Cheeks, EyePlan, EyeStyle, FaceGenes, Genes, LidGene, MouthKind, Muzzle,
    Quirk,
};
use crate::grid::{clamp, inside, rndi, Mask, AX, CW};
use crate::palette::{
    ramp_colour, BASE, BEAK, BLUSH, INK, INNER, IRIS, IRIS_LIGHT, MOUTH_IN, NOSE, PUPIL, SHADE,
    SHINE, TEAR, TEETH, TONGUE, WHITE,
};
use crate::sprite::{Ctx, Entry, Layout, PartOpts, Z_HEAD};

/// One eye's box on the working canvas. `side` is 1 for the right eye, -1
/// for the left and 0 for an eye on the axis.
#[derive(Clone, Debug)]
pub(crate) struct Eye {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) side: i32,
    pub(crate) third: bool,
}

/// The bumps or stalks that carry eyes above the head.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bumps {
    pub(crate) d: i32,
    pub(crate) ex: i32,
    pub(crate) top: i32,
    pub(crate) stalk: i32,
}

/// A box of cells, both ends included.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Area {
    pub(crate) x0: i32,
    pub(crate) x1: i32,
    pub(crate) y0: i32,
    pub(crate) y1: i32,
}

/// Where the mouth goes and how wide it is.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MouthLayout {
    pub(crate) kind: MouthKind,
    pub(crate) w: i32,
    pub(crate) y: i32,
    /// The row the mouth was laid out in before it lost a nose, if it did.
    pub(crate) with: i32,
    pub(crate) dx: i32,
}

/// The face as it is laid out on the head.
#[derive(Clone, Debug)]
pub(crate) struct Face {
    pub(crate) plan: EyePlan,
    /// The eye kind the face draws, which is not always the one its gene names.
    pub(crate) kind: EyeStyle,
    pub(crate) eyes: Vec<Eye>,
    pub(crate) bumps: Option<Bumps>,
    pub(crate) nose: bool,
    pub(crate) g: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
    /// For eyes above the head: the size the genes asked for before the eye
    /// grew to fill its bump.
    pub(crate) ask: Option<(i32, i32)>,
    pub(crate) eye_top: i32,
    pub(crate) eye_bottom: i32,
    pub(crate) clear_row: i32,
    pub(crate) mouth: MouthLayout,
    /// The part of the head that markings keep clear of.
    pub(crate) clear: Area,
    /// Beside one eye alone, where a second eye would be.
    pub(crate) beside: Option<Area>,
}

impl Face {
    pub(crate) fn empty(genes: &Genes) -> Face {
        let area = Area {
            x0: 0,
            x1: 0,
            y0: 0,
            y1: 0,
        };
        Face {
            plan: genes.face.plan,
            kind: genes.face.style,
            eyes: Vec::new(),
            bumps: None,
            nose: false,
            g: 0,
            w: 0,
            h: 0,
            ask: None,
            eye_top: 0,
            eye_bottom: 0,
            clear_row: 0,
            mouth: MouthLayout {
                kind: genes.face.mouth,
                w: 0,
                y: 0,
                with: 0,
                dx: 0,
            },
            clear: area,
            beside: None,
        }
    }
}

/// How one eye is drawn. A dark body gives every eye whites; one eye alone
/// always has whites; whites need four cells each way, or the eye is drawn dark.
fn eye_kind(f: &FaceGenes, plan: EyePlan, dark: bool, w: i32, h: i32, third: bool) -> EyeStyle {
    if dark {
        return if f.style == EyeStyle::Slit {
            EyeStyle::Slit
        } else {
            EyeStyle::Iris
        };
    }
    if third || w < 2 {
        return EyeStyle::Bead;
    }
    let single = plan == EyePlan::Cyclops;
    let mut kind = f.style;
    if single {
        kind = match kind {
            EyeStyle::Bead | EyeStyle::Sparkle | EyeStyle::Tall => EyeStyle::Sclera,
            EyeStyle::Lazy => EyeStyle::Dot,
            other => other,
        };
    }
    if has_whites(kind) && (w < 4 || h < if single { 3 } else { 4 }) {
        kind = EyeStyle::Bead;
    }
    kind
}

fn has_whites(kind: EyeStyle) -> bool {
    matches!(
        kind,
        EyeStyle::Sclera | EyeStyle::Dot | EyeStyle::Iris | EyeStyle::Slit
    )
}

/// Which cells of a `w × h` eye box the eye covers: a rectangle with its
/// corners rounded off when it is large enough to have any.
fn eye_shape(w: i32, h: i32, flat_top: bool, lidded: bool, soft: bool) -> Vec<Vec<bool>> {
    (0..h)
        .map(|j| {
            (0..w)
                .map(|i| {
                    let ci = i.min(w - 1 - i);
                    let cj = if flat_top {
                        h - 1 - j
                    } else {
                        j.min(h - 1 - j)
                    };
                    if (w >= 4 && h >= 4) || (w == 3 && h >= 5 && !lidded) || soft {
                        ci + cj >= 1
                    } else {
                        true
                    }
                })
                .collect()
        })
        .collect()
}

/// Lays the face out on the head. An eye with a coloured iris asks for the
/// room whites need; where the head has none, the face is laid out again
/// with eyes of the size their own genes give.
pub(crate) fn face_layout(ctx: &mut Ctx) {
    let grow = ctx.g.face.style == EyeStyle::Iris && !ctx.p.dark;
    let face = lay_face(ctx, grow);
    ctx.f = if grow && face.kind != EyeStyle::Iris {
        lay_face(ctx, false)
    } else {
        face
    };
}

/// Half the width of the body in a row. A head welded to a belly has no edge
/// of its own where the belly has begun: there the body is as wide as the belly.
fn w_at(l: &Layout, y: i32) -> i32 {
    let torso = if l.welded {
        l.torso.as_ref().map_or(0, |t| t.width_at(y))
    } else {
        0
    };
    l.head.width_at(y).max(torso)
}

/// The row where a welded belly first shows wider than the head.
fn crease(l: &Layout) -> i32 {
    let t = l.torso.as_ref().expect("a welded body has a belly");
    let mut crease = t.top;
    while crease < t.bottom && t.width_at(crease) <= l.head.width_at(crease) {
        crease += 1;
    }
    crease
}

fn lay_face(ctx: &Ctx, grow: bool) -> Face {
    let f = &ctx.g.face;
    let l = &ctx.l;
    let hd = &l.head;
    let a = hd.a;
    let dark = ctx.p.dark;
    let mut face = Face::empty(ctx.g);
    face.nose = f.muzzle == Muzzle::Nose || f.muzzle == Muzzle::MuzzleAndNose;
    let mut w = 3 + f.wide;
    let mut h = 3 + f.tall;
    let mut plan = f.plan;
    if f.style == EyeStyle::Tall {
        w = 3;
        h = clamp(h + 1, 4, 6);
    }
    if f.style == EyeStyle::Lazy {
        w += 1;
        h = 3;
    }
    if matches!(f.style, EyeStyle::Sclera | EyeStyle::Dot | EyeStyle::Slit) || grow {
        w = w.max(4);
        h = h.max(if f.lid == LidGene::Heavy { 5 } else { 4 });
    }
    // A wide head carries large eyes, and so does a dark body.
    if a >= 8 || dark {
        w = w.max(4);
        h = h.max(4);
    }
    if a >= 9 {
        w = w.max(5);
    }
    if plan == EyePlan::Big {
        w += 1;
        h = h.max(6.min(w));
    }
    // One eye is never taller than wide: taller, it is a slot.
    if plan == EyePlan::Cyclops {
        w = 4 + 2 * f.wide;
        h = clamp(3 + f.tall, 4, w);
    }
    // Never much taller than wide, nor taller than a good share of the head.
    h = h
        .min(w + if f.style == EyeStyle::Tall { 3 } else { 1 })
        .min(3.max((f64::from(hd.len()) * 0.45).floor() as i32));
    // One eye alone is round or square: at most a cell wider than tall under
    // a lash line, two on a dark body.
    let widest = |hh: i32| if dark { hh + 2 } else { hh + 1 };
    // How wide one eye alone is before that rule narrows it.
    let mut full = w;
    if plan == EyePlan::Cyclops && w > widest(h) {
        w = 4;
    }
    let mid = hd.widest_row();
    let span: f64;
    // The lowest row a mouth may reach, for eyes on the face.
    let mut lowest = 0;
    if plan == EyePlan::Frog || plan == EyePlan::Stalks {
        // Eyes above the head, in bumps of their own (frog) or on stalks.
        w = w.min(3 + f.wide);
        h = clamp(h, 3, 4);
        if f.style == EyeStyle::Lazy {
            w = 3;
            h = 3;
        }
        // Such an eye fills its bump, so it is as wide as tall; what kind of
        // eye it is still goes by the size its genes asked for.
        let ask = (w, h);
        face.ask = Some(ask);
        w = if a >= 8 { 4 } else { w.max(h) };
        h = w;
        let d = w.max(h) + 2;
        // The bumps stand on the crown, half above its highest row as long
        // as two rows still sit on the head.
        let crown =
            hd.rows[((hd.len() - 1).min(clamp(rndi(f64::from(hd.len()) * 0.2), 1, 3))) as usize];
        let ex = clamp(
            rndi(f64::from(a) * 0.55),
            (d >> 1) + 1,
            ((d >> 1) + 1).max((a - 2).min(crown)),
        );
        let stalk = if plan == EyePlan::Stalks {
            1 + f.row
        } else {
            0
        };
        let mut top = hd.top_at((a - 1).min(ex))
            - if plan == EyePlan::Stalks {
                d + stalk - 1
            } else {
                d - 3
            };
        if plan == EyePlan::Frog {
            top = (hd.top_at(0.max(ex - (d >> 1) + 1)) + 2 - d).max(top.min(hd.top - (d >> 1)));
        }
        face.bumps = Some(Bumps { d, ex, top, stalk });
        let x = AX + ex - (d >> 1) + ((d - w) >> 1);
        let y = top + ((d - h) >> 1);
        face.eyes = vec![
            Eye {
                x,
                y,
                w,
                h,
                side: 1,
                third: false,
            },
            Eye {
                x: CW - x - w,
                y,
                w,
                h,
                side: -1,
                third: false,
            },
        ];
        face.g = x - AX;
        face.w = w;
        face.h = h;
        // Half the span of the eyes as the markings see it: before the eye
        // filled its bump, so that a wider eye in the same bump moves nothing.
        span = f64::from(ex - (d >> 1) + ((d - ask.0) >> 1) + ask.0);
        face.eye_top = hd.top;
        face.eye_bottom = hd.top + 1.max(rndi(f64::from(hd.len()) * 0.22));
    } else {
        // The eye rows lie inside the outline with a cell of skin to spare;
        // `tight` gives that cell up in the middle rows as well.
        let fits_at = |y: i32, g: i32, ww: i32, hh: i32, tight: bool| {
            (0..hh).all(|j| {
                let base = if plan == EyePlan::Cyclops {
                    f64::from(ww) / 2.0
                } else {
                    f64::from(g + ww)
                };
                let need = base
                    + if tight || j == 0 || j == hh - 1 {
                        1.0
                    } else {
                        2.0
                    };
                f64::from(w_at(l, y + j)) >= need
            })
        };
        // Under the eyes: a clear row, a row for a nose, two for the mouth.
        let (mut g, mut y, mut room, mut clear) = (1, -1, 3 + i32::from(face.nose), 1);
        // The lowest row a mouth may reach: a head welded to a belly ends,
        // to the eye, at the crease.
        lowest = hd.bottom - 1;
        if l.welded {
            lowest = lowest.min(crease(l) + 1);
        }
        let row_share = [0.15, 0.35, 0.55, 0.75][f.row as usize];
        for _ in 0..60 {
            if y >= 0 {
                break;
            }
            let span0 = w_at(l, mid) - w - 2;
            let g0 = match plan {
                EyePlan::Cyclops => 0,
                EyePlan::Wide => clamp(span0, 1, 3),
                EyePlan::Close => 1,
                _ => clamp(rndi(f64::from(span0) * 0.5), 1, 3),
            };
            let want = hd.top
                + 2
                + rndi(f64::from(hd.len() - h - room - 2) * row_share)
                + i32::from(plan == EyePlan::Three);
            // The nearest row to the wanted one in which the eyes fit. Eyes
            // that find none move closer together before anything shrinks,
            // and eyes of three cells each way stand against the outline
            // before they shrink to two.
            let mut best = -1;
            let tights: &[bool] = if w <= 3 && h <= 3 {
                &[false, true]
            } else {
                &[false]
            };
            'search: for &tight in tights {
                let mut gg = g0;
                while gg >= g0.min(1) {
                    let mut best_cost = i32::MAX;
                    let mut yy = hd.top + 2;
                    while yy + h - 1 + room <= lowest {
                        if fits_at(yy, gg, w, h, tight) {
                            let cost = (yy - want).abs();
                            if cost < best_cost {
                                best_cost = cost;
                                best = yy;
                            }
                        }
                        yy += 1;
                    }
                    if best >= 0 {
                        g = gg;
                        break 'search;
                    }
                    gg -= 1;
                }
            }
            if best >= 0 {
                y = best;
                break;
            }
            // No room: the face gives way a little at a time. A shorter eye
            // first, then no nose, then a mouth of one row, then a narrower
            // eye; the clear row under the eyes goes last. One eye alone
            // keeps its fourth row longer.
            let was = w;
            let cyclops = plan == EyePlan::Cyclops;
            if cyclops && h == 4 && face.nose {
                face.nose = false;
                room -= 1;
            } else if cyclops && h == 4 && room - clear > 1 {
                room -= 1;
            } else if h > 3 {
                h -= 1;
            } else if face.nose {
                face.nose = false;
                room -= 1;
            } else if room - clear > 1 {
                room -= 1;
            } else if w > 2 {
                w -= if cyclops { 2 } else { 1 };
            } else if h > 2 {
                h -= 1;
            } else if clear != 0 {
                clear = 0;
                room -= 1;
            } else {
                break;
            }
            if cyclops && w < 2 {
                w = 2;
            }
            full = w.max(full - (was - w));
            if cyclops && w > widest(h) {
                w = 4;
            }
        }
        if y < 0 {
            y = hd.top + 2;
            w = 2;
            h = 2;
            g = 1;
            clear = 0;
            if plan != EyePlan::Cyclops {
                plan = EyePlan::Close;
            }
        }
        // Half the span of the eyes as laid out: the mouth and the markings
        // go by it, so that an eye made narrower later moves neither.
        span = if plan == EyePlan::Cyclops {
            f64::from(w.max(full)) / 2.0
        } else {
            f64::from(g + w)
        };
        if plan == EyePlan::Cyclops {
            face.eyes.push(Eye {
                x: AX - (w >> 1),
                y,
                w,
                h,
                side: 0,
                third: false,
            });
        } else {
            face.eyes.push(Eye {
                x: AX + g,
                y,
                w,
                h,
                side: 1,
                third: false,
            });
            face.eyes.push(Eye {
                x: AX - g - w,
                y,
                w,
                h,
                side: -1,
                third: false,
            });
            if plan == EyePlan::Three && y - 3 > hd.top && w_at(l, y - 3) >= 3 {
                face.eyes.push(Eye {
                    x: AX - 1,
                    y: y - 3,
                    w: 2,
                    h: 2,
                    side: 0,
                    third: true,
                });
            }
        }
        face.g = g;
        face.w = w;
        face.h = h;
        face.eye_top = y;
        face.eye_bottom = y + h - 1;
        face.clear_row = clear;
    }
    face.plan = plan;
    let (kw, kh) = face.ask.unwrap_or((face.w, face.h));
    face.kind = eye_kind(f, plan, dark, kw, kh, false);

    // ---- mouth ----
    let outer = span;
    let inner_at = |y: i32| (w_at(l, y) - 1).min(w_at(l, y - 1)).min(w_at(l, y + 1));
    let fit_w = |y: i32| 2 * 1.max(inner_at(y).min(inner_at(y + 1)) - 1);
    let bumps = face.bumps.is_some();
    // Under eyes carried above the head the mouth is all the face the head
    // itself shows, so it is never the two-cell kind.
    let mouth = if bumps && f.mouth == MouthKind::Tiny {
        MouthKind::Smile
    } else {
        f.mouth
    };
    let mut kept: Option<(i32, i32)> = None;
    let mut ym = 0;
    let mut m = face.mouth;
    for pass in 0..2 {
        let mut mw = 2 + 2 * f.mouth_wide;
        match mouth {
            MouthKind::Tiny => mw = 2,
            MouthKind::Cat => mw = mw.max(6),
            MouthKind::Maw => mw = mw.max(6) + 2,
            MouthKind::Beak => mw = if mw >= 6 { 6 } else { 4 },
            MouthKind::Buck => mw = mw.max(4),
            _ => {}
        }
        if bumps {
            mw = mw.max(4);
        }
        // No mouth is wider than half the head: edge to edge it is a band.
        mw = mw.min(8).min(a - (a & 1));
        // On a dark body the mouth is a pale line, and a wide one is teeth.
        if dark {
            mw = mw.min(if mouth == MouthKind::Maw { 6 } else { 4 });
        }
        if bumps {
            ym = (hd.bottom - 3).min(
                hd.top
                    + 2.max(rndi(f64::from(hd.len()) * 0.28))
                    + i32::from(face.nose)
                    + f.mouth_drop,
            );
        } else {
            ym = face.eye_bottom + 1 + face.clear_row + i32::from(face.nose);
            if f.mouth_drop != 0 && ym + 2 <= lowest && fit_w(ym + 1) >= mw {
                ym += 1;
            }
        }
        mw = mw.min(fit_w(ym));
        if !bumps {
            mw = mw.min(2 * 1.max(rndi(outer)));
        }
        // A mouth that has lost its nose stays the mouth it was.
        if let Some((kept_w, _)) = kept {
            mw = mw.min(kept_w);
        }
        m.kind = mouth;
        if mouth == MouthKind::Cat && mw < 6 {
            m.kind = MouthKind::Smile;
        }
        if mouth == MouthKind::Maw && mw < 6 {
            m.kind = MouthKind::Fangs;
        }
        if mouth == MouthKind::Beak && mw < 4 {
            m.kind = MouthKind::Tiny;
        }
        m.w = 2.max(mw - (mw & 1));
        m.y = ym;
        // A smile four cells wide curls up round a nose set on it into a
        // ring; and on a head over a torso, a nose over a mouth that reaches
        // the head's last row is a bow tie. Those faces go without the nose.
        let ring = m.kind == MouthKind::Smile && m.w == 4;
        let rest = mouth_cells(m.kind, Form::Rest, m.w, 1, 0);
        let mut rows = 1 + rest.iter().map(|c| c.1).max().unwrap_or(0);
        if f.quirk == Quirk::Blep && m.kind != MouthKind::Buck && m.kind != MouthKind::Maw {
            let under = rest
                .iter()
                .filter(|c| c.0 == -1 || c.0 == 0)
                .map(|c| c.1)
                .fold(0, i32::max);
            rows = rows.max(2 + under);
        }
        let collar = l.split && !bumps && ym + rows >= hd.bottom;
        if pass == 0 && face.nose && (ring || collar) {
            face.nose = false;
            kept = Some((m.w, ym));
        } else {
            break;
        }
    }
    // The belly and the markings keep to the row the mouth had before it
    // lost a nose, so that the loss moves nothing but the mouth.
    m.with = kept.map_or(ym, |(_, y)| y);
    m.dx = if f.quirk == Quirk::Smirk {
        if f.quirk_side != 0 {
            1
        } else {
            -1
        }
    } else {
        0
    };
    face.mouth = m;
    let r_outer = rndi(outer);
    face.clear = Area {
        x0: AX - r_outer - 1,
        x1: AX + r_outer,
        y0: if bumps {
            m.with - 1 - i32::from(kept.is_some() || face.nose)
        } else {
            face.eye_top - 3
        },
        y1: m.with + 2,
    };
    // Beside one eye alone, spots and patches keep clear of where a second
    // eye would be.
    face.beside = (plan == EyePlan::Cyclops).then_some(Area {
        x0: AX - r_outer - 4,
        x1: AX + r_outer + 3,
        y0: face.eye_top - 1,
        y1: face.eye_bottom + 1,
    });
    face
}

/// The face a mood wears is the neutral one carried along with the head:
/// it moves with the bottom of the head lobe (with the top, for eyes above
/// the head), and a row further down if a squashed head would crowd the eyes.
pub(crate) fn face_carry(ctx: &mut Ctx, f0: &Face, l0: &Layout) {
    let h0 = &l0.head;
    let h = &ctx.l.head;
    let mut dy = if f0.bumps.is_some() {
        h.top - h0.top
    } else {
        h.bottom - h0.bottom
    };
    if f0.bumps.is_none() && f0.eye_top + dy < h.top + 1 {
        dy = h.top + 1 - f0.eye_top;
    }
    let mut face = f0.clone();
    if let Some(b) = &mut face.bumps {
        b.top += dy;
    }
    for e in &mut face.eyes {
        e.y += dy;
    }
    face.eye_top += dy;
    face.eye_bottom += dy;
    face.mouth.y += dy;
    face.mouth.with += dy;
    face.clear.y0 += dy;
    face.clear.y1 += dy;
    if let Some(b) = &mut face.beside {
        b.y0 += dy;
        b.y1 += dy;
    }
    ctx.f = face;
}

/// How far the whole face moves in this pose: a glance, or the tilt of a
/// body that has no separate head, as far as it stays inside the outline.
pub(crate) fn face_shift(ctx: &mut Ctx) {
    ctx.fdx = 0;
    ctx.fdy = 0;
    if ctx.f.bumps.is_some() {
        return;
    }
    let pose = ctx.pose;
    let side = ctx.g.temper.side;
    let (look_x, look_y) = pose.look.unwrap_or((0, 0));
    let want_x = (look_x + if ctx.l.split { 0 } else { pose.tilt }) * side;
    let want_y = look_y;
    let h = &ctx.l.head;
    let f = &ctx.f;
    let ok = |dx: i32, dy: i32| {
        for e in &f.eyes {
            for j in 0..e.h {
                let y = e.y + j + dy;
                let wd = h.width_at(y) - 2;
                if e.x + dx < AX - wd || e.x + e.w - 1 + dx > AX + wd - 1 {
                    return false;
                }
            }
        }
        let m = &f.mouth;
        if m.y + 1 + dy > h.bottom - 1 || f.eye_top + dy < h.top + 1 {
            return false;
        }
        let wm = h.width_at(m.y + dy + 1) - 2;
        m.w / 2 + (dx + m.dx).abs() <= wm
    };
    for (dx, dy) in [(want_x, want_y), (want_x, 0), (0, want_y)] {
        if (dx != 0 || dy != 0) && ok(dx, dy) {
            ctx.fdx = dx;
            ctx.fdy = dy;
            return;
        }
    }
}

/// Bumps or stalks that carry the eyes above the head.
pub(crate) fn build_eye_mounts(ctx: &mut Ctx) {
    let Some(b) = ctx.f.bumps else {
        return;
    };
    let h = &ctx.l.head;
    let mut m = Mask::new();
    let x0 = AX + b.ex - (b.d >> 1);
    m.disc(x0, b.top, b.d);
    if b.stalk != 0 {
        m.block(
            AX + b.ex - 1,
            b.top + b.d - 1,
            3,
            h.top_at(b.ex) - (b.top + b.d - 1) + 2,
        );
    }
    for p in ctx.add_pair(
        "eye-mount",
        "eye-mounts",
        Z_HEAD,
        m,
        PartOpts::default(),
        true,
        None,
    ) {
        ctx.parts[p].ord -= 800;
    }
}

/// A pair of dark eyes takes its final width once the creature's outline is
/// known, on the neutral sprite alone. A dark eye is never wider than tall
/// (a dark block wider than tall is a lens), and one with corners too small
/// to round keeps a cell of skin between itself and the outline. The eye
/// gives up a column and keeps its place: the outer one where it stands
/// against the outline, the inner one otherwise.
fn narrow_dark_eyes(ctx: &mut Ctx, on_skin: &dyn Fn(i32, i32) -> bool) {
    let f = &ctx.g.face;
    let face = &mut ctx.f;
    let pair: Vec<usize> = (0..face.eyes.len())
        .filter(|&i| !face.eyes[i].third)
        .collect();
    if face.bumps.is_some()
        || face.plan == EyePlan::Cyclops
        || has_whites(face.kind)
        || pair.len() != 2
    {
        return;
    }
    let h = face.h;
    let y = face.eyes[pair[0]].y;
    let mut g = face.g;
    let mut w = face.w;
    let cornered =
        |ww: i32| !((ww >= 4 && h >= 4) || (ww == 3 && h >= 5 && f.lid == LidGene::Open));
    // Skin `off` columns from the axis on both sides: the head is mirrored,
    // but what lies over it is not.
    let skin_at = |off: i32, yy: i32| on_skin(AX + off, yy) && on_skin(AX - 1 - off, yy);
    let against = |ww: i32, g: i32| {
        if !cornered(ww) {
            return false;
        }
        if (0..h).any(|j| !skin_at(g + ww, y + j)) {
            return true;
        }
        !skin_at(g + ww - 1, y - 1) || !skin_at(g + ww - 1, y + h)
    };
    while w > 2.max(h) {
        if !against(w, g) {
            g += 1;
        }
        w -= 1;
    }
    if w >= 3 && h <= 4 && against(w, g) {
        w -= 1;
    }
    face.g = g;
    face.w = w;
    for &i in &pair {
        let e = &mut face.eyes[i];
        e.w = w;
        e.x = if e.side > 0 { AX + g } else { AX - g - w };
    }
}

/// How the lids sit in a mood. The first four are the resting lid genes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lid {
    Open,
    Lidded,
    Heavy,
    Stern,
    Focus,
    Angry,
    Sad,
}

impl From<LidGene> for Lid {
    fn from(gene: LidGene) -> Lid {
        match gene {
            LidGene::Open => Lid::Open,
            LidGene::Lidded => Lid::Lidded,
            LidGene::Heavy => Lid::Heavy,
            LidGene::Stern => Lid::Stern,
        }
    }
}

/// The brows a mood asks for; `Rest` wears the brows the genes give.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Brow {
    None,
    Thin,
    Thick,
    Short,
    Rest,
    Low,
    Cocked,
    Angry,
    Sad,
}

impl From<BrowGene> for Brow {
    fn from(gene: BrowGene) -> Brow {
        match gene {
            BrowGene::None => Brow::None,
            BrowGene::Thin => Brow::Thin,
            BrowGene::Thick => Brow::Thick,
            BrowGene::Short => Brow::Short,
        }
    }
}

/// The form the mouth takes in a mood.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Form {
    Rest,
    Tongue,
    Flat,
    FlatSmall,
    Open,
    Side,
    Zig,
    Grit,
    Pout,
    Frown,
    O,
}

/// How shut eyes are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shut {
    Arc,
    Squeeze,
    Sleep,
}

/// The marks a mood adds beside the creature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MoodMark {
    Sweat,
    Dots,
    Anger,
    Tear,
    Zz,
}

/// What a mood asks of the face.
pub(crate) struct Spec {
    lid: Lid,
    gaze: (i32, i32),
    brow: Brow,
    mouth: Form,
    blush: bool,
    shut: Option<Shut>,
    big: bool,
    pub(crate) mark: Option<MoodMark>,
}

pub(crate) fn face_spec(ctx: &Ctx) -> Spec {
    let f = &ctx.g.face;
    let t = &ctx.g.temper;
    let mut s = Spec {
        lid: f.lid.into(),
        gaze: (0, 0),
        brow: Brow::Rest,
        mouth: Form::Rest,
        blush: f.cheeks == Cheeks::Blush || f.cheeks == Cheeks::BigBlush,
        shut: None,
        big: false,
        mark: None,
    };
    match ctx.mood {
        Mood::Neutral => {}
        Mood::Working => {
            s.lid = Lid::Focus;
            s.gaze = (0, 1);
            s.brow = Brow::Low;
            s.mouth = if t.tongue { Form::Tongue } else { Form::Flat };
            s.mark = Some(MoodMark::Sweat);
        }
        Mood::Happy => {
            if t.smile_shut {
                s.shut = Some(Shut::Arc);
            } else {
                s.lid = Lid::Open;
                s.big = true;
            }
            s.mouth = Form::Open;
            s.blush = true;
            s.brow = Brow::None;
        }
        Mood::Waiting => {
            s.gaze = (t.side, -1);
            s.brow = Brow::Cocked;
            s.mouth = Form::Side;
            s.mark = Some(MoodMark::Dots);
        }
        Mood::Blocked => {
            s.mark = Some(MoodMark::Anger);
            if t.blocked == BlockedFace::Squeeze {
                s.shut = Some(Shut::Squeeze);
                s.mouth = Form::Zig;
                s.brow = Brow::None;
            } else {
                s.lid = Lid::Angry;
                s.brow = Brow::Angry;
                s.mouth = match t.blocked {
                    BlockedFace::Teeth => Form::Grit,
                    BlockedFace::Pout => Form::Pout,
                    _ => Form::Frown,
                };
            }
        }
        Mood::Sad => {
            s.lid = Lid::Sad;
            s.gaze = (0, 1);
            s.brow = Brow::Sad;
            s.mouth = Form::Frown;
            s.mark = Some(MoodMark::Tear);
        }
        Mood::Sleepy => {
            s.shut = Some(Shut::Sleep);
            s.mouth = if t.yawn { Form::O } else { Form::FlatSmall };
            s.mark = Some(MoodMark::Zz);
            s.brow = Brow::None;
        }
    }
    s
}

/// How many rows a lid covers in each column of an eye, from the top.
/// `inner` is 1 when the eye's inner side (towards the nose) is its first
/// column, -1 when it is its last, 0 for an eye on the axis. `keep` is how
/// many rows a resting lid must leave showing; a mood's lid may take more.
#[allow(clippy::too_many_arguments)]
fn lid_cuts(lid: Lid, rest: Lid, w: i32, h: i32, inner: i32, lashed: bool, keep: i32) -> Vec<i32> {
    let lash = i32::from(lashed);
    // For an eye on the axis the inner side is its middle.
    let from_inner = |i: i32| {
        if inner > 0 {
            i
        } else if inner < 0 {
            w - 1 - i
        } else {
            (2 * i - (w - 1)).abs()
        }
    };
    // A lid that slants: k rows deep at the side it comes down on, none at
    // the other.
    let slant = |k: i32, d: i32| 0.max(k - d * (k + 1) / 1.max(w - 1));
    let resting = |i: i32| match rest {
        // A resting lid leaves three rows of the eye showing, not counting
        // a lash line: fewer, and the eye is a slit.
        Lid::Lidded => i32::from(h - lash >= 4),
        Lid::Heavy => clamp(h >> 1, 0, 0.max(h - 3 - lash)),
        // A stern lid slants down towards the nose; one eye alone has no
        // side to slant to, and there the lid lies level.
        Lid::Stern => {
            if inner == 0 {
                i32::from(h - lash >= 4)
            } else if h < 4 {
                0
            } else {
                slant(if h >= 5 { 2 } else { 1 }, from_inner(i))
            }
        }
        _ => 0,
    };
    (0..w)
        .map(|i| {
            let mut c = resting(i).min(0.max(h - keep));
            match lid {
                Lid::Open => c = 0,
                Lid::Focus => c = 0.max(h - 2).min(c + 1),
                Lid::Angry => {
                    let depth = if rest == Lid::Stern && inner != 0 {
                        3
                    } else {
                        2
                    };
                    c = c.max(slant((h - 1).min(depth), from_inner(i)));
                }
                Lid::Sad => {
                    c = (if rest == Lid::Stern { 0 } else { c })
                        .max(slant((h - 1).min(2), w - 1 - from_inner(i)));
                }
                _ => {}
            }
            c.min(h - 1)
        })
        .collect()
}

/// What a mouth cell is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    Ink,
    MouthIn,
    Tongue,
    Teeth,
    Beak,
    BeakShade,
}

/// The cells of a mouth, as (dx, dy, role) with dx counted from the axis.
pub(crate) fn mouth_cells(
    kind: MouthKind,
    form: Form,
    mw: i32,
    side: i32,
    snaggle: i32,
) -> Vec<(i32, i32, Role)> {
    use Role::{Beak, BeakShade, Ink, MouthIn, Teeth, Tongue};
    let mut out = Vec::new();
    let half = mw >> 1;
    let curve = |out: &mut Vec<(i32, i32, Role)>, dir: i32, width: i32| {
        let hf = width >> 1;
        let deep = if width >= 8 {
            2
        } else if width >= 4 {
            1
        } else {
            0
        };
        for x in -hf..hf {
            let e = (x + hf).min(hf - 1 - x).min(deep);
            out.push((x, if dir > 0 { e } else { deep - e }, Ink));
        }
    };
    let line = |out: &mut Vec<(i32, i32, Role)>, width: i32| {
        for x in -(width >> 1)..(width >> 1) {
            out.push((x, 0, Ink));
        }
    };
    let fangs = |out: &mut Vec<(i32, i32, Role)>, y: i32, at: i32| {
        if snaggle >= 0 {
            out.push((at, y, Teeth));
        }
        if snaggle <= 0 {
            out.push((-at - 1, y, Teeth));
        }
    };
    if kind == MouthKind::Beak {
        // A beak is shut, or open when the mood opens the mouth.
        let open = form == Form::Open || form == Form::O;
        for x in -half..half {
            out.push((x, 0, Beak));
        }
        for x in -half + 1..half - 1 {
            if open {
                out.push((x, 1, MouthIn));
                out.push((x, 2, BeakShade));
            } else {
                out.push((x, 1, BeakShade));
            }
        }
        return out;
    }
    match form {
        Form::Open => {
            let big = kind == MouthKind::Maw;
            for x in -half..half {
                out.push((x, 0, if big { Teeth } else { MouthIn }));
            }
            if big {
                for x in -half..half {
                    out.push((x, 1, MouthIn));
                }
            }
            let y = if big { 2 } else { 1 };
            if mw <= 4 {
                out.push((-1, y, Tongue));
                out.push((0, y, Tongue));
            } else {
                for x in -half + 1..half - 1 {
                    let edge = x == -half + 1 || x == half - 2;
                    out.push((x, y, if edge { MouthIn } else { Tongue }));
                }
            }
            if kind == MouthKind::Fangs {
                fangs(&mut out, 0, if mw >= 6 { half - 2 } else { half - 1 });
            }
            if kind == MouthKind::Buck {
                out.push((-1, 0, Teeth));
                out.push((0, 0, Teeth));
            }
        }
        Form::Flat => line(&mut out, mw.min(4)),
        Form::FlatSmall => line(&mut out, 2),
        Form::Frown => {
            curve(&mut out, -1, if kind == MouthKind::Tiny { 4 } else { mw });
            if kind == MouthKind::Buck {
                out.push((-1, 1, Teeth));
                out.push((0, 1, Teeth));
            }
        }
        Form::Pout => curve(&mut out, -1, 4),
        Form::Zig => {
            if mw >= 6 {
                for x in -half..half {
                    let e = (x + half).min(half - 1 - x);
                    out.push((x, if e % 3 == 1 { 0 } else { 1 }, Ink));
                }
            } else {
                curve(&mut out, -1, 4);
            }
        }
        Form::Grit => {
            if mw >= 4 {
                for x in -half..half {
                    out.push((x, 0, Ink));
                    out.push((x, 1, Teeth));
                    out.push((x, 2, Ink));
                }
            } else {
                // A mouth too small to show teeth turns down.
                curve(&mut out, -1, 4);
            }
        }
        Form::Tongue => {
            let width = 4.max(mw.min(6));
            line(&mut out, width);
            let sx = if side > 0 {
                (width >> 1) - 2
            } else {
                -(width >> 1)
            };
            out.push((sx, 1, Tongue));
            out.push((sx + 1, 1, Tongue));
        }
        Form::Side => {
            out.push((-1 + side, 0, Ink));
            out.push((side, 0, Ink));
        }
        Form::O => {
            for (x, y) in [(-1, 0), (0, 0), (-1, 1), (0, 1)] {
                out.push((x, y, MouthIn));
            }
            if mw >= 6 {
                out.push((-1, 2, MouthIn));
                out.push((0, 2, MouthIn));
            }
        }
        Form::Rest => match kind {
            MouthKind::Smile => curve(&mut out, 1, mw),
            MouthKind::Flat => line(&mut out, mw),
            MouthKind::Tiny => line(&mut out, 2),
            MouthKind::Cat => {
                for x in -half..half {
                    let e = (x + half).min(half - 1 - x);
                    out.push((x, if e == 0 || e == half - 1 { 0 } else { 1 }, Ink));
                }
            }
            MouthKind::Fangs => {
                line(&mut out, mw);
                fangs(&mut out, 1, 0.max(half - 1 - i32::from(mw >= 6)));
            }
            MouthKind::Buck => {
                line(&mut out, mw);
                out.push((-1, 1, Teeth));
                out.push((0, 1, Teeth));
            }
            MouthKind::Maw => {
                line(&mut out, mw);
                for x in [half - 2, half - 3, -half + 1, -half + 2] {
                    out.push((x, 1, Teeth));
                }
            }
            MouthKind::Beak => {}
        },
    }
    out
}

/// A brow over one eye, before the pair is checked.
struct BrowCells {
    name: String,
    ex: i32,
    ey: i32,
    shown: Vec<(i32, i32)>,
    kind: Brow,
    sparse: bool,
}

/// Cells of an eye box: empty, white, pupil, iris, light iris or shine.
const EMPTY: u8 = 0;
const W_: u8 = b'W';
const P_: u8 = b'P';
const I_: u8 = b'I';
const IL: u8 = b'i';
const H_: u8 = b'H';

fn role_colour(cell: u8) -> u8 {
    match cell {
        W_ => WHITE,
        P_ => PUPIL,
        I_ => IRIS,
        IL => IRIS_LIGHT,
        H_ => SHINE,
        _ => INK,
    }
}

/// Draws the face of this mood over the composed body: eyes, lids, brows,
/// cheeks, nose and mouth, and a tear.
pub(crate) fn draw_face(ctx: &mut Ctx, comp: &Comp) {
    let g = ctx.g;
    let f = &g.face;
    let t = &g.temper;
    let spec = face_spec(ctx);
    let dark = ctx.p.dark;
    let ox = ctx.fdx + ctx.head_dx;
    let oy = ctx.fdy;
    let head = ctx.head as i16;
    // Brows, cheeks and mouth are drawn only on the head itself, and never
    // on its outline.
    let on_skin = |x: i32, y: i32| {
        let (xx, yy) = (x + ox, y + oy);
        if !inside(xx, yy) {
            return false;
        }
        let c = (yy * CW + xx) as usize;
        comp.own[c] == head && comp.line[c] != 1
    };
    // The body colour under a cell; a cell off the canvas array reads as empty.
    let skin = |x: i32, y: i32| {
        let c = (y + oy) * CW + x + ox;
        if c >= 0 && (c as usize) < comp.col.len() {
            comp.col[c as usize]
        } else {
            0
        }
    };
    let mut out: Vec<Entry> = Vec::new();
    let mut push = |group: &'static str, name: &str, x: i32, y: i32, col: u8| {
        if inside(x + ox, y + oy) {
            out.push(Entry {
                x: x + ox,
                y: y + oy,
                col,
                group,
                name: name.to_owned(),
            });
        }
    };
    if ctx.mood == Mood::Neutral {
        narrow_dark_eyes(ctx, &on_skin);
    }
    let face = ctx.f.clone();

    // An eye without whites cannot turn its pupil, so such eyes glance by
    // moving a cell within the face: all together, and only if every one
    // keeps a cell of skin at each side.
    let mut glance = (0, 0);
    if face.bumps.is_none()
        && spec.shut.is_none()
        && spec.gaze.1 <= 0
        && (spec.gaze.0 != 0 || spec.gaze.1 != 0)
    {
        let fits = |dx: i32, dy: i32| {
            face.eyes
                .iter()
                .all(|e| (0..e.h).all(|j| (-1..=e.w).all(|i| on_skin(e.x + i + dx, e.y + j + dy))))
        };
        if spec.gaze.0 != 0 && fits(spec.gaze.0, 0) {
            glance.0 = spec.gaze.0;
        }
        if spec.gaze.1 != 0 && fits(glance.0, spec.gaze.1) {
            glance.1 = spec.gaze.1;
        }
    }
    let mut brows: Vec<BrowCells> = Vec::new();
    for e in &face.eyes {
        let sfx = if e.third {
            "-third"
        } else if e.side > 0 {
            "-r"
        } else if e.side < 0 {
            "-l"
        } else {
            "-c"
        };
        let (w, h) = (e.w, e.h);
        let (kw, kh) = face.ask.unwrap_or((w, h));
        let kind = eye_kind(f, face.plan, dark, kw, kh, e.third);
        let whites = has_whites(kind);
        // On a light body an eye with whites wears a lash line along its
        // upper edge; on a dark body the whites stand out by themselves.
        let lashed = whites && !dark;
        // One eye alone is round, its lash line following its corners down,
        // except under a frown.
        let single = e.side == 0 && !e.third;
        let shape = eye_shape(
            w,
            h,
            lashed && (!single || spec.lid == Lid::Angry),
            f.lid != LidGene::Open,
            whites && single && h == 3,
        );
        let ex = e.x + if whites { 0 } else { glance.0 };
        let ey = e.y + if whites { 0 } else { glance.1 };
        let mut grid: Vec<Vec<u8>> = shape
            .iter()
            .map(|row| {
                row.iter()
                    .map(|&on| match (on, whites) {
                        (false, _) => EMPTY,
                        (true, true) => W_,
                        (true, false) => P_,
                    })
                    .collect()
            })
            .collect();
        // The lids come first: they decide how much of the eye shows.
        let mut cuts: Option<Vec<i32>> = None;
        if spec.shut.is_none() {
            // A dark eye at rest shows as many rows as it is wide, or more.
            let rest = if e.third || dark {
                Lid::Open
            } else {
                f.lid.into()
            };
            let mut c = lid_cuts(
                spec.lid,
                rest,
                w,
                h,
                e.side,
                lashed,
                if whites { 0 } else { w },
            );
            // A slanted lid over a rounded eye can leave one cell of the top
            // row showing, a nub on the eye's corner: the lid takes it too,
            // except under an angry brow.
            if w >= 3 && !(spec.brow == Brow::Angry && shape[0][0]) {
                for j in 0..h - 1 {
                    let open: Vec<i32> = (0..w)
                        .filter(|&i| shape[j as usize][i as usize] && j >= c[i as usize])
                        .collect();
                    if open.len() == 1 {
                        c[open[0] as usize] = j + 1;
                    }
                    if !open.is_empty() {
                        break;
                    }
                }
            }
            cuts = Some(c);
        }
        let shows = |grid: &Vec<Vec<u8>>, i: i32, j: i32| {
            j >= 0
                && j < h
                && i >= 0
                && i < w
                && grid[j as usize][i as usize] != EMPTY
                && cuts.as_ref().is_none_or(|c| j >= c[i as usize])
        };
        let at = |grid: &Vec<Vec<u8>>, i: i32, j: i32| {
            if j >= 0 && j < h && i >= 0 && i < w {
                grid[j as usize][i as usize]
            } else {
                EMPTY
            }
        };
        if !whites {
            // A dark eye, lit by a glint at the upper left of what the lid
            // leaves showing. Rows showing one cell are left out when
            // another row shows more.
            let shown_in =
                |grid: &Vec<Vec<u8>>, j: i32| (0..w).filter(|&i| shows(grid, i, j)).count();
            let mut rows_shown: Vec<i32> = (0..h).filter(|&j| shown_in(&grid, j) > 0).collect();
            if rows_shown.iter().any(|&j| shown_in(&grid, j) >= 2) {
                rows_shown.retain(|&j| shown_in(&grid, j) >= 2);
            }
            if !rows_shown.is_empty() {
                // In the top row of a small eye; a row down in one that
                // shows four rows or more.
                let sj = rows_shown[if rows_shown.len() >= 4 { 1 } else { 0 }];
                let mut si = 0;
                while si < w - 1 && !shows(&grid, si, sj) {
                    si += 1;
                }
                if w >= 4 && si == 0 && shows(&grid, 1, sj) {
                    si = 1;
                }
                grid[sj as usize][si as usize] = H_;
                // One cell; a two-by-two block in an eye five wide; two
                // side by side in wide-open happy eyes.
                if spec.big && shows(&grid, si + 1, sj) {
                    grid[sj as usize][si as usize + 1] = H_;
                } else if w >= 5
                    && shows(&grid, si + 1, sj)
                    && shows(&grid, si, sj + 1)
                    && shows(&grid, si + 1, sj + 1)
                {
                    grid[sj as usize][si as usize + 1] = H_;
                    grid[sj as usize + 1][si as usize] = H_;
                    grid[sj as usize + 1][si as usize + 1] = H_;
                }
                if kind == EyeStyle::Sparkle
                    && w >= 4
                    && h >= 4
                    && grid[(h - 2) as usize][(w - 2) as usize] == P_
                {
                    grid[(h - 2) as usize][(w - 2) as usize] = H_;
                }
            }
        } else if kind == EyeStyle::Sclera || kind == EyeStyle::Dot {
            // The pupil is the smaller part of the eye, centred, with a
            // margin of white round it.
            let top = i32::from(lashed);
            let rows = h - top;
            let mut pw = 1.max(if w >= 6 { w - 4 } else { w - 2 });
            let mut ph = clamp(if rows >= 6 { rows - 4 } else { rows - 2 }, 1, rows);
            // A dot eye has a low pupil, two rows at most.
            if kind == EyeStyle::Dot {
                pw = if w >= 5 {
                    if w & 1 != 0 {
                        3
                    } else {
                        2
                    }
                } else if w & 1 != 0 {
                    1
                } else {
                    2
                };
                ph = ph.min(2);
            }
            let slack = w - pw;
            let px = clamp((slack >> 1) + spec.gaze.0, 0, slack);
            let mut py = top + ((rows - ph) >> 1);
            // Under the rounded lash line of one eye alone, four wide, the
            // pupil keeps out of the row with the line's ends in it.
            py = clamp(
                py + spec.gaze.1,
                top + i32::from(single && lashed && w == 4 && h >= 4),
                h - ph,
            );
            // Under a frown a single eye's pupil moves down from the lid.
            if let (0, Some(c)) = (e.side, &cuts) {
                let lid = c[px as usize..(px + pw) as usize]
                    .iter()
                    .copied()
                    .min()
                    .unwrap_or(0);
                py = clamp(py.max(lid + top), top, h - ph);
            }
            for j in py..py + ph {
                for i in px..px + pw {
                    if at(&grid, i, j) != EMPTY {
                        grid[j as usize][i as usize] = P_;
                    }
                }
            }
            if pw >= 2 && ph >= 2 && at(&grid, px, py) == P_ {
                grid[py as usize][px as usize] = H_;
            }
        } else if single && lashed && w == 4 {
            // One eye alone, four wide, with a coloured iris under its
            // rounded lash line: the pupil sits a row lower, with iris at
            // both sides and white above and below.
            let lid_at = cuts.as_ref().map_or(0, |c| c[1].min(c[2]));
            let r = (h - 2).min(lid_at + 2);
            let px = clamp(1 + spec.gaze.0, 0, 2);
            let tint = if kind == EyeStyle::Slit { IL } else { I_ };
            for i in 0..w {
                if at(&grid, i, r) != EMPTY {
                    grid[r as usize][i as usize] = if i == px || i == px + 1 { P_ } else { tint };
                }
            }
        } else {
            // A coloured iris and its pupil make one disc inside the whites,
            // hanging from the lash line where there is one; it starts under
            // whatever the lid of this mood covers.
            let dw = 1.max(w - 2);
            let first = (w - dw) >> 1;
            let lid_at = cuts.as_ref().map_or(0, |c| {
                c[first as usize..(first + dw) as usize]
                    .iter()
                    .copied()
                    .min()
                    .unwrap_or(0)
            });
            let top = (h - 1).min(lid_at + i32::from(lashed));
            let rows = h - top;
            let dh = 1.max(if lashed { rows - 1 } else { rows - 2 });
            // A glance does not move the disc into a last row whose corners
            // are rounded off.
            let lowest = if shape[(h - 1) as usize][0] {
                h - dh
            } else {
                top.max(h - dh - 1)
            };
            let px = clamp(first + spec.gaze.0, 0, w - dw);
            let py = clamp(
                if lashed {
                    top
                } else {
                    top + ((rows - dh) >> 1)
                } + spec.gaze.1,
                top,
                lowest,
            );
            // The iris is a column each side of the pupil when the disc is
            // three wide or more, and a row under it when the disc has the
            // rows. A slit pupil runs the whole height of its disc.
            let flank = i32::from(dw >= 3);
            let slit = kind == EyeStyle::Slit && dw >= 3;
            let under = if slit || dh < 2 || (dw >= 3 && dh < 3) {
                0
            } else if dw <= 2 && dh >= 4 {
                2
            } else {
                1
            };
            let tint = if kind == EyeStyle::Slit { IL } else { I_ };
            for j in 0..dh {
                for i in 0..dw {
                    if at(&grid, px + i, py + j) == EMPTY {
                        continue;
                    }
                    grid[(py + j) as usize][(px + i) as usize] =
                        if j >= dh - under || i < flank || i >= dw - flank {
                            tint
                        } else {
                            P_
                        };
                }
            }
            if dw - 2 * flank >= 2 && dh - under >= 2 && at(&grid, px + flank, py) == P_ {
                grid[py as usize][(px + flank) as usize] = H_;
            }
        }
        let eye_name = format!("eye{sfx}");
        for j in 0..h {
            for i in 0..w {
                let cell = grid[j as usize][i as usize];
                if cell != EMPTY {
                    push("eyes", &eye_name, ex + i, ey + j, role_colour(cell));
                }
            }
        }
        // ---- lids ----
        let lid_name = format!("lid{sfx}");
        if let Some(shut) = spec.shut {
            for j in 0..h {
                for i in 0..w {
                    if grid[j as usize][i as usize] != EMPTY {
                        push("lids", &lid_name, ex + i, ey + j, skin(ex + i, ey + j));
                    }
                }
            }
            let mut cells: Vec<(i32, i32)> = Vec::new();
            match shut {
                Shut::Arc => {
                    let m0 = 0.max((h - 2) >> 1);
                    for i in 0..w {
                        let end = (i == 0 || i == w - 1) && w >= 3;
                        cells.push((i, if end { m0 + 1 } else { m0 }));
                    }
                }
                Shut::Sleep => {
                    let r1 = h - 1;
                    for i in 0..w {
                        let end = (i == 0 || i == w - 1) && w >= 3;
                        cells.push((i, if end { r1 - 1 } else { r1 }));
                    }
                }
                Shut::Squeeze => {
                    // Squeezed shut: a chevron whose point faces the nose;
                    // one eye alone shuts to a V.
                    let r = h >> 1;
                    if w < 2 {
                        for i in 0..w {
                            cells.push((i, r));
                        }
                    } else if e.side == 0 {
                        for i in 0..w {
                            cells.push((i, r - 1 + i.min(w - 1 - i).min(2)));
                        }
                    } else {
                        let col = |k: i32| if e.side > 0 { k } else { w - 1 - k };
                        cells.push((col(0), r));
                        cells.push((col(1), r - 1));
                        cells.push((col(1), (h - 1).min(r + 1)));
                        if w >= 3 {
                            if h >= 5 {
                                cells.push((col(2), r - 2));
                                cells.push((col(2), r + 2));
                            } else {
                                cells.push((col(2), r - 1));
                                cells.push((col(2), (h - 1).min(r + 1)));
                            }
                        }
                    }
                }
            }
            for (i, j) in cells {
                push("lids", &lid_name, ex + i, ey + clamp(j, 0, h - 1), INK);
            }
        } else {
            let c = cuts.as_ref().expect("an open eye has lid cuts");
            let lash_name = format!("lash{sfx}");
            for i in 0..w {
                for j in 0..c[i as usize] {
                    if grid[j as usize][i as usize] != EMPTY {
                        push("lids", &lid_name, ex + i, ey + j, skin(ex + i, ey + j));
                    }
                }
                // The lash line: the first cell a lid leaves showing.
                if lashed {
                    let mut j = c[i as usize];
                    while j < h && grid[j as usize][i as usize] == EMPTY {
                        j += 1;
                    }
                    if j < h - 1 {
                        push("lids", &lash_name, ex + i, ey + j, INK);
                    }
                }
            }
        }
        if e.third {
            continue;
        }
        // ---- brows ----
        // A brow keeps a row of skin between itself and the eye, so it
        // follows the lid down; an angry brow is pressed onto the eye along
        // the edge of the slanted lid.
        let brow = if spec.brow == Brow::Rest {
            Brow::from(f.brows)
        } else {
            spec.brow
        };
        let mut brow_cells: Vec<(i32, i32)> = Vec::new();
        let col = |k: i32| if e.side >= 0 { k } else { w - 1 - k }; // k counts from the inner side
        let cut_at = |k: i32| cuts.as_ref().map_or(0, |c| c[col(k) as usize]);
        let least = cuts
            .as_ref()
            .map_or(0, |c| c.iter().copied().min().unwrap_or(0));
        let resting = matches!(brow, Brow::Thin | Brow::Thick | Brow::Short);
        if resting && single {
            // One eye alone wears its resting brow in the middle, shorter
            // than the eye; under a lash line it wears none.
            let n = if brow == Brow::Short { 2 } else { 2.max(w - 2) };
            if !lashed {
                for k in (w - n) >> 1..((w - n) >> 1) + n {
                    brow_cells.push((k, least - 2));
                }
            }
        } else if resting {
            // A resting brow never runs along the whole top of its eye: that
            // is the rim of a pair of glasses.
            let both = |k: i32| {
                let x = ex + col(k);
                let y = ey + least - 2;
                on_skin(x, y) && on_skin(2 * (AX + ex - e.x) - 1 - x, y)
            };
            let block =
                !whites && !((w >= 4 && h >= 4) || (w == 3 && h >= 5 && f.lid == LidGene::Open));
            if w >= 3 && brow == Brow::Short {
                if face.g > 1 || !block {
                    brow_cells.push((col(0), least - 2));
                    brow_cells.push((col(1), least - 2));
                } else if both(w - 2) && both(w - 1) {
                    brow_cells.push((col(w - 2), least - 2));
                    brow_cells.push((col(w - 1), least - 2));
                }
            } else if w >= 3 {
                for k in 1..w {
                    brow_cells.push((col(k), least - 2));
                }
                if brow == Brow::Thick && both(w) && both(w + 1) {
                    brow_cells.push((col(w), least - 2));
                }
            }
        } else if brow == Brow::Low {
            for k in 0..w {
                brow_cells.push((col(k), least - 2));
            }
        } else if brow == Brow::Angry {
            for k in 0..w {
                brow_cells.push((col(k), cut_at(k) - 1));
                // Where the brow steps a row, the lower column is filled in
                // above, so that the brow is a stair of whole cells.
                if lashed || k == 0 {
                    continue;
                }
                let here = cut_at(k) - 1;
                let before = cut_at(k - 1) - 1;
                for r in here.min(before)..here.max(before) {
                    brow_cells.push((col(if here < before { k - 1 } else { k }), r));
                }
            }
        } else if brow == Brow::Sad {
            for k in 0..w {
                brow_cells.push((col(k), cut_at(k) - 2));
            }
        } else if brow == Brow::Cocked {
            let raised = e.side == t.side || e.side == 0;
            for k in i32::from(!raised)..w {
                brow_cells.push((col(k), least - if raised { 3 } else { 2 }));
            }
        }
        // The cells of an angry brow inside the eye's own box are drawn
        // whatever is under them.
        let angry = brow == Brow::Angry;
        let shown: Vec<(i32, i32)> = brow_cells
            .iter()
            .copied()
            .filter(|&(i, j)| {
                (angry && j >= 0 && j < h && i >= 0 && i < w)
                    || (face.bumps.is_none()
                        && on_skin(ex + i, ey + j)
                        && (angry || j >= 0 || on_skin(ex + i, ey + j - 1)))
            })
            .collect();
        // A resting brow is two cells at least.
        let sparse = 2 * shown.len() < brow_cells.len() || (resting && shown.len() < 2);
        brows.push(BrowCells {
            name: format!("brow{sfx}"),
            ex,
            ey,
            shown,
            kind: brow,
            sparse,
        });
    }
    // A brow is left off when less than half of it finds skin. The brows a
    // face wears at rest, and lowers when it works, are a pair: when one is
    // left off, so is the other.
    let paired =
        |b: &BrowCells| matches!(b.kind, Brow::Thin | Brow::Thick | Brow::Short | Brow::Low);
    let unpaired = brows.iter().any(|b| paired(b) && b.sparse);
    for b in &brows {
        let keep = b.kind == Brow::Angry || if paired(b) { !unpaired } else { !b.sparse };
        if keep {
            for &(i, j) in &b.shown {
                push("brows", &b.name, b.ex + i, b.ey + j, INK);
            }
        }
    }
    // ---- cheeks ----
    if face.bumps.is_none() {
        for e in &face.eyes {
            if e.side == 0 {
                continue;
            }
            let yb = e.y + e.h;
            let xo = if e.side > 0 { e.x + e.w - 1 } else { e.x };
            let s = e.side;
            if spec.blush {
                let mut cells = vec![(xo, yb), (xo + s, yb)];
                if f.cheeks == Cheeks::BigBlush {
                    cells.push((xo, yb + 1));
                    cells.push((xo + s, yb + 1));
                }
                let clear = cells.iter().all(|&(x, y)| {
                    on_skin(x, y) && comp.line[((y + oy) * CW + x + ox) as usize] == 0
                });
                if clear {
                    let name = if s > 0 { "cheek-r" } else { "cheek-l" };
                    for (x, y) in cells {
                        push("cheeks", name, x, y, BLUSH);
                    }
                }
            } else if f.cheeks == Cheeks::Whiskers {
                // Two short whisker marks on each cheek, at the edge of the face.
                let hd = &ctx.l.head;
                let y = (hd.bottom - 2).min(yb);
                let xe = AX + s * (hd.width_at(y) - 1) - i32::from(s < 0);
                for yy in [y, y + 2] {
                    let cells = [
                        (xe - 2 * s - ctx.fdx, yy - ctx.fdy),
                        (xe - 3 * s - ctx.fdx, yy - ctx.fdy),
                    ];
                    if cells.iter().all(|&(x, y2)| on_skin(x, y2)) {
                        let name = if s > 0 { "whiskers-r" } else { "whiskers-l" };
                        for (x, y2) in cells {
                            push("cheeks", name, x, y2, INNER);
                        }
                    }
                }
            }
        }
    }
    // ---- nose and mouth ----
    let m = face.mouth;
    // The nose sits above the mouth, clear below the eyes.
    if face.nose
        && on_skin(AX - 1, m.y - 1)
        && on_skin(AX, m.y - 1)
        && (face.bumps.is_some() || m.y - 1 > face.eye_bottom)
    {
        let c = if f.nose_dark { PUPIL } else { NOSE };
        push("nose", "nose", AX - 1, m.y - 1, c);
        push("nose", "nose", AX, m.y - 1, c);
    }
    let snaggle = if f.quirk == Quirk::Snaggle {
        if f.quirk_side != 0 {
            1
        } else {
            -1
        }
    } else {
        0
    };
    // An open mouth, or a zigzag of one, with a single row between it and
    // the eyes is no wider than four cells, or the three run together.
    let near = face.bumps.is_none() && m.y - face.eye_bottom <= 2;
    let form = if spec.mouth == Form::Zig && near {
        Form::Pout
    } else {
        spec.mouth
    };
    let wide = if near && matches!(form, Form::Open | Form::Grit | Form::O) {
        m.w.min(4)
    } else {
        m.w
    };
    let mut cells = mouth_cells(m.kind, form, wide, t.side, snaggle);
    // A blep: the tip of the tongue shows under a mouth at rest.
    if f.quirk == Quirk::Blep
        && form == Form::Rest
        && !matches!(m.kind, MouthKind::Beak | MouthKind::Buck | MouthKind::Maw)
    {
        let low = cells
            .iter()
            .filter(|c| c.0 == -1 || c.0 == 0)
            .map(|c| c.1)
            .fold(0, i32::max);
        cells.push((-1, low + 1, Role::Tongue));
        cells.push((0, low + 1, Role::Tongue));
    }
    for (dx, dy, role) in cells {
        let (x, y) = (AX + dx + m.dx, m.y + dy);
        if !on_skin(x, y) {
            continue;
        }
        let (name, col) = match role {
            Role::Beak => ("mouth", ramp_colour(BEAK, BASE)),
            Role::BeakShade => ("mouth", ramp_colour(BEAK, SHADE)),
            Role::Ink => ("mouth", INK),
            Role::MouthIn => ("mouth", MOUTH_IN),
            Role::Tongue => ("tongue", TONGUE),
            Role::Teeth => ("teeth", TEETH),
        };
        push("mouth", name, x, y, col);
    }
    drop(push);
    ctx.face.extend(out);
    // ---- a tear, which belongs to the eye it runs from ----
    if spec.mark == Some(MoodMark::Tear) {
        // It hangs from under the outer half of the eye, a cell in from a
        // rounded corner.
        let e = face
            .eyes
            .iter()
            .find(|e| e.side == t.tear_side)
            .unwrap_or(&face.eyes[0]);
        let inset = i32::from(e.w >= 4);
        let xo = if e.side >= 0 {
            e.x + e.w - 1 - inset
        } else {
            e.x + inset
        };
        for k in 0..2 {
            let (x, y) = (xo + ox, e.y + e.h + k + oy);
            if inside(x, y) {
                ctx.marks.push(Entry {
                    x,
                    y,
                    col: TEAR,
                    group: "marks",
                    name: "tear".to_owned(),
                });
            }
        }
    }
}
