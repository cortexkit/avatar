//! Building a creature in one mood: the parts, the poses and the layout of
//! the body's lobes, and the order the builders run in.

use avatar_svg::Mood;
use std::collections::{BTreeMap, BTreeSet};

use crate::body;
use crate::compose::{compose, Comp};
use crate::face::{build_eye_mounts, draw_face, face_carry, face_layout, face_shift, Face};
use crate::fit::draw_marks;
use crate::genes::{CoatKind, EyePlan, Genes, LegKind, Plan, SideKind};
use crate::grid::{clamp, lobe, rndi, Corner, Mask, CH, CW, GY};
use crate::palette::{Palette, BODY};
use crate::pattern::build_pattern;

/// One lobe of the body as half-widths per row, from row `top` to row
/// `bottom`, centred on the axis.
#[derive(Clone, Debug)]
pub(crate) struct Lobe {
    /// The greatest half-width the lobe was built for.
    pub(crate) a: i32,
    pub(crate) rows: Vec<i32>,
    pub(crate) top: i32,
    pub(crate) bottom: i32,
}

impl Lobe {
    /// The half-width in row `y`, 0 outside the lobe.
    pub(crate) fn width_at(&self, y: i32) -> i32 {
        if y >= self.top && y <= self.bottom {
            self.rows[(y - self.top) as usize]
        } else {
            0
        }
    }

    /// The first row from the top in which the lobe covers the column `dx`
    /// cells right of the axis.
    pub(crate) fn top_at(&self, dx: i32) -> i32 {
        for (i, &w) in self.rows.iter().enumerate() {
            if w > dx {
                return self.top + i as i32;
            }
        }
        self.bottom
    }

    /// The middle of the lobe's widest run of rows.
    pub(crate) fn widest_row(&self) -> i32 {
        let mut best = 0;
        for i in 1..self.rows.len() {
            if self.rows[i] > self.rows[best] {
                best = i;
            }
        }
        let mut last = best;
        while last + 1 < self.rows.len() && self.rows[last + 1] == self.rows[best] {
            last += 1;
        }
        self.top + ((best + last) >> 1) as i32
    }

    pub(crate) fn len(&self) -> i32 {
        self.rows.len() as i32
    }
}

/// Depths. Parts of one depth are welded into one outline.
pub(crate) const Z_TAIL: i32 = 10;
pub(crate) const Z_WING: i32 = 12;
pub(crate) const Z_HIND: i32 = 14;
pub(crate) const Z_TORSO: i32 = 20;
pub(crate) const Z_ARM: i32 = 24;
pub(crate) const Z_HEAD: i32 = 30;
pub(crate) const Z_FEET: i32 = 34;
pub(crate) const Z_FRONT: i32 = 36;

/// The seven moods in the reference's order.
pub(crate) const MOODS: [Mood; 7] = [
    Mood::Neutral,
    Mood::Working,
    Mood::Happy,
    Mood::Waiting,
    Mood::Blocked,
    Mood::Sad,
    Mood::Sleepy,
];

pub(crate) fn mood_index(mood: Mood) -> usize {
    match mood {
        Mood::Neutral => 0,
        Mood::Working => 1,
        Mood::Happy => 2,
        Mood::Waiting => 3,
        Mood::Blocked => 4,
        Mood::Sad => 5,
        Mood::Sleepy => 6,
    }
}

/// How a limb or the ears are carried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Car {
    Rest,
    Up,
    Cocked,
    Back,
    Droop,
    Front,
    One,
    Crossed,
    Limp,
    Half,
    Out,
    Flick,
    Bristle,
    Down,
    Spread,
    Fold,
}

/// What a mood does to the body. `stretch` adds rows to the main lobe,
/// `widen` adds columns, `hop` lifts everything, `tilt` moves the head (or
/// the face, on a body of one lobe) sideways, and the carriages say how ears,
/// arms, tail and wings are held.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pose {
    pub(crate) stretch: i32,
    pub(crate) widen: i32,
    pub(crate) hop: i32,
    pub(crate) tilt: i32,
    pub(crate) ears: Car,
    pub(crate) arms: Car,
    pub(crate) tail: Car,
    pub(crate) wings: Car,
    pub(crate) look: Option<(i32, i32)>,
}

