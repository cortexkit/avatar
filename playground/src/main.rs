//! Grid 2: the life layer. Three bodies x wonk off/on (rows) against six
//! face packs (columns), with blush + belly patch everywhere.
//!
//! Question: which face vocabulary carries personality, and does deliberate
//! imperfection (lean, side bias, eye tilt) flip cells from sterile to alive.

mod body;
mod face;
mod parts;

use body::{sample_outline_wonk, width_at, BodyParams, Wonk};
use parts::TopPart;

const CELL: f64 = 240.0;
const PAD: f64 = 12.0;

fn grid2_rows() -> Vec<(&'static str, BodyParams, TopPart, Wonk)> {
    let bean = BodyParams { height: 150.0, w_top: 78.0, w_bot: 88.0, n_top: 2.4, n_bot: 3.4, waist: 0.62 };
    let egg = BodyParams { height: 185.0, w_top: 62.0, w_bot: 84.0, n_top: 2.0, n_bot: 2.6, waist: 0.6 };
    let gremlin = BodyParams { height: 200.0, w_top: 66.0, w_bot: 58.0, n_top: 2.8, n_bot: 2.4, waist: 0.42 };
    let wonk = Wonk { lean: 0.055, side_bias: 0.05, eye_tilt: 2.6 };
    vec![
        ("bean", bean, TopPart::Ears, Wonk::default()),
        ("bean wonk", bean, TopPart::Ears, wonk),
        ("egg", egg, TopPart::Antennae, Wonk::default()),
        ("egg wonk", egg, TopPart::Antennae, wonk),
        ("gremlin", gremlin, TopPart::Tuft, Wonk::default()),
        ("gremlin wonk", gremlin, TopPart::Tuft, wonk),
    ]
}

fn row_colors(row: usize) -> (String, String) {
    let hues = [346.0, 346.0, 196.0, 196.0, 88.0, 88.0];
    let h = hues[row % hues.len()];
    (
        format!("hsl({h:.0} 68% 82%)"),
        format!("hsl({h:.0} 45% 38%)"),
    )
}

fn outline_path(points: &[body::Pt], cx: f64, cy: f64) -> String {
    let mut d = String::new();
    for (i, p) in points.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        d.push_str(&format!("{cmd}{:.2} {:.2} ", cx + p.x, cy + p.y));
    }
    d.push('Z');
    d
}

fn main() {
    let rows = grid2_rows();
    let cols = face::PACKS.len();
    let width = PAD + cols as f64 * (CELL + PAD);
    let height = PAD + rows.len() as f64 * (CELL + PAD) + 20.0;

    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width:.0}' height='{height:.0}' viewBox='0 0 {width:.0} {height:.0}'><rect width='100%' height='100%' fill='#1c1a20'/>"
    );

    for (r, (label, bp, part, wonk)) in rows.iter().enumerate() {
        let (fill, accent) = row_colors(r);
        for (c, pack) in face::PACKS.iter().enumerate() {
            let cx = PAD + c as f64 * (CELL + PAD) + CELL / 2.0;
            let cy = PAD + r as f64 * (CELL + PAD) + CELL / 2.0 + 14.0;
            svg.push_str(&format!(
                "<rect x='{:.1}' y='{:.1}' width='{CELL}' height='{CELL}' rx='10' fill='#efe9e4'/>",
                cx - CELL / 2.0,
                cy - CELL / 2.0 - 14.0,
            ));
            svg.push_str(&format!(
                "<ellipse cx='{cx:.1}' cy='{:.1}' rx='{:.1}' ry='{:.1}' fill='rgba(40,30,50,0.14)'/>",
                cy + bp.height / 2.0 + 6.0,
                width_at(bp, bp.height * 0.35) * 1.1,
                7.0,
            ));
            svg.push_str(&parts::draw(*part, bp, cx, cy, &accent));
            let outline = sample_outline_wonk(bp, 48, wonk);
            svg.push_str(&format!(
                "<path d='{}' fill='{fill}' stroke='{accent}' stroke-width='3'/>",
                outline_path(&outline, cx, cy),
            ));
            svg.push_str(&face::belly(bp, cx, cy));
            svg.push_str(&face::blush(bp, cx, cy));
            svg.push_str(&face::eyes(pack.eyes, bp, cx, cy, wonk.eye_tilt, &accent));
            svg.push_str(&face::mouth(pack.mouth, bp, cx, cy, &accent));
            if r == 0 {
                svg.push_str(&format!(
                    "<text x='{cx:.1}' y='{:.1}' text-anchor='middle' font-family='monospace' font-size='12' fill='#8f8a96'>{}</text>",
                    PAD + 9.0,
                    pack.name,
                ));
            }
        }
        svg.push_str(&format!(
            "<text x='{PAD}' y='{:.1}' font-family='monospace' font-size='12' fill='#8f8a96'>{label}</text>",
            PAD + r as f64 * (CELL + PAD) + 24.0,
        ));
    }
    svg.push_str("</svg>");

    std::fs::create_dir_all("/tmp/ava-playground").expect("mkdir");
    std::fs::write("/tmp/ava-playground/grid2.svg", &svg).expect("write");
    println!("grid2: {} rows x {cols} packs -> /tmp/ava-playground/grid2.svg", rows.len());
}
