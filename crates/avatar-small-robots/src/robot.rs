//! The whole robot: plugging every part in, fitting it to the frame, and
//! drawing one mood of it as a render tree.

use avatar_svg::{FillRule, Mood, RenderTree};

use crate::arms::{arm_draw, arm_pose, arm_pts, plug_arms, ArmPose, Arms};
use crate::body::{body_draw, head_draw, neck_draw, Core};
use crate::draw::{Ctx, El, Out};
use crate::face::{build_face, finish_face, Face};
use crate::gear::{build_gear, Gear};
use crate::geom::{
    box_of, join_box, line_path, matrix_of, mul_m, pad_box, through, up_dir, Bx, Op, Path, M6,
};
use crate::hull::Hull;
use crate::kit::{clamp, max, min, min3, Palette, Reader, P2};
use crate::marks::{face_extent, plug_chest, plug_decor, Chest, Decor};
use crate::mood::{build_temper, mood_index, mood_of, Mark, MoodSpec, Temper, MOODS};
use crate::parts::{
    cable_draw, cable_pts, ear_draw, ear_pts, mast_draw, mast_pts, pipe_draw, pipe_pts, plug_back,
    plug_sides, plug_stalks, plug_top, puff, stalk_draw, stalk_pts, Cable, Mast, Pipe, Sides,
};

/// Which group a part is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layer {
    Top,
    Side,
    Arms,
    Back,
}

/// The coordinates a part is drawn in: the head's, the upper body's, or the ground's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Frame {
    Head,
    Upper,
    Ground,
}

/// Which of the parts that can be made shorter a part belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Trim {
    Top,
    Arms,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum PartKind {
    Mast(usize),
    Stalk(f64),
    Ear(f64),
    Arm(f64),
    Cable,
    Pipe,
}

/// One part plugged into the robot.
#[derive(Clone, Debug)]
pub(crate) struct Part {
    pub(crate) kind: PartKind,
    pub(crate) class: &'static str,
    pub(crate) layer: Layer,
    pub(crate) frame: Frame,
    pub(crate) trim: Option<Trim>,
}

/// Where everything stands in one mood. The upper part of the robot rises,
/// sinks and leans as one about the base of its lowest module; the head
/// then drops and tips on its neck.
#[derive(Clone, Debug)]
pub(crate) struct Pose {
    pub(crate) lift: f64,
    pub(crate) drop: f64,
    pub(crate) uops: Vec<Op>,
    pub(crate) u: M6,
    hops: Vec<Op>,
    pub(crate) hm: M6,
    arms: [Option<ArmPose>; 2],
}

impl Pose {
    pub(crate) fn arm(&self, side: f64) -> &ArmPose {
        self.arms[usize::from(side > 0.0)]
            .as_ref()
            .expect("an arm part is posed")
    }
}

/// Where the robot stands in the 240-unit frame: its scale and offset.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fit {
    s: f64,
    x: f64,
    y: f64,
}

/// A robot built from a genome: everything the reference's creature holds.
#[derive(Clone, Debug)]
pub(crate) struct Robot {
    pub(crate) p: Palette,
    pub(crate) plan: &'static str,
    pub(crate) scale: f64,
    pub(crate) head: Hull,
    pub(crate) body: Option<Hull>,
    pub(crate) body_kind: Option<&'static str>,
    pub(crate) pelvis: Option<Hull>,
    pub(crate) neck: Option<crate::body::Neck>,
    pub(crate) body_y: f64,
    pub(crate) body_top: f64,
    pub(crate) head_y: f64,
    pub(crate) body_rivets: bool,
    pub(crate) trim_top: f64,
    pub(crate) trim_arms: f64,
    pub(crate) f: Face,
    pub(crate) gear: Gear,
    pub(crate) rise: f64,
    pub(crate) top: &'static str,
    pub(crate) masts: Vec<Mast>,
    pub(crate) tip: &'static str,
    pub(crate) tip_r: f64,
    pub(crate) ant_side: f64,
    pub(crate) stalk_lean: f64,
    pub(crate) side: &'static str,
    pub(crate) sides: Option<Sides>,
    pub(crate) side_vents: Option<[f64; 3]>,
    pub(crate) arms: Arms,
    pub(crate) back: &'static str,
    pub(crate) cable: Option<Cable>,
    pub(crate) pipe: Option<Pipe>,
    pub(crate) parts: Vec<Part>,
    pub(crate) chest: Chest,
    pub(crate) decor: Decor,
    temper: Option<Temper>,
    moods: Vec<MoodSpec>,
    poses: Vec<Pose>,
    fit: Fit,
}

