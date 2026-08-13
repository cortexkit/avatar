//! Grid 7 — the synthesis probe: TOG-class wild spline bodies under the
//! cute-ification pass, face grammar mounted on whatever shape emerges.
//! Rows sweep the domestication dial; columns are sequential random seeds.
//! Question: does silhouette entropy return WITHOUT losing the cute layer?

mod body;
mod face;
mod genome;
mod mood;
mod palette;
mod parts;
mod wild;

const CELL: f64 = 240.0;
const PAD: f64 = 12.0;

/// Draw one wild-body creature: outline, belly, blush, eyes, mouth mounted
/// via scanline extents of the actual shape (no blob assumptions).
fn draw_wild(seed: u64, dom: f64, cx: f64, cy: f64) -> String {
    let mut b = genome::Bits::new(seed);
    let wb = wild::decode(&mut b);
    // Reuse the same downstream bands as grid 5 for palette/face so the only
    // changed variable vs the blob sheets is the BODY.
    let rsel = b.byte() % 8;
    let recipe = match rsel {
        0..=2 => 0,
        3..=4 => 2,
        5..=6 => 4,
        _ => 3,
    };
    let hue = (b.byte() as f64 / 255.0 + b.byte() as f64 / 65535.0) * 360.0;
    let eyes = match b.byte() % 5 {
        0 => face::EyeKind::DotShine,
        1 => face::EyeKind::Lidded,
        2 => face::EyeKind::HappyArc,
        3 => face::EyeKind::Uneven,
        _ => face::EyeKind::HalfLid,
    };
    let mouth = match b.byte() % 6 {
        0 => face::MouthKind::Arc,
        1 => face::MouthKind::CatW,
        2 => face::MouthKind::OpenHappy,
        3 => face::MouthKind::Snaggle,
        4 => face::MouthKind::Flat,
        _ => face::MouthKind::Fangs,
    };
    let colors = palette::resolve(recipe, hue);

    let outline = wild::outline(&wb, dom, cx, cy, 172.0);
    let (min_x, min_y, max_x, max_y) = wild::bbox(&outline);
    let h = max_y - min_y;
    let mut s = String::new();
    s.push_str(&format!(
        "<ellipse cx='{:.1}' cy='{:.1}' rx='{:.1}' ry='6' fill='rgba(40,30,50,0.14)'/>",
        (min_x + max_x) / 2.0,
        max_y + 4.0,
        (max_x - min_x) * 0.42,
    ));
    let mut d = String::new();
    for (i, p) in outline.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        d.push_str(&format!("{cmd}{:.2} {:.2} ", p.0, p.1));
    }
    d.push('Z');
    // Clip id per cell: belly/blush must stay INSIDE wild concave bodies
    // (extent ellipses escape otherwise).
    let clip_id = format!("c{seed}d{}", (dom * 100.0) as u32);
    s.push_str(&format!(
        "<defs><clipPath id='{clip_id}'><path d='{d}'/></clipPath></defs><path d='{d}' fill='{}' stroke='{}' stroke-width='2.5'/>",
        colors.body, colors.outline,
    ));
    s.push_str(&format!("<g clip-path='url(#{clip_id})'>"));

    // Face mounting: search the upper band for the WIDEST eye line (fixed
    // fractions put tiny faces on narrow necks — the TOG findRect lesson:
    // the face goes where the mass is, and the mass moves per shape).
    let mut eye_y = min_y + h * 0.38;
    let mut best_w = 0.0;
    for i in 0..=12 {
        let y = min_y + h * (0.26 + 0.03 * i as f64);
        if let Some((l, r)) = wild::extent_at(&outline, y) {
            if r - l > best_w {
                best_w = r - l;
                eye_y = y;
            }
        }
    }
    if let Some((lx, rx)) = wild::extent_at(&outline, eye_y) {
        let w = (rx - lx) / 2.0;
        let fx = (lx + rx) / 2.0;
        let r = (h * 0.075).min(w * 0.34).max(4.5);
        let ex = w * 0.44;
        // Belly: inset lower-mass patch via mid-height extents.
        let belly_y = min_y + h * 0.62;
        if let Some((blx, brx)) = wild::extent_at(&outline, belly_y) {
            let bw = (brx - blx) / 2.0 * 0.62;
            let bx = (blx + brx) / 2.0;
            s.push_str(&format!(
                "<ellipse cx='{bx:.1}' cy='{:.1}' rx='{bw:.1}' ry='{:.1}' fill='{}'/>",
                belly_y + h * 0.05,
                h * 0.24,
                colors.belly,
            ));
        }
        s.push_str(&format!(
            "<ellipse cx='{:.1}' cy='{:.1}' rx='{:.1}' ry='{:.1}' fill='{}' opacity='0.8'/><ellipse cx='{:.1}' cy='{:.1}' rx='{:.1}' ry='{:.1}' fill='{}' opacity='0.8'/>",
            fx - ex * 1.28, eye_y + r * 1.3, r * 0.82, r * 0.5, colors.blush,
            fx + ex * 1.28, eye_y + r * 1.3, r * 0.82, r * 0.5, colors.blush,
        ));
        // Eyes/mouth via a temporary blob-frame shim: the face functions take
        // BodyParams only to derive (width, r, y) — hand them equivalents.
        let shim = body::BodyParams {
            height: h,
            w_top: w,
            w_bot: w,
            n_top: 2.0,
            n_bot: 2.0,
            waist: 0.5,
        };
        let face_cy = eye_y + h * 0.08; // face functions put eyes at -8% height
        let eyes_svg = face::eyes_inked(eyes, &shim, fx, face_cy, 0.0, &colors.outline, &colors.ink);
        s.push_str(&eyes_svg);
        s.push_str(&face::mouth_inked(mouth, &shim, fx, face_cy, &colors.outline, &colors.ink));
    }
    s.push_str("</g>");
    s
}

