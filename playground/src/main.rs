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

fn draw_random_hued(seed: u64, cx: f64, cy: f64, hue_map: &dyn Fn(f64) -> f64) -> String {
    let c = genome::decode(seed);
    let colors = palette::resolve(c.recipe, hue_map(c.hue));
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

/// Renders one 6x6 sheet. `hue_override(i, natural_hue)` lets panels remap
/// hue while keeping every other trait byte-identical.
fn sheet(path: &str, title: &str, hue_override: &dyn Fn(usize, f64) -> f64) {
    let cols = 6usize;
    let rows = 6usize;
    let width = PAD + cols as f64 * (CELL + PAD);
    let height = PAD + rows as f64 * (CELL + PAD) + 34.0;
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width:.0}' height='{height:.0}' viewBox='0 0 {width:.0} {height:.0}'><rect width='100%' height='100%' fill='#1c1a20'/><text x='{PAD}' y='24' font-family='monospace' font-size='15' fill='#c8c2ce'>{title}</text>"
    );
    // Seeds are sequential on purpose: no cherry-picking, run N is run N.
    for r in 0..rows {
        for c in 0..cols {
            let i = r * cols + c;
            let seed = 1000 + i as u64;
            let cx = PAD + c as f64 * (CELL + PAD) + CELL / 2.0;
            let cy = PAD + r as f64 * (CELL + PAD) + CELL / 2.0 + 28.0;
            svg.push_str(&format!(
                "<rect x='{:.1}' y='{:.1}' width='{CELL}' height='{CELL}' rx='10' fill='#efe9e4'/>",
                cx - CELL / 2.0,
                cy - CELL / 2.0 - 14.0,
            ));
            svg.push_str(&draw_random_hued(seed, cx, cy, &|h| hue_override(i, h)));
            svg.push_str(&format!(
                "<text x='{:.1}' y='{:.1}' font-family='monospace' font-size='10' fill='#8f8a96'>#{seed}</text>",
                cx - CELL / 2.0 + 8.0,
                cy - CELL / 2.0 + 2.0,
            ));
        }
    }
    svg.push_str("</svg>");
    std::fs::write(path, &svg).expect("write");
}

fn main() {
    std::fs::create_dir_all("/tmp/ava-playground").expect("mkdir");
    // Panel A: hue exactly as a hash-derived genome gives it (stateless).
    sheet(
        "/tmp/ava-playground/grid5.svg",
        "A: uniform hash hue (stateless identity, what v2.0 ships)",
        &|_, natural| natural,
    );
    // Panel B: golden-angle hue by GLOBAL INDEX. Deliberately not shippable
    // as stateless identity: spacing requires a coordinated per-fleet
    // ordering (an era-store hue allocator), so this panel is the evidence
    // for/against building that later — not a v2.0 candidate mapping.
    sheet(
        "/tmp/ava-playground/grid6.svg",
        "B: golden-angle spaced hue (needs a fleet allocator — era-store option)",
        &|i, natural| (i as f64 * 137.508 + natural * 0.08) % 360.0,
    );
    println!("grid5 (uniform) + grid6 (golden-angle) sheets rendered");
}