impl Robot {
    /// The lowest module: the lower segment, the chassis, or the head.
    pub(crate) fn low(&self) -> &Hull {
        self.pelvis
            .as_ref()
            .or(self.body.as_ref())
            .unwrap_or(&self.head)
    }

    /// Plugs everything into the modules the plan genes built, with the
    /// antennae and the arms at the given trims.
    fn assemble(r: &Reader, core: &Core, p: &Palette, trim_top: f64, trim_arms: f64) -> Self {
        let mut f = build_face(r, &core.head, core.face_tech, core.scale);
        finish_face(&mut f, p, &core.head);
        let gear = build_gear(r, core, p, core.gear_kind);
        let mut c = Robot {
            p: p.clone(),
            plan: core.plan,
            scale: core.scale,
            head: core.head.clone(),
            body: core.body.clone(),
            body_kind: core.body_kind,
            pelvis: core.pelvis.clone(),
            neck: core.neck.clone(),
            body_y: core.body_y,
            body_top: core.body_top,
            head_y: core.head_y,
            body_rivets: core.body_rivets,
            trim_top,
            trim_arms,
            f,
            rise: gear.rise,
            gear,
            top: "none",
            masts: Vec::new(),
            tip: "",
            tip_r: 0.0,
            ant_side: 0.0,
            stalk_lean: 0.0,
            side: "none",
            sides: None,
            side_vents: None,
            arms: Arms::none(p.metal),
            back: "none",
            cable: None,
            pipe: None,
            parts: Vec::new(),
            chest: Chest::none(),
            decor: Decor::plain(),
            temper: None,
            moods: Vec::new(),
            poses: Vec::new(),
            fit: Fit {
                s: 1.0,
                x: 0.0,
                y: 0.0,
            },
        };
        plug_top(r, &mut c);
        if c.f.stalk.is_some() {
            plug_stalks(r, &mut c);
        }
        plug_sides(r, &mut c);
        plug_arms(r, &mut c);
        plug_back(r, &mut c);
        let ext = face_extent(&c.f);
        let pod = c.body.is_none();
        c.chest = if let Some(b) = &c.body {
            plug_chest(
                r,
                pod,
                &c.f,
                c.body_y - b.h * 0.5,
                b.hw_at(-b.h * 0.5) - 6.5,
                b.h * 0.3,
            )
        } else if -ext.bot >= 15.0 {
            plug_chest(
                r,
                pod,
                &c.f,
                ext.bot / 2.0 + 0.5,
                c.head.hw_at(ext.bot / 2.0) - 10.0,
                -ext.bot / 2.0 - 3.5,
            )
        } else {
            Chest::none()
        };
        c.decor = plug_decor(r, &c.head, &c.f, c.plan == "pod", !pod, &c.chest, &ext);
        let temper = build_temper(r);
        c.temper = Some(temper);
        let hover = c.gear.hover;
        let rest = c.arms.rest;
        c.moods = MOODS
            .iter()
            .map(|m| mood_of(&c.f, &temper, hover, rest, *m))
            .collect();
        c.poses = (0..MOODS.len()).map(|i| pose_of(&c, &c.moods[i])).collect();
        c
    }

    pub(crate) fn mood(&self, m: Mood) -> &MoodSpec {
        &self.moods[mood_index(m)]
    }
}

