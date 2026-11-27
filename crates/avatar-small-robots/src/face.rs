//! The face: which unit it is, where its eyes and mouth go on the hull, and
//! the colours and places of its cheeks and tears.

use crate::geom::{quad, rrect, shift_pts, Cp};
use crate::hull::{face_box_of, FaceBox, Hull};
use crate::kit::{blend, clamp, hex, lerp, max, min, min3, Palette, Reader};
use avatar_svg::Rgb;

const FACE: usize = 1048;
const MOUTH: usize = 1058;
const BROWS: usize = 1062;

/// The face units, in the order the face gene picks them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tech {
    Screen,
    Lens,
    Lamp,
    Mono,
    Tri,
    Odd,
    Stalk,
}

impl Tech {
    pub(crate) const ALL: [Tech; 7] = [
        Tech::Screen,
        Tech::Lens,
        Tech::Lamp,
        Tech::Mono,
        Tech::Tri,
        Tech::Odd,
        Tech::Stalk,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tech::Screen => "screen",
            Tech::Lens => "lens",
            Tech::Lamp => "lamp",
            Tech::Mono => "mono",
            Tech::Tri => "tri",
            Tech::Odd => "odd",
            Tech::Stalk => "stalk",
        }
    }
}

/// The shapes a screen draws its eyes in.
pub const SCREEN_EYES: [&str; 6] = ["dot", "pill", "square", "wide", "ring", "pupil"];

/// The resting forms of a mouth line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Form {
    Arc,
    Cat,
    Steps,
    Flat,
    Vee,
    O,
    Zig,
    Wave,
    Bar,
}

const MOUTH_FORMS: [Form; 5] = [Form::Arc, Form::Cat, Form::Steps, Form::Flat, Form::Vee];

/// What carries the mouth: a line a screen draws, a display strip, a slot,
/// or a grille. `None` is the empty mouth a face starts from; every robot's
/// face ends with one of the others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouthKind {
    Line,
    Strip,
    Slot,
    Grid,
    None,
}

impl MouthKind {
    pub fn name(self) -> &'static str {
        match self {
            MouthKind::Line => "line",
            MouthKind::Strip => "strip",
            MouthKind::Slot => "slot",
            MouthKind::Grid => "grid",
            MouthKind::None => "none",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Mouth {
    pub(crate) kind: MouthKind,
    pub(crate) form: Form,
    pub(crate) rest: f64,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) w: f64,
    pub(crate) dh: f64,
    pub(crate) panel: Vec<Cp>,
    /// A grille's band height and its number of bars.
    pub(crate) t: f64,
    pub(crate) bars: u32,
}

impl Mouth {
    fn none() -> Self {
        Self {
            kind: MouthKind::None,
            form: Form::Arc,
            rest: 0.0,
            x: 0.0,
            y: 0.0,
            w: 0.0,
            dh: 0.0,
            panel: Vec::new(),
            t: 0.0,
            bars: 0,
        }
    }
}

/// One eye: where it is, its half-width and half-height, how far its
/// corners are cut, and what a screen draws inside it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Eye {
    pub(crate) side: f64,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) rx: f64,
    pub(crate) ry: f64,
    pub(crate) qx: f64,
    pub(crate) qy: f64,
    pub(crate) lamp: bool,
    pub(crate) big: bool,
    pub(crate) pod: bool,
    /// A round hole that shows the screen, as a share of the eye's radius.
    pub(crate) hole: f64,
    pub(crate) hole_fine: bool,
    /// The width of a ring round a pupil, and the pupil's radius.
    pub(crate) rim: Option<f64>,
    pub(crate) dot: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct Screen {
    pub(crate) w: f64,
    pub(crate) h: f64,
    pub(crate) cy: f64,
    pub(crate) r: f64,
    pub(crate) oval: bool,
    pub(crate) face_w: f64,
    pub(crate) pts: Vec<Cp>,
}

