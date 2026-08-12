//! Grid 5 — the gacha test: 36 creatures decoded from RANDOM seeds through
//! the note-009 mapping bands. No hand-picking; the sheet is the evidence
//! for (or against) design law 3: no ugly genome.

mod body;
mod face;
mod genome;
mod mood;
mod palette;
mod parts;

use body::{sample_outline_wonk, width_at, BodyParams, Wonk};

const CELL: f64 = 240.0;
const PAD: f64 = 12.0;

fn draw_random(seed: u64, cx: f64, cy: f64) -> String {
    let c = genome::decode(seed);
    let colors = palette::resolve(c.recipe, c.hue);
    let scale = 0.8;
    let bp = BodyParams {
        height: c.bp.height * scale,
        w_top: c.bp.w_top * scale,
        w_bot: c.bp.w_bot * scale,
        ..c.bp
    };
    let mut s = String::new();
    s.push_str(&format!(
        "<ellipse cx='{cx:.1}' cy='{:.1}' rx='{:.1}' ry='{:.1}' fill='rgba(40,30,50,0.14)'/>",
        cy + bp.height / 2.0 + 5.0,
        width_at(&bp, bp.height * 0.35) * 1.1,
        6.0,
    ));
    s.push_str(&parts::draw_bottom(c.bottom, &bp, cx, cy, &colors.part, &colors.outline));
    s.push_str(&parts::draw(c.top, &bp, cx, cy, &colors.part));
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
    let f = face::frame(&bp, cx, cy);
    let eyes_svg = face::eyes_inked(c.eyes, &bp, cx, cy, 0.0, &colors.outline, &colors.ink);
    if (c.eye_scale - 1.0).abs() > 0.01 {
        s.push_str(&format!(
            "<g transform='translate({cx:.1} {:.1}) scale({:.3}) translate({:.1} {:.1})'>{eyes_svg}</g>",
            f.y, c.eye_scale, -cx, -f.y,
        ));
    } else {
        s.push_str(&eyes_svg);
    }
    s.push_str(&face::mouth_inked(c.mouth, &bp, cx, cy, &colors.outline, &colors.ink));
    s
}

fn main() {
    let cols = 6usize;
    let rows = 6usize;
    let width = PAD + cols as f64 * (CELL + PAD);
    let height = PAD + rows as f64 * (CELL + PAD) + 20.0;
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width:.0}' height='{height:.0}' viewBox='0 0 {width:.0} {height:.0}'><rect width='100%' height='100%' fill='#1c1a20'/>"
    );
    // Seeds are sequential on purpose: no cherry-picking, run N is run N.
    for r in 0..rows {
        for c in 0..cols {
            let seed = 1000 + (r * cols + c) as u64;
            let cx = PAD + c as f64 * (CELL + PAD) + CELL / 2.0;
            let cy = PAD + r as f64 * (CELL + PAD) + CELL / 2.0 + 14.0;
            svg.push_str(&format!(
                "<rect x='{:.1}' y='{:.1}' width='{CELL}' height='{CELL}' rx='10' fill='#efe9e4'/>",
                cx - CELL / 2.0,
                cy - CELL / 2.0 - 14.0,
            ));
            svg.push_str(&draw_random(seed, cx, cy));
            svg.push_str(&format!(
                "<text x='{:.1}' y='{:.1}' font-family='monospace' font-size='10' fill='#8f8a96'>#{seed}</text>",
                cx - CELL / 2.0 + 8.0,
                cy - CELL / 2.0 + 2.0,
            ));
        }
    }
    svg.push_str("</svg>");
    std::fs::create_dir_all("/tmp/ava-playground").expect("mkdir");
    std::fs::write("/tmp/ava-playground/grid5.svg", &svg).expect("write");
    println!("grid5: {} random creatures -> /tmp/ava-playground/grid5.svg", rows * cols);
}