pub(crate) fn pose_of(mood: Mood) -> Pose {
    let rest = Pose {
        stretch: 0,
        widen: 0,
        hop: 0,
        tilt: 0,
        ears: Car::Rest,
        arms: Car::Rest,
        tail: Car::Rest,
        wings: Car::Rest,
        look: None,
    };
    match mood {
        Mood::Neutral => rest,
        Mood::Working => Pose {
            arms: Car::Front,
            look: Some((0, 1)),
            ..rest
        },
        Mood::Happy => Pose {
            stretch: 1,
            hop: 1,
            ears: Car::Up,
            arms: Car::Up,
            tail: Car::Up,
            wings: Car::Up,
            ..rest
        },
        Mood::Waiting => Pose {
            tilt: 1,
            ears: Car::Cocked,
            arms: Car::One,
            tail: Car::Flick,
            ..rest
        },
        Mood::Blocked => Pose {
            widen: 1,
            ears: Car::Back,
            arms: Car::Crossed,
            tail: Car::Bristle,
            wings: Car::Spread,
            ..rest
        },
        Mood::Sad => Pose {
            stretch: -1,
            ears: Car::Droop,
            arms: Car::Limp,
            tail: Car::Down,
            wings: Car::Fold,
            look: Some((0, 1)),
            ..rest
        },
        Mood::Sleepy => Pose {
            stretch: -1,
            widen: 1,
            tilt: -1,
            ears: Car::Droop,
            arms: Car::Limp,
            tail: Car::Down,
            wings: Car::Fold,
            ..rest
        },
    }
}

/// How many cells the fit has taken from each part of the creature. A limb
/// at 9 or more is left off.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Lim {
    pub(crate) body: i32,
    pub(crate) tall: i32,
    pub(crate) top: i32,
    pub(crate) torso: i32,
    pub(crate) leg: i32,
    pub(crate) wing: i32,
    pub(crate) tail: i32,
    pub(crate) arm: i32,
}

/// Where the lobes sit: the head lobe, which carries the face, and a second
/// lobe under it. In a chibi the second lobe is a small torso behind the
/// head (`split`); in a pear it is a belly wider than the head, welded to it
/// into one body (`welded`).
#[derive(Clone, Debug)]
pub(crate) struct Layout {
    pub(crate) split: bool,
    pub(crate) welded: bool,
    pub(crate) torso: Option<Lobe>,
    pub(crate) head: Lobe,
    pub(crate) bottom: i32,
}

/// The least half-width of the head lobe: a face needs a head twelve cells
/// wide, one eye can do with ten, and a head on a torso is kept larger.
pub(crate) fn least_head(g: &Genes) -> i32 {
    if g.face.plan == EyePlan::Cyclops {
        5
    } else if g.body.plan == Plan::Chibi {
        7
    } else {
        6
    }
}

