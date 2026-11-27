//! Fitting the creature into its frame, and the mood marks that float free
//! of the body.

use avatar_svg::Mood;

use crate::compose::Comp;
use crate::face::{face_spec, MoodMark};
use crate::genes::LegKind;
use crate::grid::{clamp, inside, Mask, AX, CW, N};
use crate::palette::{ANGER, SWEAT, ZZ};
use crate::sprite::{
    build_sprite, least_head, mood_index, pose_of, Box4, Built, Ctx, Entry, Lim, Memo, Win, MOODS,
};

/// A small z and a larger Z above and beside it; the small one alone where
/// that will not fit.
const ZZ_SHAPE: [&str; 7] = [
    "....zzzz", "......z.", ".....z..", "....zzzz", "zzz.....", ".z......", "zzz.....",
];
const Z_SMALL: [&str; 3] = ["zzz", ".z.", "zzz"];
const DOTS: [&str; 1] = ["z.z.z"];
/// The anger mark of comics and games: four corners round a cross-shaped
/// gap, each three cells that share edges.
const ANGER_SHAPE: [&str; 5] = [".a.a.", "aa.aa", ".....", "aa.aa", ".a.a."];
const SWEAT_SHAPE: [&str; 3] = [".s", "ss", "ss"];

fn shape_cells<'s>(shape: &'s [&'s str]) -> impl Iterator<Item = (i32, i32)> + 's {
    shape.iter().enumerate().flat_map(|(j, row)| {
        row.bytes()
            .enumerate()
            .filter(|&(_, ch)| ch != b'.')
            .map(move |(i, _)| (i as i32, j as i32))
    })
}

/// Puts a mark as near to (ax, ay) as it can stand clear of the creature and
/// inside the frame; if nowhere is clear it goes at the nearest place
/// inside, unless `clear_only` says it must be clear.
#[allow(clippy::too_many_arguments)]
fn place_mark(
    ctx: &mut Ctx,
    comp: &Comp,
    shape: &[&str],
    ax: i32,
    ay: i32,
    colour: u8,
    name: &str,
    taken: &mut Mask,
    clear_only: bool,
) -> bool {
    let win = ctx.win.expect("marks are placed in a frame");
    let h = shape.len() as i32;
    let w = shape[0].len() as i32;
    let mut best: Option<(i32, i32)> = None;
    let mut best_cost = i32::MAX;
    let mut best_clear = false;
    let mut y = win.y0;
    while y + h <= win.y0 + N {
        let mut x = win.x0;
        while x + w <= win.x0 + N {
            let clear = (-1..=h).all(|j| {
                (-1..=w).all(|i| {
                    let (xx, yy) = (x + i, y + j);
                    !(inside(xx, yy)
                        && (comp.own[(yy * CW + xx) as usize] >= 0 || taken.has(xx, yy)))
                })
            });
            let cost = (x - ax).abs() + (y - ay).abs() + if clear { 0 } else { 60 };
            if cost < best_cost {
                best_cost = cost;
                best = Some((x, y));
                best_clear = clear;
            }
            x += 1;
        }
        y += 1;
    }
    let Some((bx, by)) = best else {
        return false;
    };
    if clear_only && !best_clear {
        return false;
    }
    for (i, j) in shape_cells(shape) {
        ctx.marks.push(Entry {
            x: bx + i,
            y: by + j,
            col: colour,
            group: "marks",
            name: name.to_owned(),
        });
        taken.set_at((by + j) * CW + bx + i);
    }
    true
}

/// The mood marks of this pose, other than a tear: zz, dots, the anger mark
/// at the place the fit found for it, and a bead of sweat.
pub(crate) fn draw_marks(ctx: &mut Ctx, comp: &Comp, anger: Option<(i32, i32)>) {
    let spec = face_spec(ctx);
    let Some(win) = ctx.win else {
        return;
    };
    let h = ctx.l.head.clone();
    let side = ctx.g.temper.side;
    let a = h.a;
    let mut taken = Mask::new();
    let hx = AX + ctx.head_dx;
    let right = side > 0;
    match spec.mark {
        Some(MoodMark::Zz) => {
            if !place_mark(
                ctx,
                comp,
                &ZZ_SHAPE,
                hx + if right { a - 3 } else { -a - 5 },
                h.top - 6,
                ZZ,
                "zz",
                &mut taken,
                true,
            ) {
                place_mark(
                    ctx,
                    comp,
                    &Z_SMALL,
                    hx + if right { a - 2 } else { -a - 1 },
                    h.top - 4,
                    ZZ,
                    "zz",
                    &mut taken,
                    false,
                );
            }
        }
        Some(MoodMark::Dots) => {
            place_mark(
                ctx,
                comp,
                &DOTS,
                hx + if right { a - 3 } else { -a - 2 },
                h.top - 3,
                ZZ,
                "dots",
                &mut taken,
                false,
            );
        }
        Some(MoodMark::Anger) => {
            // The fit has already found the mark's place in the frame.
            if let Some((ax, ay)) = anger {
                for (i, j) in shape_cells(&ANGER_SHAPE) {
                    ctx.marks.push(Entry {
                        x: win.x0 + ax + i,
                        y: win.y0 + ay + j,
                        col: ANGER,
                        group: "marks",
                        name: "anger".to_owned(),
                    });
                }
            }
        }
        Some(MoodMark::Sweat) => {
            place_mark(
                ctx,
                comp,
                &SWEAT_SHAPE,
                hx + if right { -a - 2 } else { a + 1 },
                h.top + 1,
                SWEAT,
                "sweat",
                &mut taken,
                false,
            );
        }
        Some(MoodMark::Tear) | None => {}
    }
}

/// The anger mark's place beside an upper corner of the head, where a vein
/// stands out on a temple, counted in the frame with the creature moved by
/// (dx, dy): the clear place nearest the right-hand corner, or the left one
/// if that is nearer by more than two cells, never lower than the head's
/// widest row. `loose` 1 lets the mark touch the creature without covering
/// it; 2 lets it lie lower than the head too. Returns (x, y, cost).
fn anger_spot(built: &Built, win: Win, dx: i32, dy: i32, loose: u8) -> Option<(i32, i32, i32)> {
    let comp = &built.comp;
    let ctx = &built.ctx;
    let h = &ctx.l.head;
    let size = ANGER_SHAPE.len() as i32;
    let taken = |fx: i32, fy: i32| {
        let (x, y) = (fx + win.x0 - dx, fy + win.y0 - dy);
        inside(x, y) && comp.own[(y * CW + x) as usize] >= 0
    };
    let clear_at = |x0: i32, y0: i32| {
        shape_cells(&ANGER_SHAPE).all(|(i, j)| {
            let (x, y) = (x0 + i, y0 + j);
            !(taken(x, y)
                || (loose == 0
                    && (taken(x - 1, y) || taken(x + 1, y) || taken(x, y - 1) || taken(x, y + 1))))
        })
    };
    // At its best the mark's box overlaps the head's last two columns and
    // its top two rows, where the head's own corner is rounded off.
    let axis = AX + ctx.head_dx - win.x0 + dx;
    let top = h.top - win.y0 + dy;
    let lowest = if loose == 2 {
        N
    } else {
        h.widest_row() - win.y0 + dy - (size >> 1)
    };
    let right = axis + h.a - 2;
    let left = axis - h.a + 2 - size;
    let best0 = top - 3;
    let mut best: Option<(i32, i32, i32)> = None;
    for y0 in 0..=lowest.min(N - size) {
        for x0 in 0..=N - size {
            let up = (y0 - best0).abs();
            let cost = (x0 - right)
                .abs()
                .max(up)
                .min((x0 - left).abs().max(up) + 2);
            if best.is_none_or(|b| cost < b.2) && clear_at(x0, y0) {
                best = Some((x0, y0, cost));
            }
        }
    }
    best
}

/// A move of the creature within its frame that makes room for the anger
/// mark, with the mark's place.
struct Room {
    dx: i32,
    dy: i32,
    at: (i32, i32),
}

/// Finds room for the anger mark, moving the creature aside by up to
/// `reach` cells in all, never out of the frame, if it has none where it
/// stands at (dx, dy). A cell the creature moves counts for more than a
/// cell the mark is off its corner.
fn room_for_anger(
    built: &Built,
    win: Win,
    b: Box4,
    dx: i32,
    dy: i32,
    reach: i32,
    loose: u8,
) -> Option<Room> {
    let mut best: Option<(Room, i32)> = None;
    let fits_in = b.w <= N && b.h <= N;
    for my in -reach..=reach {
        for mx in -reach..=reach {
            let moved = mx.abs() + my.abs();
            if moved > reach || (moved != 0 && !fits_in) {
                continue;
            }
            let (sx, sy) = (dx + mx, dy + my);
            if moved != 0
                && (b.x0 + sx < win.x0
                    || b.x1 + sx > win.x0 + N - 1
                    || b.y0 + sy < win.y0
                    || b.y1 + sy > win.y0 + N - 1)
            {
                continue;
            }
            if let Some((x, y, off)) = anger_spot(built, win, sx, sy, loose) {
                let cost = 3 * moved + off;
                if best.as_ref().is_none_or(|(_, c)| cost < *c) {
                    best = Some((
                        Room {
                            dx: sx,
                            dy: sy,
                            at: (x, y),
                        },
                        cost,
                    ));
                }
            }
        }
    }
    best.map(|(room, _)| room)
}

/// Which group of parts reaches furthest from the axis.
fn widest_group(built: &Built) -> &'static str {
    let b = built.bounds;
    let x = if AX - b.x0 > b.x1 - AX + 1 {
        b.x0
    } else {
        b.x1
    };
    for y in b.y0..=b.y1 {
        let o = built.comp.own[(y * CW + x) as usize];
        if o >= 0 {
            return built.ctx.parts[o as usize].group;
        }
    }
    "body"
}

