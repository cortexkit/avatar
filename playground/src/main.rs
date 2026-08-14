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
    /// Absolute half inter-pupil distance CHOSEN by the search — the fit may
    /// have pulled the eyes inward off thin side masses, so consumers must
    /// not re-derive spacing from half_w.
    ipd: f64,
    r: f64,
}

/// Widest-band scan with eye-fit shrink (the EVM-free findRect).
/// SDF-verified face placement. The widest-scanline approach checked fit
/// only HORIZONTALLY at one y — eye discs crossed the silhouette vertically
/// on notched crowns and landed on thin masses (manta wings, Ufuk's grid-10
/// review). Every candidate is now scored by the signed distance of both
/// eye centers to the outline: an eye exists only where the body has
/// interior DEPTH for it. Search over height band x eye spacing; radius is
/// what the interior affords, capped by the cute proportions.
fn place_face(outline: &[(f64, f64)]) -> Option<FacePlacement> {
    let (_, min_y, _, max_y) = wild::bbox(outline);
    let h = max_y - min_y;
    let poly = merge::Poly::new(outline, 2);
    // The eye disc must clear the outline by 22% of its radius.
    const MARGIN: f64 = 1.22;
    let r_cap = h * 0.085 * 1.15;
    let r_floor = h * 0.028;
    let mut best: Option<(f64, FacePlacement)> = None;
    for yi in 0..=18 {
        let y = min_y + h * (0.20 + 0.026 * yi as f64);
        let Some((l, rgt)) = wild::extent_at(outline, y) else {
            continue;
        };
        let span = rgt - l;
        let fx = (l + rgt) / 2.0;
        for ipd_ratio in [0.44_f64, 0.36, 0.28, 0.21] {
            let ex = span / 2.0 * ipd_ratio;
            // Interior depth at the shallower of the two eye centers.
            let depth = -(poly.sdf(fx - ex, y).max(poly.sdf(fx + ex, y)));
            // Eyes must never touch EACH OTHER (the TOG pain point): the
            // widest archetype (uneven/lidded) reaches ~1.25r half-width,
            // so center spacing 2·ex needs 2·r·1.45 — cap r by the spacing.
            let r = (depth / MARGIN)
                .min(r_cap)
                .min(span * 0.30 * 1.15)
                .min(ex / 1.45);
            if r < r_floor {
                continue;
            }
            // Radius first (big cute eyes), wide spacing preferred, gentle
            // pull toward the canonical eye band (~34% down).
            let band_penalty = ((y - (min_y + h * 0.34)) / h).abs() * 0.25;
            let score = r * (1.0 + 0.22 * ipd_ratio) * (1.0 - band_penalty);
            if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                best = Some((
                    score,
                    FacePlacement {
                        fx,
                        eye_y: y,
                        half_w: span / 2.0,
                        ipd: ex,
                        r,
                    },
                ));
            }
        }
    }
    best.map(|(_, p)| p)
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
                "<path d='{d}' fill='{fill}' stroke='{}' stroke-width='1.5' stroke-linejoin='round'/>",
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
        "<defs><clipPath id='{clip_id}'><path d='{d}'/></clipPath></defs><path d='{d}' fill='{}' stroke='{}' stroke-width='1.5'/>",
        colors.body, colors.outline,
    ));
    s.push_str(&format!("<g clip-path='url(#{clip_id})'>"));

    // Placement first: the voronoi mask needs to know where the face lives.
    let placement = place_face(&outline);

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
        // Face-capsule mask: the mottle belongs to the creature's HIDE, not
        // its face — pebble edges crossing the eye/belly zone read as glare
        // artifacts (Ufuk's review). A radial-gradient mask fades cells out
        // over the face-and-belly capsule instead of hard-cutting them.
        let mask_id = format!("vm{clip_id}");
        let mut mask_defs = String::new();
        if let Some(p) = &placement {
            let cap_top = p.eye_y - p.r * 2.2;
            let cap_bot = min_y + h * 0.62 + h * 0.29;
            let cap_cy = (cap_top + cap_bot) / 2.0;
            let cap_ry = (cap_bot - cap_top) / 2.0;
            let cap_rx = (p.ipd + p.r * 2.6).max(p.half_w * 0.66);
            mask_defs = format!(
                "<defs><radialGradient id='fg{mask_id}'><stop offset='0.62' stop-color='black'/><stop offset='1' stop-color='white'/></radialGradient><mask id='{mask_id}'><rect x='{:.0}' y='{:.0}' width='{:.0}' height='{:.0}' fill='white'/><ellipse cx='{:.1}' cy='{cap_cy:.1}' rx='{cap_rx:.1}' ry='{cap_ry:.1}' fill='url(#fg{mask_id})'/></mask></defs>",
                min_x - 20.0,
                min_y - 20.0,
                max_x - min_x + 40.0,
                max_y - min_y + 40.0,
                p.fx,
            );
            s.push_str(&mask_defs);
            s.push_str(&format!("<g mask='url(#{mask_id})'>"));
        }
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
        if !mask_defs.is_empty() {
            s.push_str("</g>");
        }
    }

    // Face + mood overlays (the grid-4 layered-morph machinery on wild
    // placement: lids and squints in body color, on-demand brows, mouth
    // override, eye scale, adjuncts).
    if let Some(p) = placement {
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
        s.push_str(&format!(
            "<ellipse cx='{:.1}' cy='{:.1}' rx='{:.1}' ry='{:.1}' fill='{}' opacity='0.8'/><ellipse cx='{:.1}' cy='{:.1}' rx='{:.1}' ry='{:.1}' fill='{}' opacity='0.8'/>",
            p.fx - p.ipd * 1.28, p.eye_y + p.r * 1.3, p.r * 0.82, p.r * 0.5, colors.blush,
            p.fx + p.ipd * 1.28, p.eye_y + p.r * 1.3, p.r * 0.82, p.r * 0.5, colors.blush,
        ));
        // The shim width is derived from the CHOSEN ipd (frame computes
        // ex = width*0.42), so the fit-searched spacing survives verbatim.
        let shim = body::BodyParams {
            height: p.r / 0.085,
            w_top: p.ipd / 0.42,
            w_bot: p.ipd / 0.42,
            n_top: 2.0,
            n_bot: 2.0,
            waist: 0.5,
        };
        let face_cy = p.eye_y + shim.height * 0.08;
        let f = face::frame(&shim, p.fx, face_cy);

        // Eyes, optionally mood-scaled around the eye line.
        let eyes_svg = face::eyes_inked(
            c.eyes,
            &shim,
            p.fx,
            face_cy,
            0.0,
            &colors.outline,
            &colors.ink,
            &colors.pupil,
            colors.sclera.as_deref(),
        );
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

/// Rebuilds exactly the outline draw_full renders (same transforms), for
/// the eyes-inside checker. Kept beside draw_full in the same file so the
/// two can never drift silently.
fn outline_for(c: &Creature, cx: f64, cy: f64) -> Vec<(f64, f64)> {
    let dom = 0.32;
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
    let ext_outlines: Vec<Vec<(f64, f64)>> = c
        .exts
        .iter()
        .flat_map(|e| {
            let right = wild::extremity_outline(e, ring_r_at(e.theta), mean_r, false);
            let left: Vec<(f64, f64)> = right.iter().map(|p| (-p.0, p.1)).collect();
            [right, left]
        })
        .collect();
    let (mut bx0, mut by0, mut bx1, mut by1) = raw.bbox;
    for pts in &ext_outlines {
        for &(x, y) in pts {
            bx0 = bx0.min(x);
            by0 = by0.min(y);
            bx1 = bx1.max(x);
            by1 = by1.max(y);
        }
    }
    let scale = (172.0 / (bx1 - bx0).max(1e-6)).min(198.0 / (by1 - by0).max(1e-6));
    let mx = (bx0 + bx1) / 2.0;
    let my = (by0 + by1) / 2.0;
    let t = |p: (f64, f64)| -> (f64, f64) { (cx + (p.0 - mx) * scale, cy + (p.1 - my) * scale) };
    if c.topology == 0 && !ext_outlines.is_empty() {
        let mut shapes = vec![merge::Poly::new(&raw.pts, 3)];
        for pts in &ext_outlines {
            shapes.push(merge::Poly::new(pts, 1));
        }
        merge::merged_outline(&shapes, (bx0, by0, bx1, by1), mean_r * 0.16, 130)
            .iter()
            .map(|&p| t(p))
            .collect()
    } else {
        raw.pts.iter().map(|&p| t(p)).collect()
    }
}

/// Eyes-inside sweep: SDF-verifies both eye discs for a seed range. Run
/// with `--check N`. Prints violations; exits nonzero on any.
fn run_checker(n: u64) {
    let mut violations = 0usize;
    let mut placed = 0usize;
    let mut worst: (f64, u64) = (f64::MAX, 0);
    for seed in 4000..(4000 + n) {
        let c = decode_full(seed);
        let outline = outline_for(&c, 120.0, 120.0);
        let Some(p) = place_face(&outline) else {
            println!("seed {seed}: NO PLACEMENT");
            violations += 1;
            continue;
        };
        placed += 1;
        let poly = merge::Poly::new(&outline, 2);
        // Verify the DRAWN eye disc: the placement radius is what the
        // renderer uses, so clearance = interior depth minus radius.
        let clearance = [
            poly.sdf(p.fx - p.ipd, p.eye_y),
            poly.sdf(p.fx + p.ipd, p.eye_y),
        ]
        .iter()
        .map(|d| -d - p.r)
        .fold(f64::MAX, f64::min);
        if clearance < 0.0 {
            println!("seed {seed}: eye disc breaches outline by {:.2}", -clearance);
            violations += 1;
        }
        // Separation invariant: eye discs never touch each other (widest
        // archetype half-width 1.25r + margin).
        let sep = 2.0 * p.ipd - 2.0 * p.r * 1.45;
        if sep < -1e-9 {
            println!("seed {seed}: eyes overlap by {:.2}", -sep);
            violations += 1;
        }
        if clearance < worst.0 {
            worst = (clearance, seed);
        }
    }
    println!(
        "checked {n} seeds: {placed} placed, {violations} violations; tightest clearance {:.2} units (seed {})",
        worst.0, worst.1,
    );
    if violations > 0 {
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--check") {
        let n = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(200);
        run_checker(n);
        return;
    }
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
