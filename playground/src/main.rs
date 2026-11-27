//! Grid 10 — the freeze-package gauntlet on the FULL wild pipeline:
//!   sheet 1 (grid10.svg): 36 sequential-seed gacha, every axis genome-driven
//!     (wild body, extremities + color voice, merge flag, voronoi, palette,
//!     face pack) — design law 3 on the complete generator.
//!   sheet 2 (grid10-moods.svg): 4 cast seeds x all 7 moods on wild bodies.
//!   sheet 3 (grid10-strip.svg): natural-size 36px cells — icon legibility.

use ava_playground::fit;
use avatar_svg::wild::{
    belly_patch, belly_top_for, decode_full_counted, decode_genome, draw_full, merge, mood,
    mouth_y_for, outline_for, place_face, probe_genome, render, shape as wild, Generation,
};
use avatar_svg::{Primitive, RenderTree};

mod drawn_probe;

const CELL: f64 = 240.0;
const PAD: f64 = 12.0;

/// Projects one creature tree into the review-sheet SVG shell. The shell owns
/// captions and layout; the creature contributes only its independently
/// renderable primitives.
fn tree_fragment(mut primitives: Vec<Primitive>, operations: &[avatar_svg::TransformOp]) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TREE: AtomicU64 = AtomicU64::new(0);
    render::prepend_transform(&mut primitives, operations);
    let tree_index = NEXT_TREE.fetch_add(1, Ordering::Relaxed);
    let svg = String::from_utf8(avatar_svg::serialize_tree_with_id_prefix(
        &RenderTree {
            // Fragments carry sheet-space transforms, so their serializer
            // needs a canvas that bounds every sheet mask region.
            canvas: 5_000,
            background: None,
            pixel_grid: None,
            primitives,
        },
        &format!("wild-tree-{tree_index}"),
    ))
    .expect("tree serializer emits UTF-8 SVG");
    let start = svg.find('>').expect("tree SVG has an opening element") + 1;
    let end = svg.rfind("</svg>").expect("tree SVG has a closing element");
    svg[start..end].to_owned()
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

/// Renders the pinned population above the decoded one, same seeds, so
/// the open domestication question can be answered by looking.
///
/// The decoded value is read from the BODY byte note 009 assigned to it
/// and the decoder discards — so this shows exactly the population that
/// note described, not an invented one.
fn write_domspread(start: u64, n: usize) {
    let cell = 260.0_f64;
    let pad = 8.0;
    let label = 22.0;
    let w = pad + n as f64 * (cell + pad);
    let h = pad + 2.0 * (cell + pad + label);
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{w:.0}' height='{h:.0}' viewBox='0 0 {w:.0} {h:.0}'><rect width='100%' height='100%' fill='#141318'/>"
    );
    for (band, title) in [
        (0usize, "pinned 0.32 (ships today)"),
        (1, "decoded per creature"),
    ] {
        let y0 = pad + band as f64 * (cell + pad + label);
        svg.push_str(&format!(
            "<text x='{pad:.0}' y='{:.0}' font-family='monospace' font-size='13' fill='#8f8a96'>{title}</text>",
            y0 + 14.0
        ));
        for k in 0..n {
            let seed = start + k as u64;
            let mut c = decode_genome(&probe_genome(seed));
            if band == 1 {
                // The byte note 009 named, read the way it specified.
                c.dom = 0.12 + (probe_genome(seed)[1] as f64 / 255.0) * 0.60;
            }
            let x0 = pad + k as f64 * (cell + pad);
            let yc = y0 + label;
            svg.push_str(&format!(
                "<rect x='{x0:.1}' y='{yc:.1}' width='{cell}' height='{cell}' rx='12' fill='#efe9e4'/><text x='{:.1}' y='{:.1}' font-family='monospace' font-size='10' fill='#8f8a96'>#{seed} d={:.2}</text>",
                x0 + 7.0,
                yc + 14.0,
                c.dom,
            ));
            // Same transform shape the review sheet uses, so the two are
            // directly comparable rather than merely similar.
            let s = cell / 240.0;
            svg.push_str(&tree_fragment(
                draw_full(&c, mood::Mood::Neutral, 120.0, 120.0, Generation::LATEST),
                &[
                    render::translate(x0 + cell / 2.0 - 120.0 * s, yc + cell / 2.0 - 120.0 * s),
                    render::scale(s, s),
                ],
            ));
        }
    }
    svg.push_str("</svg>");
    let path = "/tmp/ava-playground/domspread.svg";
    std::fs::write(path, svg).expect("write domspread");
    println!("domspread: {n} seeds, pinned vs decoded -> {path}");
}