fn layout(g: &Genes, pose: &Pose, lim: &Lim) -> Layout {
    let b = &g.body;
    let legs = &g.legs;
    let two = b.plan == Plan::Chibi || b.plan == Plan::Pear;
    let least = least_head(g);
    let share = if two { b.head_share } else { 1.0 };
    let a0 = clamp(
        rndi((f64::from(b.area) * share / b.aspect).sqrt() / 2.0) - lim.body,
        least,
        10,
    );
    // A head is eight rows tall at least; with one eye alone, nine.
    let h0 = clamp(
        rndi(2.0 * f64::from(a0) * b.aspect) - lim.tall,
        if g.face.plan == EyePlan::Cyclops {
            9
        } else {
            8
        },
        if two { 12 } else { 19 },
    );
    let hh = clamp(h0 + if two { 0 } else { pose.stretch }, 7, 20);
    let mut ha = a0;
    // A squashed body spreads, unless it is already as wide as it may be.
    if pose.widen != 0 && ha < 10 && !two {
        ha += 1;
    }
    let cut_rows = if b.plan == Plan::Mound {
        1.max(rndi(f64::from(hh) * b.cut_share / (1.0 - b.cut_share)))
    } else {
        0
    };
    // A small head with a pointed top has too few wide rows for a face.
    let mut top = if b.top == Corner::Pointed && ha <= 6 && hh <= 12 {
        Corner::Round
    } else {
        b.top
    };
    // A limbless head welded to a belly, square on top, is a box on a ball.
    let limbless = (g.side.kind == SideKind::None || lim.arm >= 9)
        && (legs.kind == LegKind::None || lim.leg >= 9);
    if b.plan == Plan::Pear && limbless && (top == Corner::Square || top == Corner::Boxy) {
        top = Corner::Round;
    }
    let rows = lobe(
        ha,
        hh,
        b.peak,
        top,
        if two { Corner::Round } else { b.bot },
        cut_rows,
    );
    // How far the body is held above the ground.
    let leg_kind = if lim.leg >= 9 {
        LegKind::None
    } else {
        legs.kind
    };
    let len = if leg_kind == LegKind::None {
        0
    } else {
        0.max(legs.len - lim.leg)
    };
    let lift = match leg_kind {
        LegKind::Feet | LegKind::Crab => 1,
        LegKind::Legs | LegKind::Quad | LegKind::Tentacles => 2 + (len >> 1),
        LegKind::Bird => 2 + i32::from(len == 3),
        LegKind::Wisp => 3 + (len >> 1),
        LegKind::None => 0,
    };
    let bottom = GY - 1 - lift;
    let split = b.plan == Plan::Chibi;
    let welded = b.plan == Plan::Pear;
    let (torso, head) = if split {
        let ta = clamp(rndi(f64::from(a0) * b.torso_ratio), 3, 10);
        let vis = 2.max(b.torso_rows - lim.torso + pose.stretch);
        let t_full = vis + b.overlap;
        let trows = lobe(ta, t_full, 0.5, Corner::Round, b.torso_shape, 0);
        let torso_top = bottom - t_full + 1;
        let head = Lobe {
            a: ha,
            rows,
            top: torso_top + b.overlap - hh,
            bottom: torso_top + b.overlap - 1,
        };
        (
            Some(Lobe {
                a: ta,
                rows: trows,
                top: torso_top,
                bottom,
            }),
            head,
        )
    } else if welded {
        // The belly is one to three cells wider than the head each side and
        // reaches up into its lower part.
        let ta = 10.min(a0 + 1 + [0, 1, 1, 2][b.torso_ix as usize])
            + i32::from(pose.widen != 0 && a0 < 8);
        let vis = 3.max(b.torso_rows - lim.torso + pose.stretch);
        let into = 3.max(rndi(f64::from(hh) * 0.42));
        let t_full = vis + into;
        let trows = lobe(ta, t_full, 0.55, Corner::Round, b.torso_shape, 0);
        let torso_top = bottom - t_full + 1;
        let head = Lobe {
            a: ha,
            rows,
            top: torso_top + into - hh,
            bottom: torso_top + into - 1,
        };
        (
            Some(Lobe {
                a: ta,
                rows: trows,
                top: torso_top,
                bottom,
            }),
            head,
        )
    } else {
        (
            None,
            Lobe {
                a: ha,
                rows,
                top: bottom - hh + 1,
                bottom,
            },
        )
    };
    Layout {
        split,
        welded,
        torso,
        head,
        bottom,
    }
}

/// One named piece of the creature: its cells, the ramp it is coloured
/// from and its depth. Parts of one depth share one outline.
#[derive(Clone, Debug)]
pub(crate) struct Part {
    pub(crate) name: String,
    pub(crate) group: &'static str,
    pub(crate) z: i32,
    pub(crate) mask: Mask,
    pub(crate) ramp: u8,
    /// Cells that take another ramp.
    pub(crate) over: Option<BTreeMap<i32, u8>>,
    /// Cells drawn one step lighter than their shading gives.
    pub(crate) lift: Option<BTreeSet<i32>>,
    /// No shading of its own.
    pub(crate) flat: bool,
    /// A thin part drawn in the outline colour alone.
    pub(crate) line_only: bool,
    /// May keep a single cell of colour (a knob, a tip).
    pub(crate) single: bool,
    /// Steps into the shade for the whole part: a leg further back.
    pub(crate) darker: u8,
    /// How the part is shaded: the creature's coat sets it.
    pub(crate) style: CoatKind,
    pub(crate) on_head: bool,
    pub(crate) ord: i32,
}

/// The options a part is made with.
#[derive(Clone, Copy, Default)]
pub(crate) struct PartOpts {
    pub(crate) ramp: Option<u8>,
    pub(crate) flat: bool,
    pub(crate) line_only: bool,
    pub(crate) single: bool,
    pub(crate) darker: u8,
}

pub(crate) fn overlay(part: &mut Part, cells: &[i32], ramp: u8) {
    let over = part.over.get_or_insert_with(BTreeMap::new);
    for &c in cells {
        if part.mask.at(c) {
            over.insert(c, ramp);
        }
    }
}

/// A cell of the face or a mood mark, drawn over the body.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) col: u8,
    pub(crate) group: &'static str,
    pub(crate) name: String,
}

