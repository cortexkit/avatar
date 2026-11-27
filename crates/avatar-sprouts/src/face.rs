//! The face: eyes, cheeks, brows and a mouth on the body, and how each mood
//! changes them.

use avatar_svg::{Rgb, StrokeJoin};

use crate::body::{Body, Kind, Weld};
use crate::creature::Creature;
use crate::geometry::{
    clip_half, curve, curve_v, disc, ellipse_poly, group, inside, oval, path_item, polygon, pt,
    ptt, shape, Node, PathD, Pt, Style,
};
use crate::maths::{clamp, cos, hyp, max, min, rot, sin, unit, DEG, DOWN, TAU, UP, V};
use crate::mood::{Brows, Mood, MouthMood, BLOCKED, HAPPY, SAD, SLEEPY, WORKING};
use crate::organs::{blade, lens, BladeForm};
use crate::palette::{hsl, WHITE};
use crate::reader::{dna, Reader};

pub const EYE_PLANS: [&str; 3] = ["two", "one", "three"];
pub const EYE_STYLES: [&str; 6] = ["bead", "white", "sparkle", "iris", "lidded", "wonky"];
pub const MOUTHS: [&str; 10] = [
    "smile", "cat", "open", "snaggle", "flat", "fangs", "o", "grin", "wavy", "jag",
];
const BROWS: [&str; 4] = ["none", "thin", "thick", "leaf"];
const CHEEKS: [&str; 3] = ["blush", "freckles", "none"];
/// The resting brow's shape, as [`brow_line`] takes it.
const REST_BROW: [f64; 4] = [0.42, 0.0, 0.08, 0.05];

/// One eye: where it is, how large against the others, and which side.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Eye {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) k: f64,
    pub(crate) side: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct Face {
    pub(crate) plan: &'static str,
    pub(crate) style: &'static str,
    pub(crate) pupil: f64,
    pub(crate) mouth: &'static str,
    pub(crate) brow: &'static str,
    pub(crate) cheeks: &'static str,
    pub(crate) iris: Option<Rgb>,
    pub(crate) r: f64,
    pub(crate) rx: f64,
    pub(crate) ry: f64,
    pub(crate) ey: f64,
    pub(crate) ex: f64,
    pub(crate) my: f64,
    pub(crate) mw: f64,
    pub(crate) eyes: Vec<Eye>,
    pub(crate) cheek: Option<V>,
}