/// Checks the shipped placement's unscaled eye circles and mouth probes
/// against the body outline. Eye-kind drawing scales and mood transforms
/// are outside this proof; all distances use the neutral pose.
/// Run with `--check N`; prints violations and exits nonzero on any.
fn run_checker(n: u64) {
    let mut violations = 0usize;
    let mut placed = 0usize;
    let mut worst: (f64, u64) = (f64::MAX, 0);
    for seed in 4000..(4000 + n) {
        let c = decode_genome(&probe_genome(seed));
        let outline = outline_for(&c, 120.0, 120.0);
        let Some(p) = place_face(&outline, c.face_style.mouth_w, c.face_style.mouth_d) else {
            println!("seed {seed}: NO PLACEMENT");
            violations += 1;
            continue;
        };
        placed += 1;
        let poly = merge::Poly::new(&outline, 2);
        // Signed distance is negative inside the body, so its negation is
        // interior depth. Subtract the unscaled placement radius for clearance.
        let clearance = [
            poly.sdf(p.fx - p.ipd, p.eye_y),
            poly.sdf(p.fx + p.ipd, p.eye_y),
        ]
        .iter()
        .map(|d| -d - p.r)
        .fold(f64::MAX, f64::min);
        // Placement tests the mouth's left, right and bottom extremes using
        // genome-derived width/depth, slightly below the drawn mouth line.
        let mouth_y = p.eye_y + p.r * 2.71;
        let half = p.r * 0.8 * c.face_style.mouth_w;
        let deep = p.r * 0.62 * c.face_style.mouth_d;
        let mouth_breach = [
            poly.sdf(p.fx - half, mouth_y),
            poly.sdf(p.fx + half, mouth_y),
            poly.sdf(p.fx, mouth_y + deep),
        ]
        .iter()
        .fold(f64::MIN, |a: f64, &d| a.max(d));
        // Only meaningful when a mouth is actually drawn.
        if p.mouth_fits && mouth_breach > 0.0 {
            println!("seed {seed}: mouth breaches outline by {:.2}", mouth_breach);
            violations += 1;
        }
        if clearance < 0.0 {
            println!(
                "seed {seed}: eye disc breaches outline by {:.2}",
                -clearance
            );
            violations += 1;
        }
        // Require center spacing to exceed the unscaled eye diameter plus
        // the same extra spacing margin that placement reserves.
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

/// Measure shipped genome-to-tree build cost. Retaining every result models
/// the memory used by a row of avatars rather than one tree's transient cost.
fn run_bench(start_seed: u64, count: usize) {
    let mut timings = Vec::with_capacity(count);
    let mut trees = Vec::with_capacity(count);
    let mut primitives_total = 0usize;
    for offset in 0..count {
        let seed = start_seed + offset as u64;
        let start = std::time::Instant::now();
        let creature = decode_genome(&probe_genome(seed));
        let tree = RenderTree {
            canvas: 240,
            background: None,
            pixel_grid: None,
            primitives: draw_full(
                &creature,
                mood::Mood::Neutral,
                120.0,
                120.0,
                Generation::LATEST,
            ),
        };
        timings.push(start.elapsed().as_secs_f64() * 1000.0);
        primitives_total += tree.primitives.len();
        trees.push(tree);
    }
    timings.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let total: f64 = timings.iter().sum();
    println!(
        "built {count} trees in {total:.1} ms  ({} primitives held)",
        primitives_total
    );
    println!(
        "  per tree: min {:.2} ms  median {:.2} ms  max {:.2} ms  mean {:.2} ms",
        timings[0],
        timings[count / 2],
        timings[count - 1],
        total / count as f64,
    );
    // Touch the held trees so the optimiser cannot discard the Vec and
    // report a cost for work it never did.
    println!("  held {} trees", trees.len());
}

fn run_fixtures(start_seed: u64, count: usize, out_dir: &std::path::Path) {
    std::fs::create_dir_all(out_dir)
        .unwrap_or_else(|error| panic!("create fixture directory {}: {error}", out_dir.display()));
    for offset in 0..count {
        let seed = start_seed + offset as u64;
        let creature = decode_genome(&probe_genome(seed));
        let moods: &[mood::Mood] = if offset == 0 {
            &mood::ALL
        } else {
            &[mood::Mood::Neutral]
        };
        for &mood in moods {
            let tree = RenderTree {
                canvas: 240,
                background: None,
                pixel_grid: None,
                primitives: draw_full(&creature, mood, 120.0, 120.0, Generation::LATEST),
            };
            let json = avatar_svg::serialize_tree_json(&tree);
            let path = out_dir.join(format!("wild_{seed}__{}.json", mood::name(mood)));
            std::fs::write(&path, json.as_bytes())
                .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
            println!("{}: {} bytes", path.display(), json.len());
            if json.len() > 200 * 1024 {
                eprintln!(
                    "warning: {} is larger than 200 KiB ({} bytes)",
                    path.display(),
                    json.len()
                );
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|argument| argument == "--fixtures") {
        let usage = "--fixtures <start-seed> <count> <out-dir>";
        let start_seed = args
            .get(i + 1)
            .unwrap_or_else(|| panic!("missing start seed; usage: {usage}"))
            .parse::<u64>()
            .unwrap_or_else(|error| panic!("invalid start seed; usage: {usage}: {error}"));
        let count = args
            .get(i + 2)
            .unwrap_or_else(|| panic!("missing count; usage: {usage}"))
            .parse::<usize>()
            .unwrap_or_else(|error| panic!("invalid count; usage: {usage}: {error}"));
        assert!(count > 0, "fixture count must be greater than zero");
        let out_dir = args
            .get(i + 3)
            .unwrap_or_else(|| panic!("missing output directory; usage: {usage}"));
        run_fixtures(start_seed, count, std::path::Path::new(out_dir));
        return;
    }
    // Measurement only: reports what curve fitting would buy on real
    // contours without changing anything the pipeline draws.
    if let Some(i) = args.iter().position(|a| a == "--fit") {
        let start: u64 = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(5000);
        let count: u64 = args.get(i + 2).and_then(|v| v.parse().ok()).unwrap_or(8);
        println!(
            "{:>6}  {:>7}  {:>26}  {:>26}",
            "seed", "points", "tol 0.5mu", "tol 2.0mu"
        );
        let (mut sum_pts, mut sum_a, mut sum_b) = (0usize, 0usize, 0usize);
        for seed in start..start + count {
            let creature = decode_genome(&probe_genome(seed));
            let outline = outline_for(&creature, 120.0, 120.0);
            // Second variant: one pass of 3-point averaging before the
            // fit. Marching-squares contours are grid-quantized, so they
            // arrive as staircases rather than as a smooth curve sampled
            // densely; a fitter cannot collapse a staircase without
            // exceeding tolerance at every step.
            let smoothed: Vec<(f64, f64)> = (0..outline.len())
                .map(|i| {
                    let p = outline[(i + outline.len() - 1) % outline.len()];
                    let c = outline[i];
                    let n = outline[(i + 1) % outline.len()];
                    ((p.0 + 2.0 * c.0 + n.0) / 4.0, (p.1 + 2.0 * c.1 + n.1) / 4.0)
                })
                .collect();
            let mut row = format!("{seed:>6}  {:>7}", outline.len());
            for (slot, tol) in [(0, 0.005_f64), (1, 0.02_f64)] {
                // The outline is in canvas units and the render tree
                // stores milliunits, 100 per canvas unit. So these
                // tolerances are 0.5 and 2 milliunits: one below what the
                // tree can represent at all, one at the smallest
                // difference it can express.
                let curves = fit::fit_closed(&outline, tol);
                let dev = fit::deviation(&outline, &curves);
                row.push_str(&format!(
                    "  {:>8} curves {:>6.3}mu dev",
                    curves.len(),
                    dev * 100.0
                ));
                if slot == 0 {
                    sum_a += curves.len();
                } else {
                    sum_b += curves.len();
                }
            }
            let sc = fit::fit_closed(&smoothed, 0.02);
            row.push_str(&format!(
                "  | smoothed {:>4} curves {:>6.3}mu",
                sc.len(),
                fit::deviation(&outline, &sc) * 100.0
            ));
            sum_pts += outline.len();
            println!("{row}");
        }
        println!(
            "\ntotals: {sum_pts} segments -> {sum_a} curves ({:.1}x) at 0.5mu, {sum_b} curves ({:.1}x) at 2mu",
            sum_pts as f64 / sum_a.max(1) as f64,
            sum_pts as f64 / sum_b.max(1) as f64,
        );
        return;
    }

    if let Some(i) = args.iter().position(|a| a == "--check") {
        let n = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(200);
        run_checker(n);
        return;
    }
    // --dna reports the pipeline's real DNA appetite: bytes actually served
    // to the decoder per creature, plus the entropy the vocabulary can carry.
    // Allocation (note 009's region map) and consumption are different
    // numbers, and only the second one shapes a creature.
    if args.iter().any(|a| a == "--dna") {
        let (mut min_b, mut max_b, mut total) = (usize::MAX, 0usize, 0usize);
        let n = 2000u64;
        for seed in 4000..(4000 + n) {
            let c = decode_full_counted(seed).1;
            min_b = min_b.min(c);
            max_b = max_b.max(c);
            total += c;
        }
        println!("consumed per creature over {n} seeds:");
        println!("  min {min_b} bytes ({} bits)", min_b * 8);
        println!("  max {max_b} bytes ({} bits)", max_b * 8);
        println!(
            "  mean {:.1} bytes ({:.0} bits)",
            total as f64 / n as f64,
            total as f64 / n as f64 * 8.0
        );
        return;
    }
    // --review START N: high-fidelity contact sheet at 400px cells, 4 per
    // row. The 240px grid cells hide face-level defects (the tongue and
    // snaggle bugs both survived several sheet reviews and were caught only
    // when someone looked at one creature big). Review resolution has to
    // match the defect scale.
    // --domspread START N renders the SAME seeds twice: the shipped
    // population, where every creature sits at PINNED_DOM, and the one
    // note 009 assumed, where domestication is decoded per creature. The
    // question the open decision turns on is not whether either end of
    // the axis is legible — grid 7's slider settled that — but whether a
    // population spread across it reads as one cast. That needs two
    // populations side by side, which no artefact has ever shown.
    if let Some(i) = args.iter().position(|a| a == "--domspread") {
        let start: u64 = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(5000);
        let n: usize = args.get(i + 2).and_then(|v| v.parse().ok()).unwrap_or(12);
        write_domspread(start, n);
        return;
    }

    if let Some(i) = args.iter().position(|a| a == "--review") {
        let start: u64 = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(5000);
        let n: usize = args.get(i + 2).and_then(|v| v.parse().ok()).unwrap_or(40);
        let cell = 400.0_f64;
        let pad = 8.0;
        let cols = 4usize;
        let rows = n.div_ceil(cols);
        let w = pad + cols as f64 * (cell + pad);
        let h = pad + rows as f64 * (cell + pad);
        let mut svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='{w:.0}' height='{h:.0}' viewBox='0 0 {w:.0} {h:.0}'><rect width='100%' height='100%' fill='#141318'/>"
        );
        for k in 0..n {
            let seed = start + k as u64;
            let c = decode_genome(&probe_genome(seed));
            let col = k % cols;
            let row = k / cols;
            let x0 = pad + col as f64 * (cell + pad);
            let y0 = pad + row as f64 * (cell + pad);
            svg.push_str(&format!(
                "<rect x='{x0:.1}' y='{y0:.1}' width='{cell}' height='{cell}' rx='14' fill='#efe9e4'/>"
            ));
            // draw_full centres on (120, 120); carry the review-cell scale
            // on its primitives so the serialized tree remains flat.
            let s = cell / 240.0;
            svg.push_str(&tree_fragment(
                draw_full(&c, mood::Mood::Neutral, 120.0, 120.0, Generation::LATEST),
                &[
                    render::translate(x0 + cell / 2.0 - 120.0 * s, y0 + cell / 2.0 - 120.0 * s),
                    render::scale(s, s),
                ],
            ));
            svg.push_str(&format!(
                "<text x='{:.1}' y='{:.1}' font-family='monospace' font-size='13' fill='#8f8a96'>#{seed}</text>",
                x0 + 10.0,
                y0 + 20.0,
            ));
        }
        svg.push_str("</svg>");
        std::fs::create_dir_all("/tmp/ava-playground").expect("mkdir");
        std::fs::write("/tmp/ava-playground/review.svg", &svg).expect("write");
        println!("review: {n} creatures from seed {start} -> /tmp/ava-playground/review.svg ({w:.0}x{h:.0})");
        return;
    }
    // --moods SEED..: 7 moods across the given seeds at review resolution.
    // The face block (resting brows) and eye wonk both landed after the last
    // mood check, and both interact with mood: a resting brow competes with
    // the mood brow for the same strokes, and a shrunk eye changes what a
    // lid or squint has to cover. Grid-resolution cells cannot show either.
    // --icons START N: legibility strip at THREE natural sizes (28/36/48).
    // Law 1 says identity survives at icon size, and the strip that proved
    // it predates the face block, eye wonk, the belly rework and the
    // voronoi change — so the law is currently unproven on the vocabulary
    // that actually ships. Rendered at natural size and magnified with
    // nearest-neighbour, because a thumbnailed 400px render is a different
    // picture from a real 28px one.
    // --probe SEED: one canonical line of derived geometry for a seed.
    // The lab's ?probe=SEED emits the same line from the JS twin, so the
    // two renderers can be diffed instead of trusted. Semantic metrics
    // rather than raw SVG, because number formatting differs between the
    // languages and would drown real divergence in noise.
    //
    // This exists because a Rust-only fix (the eyes-only fallback drawing a
    // mouth) sat un-mirrored in the lab and was caught by hand. The lab is
    // what the maintainer reviews; if it drifts from canonical, the review is of the
    // wrong thing — the same family as every other instrument that was true
    // about something else.
    if let Some(i) = args.iter().position(|a| a == "--bench") {
        let seed = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(5000);
        let count = args.get(i + 2).and_then(|v| v.parse().ok()).unwrap_or(30);
        run_bench(seed, count);
        return;
    }

    if let Some(i) = args.iter().position(|a| a == "--probe") {
        let seed: u64 = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(4000);
        let c = decode_genome(&probe_genome(seed));
        let outline = outline_for(&c, 120.0, 120.0);
        let (bx0, by0, bx1, by1) = wild::bbox(&outline);
        let p = place_face(&outline, c.face_style.mouth_w, c.face_style.mouth_d);
        let belly = p
            .map(|pl| belly_patch(&outline, 120.0, belly_top_for(pl.eye_y, pl.r), 0.72).len())
            .unwrap_or(0);
        // Keep the scaling pivot separate from the rendered mouth
        // anchor: face::mouth uses a different, unscaled baseline.
        let mouth_y = p.map(|pl| mouth_y_for(pl.eye_y, pl.r));
        println!(
            "seed={seed} spokes={} exts={} topo={} vor={} pts={} bbox={:.1},{:.1},{:.1},{:.1} face={} mouthy={} belly={}{}",
            c.wb.spokes.len(),
            c.exts.len(),
            c.topology,
            u8::from(c.voronoi),
            outline.len(),
            bx0, by0, bx1, by1,
            match p {
                Some(pl) => format!(
                    "{:.1},{:.1},{:.1},{:.1},{}",
                    pl.fx, pl.eye_y, pl.ipd, pl.r, u8::from(pl.mouth_fits)
                ),
                None => "none".to_owned(),
            },
            match mouth_y {
                Some(v) => format!("{v:.1}"),
                None => "none".to_owned(),
            },
            belly,
            drawn_probe::fields(&c),
        );
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--icons") {
        let start: u64 = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(5000);
        let n: usize = args.get(i + 2).and_then(|v| v.parse().ok()).unwrap_or(12);
        let sizes = [28.0_f64, 36.0, 48.0];
        let pitch = 56.0;
        let w = 8.0 + n as f64 * pitch;
        let h = 8.0 + sizes.iter().map(|s| s + 10.0).sum::<f64>();
        let mut svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='{w:.0}' height='{h:.0}' viewBox='0 0 {w:.0} {h:.0}'><rect width='100%' height='100%' fill='#1c1a20'/>"
        );
        let mut y = 6.0;
        for size in sizes {
            for j in 0..n {
                let c = decode_genome(&probe_genome(start + j as u64));
                let x0 = 6.0 + j as f64 * pitch;
                svg.push_str(&format!(
                    "<rect x='{x0:.1}' y='{y:.1}' width='{size}' height='{size}' rx='{:.1}' fill='#efe9e4'/>",
                    size / 6.0,
                ));
                // draw_full works in a 240-unit cell; scale that tree to `size`.
                let k = size / 240.0;
                svg.push_str(&tree_fragment(
                    draw_full(&c, mood::Mood::Neutral, 120.0, 120.0, Generation::LATEST),
                    &[
                        render::translate(x0 + size / 2.0 - 120.0 * k, y + size / 2.0 - 120.0 * k),
                        render::scale(k, k),
                    ],
                ));
            }
            y += size + 10.0;
        }
        svg.push_str("</svg>");
        std::fs::create_dir_all("/tmp/ava-playground").expect("mkdir");
        std::fs::write("/tmp/ava-playground/icons.svg", &svg).expect("write");
        println!("icons: {n} creatures from {start} at 28/36/48px -> /tmp/ava-playground/icons.svg ({w:.0}x{h:.0})");
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--moods") {
        let seeds: Vec<u64> = args[i + 1..]
            .iter()
            .filter_map(|v| v.parse().ok())
            .take(5)
            .collect();
        let seeds = if seeds.is_empty() {
            vec![5000, 5003, 5009, 5011]
        } else {
            seeds
        };
        let cell = 330.0_f64;
        let pad = 6.0;
        let moods = mood::ALL;
        let w = pad + moods.len() as f64 * (cell + pad);
        let h = pad + seeds.len() as f64 * (cell + pad) + 22.0;
        let mut svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='{w:.0}' height='{h:.0}' viewBox='0 0 {w:.0} {h:.0}'><rect width='100%' height='100%' fill='#141318'/>"
        );
        for (mi, m) in moods.iter().enumerate() {
            svg.push_str(&format!(
                "<text x='{:.1}' y='16' font-family='monospace' font-size='13' fill='#8f8a96'>{}</text>",
                pad + mi as f64 * (cell + pad) + 8.0,
                mood::name(*m),
            ));
        }
        for (si, &seed) in seeds.iter().enumerate() {
            let c = decode_genome(&probe_genome(seed));
            for (mi, m) in moods.iter().enumerate() {
                let x0 = pad + mi as f64 * (cell + pad);
                let y0 = 22.0 + pad + si as f64 * (cell + pad);
                svg.push_str(&format!(
                    "<rect x='{x0:.1}' y='{y0:.1}' width='{cell}' height='{cell}' rx='12' fill='#efe9e4'/>"
                ));
                let s = cell / 240.0;
                svg.push_str(&tree_fragment(
                    draw_full(&c, *m, 120.0, 120.0, Generation::LATEST),
                    &[
                        render::translate(x0 + cell / 2.0 - 120.0 * s, y0 + cell / 2.0 - 120.0 * s),
                        render::scale(s, s),
                    ],
                ));
                svg.push_str(&format!(
                    "<text x='{:.1}' y='{:.1}' font-family='monospace' font-size='11' fill='#8f8a96'>#{seed}</text>",
                    x0 + 8.0,
                    y0 + 16.0,
                ));
            }
        }
        svg.push_str("</svg>");
        std::fs::create_dir_all("/tmp/ava-playground").expect("mkdir");
        std::fs::write("/tmp/ava-playground/moods.svg", &svg).expect("write");
        println!(
            "moods: {} seeds x 7 -> /tmp/ava-playground/moods.svg ({w:.0}x{h:.0})",
            seeds.len()
        );
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--inspect") {
        let seed: u64 = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(4000);
        let c = decode_genome(&probe_genome(seed));
        let outline = outline_for(&c, 120.0, 120.0);
        let (bx0, by0, bx1, by1) = wild::bbox(&outline);
        println!(
            "seed {seed}: topology={} exts={} outline pts={} bbox=({:.1},{:.1})-({:.1},{:.1}) h={:.1}",
            c.topology,
            c.exts.len(),
            outline.len(),
            bx0, by0, bx1, by1,
            by1 - by0,
        );
        match place_face(&outline, c.face_style.mouth_w, c.face_style.mouth_d) {
            Some(p) => println!(
                "  placed: fx={:.1} eye_y={:.1} ipd={:.1} r={:.1}",
                p.fx, p.eye_y, p.ipd, p.r
            ),
            None => println!("  NO PLACEMENT"),
        }
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
            let creature = decode_genome(&probe_genome(seed));
            let cx = PAD + c as f64 * (CELL + PAD) + CELL / 2.0;
            let cy = PAD + r as f64 * (CELL + PAD) + CELL / 2.0 + 28.0;
            let tag = match creature.topology {
                0 => "M",
                1 => "T",
                _ => "A",
            };
            let v = if creature.voronoi { "+v" } else { "" };
            cell_bg(&mut svg, cx, cy, &format!("#{seed} {tag}{v}"));
            svg.push_str(&tree_fragment(
                draw_full(
                    &creature,
                    mood::Mood::Neutral,
                    120.0,
                    120.0,
                    Generation::LATEST,
                ),
                &[render::translate(cx - 120.0, cy - 120.0)],
            ));
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
        let creature = decode_genome(&probe_genome(seed));
        for (c, &m) in mood::ALL.iter().enumerate() {
            let cx = PAD + c as f64 * (CELL + PAD) + CELL / 2.0;
            let cy = PAD + r as f64 * (CELL + PAD) + CELL / 2.0 + 28.0;
            cell_bg(&mut svg2, cx, cy, &format!("#{seed} {}", mood::name(m)));
            svg2.push_str(&tree_fragment(
                draw_full(&creature, m, 120.0, 120.0, Generation::LATEST),
                &[render::translate(cx - 120.0, cy - 120.0)],
            ));
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
        let creature = decode_genome(&probe_genome(seed));
        let x0 = 4.0 + i as f64 * 44.0;
        svg3.push_str(&format!(
            "<rect x='{x0:.1}' y='4' width='36' height='36' rx='6' fill='#efe9e4'/>",
        ));
        svg3.push_str(&tree_fragment(
            draw_full(
                &creature,
                mood::Mood::Neutral,
                120.0,
                120.0,
                Generation::LATEST,
            ),
            &[
                render::translate(x0 + 18.0 - 120.0 * k, 22.0 - 120.0 * k),
                render::scale(k, k),
            ],
        ));
    }
    svg3.push_str("</svg>");
    std::fs::write("/tmp/ava-playground/grid10-strip.svg", &svg3).expect("write");

    println!("grid10 + moods + strip -> /tmp/ava-playground/");
}
