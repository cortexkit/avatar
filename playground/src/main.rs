//! Grid 3: palette recipes x a fixed cast, plus a 28px legibility strip.
//!
//! Rows are procedural palette recipes (the future palette-band field);
//! columns are six distinct characters composed from the kept grid-1/2
//! vocabulary, each with its own base hue. The bottom strip renders row 0
//! at 28px: identity must survive icon size (design law 1).

mod body;
mod face;
mod palette;
mod parts;

use body::{sample_outline_wonk, width_at, BodyParams, Wonk};
use face::{EyeKind, MouthKind};
use parts::TopPart;

const CELL: f64 = 240.0;
const PAD: f64 = 12.0;

struct Cast {
    name: &'static str,
    bp: BodyParams,
    part: TopPart,
    eyes: EyeKind,
    mouth: MouthKind,
    hue: f64,
}

fn cast() -> Vec<Cast> {
    vec![
        Cast {
            name: "mochi",
            bp: BodyParams { height: 150.0, w_top: 78.0, w_bot: 88.0, n_top: 2.4, n_bot: 3.4, waist: 0.62 },
            part: TopPart::Ears,
            eyes: EyeKind::DotShine,
            mouth: MouthKind::CatW,
            hue: 346.0,
        },
        Cast {
            name: "pearl",
            bp: BodyParams { height: 165.0, w_top: 80.0, w_bot: 76.0, n_top: 2.0, n_bot: 2.2, waist: 0.5 },
            part: TopPart::Nub,
            eyes: EyeKind::Lidded,
            mouth: MouthKind::Arc,
            hue: 42.0,
        },
        Cast {
            name: "sprout",
            bp: BodyParams { height: 185.0, w_top: 62.0, w_bot: 84.0, n_top: 2.0, n_bot: 2.6, waist: 0.6 },
            part: TopPart::Antennae,
            eyes: EyeKind::HappyArc,
            mouth: MouthKind::OpenHappy,
            hue: 152.0,
        },
        Cast {
            name: "boulder",
            bp: BodyParams { height: 175.0, w_top: 52.0, w_bot: 95.0, n_top: 2.2, n_bot: 3.8, waist: 0.68 },
            part: TopPart::Horns,
            eyes: EyeKind::HalfLid,
            mouth: MouthKind::Flat,
            hue: 196.0,
        },
        Cast {
            name: "wisp",
            bp: BodyParams { height: 200.0, w_top: 66.0, w_bot: 58.0, n_top: 2.8, n_bot: 2.4, waist: 0.42 },
            part: TopPart::Tuft,
            eyes: EyeKind::Uneven,
            mouth: MouthKind::Snaggle,
            hue: 262.0,
        },
        Cast {
            name: "brick",
            bp: BodyParams { height: 160.0, w_top: 86.0, w_bot: 90.0, n_top: 4.4, n_bot: 4.8, waist: 0.52 },
            part: TopPart::None,
            eyes: EyeKind::DotShine,
            mouth: MouthKind::Fangs,
            hue: 88.0,
        },
    ]
}

fn draw_creature(c: &Cast, colors: &palette::Colors, cx: f64, cy: f64, scale: f64) -> String {
    let bp = BodyParams {
        height: c.bp.height * scale,
        w_top: c.bp.w_top * scale,
        w_bot: c.bp.w_bot * scale,
        ..c.bp
    };
    let wonk = Wonk { lean: 0.0, side_bias: 0.0, eye_tilt: 0.0 };
    let mut s = String::new();
    s.push_str(&format!(
        "<ellipse cx='{cx:.1}' cy='{:.1}' rx='{:.1}' ry='{:.1}' fill='rgba(40,30,50,0.14)'/>",
        cy + bp.height / 2.0 + 6.0 * scale,
        width_at(&bp, bp.height * 0.35) * 1.1,
        7.0 * scale,
    ));
    s.push_str(&parts::draw(c.part, &bp, cx, cy, &colors.part));
    let outline = sample_outline_wonk(&bp, 48, &wonk);
    let mut d = String::new();
    for (i, p) in outline.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        d.push_str(&format!("{cmd}{:.2} {:.2} ", cx + p.x, cy + p.y));
    }
    d.push('Z');
    s.push_str(&format!(
        "<path d='{d}' fill='{}' stroke='{}' stroke-width='{:.1}'/>",
        colors.body,
        colors.outline,
        3.0 * scale.max(0.5),
    ));
    s.push_str(&face::belly_colored(&bp, cx, cy, &colors.belly));
    s.push_str(&face::blush_colored(&bp, cx, cy, &colors.blush));
    s.push_str(&face::eyes_inked(c.eyes, &bp, cx, cy, 0.0, &colors.outline, &colors.ink));
    s.push_str(&face::mouth_inked(c.mouth, &bp, cx, cy, &colors.outline, &colors.ink));
    s
}

