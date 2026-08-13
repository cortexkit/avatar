//! Grid 8 — the four corrections from Ufuk's grid-7 review, layered:
//!   row 1: mirrored-always wild bodies + COMPUTED face placement
//!   row 2: + extremities (genome-grown limbs, the point-information probe)
//!   row 3: + real Voronoi surface mottle (computed, not premade)
//! Columns share seeds so each row isolates one addition's contribution.

mod body;
mod face;
mod genome;
mod mood;
mod palette;
mod parts;
mod surface;
mod wild;

const CELL: f64 = 240.0;
const PAD: f64 = 12.0;

struct Placement {
    eye_y: f64,
    fx: f64,
    half_w: f64,
    r: f64,
}

/// Computed face placement — the EVM-free upgrade over TOG's findRect:
/// scan the upper band for the line that maximizes width x vertical
/// clearance, then shrink the eye radius until eyes fit INSIDE the outline
/// at both eye centers (checked against the extents one radius above and
/// below, not just at the eye line).
fn place_face(outline: &[(f64, f64)]) -> Option<Placement> {
    let (_, min_y, _, max_y) = wild::bbox(outline);
    let h = max_y - min_y;
    let mut best: Option<(f64, f64, f64, f64)> = None; // score, y, fx, w
    for i in 0..=20 {
        let y = min_y + h * (0.22 + 0.028 * i as f64);
        let Some((lx, rx)) = wild::extent_at(outline, y) else {
            continue;
        };
        let w = (rx - lx) / 2.0;
        // Vertical room: does the shape still cover the eye band above/below?
        let probe = h * 0.07;
        let above = wild::extent_at(outline, y - probe).map_or(0.0, |(l, r)| (r - l) / 2.0);
        let below = wild::extent_at(outline, y + probe).map_or(0.0, |(l, r)| (r - l) / 2.0);
        let room = above.min(below).min(w);
        let score = w * 0.6 + room * 0.4 - (0.28 - (y - min_y) / h).abs() * h * 0.2;
        if best.map_or(true, |(s, ..)| score > s) {
            best = Some((score, y, (lx + rx) / 2.0, w));
        }
    }
    let (_, eye_y, fx, half_w) = best?;
    let mut r = (h * 0.085).min(half_w * 0.36);
    // Shrink until both eye disks fit inside the outline vertically.
    for _ in 0..4 {
        let ex = half_w * 0.44;
        let fits = [eye_y - r, eye_y + r].iter().all(|&yy| {
            wild::extent_at(outline, yy)
                .map_or(false, |(l, rr)| fx - ex - r * 0.8 >= l && fx + ex + r * 0.8 <= rr)
        });
        if fits {
            break;
        }
        r *= 0.82;
    }
    Some(Placement { eye_y, fx, half_w, r: r.max(4.0) })
}