/// The frame the finished sprite is cut from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Win {
    pub(crate) x0: i32,
    pub(crate) y0: i32,
}

/// Which lobe a marking was placed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LobeId {
    Head,
    Torso,
}

/// A spot or patch, remembered by where it lies on its lobe: a mood moves
/// the creature, not its markings.
#[derive(Clone, Debug)]
pub(crate) struct Marking {
    pub(crate) lobe: LobeId,
    pub(crate) ramp: u8,
    pub(crate) cells: Vec<(i32, i32)>,
}

/// What the neutral sprite settles that the moods carry over, and the fit's
/// place for the anger mark.
pub(crate) struct Memo<'a> {
    pub(crate) genes: &'a Genes,
    pub(crate) palette: &'a Palette,
    pub(crate) bytes: &'a [u8],
    pub(crate) f0: Option<Face>,
    pub(crate) l0: Option<Layout>,
    pub(crate) marks0: Vec<Marking>,
    pub(crate) anger: Option<(i32, i32)>,
}

/// What the builders share while one sprite is made.
pub(crate) struct Ctx<'a> {
    pub(crate) g: &'a Genes,
    pub(crate) p: &'a Palette,
    pub(crate) bytes: &'a [u8],
    pub(crate) mood: Mood,
    pub(crate) pose: Pose,
    pub(crate) lim: Lim,
    pub(crate) l: Layout,
    pub(crate) f: Face,
    pub(crate) parts: Vec<Part>,
    pub(crate) face: Vec<Entry>,
    pub(crate) marks: Vec<Entry>,
    pub(crate) head: usize,
    pub(crate) torso: Option<usize>,
    pub(crate) trunk_part: usize,
    pub(crate) fdx: i32,
    pub(crate) fdy: i32,
    pub(crate) head_dx: i32,
    pub(crate) win: Option<Win>,
}

impl Ctx<'_> {
    /// The lobe the limbs hang from, which also stands on the ground.
    pub(crate) fn trunk(&self) -> &Lobe {
        self.l.torso.as_ref().unwrap_or(&self.l.head)
    }

    pub(crate) fn add(
        &mut self,
        name: &str,
        group: &'static str,
        z: i32,
        mask: Mask,
        o: PartOpts,
        on_head: bool,
    ) -> usize {
        let ord = self.parts.len() as i32;
        self.parts.push(Part {
            name: name.to_owned(),
            group,
            z,
            mask,
            ramp: o.ramp.unwrap_or(BODY),
            over: None,
            lift: None,
            flat: o.flat,
            line_only: o.line_only,
            single: o.single,
            darker: o.darker,
            style: self.g.coat.kind,
            on_head,
            ord,
        });
        self.parts.len() - 1
    }

    /// A pair of parts: the right one as built, the left one its mirror
    /// image unless `left` gives it its own (already mirrored) mask.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_pair(
        &mut self,
        name: &str,
        group: &'static str,
        z: i32,
        right: Mask,
        o: PartOpts,
        on_head: bool,
        left: Option<Mask>,
    ) -> [usize; 2] {
        let left = left.unwrap_or_else(|| right.mirrored());
        let a = self.add(&format!("{name}-r"), group, z, right, o, on_head);
        let b = self.add(&format!("{name}-l"), group, z, left, o, on_head);
        [a, b]
    }
}

/// A sprite built on the working canvas, before it is cut from its frame.
pub(crate) struct Built<'a> {
    pub(crate) ctx: Ctx<'a>,
    pub(crate) comp: Comp,
    pub(crate) bounds: Box4,
}

/// The creature's bounding box on the working canvas.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Box4 {
    pub(crate) x0: i32,
    pub(crate) y0: i32,
    pub(crate) x1: i32,
    pub(crate) y1: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
}

fn build_body(ctx: &mut Ctx) {
    let o = PartOpts::default();
    if ctx.l.split {
        let torso = ctx.l.torso.as_ref().expect("a split body has a torso");
        let mut m = Mask::new();
        m.sym_rows(torso.top, &torso.rows);
        ctx.torso = Some(ctx.add("torso", "body", Z_TORSO, m, o, false));
    }
    let mut m = Mask::new();
    m.sym_rows(ctx.l.head.top, &ctx.l.head.rows);
    if ctx.l.welded {
        let torso = ctx.l.torso.as_ref().expect("a welded body has a belly");
        m.sym_rows(torso.top, &torso.rows);
    }
    let name = if ctx.l.split { "head" } else { "body" };
    ctx.head = ctx.add(name, "body", Z_HEAD, m, o, true);
    if ctx.l.welded {
        ctx.torso = Some(ctx.head);
    }
    ctx.trunk_part = if ctx.l.split {
        ctx.torso.expect("a split body has a torso part")
    } else {
        ctx.head
    };
}

