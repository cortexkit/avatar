//! Grid 10 — the freeze-package gauntlet on the FULL wild pipeline:
//!   sheet 1 (grid10.svg): 36 sequential-seed gacha, every axis genome-driven
//!     (wild body, extremities + color voice, merge flag, voronoi, palette,
//!     face pack) — design law 3 on the complete generator.
//!   sheet 2 (grid10-moods.svg): 4 cast seeds x all 7 moods on wild bodies.
//!   sheet 3 (grid10-strip.svg): natural-size 36px cells — icon legibility.

use ava_playground::{body, face, genome, merge, mood, palette, surface, wild};

const CELL: f64 = 240.0;
const PAD: f64 = 12.0;

/// Everything one seed decodes to on the full pipeline.
struct Creature {
    wb: wild::WildBody,
    exts: Vec<wild::Extremity>,
    ext_fills: Vec<u8>,
    sites: Vec<surface::Site>,
    recipe: usize,
    hue: f64,
    eyes: face::EyeKind,
    mouth: face::MouthKind,
    /// 0 = merged silhouette, 1 = layered tone fills, 2 = layered accent.
    topology: u8,
    voronoi: bool,
}

fn decode_full(seed: u64) -> Creature {
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
    let ext_fills: Vec<u8> = exts.iter().map(|_| b.byte() % 3).collect();
    // Topology: merged is a real character class, weighted 1/3.
    let topology = b.byte() % 3;
    // Voronoi mottle on ~40% of the population.
    let voronoi = b.byte() % 5 < 2;
    Creature {
        wb,
        exts,
        ext_fills,
        sites,
        recipe,
        hue,
        eyes,
        mouth,
        topology,
        voronoi,
    }
}

struct FacePlacement {
    fx: f64,
    eye_y: f64,
    half_w: f64,
    r: f64,
}

/// Widest-band scan with eye-fit shrink (the EVM-free findRect).
fn place_face(outline: &[(f64, f64)]) -> Option<FacePlacement> {
    let (min_x, min_y, max_x, max_y) = wild::bbox(outline);
    let h = max_y - min_y;
    let _ = max_x - min_x;
    let mut best: Option<(f64, f64, f64)> = None; // (width, y, center)
    for i in 0..=14 {
        let y = min_y + h * (0.24 + 0.028 * i as f64);
        if let Some((l, r)) = wild::extent_at(outline, y) {
            let w = r - l;
            if best.map(|b| w > b.0).unwrap_or(true) {
                best = Some((w, y, (l + r) / 2.0));
            }
        }
    }
    let (w, y, fx) = best?;
    let mut r = (h * 0.085).min(w * 0.30) * 1.15;
    // Shrink until both eyes fit inside the outline at eye_y.
    for _ in 0..6 {
        let ex = w / 2.0 * 0.44;
        if let Some((l2, r2)) = wild::extent_at(outline, y) {
            if fx - ex - r > l2 && fx + ex + r < r2 {
                break;
            }
        }
        r *= 0.85;
    }
    Some(FacePlacement {
        fx,
        eye_y: y,
        half_w: w / 2.0,
        r,
    })
}

