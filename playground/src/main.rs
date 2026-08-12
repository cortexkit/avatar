//! Grid 4: the freeze gate — all 7 moods x four cast members (with bottom
//! parts riding along). Mood must read on every kept eye archetype,
//! including deadpan's half-lids, or the archetype changes.

mod body;
mod face;
mod mood;
mod palette;
mod parts;

use body::{sample_outline_wonk, width_at, BodyParams, Wonk};
use face::{EyeKind, MouthKind};
use mood::{Mood, MoodSpec};
use parts::{BottomPart, TopPart};

const CELL: f64 = 240.0;
const PAD: f64 = 12.0;

struct Cast {
    name: &'static str,
    bp: BodyParams,
    part: TopPart,
    bottom: BottomPart,
    eyes: EyeKind,
    mouth: MouthKind,
    hue: f64,
    recipe: usize,
}

fn cast() -> Vec<Cast> {
    vec![
        Cast {
            name: "mochi",
            bp: BodyParams { height: 150.0, w_top: 78.0, w_bot: 88.0, n_top: 2.4, n_bot: 3.4, waist: 0.62 },
            part: TopPart::Ears,
            bottom: BottomPart::FeetNubs,
            eyes: EyeKind::DotShine,
            mouth: MouthKind::CatW,
            hue: 346.0,
            recipe: 0,
        },
        Cast {
            name: "sprout",
            bp: BodyParams { height: 185.0, w_top: 62.0, w_bot: 84.0, n_top: 2.0, n_bot: 2.6, waist: 0.6 },
            part: TopPart::Antennae,
            bottom: BottomPart::DuckFeet,
            eyes: EyeKind::Lidded,
            mouth: MouthKind::Arc,
            hue: 152.0,
            recipe: 2,
        },
        Cast {
            name: "boulder",
            bp: BodyParams { height: 175.0, w_top: 52.0, w_bot: 95.0, n_top: 2.2, n_bot: 3.8, waist: 0.68 },
            part: TopPart::Horns,
            bottom: BottomPart::None,
            eyes: EyeKind::HalfLid,
            mouth: MouthKind::Flat,
            hue: 196.0,
            recipe: 3,
        },
        Cast {
            name: "wisp",
            bp: BodyParams { height: 200.0, w_top: 66.0, w_bot: 58.0, n_top: 2.8, n_bot: 2.4, waist: 0.42 },
            part: TopPart::Tuft,
            bottom: BottomPart::Tail,
            eyes: EyeKind::Uneven,
            mouth: MouthKind::Snaggle,
            hue: 88.0,
            recipe: 4,
        },
    ]
}

fn mouth_for(base: MouthKind, m: &MoodSpec) -> MouthKind {
    match m.mouth {
        mood::MouthMood::Keep => base,
        mood::MouthMood::Grin => MouthKind::OpenHappy,
        mood::MouthMood::O => MouthKind::OMouth,
        mood::MouthMood::Pressed => MouthKind::Flat,
        mood::MouthMood::Down => MouthKind::DownArc,
    }
}