/// Lens pods on stalks: their radius, where the stalks stand, how long
/// they are, and the lens proportions.
#[derive(Clone, Debug)]
pub(crate) struct Stalk {
    pub(crate) r: f64,
    pub(crate) bx: f64,
    pub(crate) len: f64,
    pub(crate) shape: [f64; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BrowKind {
    Rest,
    Mood,
    Bar,
    Heavy,
}

#[derive(Clone, Debug)]
pub(crate) struct Face {
    pub(crate) tech: Tech,
    pub(crate) eyes: Vec<Eye>,
    pub(crate) stalk: Option<Stalk>,
    pub(crate) screen: Option<Screen>,
    pub(crate) lcd: bool,
    pub(crate) eye_style: &'static str,
    pub(crate) mouth: Mouth,
    pub(crate) brow_kind: BrowKind,
    pub(crate) brow_room: bool,
    pub(crate) bezel: f64,
    pub(crate) rest_lid: f64,
    pub(crate) bx: FaceBox,
    pub(crate) lamp_pupil: bool,
    pub(crate) iris: f64,
    pub(crate) pupil: f64,
    pub(crate) glint: f64,
    /// The screen's colours: its glass, the glass's edge, and what lights on it.
    pub(crate) back: Rgb,
    pub(crate) back_line: Rgb,
    pub(crate) on_base: Rgb,
    pub(crate) on_lite_base: Rgb,
    pub(crate) cheeks: Option<[[f64; 3]; 2]>,
    tear: Tear,
}

/// Where a tear can run from.
#[derive(Clone, Copy, Debug)]
enum Tear {
    Nowhere,
    Screen { ex: f64, erx: f64, ty: f64 },
    Hull,
}

/// Whether an upright ellipse round (x, y) lies on the hull, at least `m`
/// inside its outline, tested at eight points of the ellipse.
// The reference writes the diagonal as 0.7071, not as 1/sqrt(2).
#[allow(clippy::approx_constant)]
const RING: [[f64; 2]; 8] = [
    [1.0, 0.0],
    [0.7071, 0.7071],
    [0.0, 1.0],
    [-0.7071, 0.7071],
    [-1.0, 0.0],
    [-0.7071, -0.7071],
    [0.0, -1.0],
    [0.7071, -0.7071],
];

fn on_hull(hull: &Hull, x: f64, y: f64, rx: f64, ry: f64, m: f64) -> bool {
    if y - ry < -hull.h + m || y + ry > -m {
        return false;
    }
    RING.iter()
        .all(|t| hull.hw_at(y + t[1] * ry) >= (x + t[0] * rx).abs() + m)
}

/// How far to each side a mouth `mh` tall may reach at height y, up to `most`.
fn mouth_room(hull: &Hull, y: f64, mh: f64, most: f64) -> f64 {
    let mut w = most;
    while w > 7.0 {
        if on_hull(hull, 0.0, y, w, mh, 4.0) {
            return w;
        }
        w -= 1.0;
    }
    7.0
}

struct Fit {
    r: f64,
    ey: f64,
    my: Option<f64>,
}

/// The largest eyes of one arrangement the hull has room for, at the height
/// that puts the face nearest the middle of the hull.
fn fit_eyes(
    hull: &Hull,
    eyes_of: &dyn Fn(f64, f64) -> Vec<Eye>,
    mouth_half: f64,
    floor: f64,
) -> Fit {
    let h = hull.h;
    let m = 3.6;
    let mut r = min(hull.w * 0.8, h * 0.44);
    while r >= 4.0 {
        let mut best: Option<(Fit, f64)> = None;
        let mut ey = -h + r + m;
        while ey <= floor - r {
            let eyes = eyes_of(r, ey);
            let fits = eyes.iter().all(|e| {
                e.y + e.ry <= floor && on_hull(hull, e.x, e.y, e.rx * 1.03, e.ry * 1.03, m)
            });
            if fits {
                let top = eyes.iter().map(|e| e.y - e.ry).fold(f64::INFINITY, min);
                let low = eyes.iter().map(|e| e.y + e.ry).fold(f64::NEG_INFINITY, max);
                let mut my = None;
                let mut ok = true;
                if mouth_half != 0.0 {
                    let y = low + 3.0 + mouth_half;
                    my = Some(y);
                    ok = on_hull(hull, 0.0, y, max(11.0, r * 0.75), mouth_half, 4.0);
                }
                if ok {
                    let bottom = my.map_or(low, |y| y + mouth_half);
                    let off = ((top + bottom) / 2.0 + h * 0.5).abs();
                    if best.as_ref().is_none_or(|(_, o)| off < *o) {
                        best = Some((Fit { r, ey, my }, off));
                    }
                }
            }
            ey += 1.5;
        }
        if let Some((fit, _)) = best {
            return fit;
        }
        r -= 0.5;
    }
    Fit {
        r: 4.0,
        ey: -h * 0.55,
        my: (mouth_half != 0.0).then_some(-h * 0.3),
    }
}

/// How far to each side something `ry` tall may reach at height y.
fn reach_at(hull: &Hull, y: f64, ry: f64, m: f64) -> f64 {
    min3(hull.hw_at(y - ry), hull.hw_at(y), hull.hw_at(y + ry)) - m
}

fn lens_eye(side: f64, x: f64, y: f64, r: f64, sd: [f64; 4], lamp: bool, big: bool) -> Eye {
    Eye {
        side,
        x,
        y,
        rx: r * sd[0],
        ry: r * sd[1],
        qx: r * sd[2],
        qy: r * sd[3],
        lamp,
        big,
        ..Eye::default()
    }
}

/// Lays out the eyes of one arrangement for a radius and a height.
type EyesOf = Box<dyn Fn(f64, f64) -> Vec<Eye>>;

fn apart(r: f64) -> f64 {
    max(4.0, r * 0.3)
}

pub(crate) fn build_face(r: &Reader, hull: &Hull, tech: Tech, scale: f64) -> Face {
    let o = FACE;
    let om = MOUTH;
    let h = hull.h;
    let tee = hull.kind == "tee";
    let size = || r.unit(o + 3, 0, 3);
    let spacing = || r.unit(o + 3, 3, 3);
    let form = || MOUTH_FORMS[r.pick(om + 1, 0, &[10, 6, 6, 6, 4], 0)];
    let rest = || r.lin(om + 1, 5, 3, 0.05, 0.75);
    let mut f = Face {
        tech,
        eyes: Vec::new(),
        stalk: None,
        screen: None,
        lcd: false,
        eye_style: "",
        mouth: Mouth::none(),
        brow_kind: BrowKind::Mood,
        brow_room: true,
        bezel: 0.0,
        rest_lid: 0.0,
        bx: FaceBox {
            cy: 0.0,
            fw: 0.0,
            fh: 0.0,
        },
        lamp_pupil: false,
        iris: 0.0,
        pupil: 0.0,
        glint: 0.0,
        back: hex(0),
        back_line: hex(0),
        on_base: hex(0),
        on_lite_base: hex(0),
        cheeks: None,
        tear: Tear::Nowhere,
    };
    if tech == Tech::Screen {
        // A hull with a jaw carries its screen above the jaw.
        let mut s = face_box_of(hull, 5.5, if tee { hull.jaw_h + 2.0 } else { 0.0 });
        let shape_bits = r.raw(o + 1, 0, 2);
        let asked = ["round", "rounder", "pill", "oval"][shape_bits as usize];
        let mut shape = asked;
        let most = min(s.fh, h * 0.3);
        let mut sw = min(s.fw, hull.hw_at(s.cy) * 0.66) * r.lin(o + 1, 5, 2, 0.86, 1.0);
        let mut sh = most * r.lin(o + 4, 5, 2, 0.82, 1.0);
        sh = min(most, clamp(sh, sw / 1.7, sw * 0.95));
        sw = min(sw, sh * 1.7);
        let cy = s.cy - (s.fh - sh) * 0.4;
        sw = min3(
            sw,
            hull.hw_at(cy - sh * 0.75) - 11.0,
            hull.hw_at(cy + sh * 0.75) - 11.0,
        );
        // A head too narrow where the screen would sit looks through lenses.
        if sw < 23.0 {
            f.tech = Tech::Lens;
        } else {
            f.lcd = r.pick(o + 1, 2, &[3, 1], 0) == 1;
            if f.lcd {
                shape = "panel";
            } else if (shape == "pill" && sw < sh * 1.3) || (shape == "oval" && sw < sh * 1.4) {
                shape = "rounder";
            }
            let k = if asked == "oval" {
                0.88
            } else if asked == "pill" {
                0.94
            } else {
                1.0
            };
            let mut style = SCREEN_EYES[r.pick(o + 2, 0, &[8, 8, 5, 4, 4, 3], 0)];
            // Wide eyes side by side on a screen are a pair of sunglasses or a
            // visor: the screen draws dots instead.
            if style == "wide" {
                style = "dot";
            }
            let dims = match style {
                "pill" => [0.66, 1.1, 0.66, 0.66],
                "square" => [0.9, 0.9, 0.3, 0.3],
                "wide" => [1.14, 0.7, 0.7, 0.7],
                _ => [1.0, 1.0, 1.0, 1.0],
            };
            let r0 = min(sw * 0.4, sh * 0.52) * k * lerp(0.86, 1.0, size());
            let rx = r0 * dims[0];
            let ry = r0 * dims[1];
            let ex = clamp(
                sw * k * lerp(0.46, 0.6, spacing()),
                rx + 2.5,
                sw * k - rx - 3.0,
            );
            let ey = max(
                cy - sh * k + ry + 3.5,
                cy - sh * k * r.lin(o + 3, 6, 2, 0.16, 0.28),
            );
            let snug = shape != asked && k < 1.0;
            let gw = if snug {
                min(sw, max(sw * k, ex + rx + 3.0))
            } else {
                sw
            };
            let gh = if snug { sh * k } else { sh };
            let oval = shape == "oval";
            let corner = match shape {
                "panel" => min([4.5, 6.0, 7.5, 9.0][shape_bits as usize], gh * 0.5),
                "round" => min(9.0, gh * 0.5),
                "rounder" => min(15.0, gh * 0.7),
                _ => gh,
            };
            let pts = if oval {
                shift_pts(&quad(-gh, gh, gw, gw, gw, gh, gw, gh), 0.0, cy)
            } else {
                rrect(-gw, cy - gh, gw, cy + gh, corner)
            };
            f.screen = Some(Screen {
                w: gw,
                h: gh,
                cy,
                r: corner,
                oval,
                face_w: sw * if asked == "oval" { 0.78 } else { 1.0 },
                pts,
            });
            f.bx = FaceBox { cy, fw: sw, fh: sh };
            s.cy = cy;
            let mut eye_of = Eye {
                hole: if style == "ring" {
                    0.56
                } else if style == "pupil" {
                    0.36
                } else {
                    0.0
                },
                ..Eye::default()
            };
            // On a pale screen every eye has a pupil: a dark ring round a
            // dark pupil, or a pale pupil in a dark shape.
            if f.lcd && style == "dot" {
                let rim = clamp(r0 * 0.26, 2.3, 3.4);
                eye_of.rim = Some(rim);
                eye_of.dot = (r0 - rim) * 0.56;
            } else if f.lcd && eye_of.hole == 0.0 {
                eye_of.hole = 0.4;
                eye_of.hole_fine = true;
            }
            for side in [-1.0, 1.0] {
                f.eyes.push(Eye {
                    side,
                    x: side * ex,
                    y: ey,
                    rx,
                    ry,
                    qx: r0 * dims[2],
                    qy: r0 * dims[3],
                    ..eye_of.clone()
                });
            }
            f.eye_style = style;
            let mw = min(sw * k - 9.0, sw * r.lin(om, 0, 3, 0.26, 0.46));
            // The mouth takes what is left under the eyes.
            let top = ey + ry + 3.5;
            let bottom = s.cy + sh * k - 4.0;
            let dh = max(2.5, min3(mw * 0.4, 8.5, (bottom - top) / 3.3));
            f.mouth = Mouth {
                kind: MouthKind::Line,
                form: form(),
                rest: rest(),
                x: 0.0,
                y: lerp(top + dh, bottom - dh * 2.3, 0.55),
                w: mw,
                dh,
                ..Mouth::none()
            };
            let brow_room = ey - ry - 4.5 - (s.cy - sh * k) >= 4.0;
            f.brow_kind = if brow_room && r.flag(BROWS, 0) {
                BrowKind::Rest
            } else {
                BrowKind::Mood
            };
            f.brow_room = brow_room;
            f.bezel = if r.flag(o + 5, 5) { 4.8 } else { 3.4 };
            f.rest_lid = 0.0;
            return f;
        }
    }
    let tech = f.tech;
    let lamp = tech == Tech::Lamp;
    let stalk = tech == Tech::Stalk;
    // Eyes on stalks always have a mouth, and so does every other face: one
    // eye or three over no mouth was a porthole or a blank face whose moods
    // did not read, so the gene's fourth choice is a slot.
    let weights: &[u32] = if tech == Tech::Mono || tech == Tech::Tri {
        &[5, 5, 3, 3]
    } else {
        &[6, 6, 4, 0]
    };
    let mut kind = [
        MouthKind::Strip,
        MouthKind::Slot,
        MouthKind::Grid,
        MouthKind::Slot,
    ][r.pick(om + 2, 0, weights, 0)];
    let mh = match kind {
        MouthKind::Strip => 9.5,
        MouthKind::Grid => 8.5,
        _ => 7.0,
    };
    let mut shape = ["round", "square", "tall", "wide"][r.pick(o + 2, 0, &[10, 3, 2, 1], 0)];
    // Wide lenses side by side are a pair of goggles: two eyes look through
    // round ones.
    if shape == "wide" && matches!(tech, Tech::Lens | Tech::Lamp | Tech::Odd) {
        shape = "round";
    }
    let mut sd = match shape {
        "square" => [0.92, 0.92, 0.36, 0.36],
        "tall" => [0.86, 1.06, 0.86, 1.06],
        "wide" => [1.08, 0.86, 1.08, 0.86],
        _ => [1.0, 1.0, 1.0, 1.0],
    };
    // A cyclops's square housing would be a washing machine's window: its
    // eye is a circle as wide as the square would have been.
    if tech == Tech::Mono && shape == "square" {
        sd[2] = sd[0];
        sd[3] = sd[1];
    }
    // A lamp with no pupil is a headlight or a goggle lens, so every lamp has one.
    f.lamp_pupil = lamp;
    f.iris = r.lin(o + 5, 0, 2, 0.46, 0.6);
    if !lamp || f.lamp_pupil {
        f.pupil = f.iris * r.lin(o + 5, 2, 2, 0.38, 0.54);
    }
    f.glint = if r.flag(o + 5, 4) { 1.0 } else { -1.0 };
    let eye = move |side: f64, x: f64, y: f64, rr: f64, big: bool| {
        lens_eye(side, x, y, rr, sd, lamp, big)
    };
    let eyes_of: Option<EyesOf> = match tech {
        Tech::Lens | Tech::Lamp => Some(Box::new(move |rr, y| {
            vec![
                eye(-1.0, -(rr * sd[0] + apart(rr)), y, rr, false),
                eye(1.0, rr * sd[0] + apart(rr), y, rr, false),
            ]
        })),
        Tech::Mono => Some(Box::new(move |rr, y| vec![eye(0.0, 0.0, y, rr, false)])),
        Tech::Tri => {
            if r.flag(o + 4, 1) {
                Some(Box::new(move |rr, y| {
                    let q = rr * 0.7;
                    vec![
                        eye(-1.0, -(2.0 * q * sd[0] + 1.6 * apart(q)), y, q, false),
                        eye(0.0, 0.0, y, q, false),
                        eye(1.0, 2.0 * q * sd[0] + 1.6 * apart(q), y, q, false),
                    ]
                }))
            } else {
                Some(Box::new(move |rr, y| {
                    let q = rr * 0.5;
                    vec![
                        eye(
                            -1.0,
                            -((rr + q) * sd[0] + 1.6 * apart(q)),
                            y - rr * 0.44,
                            q,
                            false,
                        ),
                        eye(0.0, 0.0, y, rr, true),
                        eye(
                            1.0,
                            (rr + q) * sd[0] + 1.6 * apart(q),
                            y - rr * 0.44,
                            q,
                            false,
                        ),
                    ]
                }))
            }
        }
        Tech::Odd => {
            let big_side = if r.flag(o + 4, 2) { 1.0 } else { -1.0 };
            Some(Box::new(move |rr, y| {
                let q = rr * 0.6;
                let half = (rr + q) * sd[0] + apart(rr);
                let mut list = vec![
                    eye(big_side, big_side * (half - rr * sd[0]), y, rr, true),
                    eye(
                        -big_side,
                        -big_side * (half - q * sd[0]),
                        y + (rr - q) * 0.35,
                        q,
                        false,
                    ),
                ];
                list.sort_by(|a, b| a.side.total_cmp(&b.side));
                list
            }))
        }
        _ => None,
    };
    // A hull with a jaw carries its eyes above the jaw and its mouth in it.
    let floor = if tee { -hull.jaw_h - 3.0 } else { -4.0 };
    let my: f64;
    if let Some(eyes_of) = &eyes_of {
        let best = fit_eyes(hull, eyes_of.as_ref(), if tee { 0.0 } else { mh }, floor);
        // Three eyes share the room two would have, so the size gene does not
        // shrink them: smaller, the side eyes were specks at icon size.
        let shrink = if tech == Tech::Tri {
            1.0
        } else {
            lerp(0.74, 1.0, size())
        };
        let rr = best.r * shrink;
        f.eyes = eyes_of(rr, best.ey);
        if tech == Tech::Lens || lamp {
            // Two eyes are set apart by their own gene, as far as the hull lets them go.
            let e = &f.eyes[1];
            let most = reach_at(hull, e.y, e.ry * 0.7, 3.6) - e.rx;
            let ex = lerp(e.x, max(e.x, min(most, e.x + rr * 0.8)), spacing());
            f.eyes[0].x = -ex;
            f.eyes[1].x = ex;
        }
        my = if tee {
            -hull.jaw_h / 2.0 + 1.5
        } else {
            best.my.unwrap_or(0.0) - (best.r - rr) * 0.6
        };
    } else {
        let rr = r.lin(o + 3, 0, 3, 17.0, 23.5) * scale;
        f.stalk = Some(Stalk {
            r: rr,
            bx: max(rr + apart(rr), hull.w * lerp(0.4, 0.6, spacing())),
            len: r.lin(o + 3, 6, 2, 12.0, 26.0) * scale,
            shape: sd,
        });
        my = -h * 0.5;
    }
    let span = if f.eyes.is_empty() {
        hull.w * 0.6
    } else {
        f.eyes
            .iter()
            .map(|e| e.x.abs() + e.rx)
            .fold(f64::NEG_INFINITY, max)
    };
    // A strip or a grille takes at most half the hull's width where it sits;
    // a strip with no room for that is a slot instead.
    let mut most = if tee {
        hull.jaw_w - 9.0
    } else {
        mouth_room(hull, my, mh, span)
    };
    if kind == MouthKind::Strip && min(most, hull.hw_at(my) * 0.5) - 6.0 < 9.0 {
        kind = MouthKind::Slot;
    }
    if kind != MouthKind::Slot {
        most = min(most, hull.hw_at(my) * 0.5) - if kind == MouthKind::Strip { 6.0 } else { 0.0 };
    }
    let mw = max(
        7.0,
        min(
            most,
            span * r.lin(
                om,
                0,
                3,
                if stalk { 0.5 } else { 0.34 },
                if stalk { 0.82 } else { 0.64 },
            ),
        ),
    );
    let dh = match kind {
        MouthKind::Strip => min(mw * 0.36, 5.2),
        MouthKind::Grid => min(mw * 0.34, 7.0),
        _ => min(mw * 0.4, if stalk { 11.0 } else { 8.0 }),
    };
    f.mouth = Mouth {
        kind,
        form: form(),
        rest: rest(),
        x: 0.0,
        y: my,
        w: mw,
        dh,
        ..Mouth::none()
    };
    if kind == MouthKind::Strip {
        f.mouth.panel = rrect(-mw - 6.0, my - 8.2, mw + 6.0, my + 8.2, 6.0);
    }
    if kind == MouthKind::Grid {
        f.mouth.t = r.lin(om, 3, 2, 7.5, 10.5);
        f.mouth.bars = 3 + r.raw(om, 5, 1) * 2;
    }
    f.bx = FaceBox {
        cy: if f.eyes.is_empty() { my } else { f.eyes[0].y },
        fw: span,
        fh: if f.eyes.is_empty() { mh } else { f.eyes[0].ry },
    };
    let bk = if stalk {
        0
    } else {
        r.pick(BROWS, 1, &[3, 3, 2], 0)
    };
    f.brow_kind = [BrowKind::Mood, BrowKind::Bar, BrowKind::Heavy][bk];
    f.brow_room = !f.eyes.iter().any(|e| {
        hull.top_at(e.x) + 4.0 > e.y - e.ry - 9.5
            || hull.hw_at(e.y - e.ry - 8.0) < e.x.abs() + e.rx * 0.8
    });
    f.rest_lid = [0.0, 0.0, 0.1, 0.17][r.raw(BROWS, 3, 2) as usize];
    f
}

/// The colours of a face, and where its cheeks and its tear go.
pub(crate) fn finish_face(f: &mut Face, p: &Palette, hull: &Hull) {
    if let Some(s) = &f.screen {
        // Dark glass that glows, or a pale panel with dark marks on it.
        if f.lcd {
            f.back = hex(0xc9d3bd);
            f.back_line = hex(0x8b957d);
            f.on_base = hex(0x262c27);
            f.on_lite_base = hex(0x262c27);
        } else {
            f.back = p.glass;
            f.back_line = hex(0x0b0b10);
            f.on_base = p.glow;
            f.on_lite_base = p.glow_lite;
        }
        // The cheeks keep to the room the face is laid out in, and to the glass.
        let e = &f.eyes[1];
        let x = min3(e.x + e.rx * 0.7, s.face_w - 7.5, s.w - 6.2);
        let y = e.y + e.ry + 5.2;
        if (y + 3.4 <= s.cy + s.h - 2.5 && x - 5.5 >= f.mouth.w * 1.16 + 1.5)
            || y + 3.4 < f.mouth.y - f.mouth.dh - 3.0
        {
            f.cheeks = Some([[-x, y, 5.2], [x, y, 5.2]]);
        }
        let ty = e.y + e.ry + 6.5;
        if ty + 5.0 <= s.cy + s.h - 2.0 {
            f.tear = Tear::Screen {
                ex: e.x,
                erx: e.rx,
                ty,
            };
        }
    } else if !f.eyes.is_empty() {
        let lo = f
            .eyes
            .iter()
            .map(|q| q.y + q.ry)
            .fold(f64::NEG_INFINITY, max);
        let out = f
            .eyes
            .iter()
            .map(|q| q.x.abs() + q.rx * 0.2)
            .fold(f64::NEG_INFINITY, max);
        let y = lo + 6.5;
        let clear = f.mouth.kind == MouthKind::None || y + 5.0 < f.mouth.y - 10.0;
        let x = max(
            out,
            if clear {
                0.0
            } else {
                f.mouth.w
                    + if f.mouth.kind == MouthKind::Strip {
                        15.0
                    } else {
                        9.0
                    }
            },
        );
        if hull.hw_at(y) >= x + 10.0 && hull.hw_at(y + 4.0) >= x + 9.0 {
            f.cheeks = Some([[-x, y, 6.2], [x, y, 6.2]]);
        }
        f.tear = Tear::Hull;
    }
}

impl Face {
    /// What lights on the screen in a mood whose screen brightness is `screen`.
    pub(crate) fn on(&self, screen: f64) -> Rgb {
        blend(self.back, self.on_base, lerp(0.42, 1.0, screen))
    }

    pub(crate) fn on_lite(&self, screen: f64) -> Rgb {
        blend(self.back, self.on_lite_base, lerp(0.42, 1.0, screen))
    }

    /// Where a tear on `side` starts and how large it is, if there is room for one.
    pub(crate) fn tear_at(&self, side: f64, hull: &Hull) -> Option<[f64; 3]> {
        match self.tear {
            Tear::Nowhere => None,
            Tear::Screen { ex, erx, ty } => Some([side * (ex + erx * 0.15), ty, 4.4]),
            Tear::Hull => {
                let q = if side < 0.0 {
                    &self.eyes[0]
                } else {
                    &self.eyes[self.eyes.len() - 1]
                };
                let s = if q.side != 0.0 { q.side } else { side };
                let tx = q.x + s * q.rx * 0.55;
                let ty = q.y + q.ry + 5.5;
                (hull.hw_at(ty + 6.0) >= tx.abs() + 6.0).then_some([tx, ty, 5.4])
            }
        }
    }
}