/// Moves a part sideways, with the cells that take other ramps.
fn shift_part(p: &mut Part, dx: i32) {
    p.mask = p.mask.shifted(dx, 0);
    if let Some(over) = &p.over {
        p.over = Some(over.iter().map(|(&c, &r)| (c + dx, r)).collect());
    }
    if let Some(lift) = &p.lift {
        p.lift = Some(lift.iter().map(|&c| c + dx).collect());
    }
}

/// One creature in one mood, on the working canvas. With a frame the mood
/// marks are placed too; the fit builds without one. `ease` is a set of
/// bits: 1 puts arms, wings and tail back at rest; 2 gives up the stretch,
/// the hop and the spread; 4 rests the ears and the head.
pub(crate) fn build_sprite<'a>(
    memo: &mut Memo<'a>,
    mood: Mood,
    lim: Lim,
    win: Option<Win>,
    ease: u32,
) -> Built<'a> {
    let mut pose = pose_of(mood);
    if ease & 1 != 0 {
        pose.arms = Car::Rest;
        pose.wings = Car::Rest;
        pose.tail = Car::Rest;
    }
    if ease & 2 != 0 {
        pose.stretch = 0;
        pose.widen = 0;
        pose.hop = 0;
    }
    if ease & 4 != 0 {
        pose.ears = Car::Rest;
        pose.tilt = 0;
    }
    let mut ctx = Ctx {
        g: memo.genes,
        p: memo.palette,
        bytes: memo.bytes,
        mood,
        pose,
        lim,
        l: layout(memo.genes, &pose, &lim),
        f: Face::empty(memo.genes),
        parts: Vec::new(),
        face: Vec::new(),
        marks: Vec::new(),
        head: 0,
        torso: None,
        trunk_part: 0,
        fdx: 0,
        fdy: 0,
        head_dx: 0,
        win,
    };
    build_body(&mut ctx);
    if mood == Mood::Neutral {
        face_layout(&mut ctx);
    } else {
        face_carry(
            &mut ctx,
            memo.f0.as_ref().expect("the neutral sprite is built first"),
            memo.l0.as_ref().expect("the neutral sprite is built first"),
        );
    }
    face_shift(&mut ctx);
    build_eye_mounts(&mut ctx);
    if ctx.f.bumps.is_none() {
        body::build_top(&mut ctx);
    }
    body::build_side(&mut ctx);
    body::build_legs(&mut ctx);
    body::build_tail(&mut ctx);
    body::build_coat(&mut ctx);
    build_pattern(&mut ctx, &mut memo.marks0);
    // A head on a torso leans over; a body of one lobe only turns its face.
    ctx.head_dx = if ctx.l.split {
        ctx.pose.tilt * ctx.g.temper.side
    } else {
        0
    };
    if ctx.head_dx != 0 {
        let dx = ctx.head_dx;
        for p in ctx.parts.iter_mut().filter(|p| p.on_head) {
            shift_part(p, dx);
        }
    }
    let comp = compose(&ctx.parts);
    draw_face(&mut ctx, &comp);
    if win.is_some() {
        draw_marks(&mut ctx, &comp, memo.anger);
    }
    if mood == Mood::Neutral {
        // The moods carry over the face as the neutral sprite finished it,
        // after its dark eyes took their final width.
        memo.f0 = Some(ctx.f.clone());
        memo.l0 = Some(ctx.l.clone());
    }
    let (mut x0, mut y0, mut x1, mut y1) = (CW, CH, -1, -1);
    for y in 0..CH {
        for x in 0..CW {
            if comp.own[(y * CW + x) as usize] >= 0 {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    Built {
        ctx,
        comp,
        bounds: Box4 {
            x0,
            y0,
            x1,
            y1,
            w: x1 - x0 + 1,
            h: y1 - y0 + 1,
        },
    }
}

/// The column of a flat index, as the reference's `%` gives it.
pub(crate) fn col_of(c: i32) -> i32 {
    c % CW
}

/// The row of a flat index.
pub(crate) fn row_of(c: i32) -> i32 {
    (c - c % CW) / CW
}