fn draw_creature_mood(c: &Cast, m: Mood, cx: f64, cy: f64) -> String {
    let colors = palette::resolve(c.recipe, c.hue);
    let spec = mood::spec(m);
    let bp = c.bp;
    let scale = 0.82;
    let bp = BodyParams {
        height: bp.height * scale,
        w_top: bp.w_top * scale,
        w_bot: bp.w_bot * scale,
        ..bp
    };
    let mut s = String::new();
    s.push_str(&format!(
        "<ellipse cx='{cx:.1}' cy='{:.1}' rx='{:.1}' ry='{:.1}' fill='rgba(40,30,50,0.14)'/>",
        cy + bp.height / 2.0 + 5.0,
        width_at(&bp, bp.height * 0.35) * 1.1,
        6.0,
    ));
    s.push_str(&parts::draw_bottom(c.bottom, &bp, cx, cy, &colors.part, &colors.outline));
    s.push_str(&parts::draw(c.part, &bp, cx, cy, &colors.part));
    let outline = sample_outline_wonk(&bp, 48, &Wonk::default());
    let mut d = String::new();
    for (i, p) in outline.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        d.push_str(&format!("{cmd}{:.2} {:.2} ", cx + p.x, cy + p.y));
    }
    d.push('Z');
    s.push_str(&format!(
        "<path d='{d}' fill='{}' stroke='{}' stroke-width='2.5'/>",
        colors.body, colors.outline,
    ));
    s.push_str(&face::belly_colored(&bp, cx, cy, &colors.belly));
    s.push_str(&face::blush_colored(&bp, cx, cy, &colors.blush));

    // Eyes with mood scale about the eye-line center.
    let f = face::frame(&bp, cx, cy);
    let eyes_svg = face::eyes_inked(c.eyes, &bp, cx, cy, 0.0, &colors.outline, &colors.ink);
    if (spec.eye_scale - 1.0).abs() > 0.01 {
        s.push_str(&format!(
            "<g transform='translate({cx:.1} {:.1}) scale({:.3}) translate({:.1} {:.1})'>{eyes_svg}</g>",
            f.y, spec.eye_scale, -cx, -f.y,
        ));
    } else {
        s.push_str(&eyes_svg);
    }
    // Mood overlays: lids (angle mirrored per side), squint, brows.
    for (x, side) in [(f.lx, -1.0), (f.rx, 1.0)] {
        s.push_str(&mood::lid_overlay(
            x, f.y, f.r * spec.eye_scale, spec.lid, spec.lid_angle * side, &colors.body, &colors.outline,
        ));
        s.push_str(&mood::squint_overlay(
            x, f.y, f.r * spec.eye_scale, spec.squint, &colors.body, &colors.outline,
        ));
        s.push_str(&mood::brow(spec.brows, x, f.y, f.r, side, &colors.ink));
    }
    s.push_str(&face::mouth_inked(mouth_for(c.mouth, &spec), &bp, cx, cy, &colors.outline, &colors.ink));
    s.push_str(&mood::adjunct(spec.adjunct, cx, cy - bp.height / 2.0, 1.0, &colors.ink));
    s
}

fn main() {
    let cast = cast();
    let cols = cast.len();
    let rows = mood::ALL.len();
    let width = PAD + cols as f64 * (CELL + PAD);
    let height = PAD + rows as f64 * (CELL + PAD) + 20.0;

    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width:.0}' height='{height:.0}' viewBox='0 0 {width:.0} {height:.0}'><rect width='100%' height='100%' fill='#1c1a20'/>"
    );

    for (r, m) in mood::ALL.iter().enumerate() {
        for (c, member) in cast.iter().enumerate() {
            let cx = PAD + c as f64 * (CELL + PAD) + CELL / 2.0;
            let cy = PAD + r as f64 * (CELL + PAD) + CELL / 2.0 + 14.0;
            svg.push_str(&format!(
                "<rect x='{:.1}' y='{:.1}' width='{CELL}' height='{CELL}' rx='10' fill='#efe9e4'/>",
                cx - CELL / 2.0,
                cy - CELL / 2.0 - 14.0,
            ));
            svg.push_str(&draw_creature_mood(member, *m, cx, cy));
            if r == 0 {
                svg.push_str(&format!(
                    "<text x='{cx:.1}' y='{:.1}' text-anchor='middle' font-family='monospace' font-size='12' fill='#8f8a96'>{}</text>",
                    PAD + 9.0,
                    member.name,
                ));
            }
        }
        svg.push_str(&format!(
            "<text x='{PAD}' y='{:.1}' font-family='monospace' font-size='12' fill='#8f8a96'>{}</text>",
            PAD + r as f64 * (CELL + PAD) + 24.0,
            mood::name(*m),
        ));
    }
    svg.push_str("</svg>");

    std::fs::create_dir_all("/tmp/ava-playground").expect("mkdir");
    std::fs::write("/tmp/ava-playground/grid4.svg", &svg).expect("write");
    println!("grid4: {rows} moods x {cols} cast -> /tmp/ava-playground/grid4.svg");
}
