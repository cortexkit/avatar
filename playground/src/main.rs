//! Grid 1: body-blob proportion space x top-slot part families.
//!
//! One self-contained SVG sheet per run: rows are body proportion presets,
//! columns are top parts. Faces are minimal (dot-shine eyes + tiny mouth) so
//! cuteness is assessable; everything else waits for later grids.

mod body;
mod parts;

use body::{outline_path, sample_outline, width_at, BodyParams};

const CELL: f64 = 240.0;
const PAD: f64 = 12.0;

/// Row presets spanning the proportion space deliberately wide — from squat
/// bean to tall gremlin — so the grid shows where "alive" lives.
fn body_presets() -> Vec<(&'static str, BodyParams)> {
    vec![
        (
            "squat bean",
            BodyParams { height: 150.0, w_top: 78.0, w_bot: 88.0, n_top: 2.4, n_bot: 3.4, waist: 0.62 },
        ),
        (
            "round pearl",
            BodyParams { height: 165.0, w_top: 80.0, w_bot: 76.0, n_top: 2.0, n_bot: 2.2, waist: 0.5 },
        ),
        (
            "egg upright",
            BodyParams { height: 185.0, w_top: 62.0, w_bot: 84.0, n_top: 2.0, n_bot: 2.6, waist: 0.6 },
        ),
        (
            "pear heavy",
            BodyParams { height: 175.0, w_top: 52.0, w_bot: 95.0, n_top: 2.2, n_bot: 3.8, waist: 0.68 },
        ),
        (
            "tall gremlin",
            BodyParams { height: 200.0, w_top: 66.0, w_bot: 58.0, n_top: 2.8, n_bot: 2.4, waist: 0.42 },
        ),
        (
            "brick chonk",
            BodyParams { height: 160.0, w_top: 86.0, w_bot: 90.0, n_top: 4.4, n_bot: 4.8, waist: 0.52 },
        ),
    ]
}

/// Pastel body fills per row (hue steps), dark accent shared.
fn row_colors(row: usize) -> (String, String) {
    let hues = [346.0, 42.0, 152.0, 196.0, 262.0, 88.0];
    let h = hues[row % hues.len()];
    (
        format!("hsl({h:.0} 68% 82%)"),
        format!("hsl({h:.0} 45% 38%)"),
    )
}

fn face(bp: &BodyParams, cx: f64, cy: f64, accent: &str) -> String {
    // Eyes: large, wide-set, LOW on the mass (baby schema); dot + shine.
    let ey = -bp.height * 0.08; // slightly above vertical center of the blob
    let w = width_at(bp, ey);
    let ex = w * 0.42;
    let er = (bp.height * 0.085).min(w * 0.3);
    let mut s = String::new();
    for side in [-1.0, 1.0] {
        let x = cx + side * ex;
        let y = cy + ey;
        s.push_str(&format!(
            "<circle cx='{x:.1}' cy='{y:.1}' r='{er:.1}' fill='#241f26'/><circle cx='{:.1}' cy='{:.1}' r='{:.1}' fill='white' opacity='0.9'/>",
            x + er * 0.32,
            y - er * 0.35,
            er * 0.3,
        ));
    }
    // Tiny mouth: short arc below the eye line.
    let my = ey + bp.height * 0.14;
    s.push_str(&format!(
        "<path d='M{:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1}' stroke='{accent}' stroke-width='2.4' fill='none' stroke-linecap='round'/>",
        cx - er * 0.5,
        cy + my,
        cx,
        cy + my + er * 0.45,
        cx + er * 0.5,
        cy + my,
    ));
    s
}

fn main() {
    let presets = body_presets();
    let cols = parts::ALL.len();
    let rows = presets.len();
    let width = PAD + cols as f64 * (CELL + PAD);
    let height = PAD + rows as f64 * (CELL + PAD) + 20.0;

    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width:.0}' height='{height:.0}' viewBox='0 0 {width:.0} {height:.0}'><rect width='100%' height='100%' fill='#1c1a20'/>"
    );

    for (r, (label, bp)) in presets.iter().enumerate() {
        let (fill, accent) = row_colors(r);
        for (c, part) in parts::ALL.iter().enumerate() {
            let cx = PAD + c as f64 * (CELL + PAD) + CELL / 2.0;
            let cy = PAD + r as f64 * (CELL + PAD) + CELL / 2.0 + 14.0;
            // Cell backdrop for contrast judgment.
            svg.push_str(&format!(
                "<rect x='{:.1}' y='{:.1}' width='{CELL}' height='{CELL}' rx='10' fill='#efe9e4'/>",
                cx - CELL / 2.0,
                cy - CELL / 2.0 - 14.0,
            ));
            // Ground shadow: cheap grounding, big cuteness lever.
            svg.push_str(&format!(
                "<ellipse cx='{cx:.1}' cy='{:.1}' rx='{:.1}' ry='{:.1}' fill='rgba(40,30,50,0.14)'/>",
                cy + bp.height / 2.0 + 6.0,
                width_at(bp, bp.height * 0.35) * 1.1,
                7.0,
            ));
            // Part BEHIND the body where it should peek from the mass.
            svg.push_str(&parts::draw(*part, bp, cx, cy, &accent));
            let outline = sample_outline(bp, 48);
            svg.push_str(&format!(
                "<path d='{}' fill='{fill}' stroke='{accent}' stroke-width='3'/>",
                outline_path(&outline, cx, cy),
            ));
            svg.push_str(&face(bp, cx, cy, &accent));
        }
        svg.push_str(&format!(
            "<text x='{PAD}' y='{:.1}' font-family='monospace' font-size='12' fill='#8f8a96'>{label}</text>",
            PAD + r as f64 * (CELL + PAD) + 12.0,
        ));
    }
    svg.push_str("</svg>");

    std::fs::create_dir_all("/tmp/ava-playground").expect("mkdir");
    std::fs::write("/tmp/ava-playground/grid1.svg", &svg).expect("write");
    println!("grid1: {rows} bodies x {cols} parts -> /tmp/ava-playground/grid1.svg");
}
