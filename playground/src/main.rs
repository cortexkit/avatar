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
    belly_variant: u8,
    /// Per-creature face variation. Measured against TOG, the vocabulary
    /// spent 16 bits on the face against TOG's ~500, which meant two
    /// creatures rolling the same eye+mouth archetype had *identical*
    /// faces — only 30 faces existed in the whole population. These are
    /// the visible half of TOG's face DNA (proportion and posture), not
    /// its invisible half (iris saturation, lash removal arcs), and they
    /// are all expressed as transforms bounded so that no value can break
    /// a placement invariant.
    face_style: FaceStyle,
}

#[derive(Clone, Copy)]
struct FaceStyle {
    /// >1 taller, <1 wider. Applied as min-scaling on both axes so an eye
    /// never grows past the placed radius the invariants were proved for.
    eye_aspect: f64,
    /// Signed one-eye shrink (negative = left, positive = right). Shrinking
    /// cannot violate clearance or separation, while enlarging could. Live:
    /// the archetype renderers take a per-side radius, so mismatched eyes
    /// are available to every archetype instead of only to `Uneven`.
    eye_wonk: f64,
    /// Mouth scale; feeds the placement search's mouth-fit test, so a wide
    /// mouth changes where the face can sit rather than escaping the body.
    mouth_w: f64,
    mouth_d: f64,
    /// Resting brow: 0 none, 1 flat, 2 raised, 3 angled. Mood brows still
    /// override, so this is the creature's face at rest.
    brow: u8,
}

fn decode_full(seed: u64) -> Creature {
    decode_full_counted(seed).0
}

/// Decode plus the bytes it actually consumed. One decode path, so a DNA
/// measurement can never drift from the decoder it claims to measure.
fn decode_full_counted(seed: u64) -> (Creature, usize) {
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
    // Trailing read on purpose: appending keeps every earlier field's bytes
    // stable for a given seed.
    let belly_variant = b.byte() % 4;
    // Trailing block: appended so every earlier field's bytes stay put.
    let face_style = FaceStyle {
        eye_aspect: b.lin(0.80, 1.28),
        // SIGNED: negative shrinks the left eye, positive the right, so the
        // smaller eye is not always on the same side fleet-wide.
        eye_wonk: b.lin(-0.20, 0.20),
        mouth_w: b.lin(0.75, 1.30),
        mouth_d: b.lin(0.70, 1.35),
        // Most creatures rest brow-less; a resting brow is a strong
        // personality mark and reads as permanent attitude.
        brow: match b.byte() % 8 {
            0 | 1 => 1,
            2 => 2,
            3 => 3,
            _ => 0,
        },
    };
    let creature = Creature {
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
        belly_variant,
        face_style,
    };
    (creature, b.consumed())
}

/// Belly patch: the body outline scaled toward a low anchor, then clipped
/// to the region below `top_y`. Following the silhouette keeps the patch
/// organic on any body shape, and the inward scale guarantees it never
/// touches the outline.
fn belly_patch(outline: &[(f64, f64)], top_y: f64, scale: f64) -> Vec<(f64, f64)> {
    let (_, _, _, max_y) = wild::bbox(outline);
    let bottom = max_y - (max_y - top_y) * 0.06;
    if bottom <= top_y {
        return Vec::new();
    }
    // Walk the body's own WIDTH PROFILE down the belly band — the blob
    // generator's approach, generalised to wild silhouettes. Three earlier
    // shapes failed because they were foreign geometry laid over the body: a
    // fixed ellipse (a ball on lumpy bodies, a smear across split legs), a
    // horizontal clip (flat top edge), and a wide ellipse clip (still flat).
    // A profile walk borrows every horizontal from the creature, and the
    // sqrt ramp closes the top to a point so there is no top edge at all.
    // extent_at returns the widest span at each height, so on a split body
    // the patch follows one mass instead of bridging the gap.
    let steps = 40;
    let mut left = Vec::with_capacity(steps + 1);
    let mut right = Vec::with_capacity(steps + 1);
    // Follow ONE mass down the band: start on the widest span, then at each
    // step take the span whose centre is nearest the previous one. Taking
    // the widest at every height lets the walk hop between legs, and the
    // resulting zig-zag polygon renders as horizontal striping.
    let mut anchor: Option<f64> = None;
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let y = top_y + (bottom - top_y) * t;
        let spans = wild::extents_at(outline, y);
        if spans.is_empty() {
            continue;
        }
        let (l, r) = match anchor {
            None => *spans
                .iter()
                .max_by(|a, b| (a.1 - a.0).partial_cmp(&(b.1 - b.0)).unwrap())
                .unwrap(),
            Some(prev) => *spans
                .iter()
                .min_by(|a, b| {
                    let da = ((a.0 + a.1) / 2.0 - prev).abs();
                    let db = ((b.0 + b.1) / 2.0 - prev).abs();
                    da.partial_cmp(&db).unwrap()
                })
                .unwrap(),
        };
        let cx = (l + r) / 2.0;
        anchor = Some(cx);
        // sqrt ramp: rounded crown. Cubic pull-in at the base keeps the
        // patch clear of the silhouette where the body narrows to feet.
        let hw = (r - l) / 2.0 * scale * t.sqrt() * (1.0 - 0.3 * t * t * t);
        left.push((cx - hw, y));
        right.push((cx + hw, y));
    }
    if left.len() < 3 {
        return Vec::new();
    }
    left.extend(right.into_iter().rev());
    // Suppression, TOG's rule: a feature too small to read should not be
    // drawn. Kept deliberately tight — an earlier, broader version deleted
    // perfectly good bellies (seed 5000) while chasing a defect that turned
    // out to be a colour problem, not a size one.
    let (bx0, by0, bx1, by1) = wild::bbox(&left);
    let (fx0, fy0, fx1, fy1) = wild::bbox(outline);
    if bx1 - bx0 < (fx1 - fx0) * 0.10 || by1 - by0 < (fy1 - fy0) * 0.08 {
        return Vec::new();
    }
    left
}