pub(crate) fn build_face(r: &Reader<'_>, b: &Body, lobes: &[[V; 4]]) -> Face {
    let o = dna::FACE;
    let mut plan = EYE_PLANS[r.pick(o, &[22, 2, 2])];
    let style = EYE_STYLES[r.pick(o + 1, &[4, 5, 3, 3, 2, 2])];
    let pupil = r.lin(o + 6, 0.5, 0.68);
    let mouth = MOUTHS[r.pick(o + 8, &[5, 4, 4, 3, 3, 3, 2, 3, 2, 2])];
    let mut brow = BROWS[r.pick(o + 11, &[9, 3, 2, 2])];
    let cheeks = CHEEKS[r.pick(o + 12, &[6, 2, 2])];
    // Which eye is the smaller one, and by how much.
    let wonk = r.sgn(o + 14);
    let iris = (style == "iris").then(|| hsl(r.lin(o + 15, 0.0, 360.0), 62.0, 52.0));
    // Above 1 the eyes are taller than wide.
    let aspect = r.lin(o + 4, 0.86, 1.22);
    // Half the distance between the eyes, in eye radii.
    let gap = r.lin(o + 3, 1.16, 1.8);
    // How far the mouth sits under the eyes, in eye radii.
    let drop = r.lin(o + 10, 0.3, 0.75);
    let wide = r.lin(o + 9, 0.75, 1.3);
    let [y0, y1, pref] = b.eye;
    // A gourd with its face on the lower lobe has its brows just under the
    // waist (the fifth point of its outline), where a pair of leaves reads
    // as the brim of a hat: its brows are lines.
    if brow == "leaf" && b.kind == Kind::Gourd && b.half[4].y < pref {
        brow = "thick";
    }
    let at = clamp(pref + r.sgn(o + 5) * (y1 - y0) * 0.3, y0, y1);
    let kx = 1.0 / aspect.sqrt();
    let ky = aspect.sqrt();
    let big = if plan == "one" { 1.42 } else { 1.0 };
    let room = |x: f64, y: f64, m: f64| y > b.top + m && y < b.bottom - m && x.abs() + m <= b.hw(y);
    let fits = |ey: f64, r: f64| {
        let rx = r * kx * big;
        let ry = r * ky * big;
        let ex = if plan == "one" { 0.0 } else { gap * r };
        for k in 0..8 {
            let a = TAU * f64::from(k) / 8.0;
            if !room(ex + rx * 1.12 * cos(a), ey + ry * 1.12 * sin(a), 1.5) {
                return false;
            }
        }
        let my = ey + ry + r * drop;
        room(r * 0.62 * wide, my, 2.0) && room(0.0, my + r * 0.75, 2.5)
    };
    // The largest face that fits: try the eye line a little higher and
    // lower, and only then make the face smaller.
    let mut radius = clamp(r.lin(o + 2, 0.27, 0.39) * b.hw(pref), 10.0, 21.0);
    let mut ey = at;
    let mut found = false;
    while !found && radius >= 6.5 {
        for k in 0..=12_u32 {
            if found {
                break;
            }
            let y = at + (if k % 2 == 1 { 1.0 } else { -1.0 }) * (f64::from(k) / 2.0).ceil() * 3.0;
            if y >= y0 - 6.0 && y <= y1 + 6.0 && fits(y, radius) {
                ey = y;
                found = true;
            }
        }
        if !found {
            radius *= 0.94;
        }
    }
    if !found {
        radius = 6.5;
    }
    // The brows at rest lie on the body too: a brow above the outline of
    // the head reads as the brim of a hat or a headband. The eye line moves
    // to where they do, if the eyes still fit there; otherwise the face has
    // no brows at rest.
    let brow_fits = |ey: f64, r: f64| {
        let ex = if plan == "one" { 0.0 } else { gap * r };
        let side = if plan == "one" { 0.0 } else { 1.0 };
        let line = brow_line(
            ex,
            ey,
            r * kx * big,
            r * ky * big,
            r,
            side,
            REST_BROW,
            f64::NEG_INFINITY,
            0.0,
        );
        brow_on_body(b, lobes, &line, brow)
    };
    if brow != "none" && !brow_fits(ey, radius) {
        let mut moved = false;
        for k in 0..=12_u32 {
            let y = at + (if k % 2 == 1 { 1.0 } else { -1.0 }) * (f64::from(k) / 2.0).ceil() * 3.0;
            if y >= y0 - 6.0 && y <= y1 + 6.0 && fits(y, radius) && brow_fits(y, radius) {
                ey = y;
                moved = true;
                break;
            }
        }
        if !moved {
            brow = "none";
        }
    }
    let rad = radius;
    let rx = rad * kx * big;
    let ry = rad * ky * big;
    let ex = if plan == "one" { 0.0 } else { gap * rad };
    let my = ey + ry + rad * drop;
    let mw = rad * 0.62 * wide;
    let mut eyes = if plan == "one" {
        vec![Eye {
            x: 0.0,
            y: ey,
            k: 1.0,
            side: 0.0,
        }]
    } else {
        vec![
            Eye {
                x: ex,
                y: ey,
                k: 1.0,
                side: 1.0,
            },
            Eye {
                x: -ex,
                y: ey,
                k: 1.0,
                side: -1.0,
            },
        ]
    };
    if plan == "three" {
        // A third, smaller eye above the other two, if the forehead has room.
        let k = 0.6;
        let y3 = ey - ry - rad * k * ky * 1.08;
        if room(rad * k * kx * 1.15, y3 - rad * k * ky * 1.15, 1.5) {
            eyes.push(Eye {
                x: 0.0,
                y: y3,
                k,
                side: 0.0,
            });
        } else {
            plan = "two";
        }
    }
    // One eye a little smaller than the other: on the right when the gene
    // is above 0.
    if plan == "two" && (style == "wonky" || wonk.abs() > 0.6) {
        eyes[if wonk > 0.0 { 0 } else { 1 }].k = if style == "wonky" { 0.72 } else { 0.86 };
    }
    // Cheeks sit under the outer corners of the eyes, if there is room.
    let cy = ey + ry * 0.9 + rad * 0.2;
    let mut cx = if plan == "one" {
        rad * 1.75
    } else {
        ex + rx * 0.85
    };
    if cx + rad * 0.4 > b.hw(cy) {
        cx = b.hw(cy) - rad * 0.45;
    }
    let cheek = (cx > if plan == "one" { rad * 1.2 } else { ex * 0.75 }).then_some([cx, cy]);
    Face {
        plan,
        style,
        pupil,
        mouth,
        brow,
        cheeks,
        iris,
        r: rad,
        rx,
        ry,
        ey,
        ex,
        my,
        mw,
        eyes,
        cheek,
    }
}