fn draw_full(c: &Creature, m: mood::Mood, cx: f64, cy: f64) -> String {
    let colors = palette::resolve(c.recipe, c.hue);
    let dom = 0.32;
    let spec = mood::spec(m);

    let raw = wild::build_raw(&c.wb, dom);
    let mean_r = c.wb.spokes.iter().map(|s| s.r).sum::<f64>() / c.wb.spokes.len() as f64;
    let ring_r_at = |theta: f64| -> f64 {
        let t = theta.rem_euclid(std::f64::consts::TAU);
        let tm = (std::f64::consts::PI - t).rem_euclid(std::f64::consts::TAU);
        c.wb
            .spokes
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

    let ext_outlines: Vec<(usize, Vec<(f64, f64)>)> = c
        .exts
        .iter()
        .enumerate()
        .flat_map(|(i, e)| {
            let right = wild::extremity_outline(e, ring_r_at(e.theta), mean_r, false);
            let left: Vec<(f64, f64)> = right.iter().map(|p| (-p.0, p.1)).collect();
            [(i, right), (i, left)]
        })
        .collect();

    let (mut bx0, mut by0, mut bx1, mut by1) = raw.bbox;
    for (_, pts) in &ext_outlines {
        for &(x, y) in pts {
            bx0 = bx0.min(x);
            by0 = by0.min(y);
            bx1 = bx1.max(x);
            by1 = by1.max(y);
        }
    }
    let bw = (bx1 - bx0).max(1e-6);
    let bh = (by1 - by0).max(1e-6);
    let scale = (172.0 / bw).min(198.0 / bh);
    let mx = (bx0 + bx1) / 2.0;
    let my = (by0 + by1) / 2.0;
    let t = |p: (f64, f64)| -> (f64, f64) { (cx + (p.0 - mx) * scale, cy + (p.1 - my) * scale) };

    let merged_mode = c.topology == 0 && !ext_outlines.is_empty();
    let outline: Vec<(f64, f64)> = if merged_mode {
        let mut shapes = vec![merge::Poly::new(&raw.pts, 3)];
        for (_, pts) in &ext_outlines {
            shapes.push(merge::Poly::new(pts, 1));
        }
        let merged = merge::merged_outline(&shapes, (bx0, by0, bx1, by1), mean_r * 0.16, 130);
        merged.iter().map(|&p| t(p)).collect()
    } else {
        raw.pts.iter().map(|&p| t(p)).collect()
    };
    let (min_x, min_y, max_x, max_y) = wild::bbox(&outline);
    let h = max_y - min_y;

    let mut s = String::new();
    s.push_str(&format!(
        "<ellipse cx='{cx:.1}' cy='{:.1}' rx='{:.1}' ry='6' fill='rgba(40,30,50,0.14)'/>",
        max_y + 4.0,
        (max_x - min_x) * 0.42,
    ));

    if !merged_mode {
        for (ei, pts) in &ext_outlines {
            let fill = match (c.topology, c.ext_fills[*ei]) {
                (2, _) => colors.part.clone(),
                (_, 0) => palette::tone(c.recipe, c.hue, 1),
                (_, 1) => palette::tone(c.recipe, c.hue, -1),
                _ => colors.body.clone(),
            };
            let mut d = String::new();
            for (i, p) in pts.iter().enumerate() {
                let q = t(*p);
                let cmd = if i == 0 { 'M' } else { 'L' };
                d.push_str(&format!("{cmd}{:.2} {:.2} ", q.0, q.1));
            }
            d.push('Z');
            s.push_str(&format!(
                "<path d='{d}' fill='{fill}' stroke='{}' stroke-width='2.5' stroke-linejoin='round'/>",
                colors.outline,
            ));
        }
    }

    let mut d = String::new();
    for (i, p) in outline.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        d.push_str(&format!("{cmd}{:.2} {:.2} ", p.0, p.1));
    }
    d.push('Z');
    let clip_id = format!("g10c{}m{}", c.hue as u32, m as u8);
    s.push_str(&format!(
        "<defs><clipPath id='{clip_id}'><path d='{d}'/></clipPath></defs><path d='{d}' fill='{}' stroke='{}' stroke-width='2.5'/>",
        colors.body, colors.outline,
    ));
    s.push_str(&format!("<g clip-path='url(#{clip_id})'>"));

    if c.voronoi {
        let cell_bbox = (min_x, min_y, max_x, max_y);
        let mut scaled: Vec<surface::Site> = c
            .sites
            .iter()
            .map(|site| surface::Site {
                x: cx + site.x * (max_x - min_x) / 2.0,
                y: (min_y + max_y) / 2.0 + site.y * (max_y - min_y) / 2.0,
                shade: site.shade,
            })
            .collect();
        surface::relax(&mut scaled, cell_bbox, 2);
        let (light, dark) = palette::mottle(c.recipe, c.hue);
        for (site, cell) in scaled.iter().zip(surface::cells(&scaled, cell_bbox)) {
            if cell.len() < 3 || site.shade == 2 {
                continue;
            }
            let fill = if site.shade == 0 { &dark } else { &light };
            s.push_str(&format!(
                "<path d='{}' fill='{fill}'/>",
                surface::cell_path(&cell, 0.88),
            ));
        }
    }

    // Face + mood overlays (the grid-4 layered-morph machinery on wild
    // placement: lids and squints in body color, on-demand brows, mouth
    // override, eye scale, adjuncts).
    if let Some(p) = place_face(&outline) {
        let belly_y = min_y + h * 0.62;
        if let Some((blx, brx)) = wild::extent_at(&outline, belly_y) {
            let bw2 = (brx - blx) / 2.0 * 0.62;
            s.push_str(&format!(
                "<ellipse cx='{:.1}' cy='{:.1}' rx='{bw2:.1}' ry='{:.1}' fill='{}'/>",
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
            height: p.r / 0.085,
            w_top: p.half_w,
            w_bot: p.half_w,
            n_top: 2.0,
            n_bot: 2.0,
            waist: 0.5,
        };
        let face_cy = p.eye_y + shim.height * 0.08;
        let f = face::frame(&shim, p.fx, face_cy);

        // Eyes, optionally mood-scaled around the eye line.
        let eyes_svg = face::eyes_inked(c.eyes, &shim, p.fx, face_cy, 0.0, &colors.outline, &colors.ink);
        if (spec.eye_scale - 1.0).abs() > 0.01 {
            s.push_str(&format!(
                "<g transform='translate({:.1} {:.1}) scale({:.3}) translate({:.1} {:.1})'>{eyes_svg}</g>",
                p.fx, f.y, spec.eye_scale, -p.fx, -f.y,
            ));
        } else {
            s.push_str(&eyes_svg);
        }

        // Mood layers per eye: squint from below, lid from above, brows.
        for (x, side) in [(f.lx, -1.0), (f.rx, 1.0)] {
            s.push_str(&mood::squint_overlay(x, f.y, f.r, spec.squint, &colors.body, &colors.outline));
            s.push_str(&mood::lid_overlay(
                x,
                f.y,
                f.r,
                spec.lid,
                spec.lid_angle * side,
                &colors.body,
                &colors.outline,
            ));
            s.push_str(&mood::brow(spec.brows, x, f.y, f.r, side, &colors.ink));
        }

        // Mouth: mood override or the pack's own.
        let mouth_kind = match spec.mouth {
            mood::MouthMood::Keep => c.mouth,
            mood::MouthMood::Grin => face::MouthKind::OpenHappy,
            mood::MouthMood::O => face::MouthKind::OMouth,
            mood::MouthMood::Pressed => face::MouthKind::Flat,
            mood::MouthMood::Down => face::MouthKind::DownArc,
        };
        s.push_str(&face::mouth_inked(mouth_kind, &shim, p.fx, face_cy, &colors.outline, &colors.ink));
        s.push_str("</g>");

        // Adjuncts live OUTSIDE the body clip (Zz floats beside the crown).
        s.push_str(&mood::adjunct(spec.adjunct, cx, min_y, h / 200.0, &colors.ink));
    } else {
        s.push_str("</g>");
    }
    s
}

fn cell_bg(svg: &mut String, cx: f64, cy: f64, label: &str) {
    svg.push_str(&format!(
        "<rect x='{:.1}' y='{:.1}' width='{CELL}' height='{CELL}' rx='10' fill='#efe9e4'/>",
        cx - CELL / 2.0,
        cy - CELL / 2.0 - 14.0,
    ));
    svg.push_str(&format!(
        "<text x='{:.1}' y='{:.1}' font-family='monospace' font-size='10' fill='#8f8a96'>{label}</text>",
        cx - CELL / 2.0 + 8.0,
        cy - CELL / 2.0 + 2.0,
    ));
}

fn main() {
    std::fs::create_dir_all("/tmp/ava-playground").expect("mkdir");

    // Sheet 1: gacha 36, sequential seeds, neutral mood.
    let cols = 6usize;
    let rows = 6usize;
    let width = PAD + cols as f64 * (CELL + PAD);
    let height = PAD + rows as f64 * (CELL + PAD) + 34.0;
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width:.0}' height='{height:.0}' viewBox='0 0 {width:.0} {height:.0}'><rect width='100%' height='100%' fill='#1c1a20'/><text x='{PAD}' y='24' font-family='monospace' font-size='15' fill='#c8c2ce'>grid 10 — full-pipeline gacha (wild + extremities + topology + voronoi from DNA)</text>"
    );
    for r in 0..rows {
        for c in 0..cols {
            let i = r * cols + c;
            let seed = 4000 + i as u64;
            let creature = decode_full(seed);
            let cx = PAD + c as f64 * (CELL + PAD) + CELL / 2.0;
            let cy = PAD + r as f64 * (CELL + PAD) + CELL / 2.0 + 28.0;
            let tag = match creature.topology {
                0 => "M",
                1 => "T",
                _ => "A",
            };
            let v = if creature.voronoi { "+v" } else { "" };
            cell_bg(&mut svg, cx, cy, &format!("#{seed} {tag}{v}"));
            svg.push_str(&draw_full(&creature, mood::Mood::Neutral, cx, cy));
        }
    }
    svg.push_str("</svg>");
    std::fs::write("/tmp/ava-playground/grid10.svg", &svg).expect("write");

    // Sheet 2: 4 cast x 7 moods.
    let cast: [u64; 4] = [4002, 4007, 4013, 4021];
    let width2 = PAD + 7.0 * (CELL + PAD);
    let height2 = PAD + 4.0 * (CELL + PAD) + 34.0;
    let mut svg2 = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width2:.0}' height='{height2:.0}' viewBox='0 0 {width2:.0} {height2:.0}'><rect width='100%' height='100%' fill='#1c1a20'/><text x='{PAD}' y='24' font-family='monospace' font-size='15' fill='#c8c2ce'>grid 10 — moods on the wild pipeline</text>"
    );
    for (r, &seed) in cast.iter().enumerate() {
        let creature = decode_full(seed);
        for (c, &m) in mood::ALL.iter().enumerate() {
            let cx = PAD + c as f64 * (CELL + PAD) + CELL / 2.0;
            let cy = PAD + r as f64 * (CELL + PAD) + CELL / 2.0 + 28.0;
            cell_bg(&mut svg2, cx, cy, &format!("#{seed} {}", mood::name(m)));
            svg2.push_str(&draw_full(&creature, m, cx, cy));
        }
    }
    svg2.push_str("</svg>");
    std::fs::write("/tmp/ava-playground/grid10-moods.svg", &svg2).expect("write");

    // Sheet 3: honest 36px strip (natural size, group-scaled creatures).
    let strip_seeds: [u64; 8] = [4000, 4002, 4007, 4009, 4013, 4021, 4027, 4033];
    let k = 36.0 / (CELL + 30.0);
    let mut svg3 = String::from(
        "<svg xmlns='http://www.w3.org/2000/svg' width='352' height='44' viewBox='0 0 352 44'><rect width='100%' height='100%' fill='#1c1a20'/>",
    );
    for (i, &seed) in strip_seeds.iter().enumerate() {
        let creature = decode_full(seed);
        let x0 = 4.0 + i as f64 * 44.0;
        svg3.push_str(&format!(
            "<rect x='{x0:.1}' y='4' width='36' height='36' rx='6' fill='#efe9e4'/>",
        ));
        let inner = draw_full(&creature, mood::Mood::Neutral, 0.0, 0.0);
        svg3.push_str(&format!(
            "<g transform='translate({:.2} 22) scale({k:.4})'>{inner}</g>",
            x0 + 18.0,
        ));
    }
    svg3.push_str("</svg>");
    std::fs::write("/tmp/ava-playground/grid10-strip.svg", &svg3).expect("write");

    println!("grid10 + moods + strip -> /tmp/ava-playground/");
}