/// Monotonic id source for SVG defs. Local ids in one document must be
/// unique; deriving them from content invites collisions that render as
/// silent visual corruption rather than as errors.
fn next_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Copy)]
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
fn place_face(outline: &[(f64, f64)], mouth_w: f64, mouth_d: f64) -> Option<FacePlacement> {
    let (_, min_y, _, max_y) = wild::bbox(outline);
    let h = max_y - min_y;
    let poly = merge::Poly::new(outline, 2);
    // The eye disc must clear the outline by 22% of its radius.
    const MARGIN: f64 = 1.22;
    let r_cap = h * 0.085 * 1.15;
    let r_floor = h * 0.028;
    // Two-tier search (both rules mined from TOG's face engine):
    // 1. face.tsx places the face only where the MOUTH also fits below it
    //    (its mouth-finder repositions centerY above the found mouth rect) —
    //    so mouth-fit joins the scoring here: the drawn mouth's extremes
    //    (widest family reaches ±0.8r, deepest +0.62r below the mouth line
    //    at eye_y + 2.71r) must sit inside the silhouette.
    // 2. face.tsx clamps the face UP to a minimum rather than skipping it —
    //    a creature must ALWAYS have a face, so eyes-only placement is the
    //    guaranteed fallback when no candidate also fits a mouth.
    let mut best: Option<(f64, FacePlacement)> = None;
    let mut best_eyes_only: Option<(f64, FacePlacement)> = None;
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
            let candidate = FacePlacement {
                fx,
                eye_y: y,
                half_w: span / 2.0,
                ipd: ex,
                r,
            };
            if best_eyes_only.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                best_eyes_only = Some((score, candidate));
            }
            // Mouth extents scale with the creature's own mouth size, so a
            // wide mouth moves the face to where it fits instead of
            // overflowing the silhouette.
            let mouth_y = y + r * 2.71;
            let half = r * 0.8 * mouth_w;
            let deep = r * 0.62 * mouth_d;
            let mouth_ok = poly.sdf(fx - half, mouth_y) < -1.0
                && poly.sdf(fx + half, mouth_y) < -1.0
                && poly.sdf(fx, mouth_y + deep) < -1.0;
            if mouth_ok && best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                best = Some((score, candidate));
            }
        }
    }
    best.or(best_eyes_only).map(|(_, p)| p)
}