/// Which group of parts reaches highest.
fn tallest_group(built: &Built) -> &'static str {
    let b = built.bounds;
    for x in b.x0..=b.x1 {
        let o = built.comp.own[(b.y0 * CW + x) as usize];
        if o >= 0 {
            return built.ctx.parts[o as usize].group;
        }
    }
    "body"
}

/// A happy creature leaves the ground: one row, or two if it is the hopping kind.
fn hop_of(memo: &Memo, mood: Mood) -> i32 {
    let hop = pose_of(mood).hop;
    if hop != 0 {
        hop + i32::from(memo.genes.temper.hop)
    } else {
        0
    }
}

/// The fit: how much each part gave way, the frame, and each mood's shift
/// within the frame and the pose components it put back at rest.
#[derive(Clone, Debug)]
pub(crate) struct Fit {
    pub(crate) lim: Lim,
    pub(crate) win: Win,
    pub(crate) shifts: [(i32, i32); 7],
    pub(crate) eases: [u32; 7],
    pub(crate) anger: Option<(i32, i32)>,
}

/// A blocked pose that fits its frame but leaves no room for the anger mark.
struct Crowded<'a> {
    dx: i32,
    dy: i32,
    ease: u32,
    built: Built<'a>,
    b: Box4,
}

/// Fits the creature into the frame. A body larger than the frame can hold
/// with its limbs gives way first; then a limb that overshoots is shortened
/// a cell at a time; a limb that still cannot fit is left off. The moods
/// never shrink the creature: a pose that would leave the frame is eased.
/// `only` limits the poses fitted to one mood; each mood's pose is fitted on
/// its own, so the others are not needed to draw it.
pub(crate) fn fit(memo: &mut Memo, only: Option<Mood>) -> Fit {
    let mut lim = Lim::default();
    let least = least_head(memo.genes);
    let mut win = Win { x0: 0, y0: 0 };
    for _ in 0..80 {
        let neutral = build_sprite(memo, Mood::Neutral, lim, None, 0);
        let nb = neutral.bounds;
        // The frame is centred on the axis; a tail may push it two cells aside.
        let fx = clamp(AX - N / 2, nb.x1 - N + 1, nb.x0);
        let wide = nb.w > N || (fx - (AX - N / 2)).abs() > 2;
        let tall = nb.h > N;
        win = Win {
            x0: fx,
            y0: nb.y0 - ((N - nb.h + 1) >> 1),
        };
        if !wide && !tall {
            // Each mood takes the fullest pose that fits the same frame.
            let mut shifts = [(0, 0); 7];
            let mut eases = [0; 7];
            let mut anger = None;
            for mood in MOODS {
                if only.is_some_and(|m| m != mood) {
                    continue;
                }
                let k = mood_index(mood);
                let mut crowded: Vec<Crowded> = Vec::new();
                // The pose is tried whole, then with one component at rest,
                // then two, then all three: the ears first, the stretch and
                // the hop last, because those carry most of a pose.
                for ease in [0, 4, 1, 2, 5, 6, 3, 7] {
                    let built = if ease == 0 && mood == Mood::Neutral {
                        None
                    } else {
                        Some(build_sprite(memo, mood, lim, None, ease))
                    };
                    let b = built.as_ref().map_or(nb, |built| built.bounds);
                    // A mood may slide a little to stay inside the frame.
                    let mut dy = if ease & 2 != 0 {
                        0
                    } else {
                        -hop_of(memo, mood)
                    };
                    if b.y0 + dy < win.y0 {
                        dy = win.y0 - b.y0;
                    }
                    if b.y1 + dy > win.y0 + N - 1 {
                        dy = win.y0 + N - 1 - b.y1;
                    }
                    let mut dx = 0;
                    if b.x0 < win.x0 {
                        dx = win.x0 - b.x0;
                    }
                    if b.x1 + dx > win.x0 + N - 1 {
                        dx = win.x0 + N - 1 - b.x1;
                    }
                    let fits = b.w <= N && b.h <= N && b.x0 + dx >= win.x0 && b.y0 + dy >= win.y0;
                    if !(fits || ease == 7) {
                        continue;
                    }
                    if mood != Mood::Blocked {
                        shifts[k] = (dx, dy);
                        eases[k] = ease;
                        break;
                    }
                    // The blocked pose must also leave the anger mark its
                    // place beside the head. The creature may step up to three
                    // cells aside, or further with the whole pose at rest.
                    let built = built.expect("the blocked pose is built");
                    let reach = if ease == 7 { 6 } else { 3 };
                    if let Some(room) = room_for_anger(&built, win, b, dx, dy, reach, 0) {
                        shifts[k] = (room.dx, room.dy);
                        eases[k] = ease;
                        anger = Some(room.at);
                        break;
                    }
                    crowded.push(Crowded {
                        dx,
                        dy,
                        ease,
                        built,
                        b,
                    });
                    if ease == 7 {
                        // No room near the head: the creature fills its frame.
                        // Now the mark may touch it, then lie lower than the
                        // head, in the fullest pose that leaves such a place.
                        let mut last: Option<(Room, u32)> = None;
                        'loose: for loose in [1, 2] {
                            for p in &crowded {
                                if let Some(r) =
                                    room_for_anger(&p.built, win, p.b, p.dx, p.dy, 6, loose)
                                {
                                    last = Some((r, p.ease));
                                    break 'loose;
                                }
                            }
                        }
                        // Failing that, the mark is drawn at the right-hand
                        // corner of the head, over whatever grows there.
                        let (shift, ease, at) = last.map_or_else(
                            || {
                                let p = &crowded[0];
                                let hd = &p.built.ctx.l.head;
                                let size = ANGER_SHAPE.len() as i32;
                                let at = (
                                    clamp(
                                        AX + p.built.ctx.head_dx - win.x0 + p.dx + hd.a - 2,
                                        0,
                                        N - size,
                                    ),
                                    clamp(hd.top - win.y0 + p.dy - 3, 0, N - size),
                                );
                                ((p.dx, p.dy), p.ease, at)
                            },
                            |(r, ease)| ((r.dx, r.dy), ease, r.at),
                        );
                        shifts[k] = shift;
                        eases[k] = ease;
                        anger = Some(at);
                    }
                }
            }
            return Fit {
                lim,
                win,
                shifts,
                eases,
                anger,
            };
        }
        let l = &neutral.ctx.l;
        let rows = l.head.len();
        let g = memo.genes;
        if wide {
            let group = widest_group(&neutral);
            let limb = match group {
                "tail" => Some(Limb::Tail),
                "wings" => Some(Limb::Wing),
                "arms" => Some(Limb::Arm),
                "legs" => Some(Limb::Leg),
                "body" | "coat" => None,
                _ => Some(Limb::Top),
            };
            let big_torso = l.torso.as_ref().is_some_and(|t| t.a > 9);
            if l.head.a > 8 || (big_torso && l.head.a > least) {
                lim.body += 1;
            } else if limb.is_some_and(|limb| *limb.of(&mut lim) < 3) {
                *limb.expect("checked above").of(&mut lim) += 1;
            } else if l.head.a > least {
                lim.body += 1;
            } else if let Some(limb) = limb
                .map(|limb| if limb == Limb::Wing { Limb::Arm } else { limb })
                .filter(|limb| *limb.of(&mut lim) < 9)
            {
                *limb.of(&mut lim) = 9;
            } else {
                break;
            }
        } else {
            let group = tallest_group(&neutral);
            let top = !matches!(group, "body" | "coat" | "eye-mounts" | "legs");
            let legged = g.legs.kind != LegKind::None && lim.leg < 9;
            // How far what grows on the head stands above it. Tall ears give
            // way first; short ones are spared until the legs and the torso
            // have given what they can.
            let stands = if top { l.head.top - nb.y0 } else { 0 };
            if rows > 14 {
                lim.tall += 1;
            } else if top && stands > 5 && lim.top < 4 {
                lim.top += 1;
            } else if legged && g.legs.len - lim.leg > 0 {
                lim.leg += 1;
            } else if g.body.torso_rows != 0 && g.body.torso_rows - lim.torso > 3 {
                lim.torso += 1;
            } else if top && stands > 3 && lim.top < 4 {
                lim.top += 1;
            } else if rows > 8 {
                lim.tall += 1;
            } else if top && lim.top < 9 {
                lim.top = 9;
            } else if legged {
                lim.leg = 9;
            } else {
                break;
            }
        }
    }
    Fit {
        lim,
        win,
        shifts: [(0, 0); 7],
        eases: [7; 7],
        anger: None,
    }
}

/// The parts of the creature the fit can shorten.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Limb {
    Tail,
    Wing,
    Arm,
    Leg,
    Top,
}

impl Limb {
    fn of(self, lim: &mut Lim) -> &mut i32 {
        match self {
            Limb::Tail => &mut lim.tail,
            Limb::Wing => &mut lim.wing,
            Limb::Arm => &mut lim.arm,
            Limb::Leg => &mut lim.leg,
            Limb::Top => &mut lim.top,
        }
    }
}