/// Where everything stands in one mood. A pod has no neck, so its whole
/// hull takes the tip and the drop.
fn pose_of(c: &Robot, m: &MoodSpec) -> Pose {
    let g = &c.gear;
    let pod = c.body.is_none();
    let landed = m.land && g.hover;
    let mut lift = if landed {
        -(c.rise - g.land)
    } else {
        clamp(m.lift, -g.squat, g.lift_max)
    };
    let mut drop = m.drop;
    let mut tilt = m.tilt;
    let mut lean = m.lean;
    if pod {
        if !landed {
            lift = clamp(lift - drop * 0.6, -g.squat, g.lift_max);
        }
        lean += tilt * 0.75;
        drop = 0.0;
        tilt = 0.0;
    }
    let uops = vec![Op::T(0.0, -(c.rise + lift)), Op::R(lean)];
    let u = matrix_of(&uops);
    let hops = if pod {
        Vec::new()
    } else {
        vec![Op::T(0.0, c.head_y + drop), Op::R(tilt)]
    };
    let hm = mul_m(u, matrix_of(&hops));
    let mut pose = Pose {
        lift,
        drop,
        uops,
        u,
        hops,
        hm,
        arms: [None, None],
    };
    let ground = c.rise + lift;
    if c.arms.style != "none" {
        for (i, side) in [-1.0, 1.0].into_iter().enumerate() {
            pose.arms[i] = Some(arm_pose(c, side, m.gesture(side), drop, ground));
        }
    }
    pose
}

fn part_pts(c: &Robot, part: &Part, m: &MoodSpec, pose: &Pose) -> Vec<P2> {
    match part.kind {
        PartKind::Mast(i) => mast_pts(c, &c.masts[i], m),
        PartKind::Stalk(side) => stalk_pts(c, side, m),
        PartKind::Ear(side) => ear_pts(c, side),
        PartKind::Arm(side) => arm_pts(c, side, pose),
        PartKind::Cable => cable_pts(c, m, pose),
        PartKind::Pipe => pipe_pts(c, m),
    }
}

fn part_draw(out: &mut Out, ctx: &Ctx, c: &Robot, part: &Part, m: &MoodSpec, pose: &Pose) {
    match part.kind {
        PartKind::Mast(i) => mast_draw(out, ctx, c, &c.masts[i], m),
        PartKind::Stalk(side) => stalk_draw(out, ctx, c, side, m),
        PartKind::Ear(side) => ear_draw(out, ctx, c, side),
        PartKind::Arm(side) => arm_draw(out, ctx, c, side, m, pose),
        PartKind::Cable => cable_draw(out, ctx, c, m, pose),
        PartKind::Pipe => pipe_draw(out, ctx, c, m),
    }
}

/// A part's points in ground coordinates.
fn part_ground_pts(c: &Robot, part: &Part, m: &MoodSpec, pose: &Pose) -> Vec<P2> {
    let pts = part_pts(c, part, m, pose);
    match part.frame {
        Frame::Ground => pts,
        Frame::Head => through(pose.hm, &pts),
        Frame::Upper => through(pose.u, &pts),
    }
}

/// The boxes one mood's drawing fills, in ground coordinates: all of it,
/// and each of the parts that can be made shorter.
struct MoodBoxes {
    all: Option<Bx>,
    top: Option<Bx>,
    arms: Option<Bx>,
}