fn draw_creature(seed: u64, extremities_on: bool, voronoi_on: bool, cx: f64, cy: f64) -> String {
    let mut b = genome::Bits::new(seed);
    let wb = wild::decode(&mut b);
    let exts = wild::decode_extremities(&mut b);
    let sites = surface::decode_sites(&mut b);
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
    let dom = 0.32; // semi-wild band: the population's proposed center

    // Body outline in cell coordinates plus the transform needed to carry
    // ring-frame extremities through the same normalize step.
    let outline = wild::outline(&wb, dom, cx, cy, 172.0);
    let (min_x, min_y, max_x, max_y) = wild::bbox(&outline);
    let h = max_y - min_y;

    // Ring-frame -> cell transform recovery: outline() normalizes internally,
    // so recover scale/offset by round-tripping two probe points. Cheap and
    // honest for playground purposes.
    let probe = wild::outline(&wb, dom, 0.0, 0.0, 172.0);
    let (pmin_x, pmin_y, pmax_x, pmax_y) = wild::bbox(&probe);
    let raw = wild::outline_raw(&wb, dom);
    let (rmin_x, rmin_y, rmax_x, rmax_y) = wild::bbox(&raw);
    let sx = (pmax_x - pmin_x) / (rmax_x - rmin_x).max(1e-9);
    let sy = (pmax_y - pmin_y) / (rmax_y - rmin_y).max(1e-9);
    let s_avg = (sx + sy) / 2.0;
    let rcx = (rmin_x + rmax_x) / 2.0;
    let rcy = (rmin_y + rmax_y) / 2.0;
    let to_cell = |p: (f64, f64)| -> (f64, f64) { (cx + (p.0 - rcx) * s_avg, cy + (p.1 - rcy) * sy.min(sx * 1.15)) };

    let mean_r = wb.spokes.iter().map(|s| s.r).sum::<f64>() / wb.spokes.len() as f64;
    let ring_r_at = |theta: f64| -> f64 {
        // Nearest-spoke radius (mirrored lookup): good enough for anchoring.
        let t = theta.rem_euclid(std::f64::consts::TAU);
        let tm = (std::f64::consts::PI - t).rem_euclid(std::f64::consts::TAU);
        wb.spokes
            .iter()
            .map(|s| {
                let d1 = (s.theta - t).abs().min(std::f64::consts::TAU - (s.theta - t).abs());
                let d2 = (s.theta - tm).abs().min(std::f64::consts::TAU - (s.theta - tm).abs());
                (d1.min(d2), s.r)
            })
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .map(|(_, r)| r)
            .unwrap_or(mean_r)
    };

    let mut s = String::new();
    s.push_str(&format!(
        "<ellipse cx='{:.1}' cy='{:.1}' rx='{:.1}' ry='6' fill='rgba(40,30,50,0.14)'/>",
        (min_x + max_x) / 2.0,
        max_y + 4.0,
        (max_x - min_x) * 0.42,
    ));

    // Extremities first: body draw covers the grafts (parts-layer trick).
    if extremities_on {
        for e in &exts {
            for mirror in [false, true] {
                let pts = wild::extremity_outline(e, ring_r_at(e.theta), mean_r, mirror);
                let mut d = String::new();
                for (i, p) in pts.iter().enumerate() {
                    let q = to_cell(*p);
                    let cmd = if i == 0 { 'M' } else { 'L' };
                    d.push_str(&format!("{cmd}{:.2} {:.2} ", q.0, q.1));
                }
                d.push('Z');
                s.push_str(&format!(
                    "<path d='{d}' fill='{}' stroke='{}' stroke-width='2.5' stroke-linejoin='round'/>",
                    colors.body, colors.outline,
                ));
            }
        }
    }

    let mut d = String::new();
    for (i, p) in outline.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        d.push_str(&format!("{cmd}{:.2} {:.2} ", p.0, p.1));
    }
    d.push('Z');
    let clip_id = format!("g8c{seed}e{}v{}", extremities_on as u8, voronoi_on as u8);
    s.push_str(&format!(
        "<defs><clipPath id='{clip_id}'><path d='{d}'/></clipPath></defs><path d='{d}' fill='{}' stroke='{}' stroke-width='2.5'/>",
        colors.body, colors.outline,
    ));
    s.push_str(&format!("<g clip-path='url(#{clip_id})'>"));

    // Real Voronoi mottle, clipped to the body.
    if voronoi_on {
        let scaled: Vec<surface::Site> = sites
            .iter()
            .map(|site| surface::Site {
                x: (min_x + max_x) / 2.0 + site.x * (max_x - min_x) / 2.0,
                y: (min_y + max_y) / 2.0 + site.y * (max_y - min_y) / 2.0,
                shade: site.shade,
            })
            .collect();
        for (site, cell) in scaled.iter().zip(surface::cells(&scaled, (min_x, min_y, max_x, max_y))) {
            if cell.len() < 3 || site.shade == 2 {
                continue;
            }
            let mut cd = String::new();
            for (i, p) in cell.iter().enumerate() {
                let cmd = if i == 0 { 'M' } else { 'L' };
                cd.push_str(&format!("{cmd}{:.1} {:.1} ", p.0, p.1));
            }
            cd.push('Z');
            let fill = if site.shade == 0 { "rgba(20,10,30,0.10)" } else { "rgba(255,255,255,0.13)" };
            s.push_str(&format!("<path d='{cd}' fill='{fill}' stroke='rgba(20,10,30,0.06)' stroke-width='1.2'/>"));
        }
    }

    // Face: computed placement.
    if let Some(p) = place_face(&outline) {
        let belly_y = min_y + h * 0.62;
        if let Some((blx, brx)) = wild::extent_at(&outline, belly_y) {
            let bw = (brx - blx) / 2.0 * 0.62;
            s.push_str(&format!(
                "<ellipse cx='{:.1}' cy='{:.1}' rx='{bw:.1}' ry='{:.1}' fill='{}'/>",
                (blx + brx) / 2.0,
                belly_y + h * 0.05,
                h * 0.24,
                colors.belly,
            ));
        }
        let ex = p.half_w * 0.44;
        s.push_str(&format!(
            "<ellipse cx='{:.1}' cy='{:.1}' rx='{:.1}' ry='{:.1}' fill='{}' opacity='0.8'/><ellipse cx='{:.1}' cy='{:.1}' rx='{:.1}' ry='{:.1}' fill='{}' opacity='0.8'/>",
            p.fx - ex * 1.28, p.eye_y + p.r * 1.3, p.r * 0.82, p.r * 0.5, colors.blush,
            p.fx + ex * 1.28, p.eye_y + p.r * 1.3, p.r * 0.82, p.r * 0.5, colors.blush,
        ));
        let shim = body::BodyParams {
            height: p.r / 0.085, // face fns derive r = height*0.085 — invert
            w_top: p.half_w,
            w_bot: p.half_w,
            n_top: 2.0,
            n_bot: 2.0,
            waist: 0.5,
        };
        let face_cy = p.eye_y + shim.height * 0.08;
        s.push_str(&face::eyes_inked(eyes, &shim, p.fx, face_cy, 0.0, &colors.outline, &colors.ink));
        s.push_str(&face::mouth_inked(mouth, &shim, p.fx, face_cy, &colors.outline, &colors.ink));
    }
    s.push_str("</g>");
    s
}