/// One brow over an eye at (`x`, `y`) with radii `a` and `b`, in a face of
/// eye radius `r`: its inner end, its middle and its outer end. `shape` is
/// how far above the eye the brow sits, how far its inner end drops, its
/// outer end drops and its middle rises, in eye radii. The inner end comes
/// no nearer the middle of the face than `clear`, so that the brows stop
/// short of a third eye between them instead of running through it. `drop`
/// lowers the whole brow.
#[allow(clippy::too_many_arguments)]
fn brow_line(
    x: f64,
    y: f64,
    a: f64,
    b: f64,
    r: f64,
    side: f64,
    shape: [f64; 4],
    clear: f64,
    drop: f64,
) -> [V; 3] {
    let [up, inner, outer, arch] = shape;
    let s = if side != 0.0 { side } else { 1.0 };
    let slant = if side != 0.0 { 1.0 } else { 0.0 };
    let half = a * 0.95;
    let by = y - b - r * up + drop;
    let mut pin = [x - s * half, by + r * inner * slant];
    let pout = [x + s * half, by + r * outer * slant];
    let mut mid = x;
    if s * pin[0] < clear {
        pin[0] = s * clear;
        mid = (pin[0] + pout[0]) / 2.0;
    }
    let pmid = [mid, by - r * arch + r * (inner + outer) * 0.5 * slant];
    [pin, pmid, pout]
}