fn mood_boxes(c: &Robot, m: &MoodSpec, pose: &Pose) -> MoodBoxes {
    let pad = 2.6;
    let mut bx = Some(box_of(&through(pose.hm, &c.head.poly)));
    if let Some(b) = &c.body {
        bx = join_box(
            bx,
            Some(box_of(&through(
                mul_m(pose.u, [1.0, 0.0, 0.0, 1.0, 0.0, c.body_y]),
                &b.poly,
            ))),
        );
    }
    if let Some(pv) = &c.pelvis {
        bx = join_box(bx, Some(box_of(&through(pose.u, &pv.poly))));
    }
    let mut bx = join_box(
        bx.map(|b| pad_box(b, pad)),
        Some(pad_box(box_of(&c.gear.pts(c, m, pose)), pad)),
    );
    let mut out = MoodBoxes {
        all: None,
        top: None,
        arms: None,
    };
    for part in &c.parts {
        let b = Some(pad_box(
            box_of(&part_ground_pts(c, part, m, pose)),
            pad + 1.0,
        ));
        bx = join_box(bx, b);
        match part.trim {
            Some(Trim::Top) => out.top = join_box(out.top, b),
            Some(Trim::Arms) => out.arms = join_box(out.arms, b),
            None => {}
        }
    }
    out.all = bx;
    out
}

/// The frame the robot is fitted into, less a margin.
const ROOM_W: f64 = 216.0;
const ROOM_H: f64 = 218.0;

/// Which of the antennae and the arms reach past what the frame can hold,
/// taking all seven moods together.
fn overreach(c: &Robot) -> (bool, bool) {
    let mut union = MoodBoxes {
        all: None,
        top: None,
        arms: None,
    };
    for i in 0..MOODS.len() {
        let b = mood_boxes(c, &c.moods[i], &c.poses[i]);
        union.all = join_box(union.all, b.all);
        union.top = join_box(union.top, b.top);
        union.arms = join_box(union.arms, b.arms);
    }
    let a = union.all.expect("a robot fills a box");
    let long = |b: Option<Bx>| {
        b.is_some_and(|b| {
            (a[2] - a[0] > ROOM_W && (b[0] <= a[0] + 0.5 || b[2] >= a[2] - 0.5))
                || (a[3] - a[1] > ROOM_H && b[1] <= a[1] + 0.5)
        })
    };
    (long(union.top), long(union.arms))
}

/// Where the robot stands in the frame. It is drawn at its own size and
/// made smaller only if some mood would otherwise leave the frame; at rest
/// it is centred.
fn fit(c: &Robot) -> Fit {
    let mut all = None;
    let mut neutral = None;
    for i in 0..MOODS.len() {
        let b = mood_boxes(c, &c.moods[i], &c.poses[i]).all;
        if i == 0 {
            neutral = b;
        }
        all = join_box(all, b);
    }
    let ground = Some([-c.gear.foot, 0.0, c.gear.foot, 5.0]);
    let all = join_box(all, ground).expect("a robot fills a box");
    let rest = join_box(neutral, ground).expect("a robot fills a box");
    let s = min3(1.0, 220.0 / (all[2] - all[0]), 222.0 / (all[3] - all[1]));
    let x = clamp(
        120.0 - s * (rest[0] + rest[2]) / 2.0,
        10.0 - s * all[0],
        230.0 - s * all[2],
    );
    let y = clamp(
        122.0 - s * (rest[1] + rest[3]) / 2.0,
        9.0 - s * all[1],
        231.0 - s * all[3],
    );
    Fit { s, x, y }
}

/// Builds the robot a genome describes. If some mood reaches past the frame,
/// whatever reaches furthest (the antennae, or the arms) is made a tenth
/// shorter and everything is plugged in again.
pub(crate) fn creature(bytes: &[u8; crate::GENOME_LEN]) -> Robot {
    let r = Reader::new(bytes);
    let p = crate::kit::build_palette(&r);
    let core = crate::body::build_core(&r);
    let mut trim_top = 1.0;
    let mut trim_arms = 1.0;
    let mut pass = 0;
    let mut c = loop {
        let c = Robot::assemble(&r, &core, &p, trim_top, trim_arms);
        let (top, arms) = if pass < 9 {
            overreach(&c)
        } else {
            (false, false)
        };
        let mut changed = false;
        if top && trim_top > 0.56 {
            trim_top *= 0.9;
            changed = true;
        }
        if arms && trim_arms > 0.56 {
            trim_arms *= 0.9;
            changed = true;
        }
        if !changed {
            break c;
        }
        pass += 1;
    };
    c.fit = fit(&c);
    c
}