fn draw_full(c: &Creature, m: mood::Mood, cx: f64, cy: f64) -> String {
    let colors = palette::resolve(c.recipe, c.hue);
    let dom = 0.32;
    let spec = mood::spec(m);

    let raw = wild::build_raw(&c.wb, dom);
    let mean_r = c.wb.spokes.iter().map(|s| s.r).sum::<f64>() / c.wb.spokes.len() as f64;
    // Root radius measured on the ACTUAL post-domestication outline, not on
    // the nearest raw spoke: smoothing moves the curve inward, and roots
    // that trusted the raw radius floated outside the body (seed 5012's
    // detached limbs).
    let ring_r_at =
        |theta: f64| -> f64 { wild::radius_along(&raw.pts, theta).unwrap_or(mean_r) };

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
    // Unique per DRAW, from a process counter. url(#) resolves
    // document-wide to the first match, so any id derived from creature
    // content or cell position collides as soon as two draws share it:
    // hue+mood collided across same-hue cells, and position collided the
    // moment the review renderer drew every creature at local (120,120)
    // and moved them with an outer transform — 40 creatures clipped by the
    // first one's silhouette. A counter cannot collide by construction,
    // which is the only property this id actually needs.
    let clip_id = format!("c{}", next_id());
    s.push_str(&format!(
        "<defs><clipPath id='{clip_id}'><path d='{d}'/></clipPath></defs><path d='{d}' fill='{}' stroke='{}' stroke-width='1.5'/>",
        colors.body, colors.outline,
    ));
    s.push_str(&format!("<g clip-path='url(#{clip_id})'>"));

    // Placement first: the voronoi mask needs to know where the face lives.
    let placement = place_face(&outline, c.face_style.mouth_w, c.face_style.mouth_d);

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
                surface::cell_path(&cell, 0.8),
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
        // Belly follows the BODY, not an ellipse: the outline shrunk toward
        // a lower anchor and cut off above the belly line. A fixed ellipse
        // fits only ellipse-shaped bodies — it read as a ball stuck on a
        // lumpy body (seed 5001) and as a smear across the gap between two
        // legs (seed 5022). Shrinking the silhouette itself cannot produce
        // either: the patch is always the creature's own shape.
        let belly_top = p.eye_y + p.r * 3.4;
        let belly = belly_patch(&outline, belly_top, 0.72);
        if belly.len() >= 3 {
            let mut d = String::new();
            for (i, q) in belly.iter().enumerate() {
                d.push_str(&format!(
                    "{}{:.1} {:.1} ",
                    if i == 0 { 'M' } else { 'L' },
                    q.0,
                    q.1
                ));
            }
            d.push('Z');
            s.push_str(&format!(
                "<path d='{d}' fill='{}'/>",
                palette::belly_fill(c.recipe, c.hue, c.belly_variant),
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
            c.face_style.eye_wonk,
        );
        // Per-creature eye shape, bounded so the invariants still hold:
        // both axis factors are <= 1, so an eye can only shrink inside the
        // placed radius that clearance and separation were proved against.
        // Aspect is applied to the pair; wonk shrinks the right eye alone,
        // which is the ugly-cute lever generalised beyond the one archetype
        // that had it baked in.
        let st = c.face_style;
        let (ax, ay) = if st.eye_aspect >= 1.0 {
            (1.0 / st.eye_aspect, 1.0)
        } else {
            (1.0, st.eye_aspect)
        };
        let sx = ax * spec.eye_scale;
        let sy = ay * spec.eye_scale;
        if (sx - 1.0).abs() > 0.005 || (sy - 1.0).abs() > 0.005 {
            s.push_str(&format!(
                "<g transform='translate({:.1} {:.1}) scale({sx:.3} {sy:.3}) translate({:.1} {:.1})'>{eyes_svg}</g>",
                p.fx, f.y, -p.fx, -f.y,
            ));
        } else {
            s.push_str(&eyes_svg);
        }
        // eye_wonk decodes but stays dormant: expressing it needs a per-eye
        // radius inside the archetype renderers, and the obvious shortcut
        // (redrawing the pair scaled about one eye) draws the other eye
        // twice. Dormant-field law — the DNA is reserved and the feature
        // lights up when the renderers take a per-eye scale.

        // Mood layers per eye — ARCHETYPE-AWARE (Ufuk's lab find: lids and
        // squints are disc-eye concepts; stacked on line eyes they read as
        // line noise). The spec stays semantic; each archetype decides what
        // expresses it:
        //  - HappyArc is already-closed line art: brows alone carry mood —
        //    a lid over a closed eye is another arc, a squint another line.
        //  - HalfLid has a built-in lid: only mood lids that close FURTHER
        //    than the archetype's own (sleepy-scale, >0.45) add anything.
        //  - Uneven's discs differ per eye (1.25r / 0.62r): overlays must
        //    track each disc's radius and center or they float as stray
        //    curves.
        let (do_squint, do_lid) = match c.eyes {
            face::EyeKind::HappyArc => (false, false),
            face::EyeKind::HalfLid => (false, spec.lid > 0.45),
            _ => (true, true),
        };
        for (x, side) in [(f.lx, -1.0), (f.rx, 1.0)] {
            // Overlays land on the radius the archetype actually DREW,
            // including the wonk shrink; an overlay sized for the full
            // radius floats free of a shrunk eye.
            let (er, ey) = match c.eyes {
                face::EyeKind::Uneven if side < 0.0 => (f.r * 1.25, f.y),
                face::EyeKind::Uneven => (f.r * 0.62, f.y + f.r * 0.15),
                _ => (
                    f.r * face::wonk_factor(c.eyes, c.face_style.eye_wonk, side > 0.0),
                    f.y,
                ),
            };
            if do_squint {
                s.push_str(&mood::squint_overlay(x, ey, er, spec.squint, &colors.body, &colors.outline));
            }
            if do_lid {
                s.push_str(&mood::lid_overlay(
                    x,
                    ey,
                    er,
                    spec.lid,
                    spec.lid_angle * side,
                    &colors.body,
                    &colors.outline,
                ));
            }
            // Mood brows win; otherwise the creature's RESTING brow shows.
            // A permanent brow reads as standing attitude rather than a
            // reaction, which is exactly the personality range the face
            // vocabulary was missing.
            let brows = if spec.brows == mood::Brows::None {
                match st.brow {
                    1 => mood::Brows::Focus,
                    2 => mood::Brows::Raised,
                    3 => mood::Brows::Knit,
                    _ => mood::Brows::None,
                }
            } else {
                spec.brows
            };
            s.push_str(&mood::brow(brows, x, f.y, f.r, side, &colors.ink));
        }

        // Mouth: mood override or the pack's own.
        let mouth_kind = match spec.mouth {
            mood::MouthMood::Keep => c.mouth,
            mood::MouthMood::Grin => face::MouthKind::OpenHappy,
            mood::MouthMood::O => face::MouthKind::OMouth,
            mood::MouthMood::Pressed => face::MouthKind::Flat,
            mood::MouthMood::Down => face::MouthKind::DownArc,
        };
        let mouth_svg =
            face::mouth_inked(mouth_kind, &shim, p.fx, face_cy, &colors.outline, &colors.ink);
        // Per-creature mouth proportion. The placement search already tested
        // fit at exactly these factors, so a wide mouth was accounted for
        // when the face was positioned rather than overflowing afterwards.
        let my = face_cy + shim.height * 0.15;
        if (st.mouth_w - 1.0).abs() > 0.02 || (st.mouth_d - 1.0).abs() > 0.02 {
            s.push_str(&format!(
                "<g transform='translate({:.1} {my:.1}) scale({:.3} {:.3}) translate({:.1} {:.1})'>{mouth_svg}</g>",
                p.fx, st.mouth_w, st.mouth_d, -p.fx, -my,
            ));
        } else {
            s.push_str(&mouth_svg);
        }
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
    // Must match draw_full exactly — a checker that rebuilds the geometry
    // differently is measuring a creature nobody renders.
    let ring_r_at =
        |theta: f64| -> f64 { wild::radius_along(&raw.pts, theta).unwrap_or(mean_r) };
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
        let Some(p) = place_face(&outline, c.face_style.mouth_w, c.face_style.mouth_d) else {
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
        // Mouth-fit (TOG rule): drawn mouth extremes inside the silhouette,
        // at the creature's own mouth scale.
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
        if mouth_breach > 0.0 {
            println!("seed {seed}: mouth breaches outline by {:.2}", mouth_breach);
            violations += 1;
        }
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
        println!("  mean {:.1} bytes ({:.0} bits)", total as f64 / n as f64, total as f64 / n as f64 * 8.0);
        return;
    }
    // --review START N: high-fidelity contact sheet at 400px cells, 4 per
    // row. The 240px grid cells hide face-level defects (the tongue and
    // snaggle bugs both survived several sheet reviews and were caught only
    // when someone looked at one creature big). Review resolution has to
    // match the defect scale.
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
            let c = decode_full(seed);
            let col = k % cols;
            let row = k / cols;
            let x0 = pad + col as f64 * (cell + pad);
            let y0 = pad + row as f64 * (cell + pad);
            svg.push_str(&format!(
                "<rect x='{x0:.1}' y='{y0:.1}' width='{cell}' height='{cell}' rx='14' fill='#efe9e4'/>"
            ));
            // draw_full centres on (cx, cy) with its own 172-unit scale, so
            // scale the whole group up to fill the review cell.
            let s = cell / 240.0;
            svg.push_str(&format!(
                "<g transform='translate({:.1} {:.1}) scale({s:.4})'>{}</g>",
                x0 + cell / 2.0 - 120.0 * s,
                y0 + cell / 2.0 - 120.0 * s,
                draw_full(&c, mood::Mood::Neutral, 120.0, 120.0),
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
            let c = decode_full(seed);
            for (mi, m) in moods.iter().enumerate() {
                let x0 = pad + mi as f64 * (cell + pad);
                let y0 = 22.0 + pad + si as f64 * (cell + pad);
                svg.push_str(&format!(
                    "<rect x='{x0:.1}' y='{y0:.1}' width='{cell}' height='{cell}' rx='12' fill='#efe9e4'/>"
                ));
                let s = cell / 240.0;
                svg.push_str(&format!(
                    "<g transform='translate({:.1} {:.1}) scale({s:.4})'>{}</g>",
                    x0 + cell / 2.0 - 120.0 * s,
                    y0 + cell / 2.0 - 120.0 * s,
                    draw_full(&c, *m, 120.0, 120.0),
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
        println!("moods: {} seeds x 7 -> /tmp/ava-playground/moods.svg ({w:.0}x{h:.0})", seeds.len());
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--inspect") {
        let seed: u64 = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(4000);
        let c = decode_full(seed);
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