fn main() {
    std::fs::create_dir_all("/tmp/ava-playground").expect("mkdir");
    let cols = 6usize;
    let rows: [(bool, bool, &str); 3] = [
        (false, false, "mirrored + computed face"),
        (true, false, "+ extremities"),
        (true, true, "+ voronoi"),
    ];
    let width = PAD + cols as f64 * (CELL + PAD);
    let height = PAD + rows.len() as f64 * (CELL + PAD) + 34.0;
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width:.0}' height='{height:.0}' viewBox='0 0 {width:.0} {height:.0}'><rect width='100%' height='100%' fill='#1c1a20'/><text x='{PAD}' y='24' font-family='monospace' font-size='15' fill='#c8c2ce'>grid 8 — mirrored wild + computed faces / + extremities / + voronoi</text>"
    );
    for (r, &(ext, vor, label)) in rows.iter().enumerate() {
        for c in 0..cols {
            let seed = 3000 + c as u64;
            let cx = PAD + c as f64 * (CELL + PAD) + CELL / 2.0;
            let cy = PAD + r as f64 * (CELL + PAD) + CELL / 2.0 + 28.0;
            svg.push_str(&format!(
                "<rect x='{:.1}' y='{:.1}' width='{CELL}' height='{CELL}' rx='10' fill='#efe9e4'/>",
                cx - CELL / 2.0,
                cy - CELL / 2.0 - 14.0,
            ));
            svg.push_str(&draw_creature(seed, ext, vor, cx, cy));
            svg.push_str(&format!(
                "<text x='{:.1}' y='{:.1}' font-family='monospace' font-size='10' fill='#8f8a96'>#{seed} {label}</text>",
                cx - CELL / 2.0 + 8.0,
                cy - CELL / 2.0 + 2.0,
            ));
        }
    }
    svg.push_str("</svg>");
    std::fs::write("/tmp/ava-playground/grid8.svg", &svg).expect("write");
    println!("grid8 -> /tmp/ava-playground/grid8.svg");
}