/// A cog wheel, as an outline with a hole in it.
fn cog_path(cx: f64, cy: f64, r: f64) -> Path {
    let mut pts = Vec::new();
    let mut hole = Vec::new();
    for k in 0..8 {
        let fk = f64::from(k);
        for t in [[-13.0, 0.72], [-8.0, 1.0], [8.0, 1.0], [13.0, 0.72]] {
            let u = up_dir(fk * 45.0 + t[0]);
            pts.push([cx + u[0] * r * t[1], cy + u[1] * r * t[1]]);
        }
        let v = up_dir(fk * 45.0 + 22.5);
        hole.push([cx + v[0] * r * 0.33, cy + v[1] * r * 0.33]);
    }
    let mut d = line_path(&pts, true);
    d.append(line_path(&hole, true));
    d
}

/// The marks that float beside the head, and puffs of steam from a robot
/// that boils over. They hang just outside the head and whatever stands on
/// it, do not tip with the head, and are kept inside the frame.
fn marks(out: &mut Out, ctx: &Ctx, c: &Robot, m: &MoodSpec, pose: &Pose) {
    let p = &c.p;
    let f = c.fit;
    let hb = box_of(&through(pose.hm, &c.head.poly));
    let edge_at = |sd: f64| {
        if sd > 0.0 {
            (230.0 - f.x) / f.s
        } else {
            (f.x - 10.0) / f.s
        }
    };
    if m.steam && ctx.size >= 48.0 {
        let y = hb[1] + (hb[3] - hb[1]) * 0.32;
        for sd in [-1.0, 1.0] {
            let near = if sd > 0.0 { hb[2] + 5.0 } else { -hb[0] + 5.0 };
            puff(out, c, sd * min(near, edge_at(sd) - 17.0), y, 0.9, sd);
        }
    }
    let Some(mark) = m.mark else { return };
    let sd = if m.mark_side != 0.0 { m.mark_side } else { 1.0 };
    // How far the robot reaches on that side near the top of its head.
    let mut reach = if sd > 0.0 { hb[2] } else { -hb[0] };
    for part in &c.parts {
        if part.frame == Frame::Ground {
            continue;
        }
        for q in part_ground_pts(c, part, m, pose) {
            if q[1] < hb[1] + 34.0 && q[1] > hb[1] - 30.0 {
                reach = max(reach, sd * q[0]);
            }
        }
    }
    let (big, up) = match mark {
        Mark::Anger => (15.0, 13.0),
        Mark::Busy => (27.0, 24.0),
        Mark::Dots => (29.0, 21.0),
        Mark::Zz => (29.0, 28.0),
    };
    let y0 = max(hb[1] + (hb[3] - hb[1]) * 0.12, (9.0 - f.y) / f.s + up);
    let x0 = sd * min(reach + 8.0, edge_at(sd) - big - 6.0);
    let k = ctx.mark_k;
    match mark {
        Mark::Dots => {
            for i in 0..3 {
                let fi = f64::from(i);
                out.put(El::circle([x0 + sd * fi * 9.5, y0 - fi * 5.5], 3.3 * k).fill(p.mark_ink));
            }
        }
        Mark::Anger => {
            // Four corners round a gap: the mark comics draw on a cross forehead.
            let cx = x0 + sd * 6.0;
            let cy = y0 - 3.0;
            let mut d = Path::new();
            for u in [1.0, -1.0] {
                for v in [1.0, -1.0] {
                    d.move_to([cx + u * 7.0, cy + v * 2.0]);
                    d.line_to([cx + u * 2.0, cy + v * 2.0]);
                    d.line_to([cx + u * 2.0, cy + v * 7.0]);
                }
            }
            out.put(El::path(&d).stroke(p.anger, 2.7 * k).round());
        }
        Mark::Busy => {
            let mut d = cog_path(x0 + sd * 9.0, y0 - 4.0, 8.6 * min(k, 1.25));
            d.append(cog_path(x0 + sd * 21.5, y0 - 16.5, 5.6 * min(k, 1.25)));
            out.put(El::path_rule(&d, FillRule::EvenOdd).fill(p.mark_ink));
        }
        Mark::Zz => {
            let zed = |d: &mut Path, x: f64, y: f64, s: f64| {
                d.move_to([x, y]);
                d.line_to([x + s, y]);
                d.line_to([x, y + s]);
                d.line_to([x + s, y + s]);
            };
            let a = [if sd > 0.0 { x0 } else { x0 - 8.0 }, y0 - 6.0];
            let b = [if sd > 0.0 { x0 + 10.0 } else { x0 - 22.0 }, y0 - 24.0];
            let mut d = Path::new();
            zed(&mut d, a[0], a[1], 8.0);
            zed(&mut d, b[0], b[1], 12.0);
            out.put(El::path(&d).stroke(p.mark_ink, 2.5 * k).round());
        }
    }
}