fn main() {
    let cast = cast();
    let cols = cast.len();
    let rows = palette::RECIPES;
    let strip_h = 60.0;
    let width = PAD + cols as f64 * (CELL + PAD);
    let height = PAD + rows as f64 * (CELL + PAD) + strip_h + 30.0;

    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width:.0}' height='{height:.0}' viewBox='0 0 {width:.0} {height:.0}'><rect width='100%' height='100%' fill='#1c1a20'/>"
    );

    for r in 0..rows {
        for (c, member) in cast.iter().enumerate() {
            let colors = palette::resolve(r, member.hue);
            let cx = PAD + c as f64 * (CELL + PAD) + CELL / 2.0;
            let cy = PAD + r as f64 * (CELL + PAD) + CELL / 2.0 + 14.0;
            // Dark rows judge against dark AND light backdrops alternately:
            // fleet surfaces have both.
            let bg = if r == 3 && c % 2 == 1 { "#efe9e4" } else if r == 3 { "#2a2733" } else { "#efe9e4" };
            svg.push_str(&format!(
                "<rect x='{:.1}' y='{:.1}' width='{CELL}' height='{CELL}' rx='10' fill='{bg}'/>",
                cx - CELL / 2.0,
                cy - CELL / 2.0 - 14.0,
            ));
            svg.push_str(&draw_creature(member, &colors, cx, cy, 0.82));
            if r == 0 {
                svg.push_str(&format!(
                    "<text x='{cx:.1}' y='{:.1}' text-anchor='middle' font-family='monospace' font-size='12' fill='#8f8a96'>{}</text>",
                    PAD + 9.0,
                    member.name,
                ));
            }
        }
        let label = palette::resolve(r, 0.0).name;
        svg.push_str(&format!(
            "<text x='{PAD}' y='{:.1}' font-family='monospace' font-size='12' fill='#8f8a96'>{label}</text>",
            PAD + r as f64 * (CELL + PAD) + 24.0,
        ));
    }

    // 28px legibility strip: pastel-analogous cast at icon size.
    let sy = PAD + rows as f64 * (CELL + PAD) + 20.0;
    svg.push_str(&format!(
        "<text x='{PAD}' y='{:.1}' font-family='monospace' font-size='12' fill='#8f8a96'>28px strip:</text>",
        sy + 8.0,
    ));
    for (c, member) in cast.iter().enumerate() {
        let colors = palette::resolve(0, member.hue);
        let cx = 130.0 + c as f64 * 44.0;
        let cy = sy + 14.0;
        svg.push_str(&format!(
            "<rect x='{:.1}' y='{:.1}' width='36' height='36' rx='6' fill='#efe9e4'/>",
            cx - 18.0,
            cy - 4.0,
        ));
        svg.push_str(&draw_creature(member, &colors, cx, cy + 14.0, 0.115));
    }

    svg.push_str("</svg>");
    std::fs::create_dir_all("/tmp/ava-playground").expect("mkdir");
    std::fs::write("/tmp/ava-playground/grid3.svg", &svg).expect("write");

    // Standalone icon strip at NATURAL 36px cell size: rasterizing this file
    // 1:1 shows real icon-size pixels; the big sheet's strip gets distorted
    // by thumbnailer scaling.
    let mut strip = String::from(
        "<svg xmlns='http://www.w3.org/2000/svg' width='264' height='44' viewBox='0 0 264 44'><rect width='100%' height='100%' fill='#1c1a20'/>",
    );
    for (c, member) in cast.iter().enumerate() {
        let colors = palette::resolve(0, member.hue);
        let cx = 4.0 + c as f64 * 44.0 + 18.0;
        strip.push_str(&format!(
            "<rect x='{:.1}' y='4' width='36' height='36' rx='6' fill='#efe9e4'/>",
            cx - 18.0,
        ));
        strip.push_str(&draw_creature(member, &colors, cx, 22.0, 0.115));
    }
    strip.push_str("</svg>");
    std::fs::write("/tmp/ava-playground/grid3-strip.svg", &strip).expect("write strip");
    println!("grid3: {rows} recipes x {cols} cast -> grid3.svg + grid3-strip.svg");
}