fn main() {
    std::fs::create_dir_all("/tmp/ava-playground").expect("mkdir");
    let cols = 6usize;
    let doms = [0.0_f64, 0.35, 0.7];
    let rows = doms.len();
    let width = PAD + cols as f64 * (CELL + PAD);
    let height = PAD + rows as f64 * (CELL + PAD) + 34.0;
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width:.0}' height='{height:.0}' viewBox='0 0 {width:.0} {height:.0}'><rect width='100%' height='100%' fill='#1c1a20'/><text x='{PAD}' y='24' font-family='monospace' font-size='15' fill='#c8c2ce'>grid 7 — wild spline bodies x domestication (rows: 0.00 / 0.35 / 0.70)</text>"
    );
    for (r, &dom) in doms.iter().enumerate() {
        for c in 0..cols {
            let seed = 2000 + c as u64; // same seed per column across rows
            let cx = PAD + c as f64 * (CELL + PAD) + CELL / 2.0;
            let cy = PAD + r as f64 * (CELL + PAD) + CELL / 2.0 + 28.0;
            svg.push_str(&format!(
                "<rect x='{:.1}' y='{:.1}' width='{CELL}' height='{CELL}' rx='10' fill='#efe9e4'/>",
                cx - CELL / 2.0,
                cy - CELL / 2.0 - 14.0,
            ));
            svg.push_str(&draw_wild(seed, dom, cx, cy));
            svg.push_str(&format!(
                "<text x='{:.1}' y='{:.1}' font-family='monospace' font-size='10' fill='#8f8a96'>#{seed} d{dom:.2}</text>",
                cx - CELL / 2.0 + 8.0,
                cy - CELL / 2.0 + 2.0,
            ));
        }
    }
    svg.push_str("</svg>");
    std::fs::write("/tmp/ava-playground/grid7.svg", &svg).expect("write");
    println!("grid7: {} wild creatures -> /tmp/ava-playground/grid7.svg", rows * cols);
}