fn layer(out: &mut Out, ctx: &Ctx, c: &Robot, which: Layer, m: &MoodSpec, pose: &Pose) {
    for part in c.parts.iter().filter(|p| p.layer == which) {
        out.group(part.class, &[], |out| part_draw(out, ctx, c, part, m, pose));
    }
}

/// One mood of the robot drawn `size` pixels wide, as the reference draws it.
pub(crate) fn render(c: &Robot, mood: Mood, size: f64) -> RenderTree {
    let m = c.mood(mood);
    let pose = &c.poses[mood_index(mood)];
    let ctx = Ctx::new(size);
    let pod = c.body.is_none();
    let g = &c.gear;
    let f = c.fit;
    let mut out = Out::new();
    // The shadow is smaller and fainter under a robot that is off the ground.
    let high = if g.hover {
        clamp((c.rise + pose.lift) / 60.0, 0.2, 1.0)
    } else {
        clamp(pose.lift / 30.0, 0.0, 0.4)
    };
    let k = 1.0 - 0.32 * high;
    out.put(
        El::ellipse(f.x, f.y + 2.2 * f.s, g.foot * f.s * k, 5.4 * f.s * k)
            .class("rb-shadow")
            .fill(c.p.shadow)
            .opacity(0.15 - 0.05 * high)
            .ground_shadow(),
    );
    out.group("rb-robot", &[Op::T(f.x, f.y), Op::S(f.s, f.s)], |out| {
        out.group("rb-back", &[], |out| {
            layer(out, &ctx, c, Layer::Back, m, pose)
        });
        out.group("rb-gear", &[], |out| g.draw(out, &ctx, c, m, pose));
        out.group("rb-upper", &pose.uops, |out| {
            if !pod {
                out.group("rb-neck", &[], |out| neck_draw(out, &ctx, c, pose));
                out.group("rb-chassis", &[], |out| body_draw(out, &ctx, c, m));
            }
            let head = |out: &mut Out| {
                out.group("rb-antennae", &[], |out| {
                    layer(out, &ctx, c, Layer::Top, m, pose)
                });
                out.group("rb-ears", &[], |out| {
                    layer(out, &ctx, c, Layer::Side, m, pose)
                });
                out.group("rb-head", &[], |out| head_draw(out, &ctx, c, m));
            };
            if pod {
                head(out);
            } else {
                out.group("rb-head-group", &pose.hops, head);
            }
            out.group("rb-arms", &[], |out| {
                layer(out, &ctx, c, Layer::Arms, m, pose)
            });
        });
        out.group("rb-marks", &[], |out| marks(out, &ctx, c, m, pose));
    });
    RenderTree {
        canvas: 240,
        background: None,
        pixel_grid: None,
        primitives: out.prims,
    }
}