/// Whether a brow drawn along `line` lies on the body, clear of its
/// outline: points along it, each with the room the brow takes there, lie
/// inside the body, whatever is welded to it and the lobes of a rosette as
/// they lie at rest. A leaf brow is wider than a line in its middle and
/// reaches a little past its outer end.
fn brow_on_body(b: &Body, lobes: &[[V; 4]], line: &[V; 3], brow: &str) -> bool {
    let [pin, pmid, pout] = *line;
    let between = |p: V, q: V| [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
    let mut marks: Vec<(V, f64)> = Vec::new();
    if brow == "leaf" {
        let l = hyp(pout[0] - pin[0], pout[1] - pin[1]);
        let v = unit(pout[0] - pin[0], pout[1] - pin[1]);
        for (t, k) in [
            (0.0, 0.0),
            (0.25, 0.14),
            (0.5, 0.2),
            (0.75, 0.14),
            (1.0, 0.0),
        ] {
            let at = l * 1.08 * t;
            marks.push(([pin[0] + v[0] * at, pin[1] + v[1] * at], 2.0 + l * k));
        }
    } else {
        for q in [pin, between(pin, pmid), pmid, between(pmid, pout), pout] {
            marks.push((q, 3.0));
        }
    }
    let has = |x: f64, y: f64| {
        inside(&b.poly, x, y)
            || b.welds.iter().any(|w| match w {
                Weld::Disc(d) => hyp(x - d[0], y - d[1]) < d[2],
                Weld::Axis(_) => false,
            })
            || lobes.iter().any(|q| inside(q, x, y))
    };
    marks.iter().all(|&(q, m)| {
        has(q[0], q[1])
            && (0..8).all(|k| {
                let a = TAU * f64::from(k) / 8.0;
                has(q[0] + m * cos(a), q[1] + m * sin(a))
            })
    })
}

/// One mouth. `w` is its half-width, `d` its unit of depth, `y` the height
/// of the line the lips meet on.
fn mouth_items(c: &Creature, kind: &str, w: f64, d: f64, y: f64) -> Vec<Node> {
    let p = &c.p;
    let mut items = Vec::new();
    let pen = || Style::line(p.mouth, 2.4, "sp-mouth-line");
    let at = |u: f64, v: f64| pt(u * w, y + v * d);
    // On a dark body the lips of an open mouth are drawn pale.
    let inside = || {
        Style::filled(
            p.mouth_inside,
            if p.dark { p.mouth } else { p.outline },
            1.5,
            "sp-mouth-inside",
        )
        .round_join()
    };
    let tooth = |pts: &[Pt]| {
        let pts: Vec<V> = pts.iter().map(|q| q.v()).collect();
        polygon(
            &pts,
            Style::filled(WHITE, p.outline, 1.0, "sp-tooth").round_join(),
        )
    };
    let tongue = |u: f64, v: f64, k: f64| {
        oval(
            u * w,
            y + v * d,
            w * 0.42 * k,
            d * 0.42 * k,
            Style::fill(p.tongue, "sp-tongue"),
        )
    };
    match kind {
        "smile" => items.push(curve(&[at(-1.0, 0.0), at(0.0, 0.8), at(1.0, 0.0)], pen())),
        "bigsmile" => items.push(curve(
            &[at(-1.15, -0.1), at(0.0, 1.25), at(1.15, -0.1)],
            pen(),
        )),
        "cat" => items.push(curve(
            &[
                at(-1.0, 0.0),
                at(-0.5, 0.7),
                ptt(0.0, y, 0.1),
                at(0.5, 0.7),
                at(1.0, 0.0),
            ],
            pen(),
        )),
        "open" => {
            items.push(shape(
                &[
                    ptt(-w, y, 0.2),
                    at(-0.6, 1.2),
                    at(0.0, 1.55),
                    at(0.6, 1.2),
                    ptt(w, y, 0.2),
                ],
                inside(),
            ));
            items.push(tongue(0.0, 1.05, 1.0));
        }
        "bigopen" => {
            items.push(shape(
                &[
                    ptt(-w * 1.15, y - d * 0.1, 0.2),
                    at(-0.7, 1.5),
                    at(0.0, 2.0),
                    at(0.7, 1.5),
                    ptt(w * 1.15, y - d * 0.1, 0.2),
                ],
                inside(),
            ));
            items.push(tongue(0.0, 1.4, 1.15));
        }
        "snaggle" => {
            items.push(curve(&[at(-1.0, 0.0), at(0.0, 0.8), at(1.0, 0.0)], pen()));
            items.push(tooth(&[at(0.2, 0.6), at(0.62, 0.42), at(0.46, 1.5)]));
        }
        "fangs" => {
            items.push(curve(&[at(-1.0, 0.0), at(0.0, 0.7), at(1.0, 0.0)], pen()));
            items.push(tooth(&[at(-0.66, 0.3), at(-0.32, 0.56), at(-0.5, 1.4)]));
            items.push(tooth(&[at(0.66, 0.3), at(0.32, 0.56), at(0.5, 1.4)]));
        }
        "flat" => items.push(curve(&[at(-0.9, 0.2), at(0.9, 0.2)], pen())),
        "pressed" => items.push(curve(&[at(-0.6, 0.3), at(0.6, 0.3)], pen())),
        "tongue" => {
            items.push(curve(&[at(-0.6, 0.3), at(0.6, 0.3)], pen()));
            items.push(oval(
                w * 0.42,
                y + d * 0.62,
                w * 0.24,
                d * 0.38,
                Style::filled(p.tongue, p.outline, 1.2, "sp-tongue"),
            ));
        }
        "o" => items.push(oval(0.0, y + d * 0.5, w * 0.34, d * 0.52, inside())),
        "sleep" => items.push(oval(0.0, y + d * 0.45, w * 0.22, d * 0.3, inside())),
        "grin" => {
            items.push(shape(
                &[
                    ptt(-w, y, 0.2),
                    at(0.0, 0.5),
                    ptt(w, y, 0.2),
                    at(0.55, 1.2),
                    at(0.0, 1.45),
                    at(-0.55, 1.2),
                ],
                Style::filled(WHITE, p.outline, 1.5, "sp-teeth").round_join(),
            ));
            items.push(curve(
                &[at(-0.72, 0.72), at(0.0, 0.98), at(0.72, 0.72)],
                Style::line(p.outline, 1.0, "sp-mouth-line"),
            ));
        }
        "wavy" => items.push(curve(
            &[
                at(-1.0, 0.3),
                at(-0.5, -0.1),
                at(0.0, 0.5),
                at(0.5, -0.1),
                at(1.0, 0.3),
            ],
            pen(),
        )),
        "jag" => {
            let mut item = curve(
                &[
                    ptt(-w, y, 0.0),
                    ptt(-w * 0.5, y + d * 0.8, 0.0),
                    ptt(0.0, y + d * 0.1, 0.0),
                    ptt(w * 0.5, y + d * 0.8, 0.0),
                    ptt(w, y, 0.0),
                ],
                pen(),
            );
            item.style_mut().join = Some(StrokeJoin::Miter);
            items.push(item);
        }
        "frown" => items.push(curve(
            &[at(-0.85, 0.75), at(0.0, 0.1), at(0.85, 0.75)],
            pen(),
        )),
        "down" => items.push(curve(
            &[at(-0.7, 0.85), at(-0.2, 0.35), at(0.25, 0.3), at(0.7, 0.8)],
            pen(),
        )),
        _ => {
            // Bared teeth: a box of them with the lips drawn back.
            items.push(shape(
                &[
                    at(-0.9, 0.0),
                    at(0.0, -0.12),
                    at(0.9, 0.0),
                    at(0.86, 1.0),
                    at(0.0, 1.1),
                    at(-0.86, 1.0),
                ],
                Style::filled(WHITE, p.outline, 1.5, "sp-teeth").round_join(),
            ));
            let mut lines = PathD::default();
            lines.push_line(at(-0.3, -0.05).v(), at(-0.3, 1.05).v());
            lines.push_line(at(0.3, -0.05).v(), at(0.3, 1.05).v());
            lines.push_line(at(-0.88, 0.5).v(), at(0.88, 0.5).v());
            items.push(path_item(
                lines,
                [-w, y, w, y + d],
                Style::line(p.outline, 1.0, "sp-mouth-line"),
            ));
        }
    }
    items
}

/// The face in one mood, as groups of items from back to front.
pub(crate) fn face_items(c: &Creature, m: &Mood) -> Vec<Node> {
    let f = &c.f;
    let p = &c.p;
    let t = &c.t;
    let r = f.r;
    let mut cheeks = Vec::new();
    let mut eyes = Vec::new();
    let mut lids = Vec::new();
    let mut brows = Vec::new();
    let mut marks = Vec::new();
    let whites = f.style != "bead" && f.style != "sparkle";
    if let (Some(cheek), true) = (f.cheek, f.cheeks != "none") {
        for s in [1.0, -1.0] {
            let cx = s * cheek[0];
            let cy = cheek[1];
            if f.cheeks == "blush" {
                cheeks.push(oval(
                    cx,
                    cy,
                    r * 0.55,
                    r * 0.33,
                    Style::fill(p.blush, "sp-cheek").op(min(
                        1.0,
                        p.blush_alpha * if m.index == HAPPY { 1.2 } else { 1.0 },
                    )),
                ));
            } else {
                for [u, v] in [[-0.3, 0.0], [0.3, -0.05], [0.0, 0.3]] {
                    cheeks.push(disc(
                        cx + u * r,
                        cy + v * r,
                        1.3,
                        Style::fill(p.deep, "sp-freckle").op(0.55),
                    ));
                }
            }
        }
    }
    let closed = m.lid >= 0.85;
    let arcs = !closed && m.squint >= 0.4 && t.smile_eyes;
    let lid_line_width = if p.dark { 3.8 } else { 3.0 };
    for e in &f.eyes {
        let a = f.rx * e.k * m.eye;
        let b = f.ry * e.k * m.eye;
        let (x, y, side) = (e.x, e.y, e.side);
        let mut one = Vec::new();
        let mut lid = Vec::new();
        if closed {
            // Shut for the night: one line, curved like a lowered lid.
            lid.push(curve(
                &[
                    pt(x - a, y + b * 0.1),
                    pt(x, y + b * 0.6),
                    pt(x + a, y + b * 0.1),
                ],
                Style::line(p.ink, lid_line_width, "sp-lid-line"),
            ));
        } else if arcs {
            // Eyes squeezed shut by a smile.
            lid.push(curve(
                &[
                    pt(x - a, y + b * 0.3),
                    pt(x, y - b * 0.5),
                    pt(x + a, y + b * 0.3),
                ],
                Style::line(p.ink, lid_line_width, "sp-lid-line"),
            ));
        } else {
            // Where it looks. A wonky pair looks two ways at once.
            let mut gx = m.gaze[0];
            let mut gy = m.gaze[1];
            if f.style == "wonky" && side != 0.0 {
                gx = clamp(gx + side * 0.45, -1.0, 1.0);
                gy = clamp(gy - side * 0.3, -1.0, 1.0);
            }
            if whites {
                one.push(oval(
                    x,
                    y,
                    a,
                    b,
                    Style::filled(p.white, p.outline, 1.6, "sp-sclera"),
                ));
                let mut pr = f.pupil * min(a, b) * if m.index == BLOCKED { 0.82 } else { 1.0 };
                let (px, py);
                if f.style == "iris" {
                    let ir = min(a, b) * 0.8;
                    px = x + gx * (a - ir) * 0.9;
                    py = y + gy * (b - ir) * 0.9;
                    one.push(disc(
                        px,
                        py,
                        ir,
                        Style::fill(f.iris.expect("an iris eye has an iris colour"), "sp-iris"),
                    ));
                    pr = ir * 0.56;
                } else {
                    px = x + gx * (a - pr) * 0.85;
                    py = y + gy * (b - pr) * 0.85;
                }
                one.push(disc(px, py, pr, Style::fill(p.pupil, "sp-pupil")));
                one.push(disc(
                    px + pr * 0.32,
                    py - pr * 0.36,
                    pr * 0.3,
                    Style::fill(WHITE, "sp-shine").op(0.92),
                ));
            } else {
                // A bead of an eye. On a dark body it gets a pale backing.
                if p.dark {
                    one.push(oval(
                        x,
                        y,
                        a * 1.14,
                        b * 1.14,
                        Style::fill(p.white, "sp-sclera"),
                    ));
                }
                let px = x + gx * a * 0.12;
                let py = y + gy * b * 0.12;
                one.push(oval(
                    px,
                    py,
                    a * 0.9,
                    b * 0.9,
                    Style::fill(p.pupil, "sp-pupil"),
                ));
                one.push(disc(
                    px + a * 0.3,
                    py - b * 0.34,
                    a * 0.26,
                    Style::fill(WHITE, "sp-shine").op(0.92),
                ));
                if f.style == "sparkle" {
                    one.push(disc(
                        px - a * 0.3,
                        py + b * 0.32,
                        a * 0.13,
                        Style::fill(WHITE, "sp-shine").op(0.85),
                    ));
                }
            }
            // The upper lid: the body's own colour over the top of the eye,
            // down to a line that tilts with the mood.
            let cover = max(m.lid, if f.style == "lidded" { 0.26 } else { 0.0 });
            let grow = if p.dark && !whites { 1.24 } else { 1.1 };
            if cover > 0.02 {
                let o = [x, y - b + 2.0 * b * cover];
                let n = rot(DOWN, -side * m.lid_a * DEG);
                let (poly, _) = clip_half(&ellipse_poly(x, y, a * grow, b * grow, 20), o, n);
                lid.push(polygon(&poly, Style::fill(p.body, "sp-lid")));
                let (_, cut) = clip_half(&ellipse_poly(x, y, a * 1.02, b * 1.02, 20), o, n);
                if cut.len() == 2 {
                    lid.push(curve_v(&cut, Style::line(p.outline, 1.8, "sp-lid-line")));
                }
            }
            // The lower lid, pushed up by a smile.
            if m.squint > 0.05 {
                let o = [x, y + b - 2.0 * b * m.squint * 0.5];
                let (poly, _) = clip_half(&ellipse_poly(x, y, a * grow, b * grow, 20), o, UP);
                lid.push(polygon(&poly, Style::fill(p.body, "sp-lid sp-lid-lower")));
                let (_, cut) = clip_half(&ellipse_poly(x, y, a * 1.02, b * 1.02, 20), o, UP);
                if cut.len() == 2 {
                    lid.push(curve_v(
                        &[cut[0], [x, o[1] - b * 0.14], cut[1]],
                        Style::line(p.outline, 1.6, "sp-lid-line"),
                    ));
                }
            }
        }
        if !one.is_empty() {
            eyes.push(group(one));
        }
        if !lid.is_empty() {
            lids.push(group(lid));
        }
        // Brows. A mood draws its own; otherwise the resting brows show.
        let kind = match m.brows {
            Brows::Rest | Brows::None => {
                if f.brow == "none" {
                    None
                } else {
                    Some(Brows::Rest)
                }
            }
            other => Some(other),
        };
        if let (Some(kind), true) = (kind, e.k >= 0.7) {
            let shape = match kind {
                Brows::Rest | Brows::None => REST_BROW,
                Brows::Focus => [0.3, 0.14, 0.0, 0.0],
                Brows::Raised => [0.74, 0.0, 0.05, 0.2],
                Brows::Knit => [0.36, 0.5, -0.12, 0.0],
                Brows::Worry => [0.5, -0.42, 0.14, 0.04],
            };
            // A third eye sits between the brows; they stop short of it,
            // clear of its outline.
            let clear = if f.plan == "three" {
                f.rx * 0.6 * m.eye * 1.2 + 2.5
            } else {
                f64::NEG_INFINITY
            };
            // A mood's brows may rise off the head; they come down onto it,
            // but no lower than a little above the eye.
            let room = max(0.0, r * (shape[0] - 0.15));
            let lobes = c.lobes.as_ref().map_or(&[][..], |l| l.quads.as_slice());
            let mut drop = 0.0;
            let mut line = brow_line(x, y, a, b, r, side, shape, clear, drop);
            while drop < room && !brow_on_body(&c.b, lobes, &line, f.brow) {
                drop = min(room, drop + 1.0);
                line = brow_line(x, y, a, b, r, side, shape, clear, drop);
            }
            let [pin, pmid, pout] = line;
            if f.brow == "leaf" {
                let l = hyp(pout[0] - pin[0], pout[1] - pin[1]);
                let v = unit(pout[0] - pin[0], pout[1] - pin[1]);
                let mut item = lens(pin, v, l * 1.08, l * 0.2, 0.15);
                *item.style_mut() = Style::filled(p.leaf, p.leaf_dark, 1.1, "sp-brow").round_join();
                brows.push(item);
            } else {
                brows.push(curve_v(
                    &[pin, pmid, pout],
                    Style::line(
                        p.ink,
                        if f.brow == "thick" {
                            3.6
                        } else if f.brow == "thin" {
                            2.3
                        } else {
                            3.0
                        },
                        "sp-brow",
                    ),
                ));
            }
        }
    }
    // The mouth.
    let open = matches!(f.mouth, "open" | "grin" | "jag" | "o");
    let kind = match m.mouth {
        MouthMood::Rest => f.mouth,
        MouthMood::Pressed => {
            if t.tongue {
                "tongue"
            } else {
                "pressed"
            }
        }
        MouthMood::Grin => {
            if open || !t.smile_eyes {
                "bigopen"
            } else {
                "bigsmile"
            }
        }
        MouthMood::O => "o",
        MouthMood::Grit => {
            if t.grit {
                "grit"
            } else {
                "frown"
            }
        }
        MouthMood::Down => "down",
        MouthMood::Sleep => "sleep",
    };
    let mouth = mouth_items(c, kind, f.mw, r * 0.42, f.my);
    // Mood marks: a bead of sweat, a tear, the letters of sleep.
    let wet = || {
        Style::filled(
            crate::palette::rgb(0xa6dcf2),
            crate::palette::rgb(0x5b9dbd),
            1.0,
            "sp-mark",
        )
        .round_join()
    };
    let drop = BladeForm {
        n: 6,
        peak: 1.8,
        round: 1.0,
        tip: 1.0,
        base: 0.1,
        ..BladeForm::default()
    };
    if m.index == WORKING {
        let b = blade(
            [t.side * (f.ex + f.rx + r * 0.5), f.ey - f.ry * 1.25],
            DOWN,
            r * 0.95,
            r * 0.3,
            &drop,
            0.0,
            0.0,
            0.0,
        );
        marks.push(shape(&b.out, wet()));
    }
    if m.index == SAD {
        let b = blade(
            [-t.side * (f.ex + f.rx * 0.35), f.ey + f.ry * 0.9],
            DOWN,
            r * 0.85,
            r * 0.27,
            &drop,
            0.0,
            0.0,
            0.0,
        );
        marks.push(shape(&b.out, wet()));
    }
    if m.index == SLEEPY {
        let x0 = t.side * (c.b.hw(f.ey - f.ry * 2.0) * 0.7 + r * 0.9);
        let y0 = f.ey - f.ry - r * 1.5;
        for [u, v, k] in [[0.0, 0.0, 1.0], [t.side * 0.95, -1.05, 0.68]] {
            let x = x0 + u * r;
            let y = y0 + v * r;
            let z = r * 0.36 * k;
            let pts = [
                ptt(x - z, y - z, 0.0),
                ptt(x + z, y - z, 0.0),
                ptt(x - z, y + z, 0.0),
                ptt(x + z, y + z, 0.0),
            ];
            marks.push(curve(
                &pts,
                Style::line(WHITE, 4.6, "sp-mark-halo").op(0.75),
            ));
            marks.push(curve(&pts, Style::line(p.outline, 2.1, "sp-mark")));
        }
    }
    let mut out = Vec::new();
    if !cheeks.is_empty() {
        out.push(group(cheeks));
    }
    out.push(group(eyes));
    out.push(group(lids));
    out.push(group(brows));
    out.push(group(mouth));
    if !marks.is_empty() {
        out.push(group(marks));
    }
    out
}
