//! The single wild generator, shared by stored-avatar dispatch and the playground.

pub mod body;
pub mod face;
pub mod genome;
pub mod math;
pub mod merge;
pub mod mood;
pub mod palette;
pub mod render;
#[path = "wild.rs"]
pub mod shape;
pub mod surface;
use shape as wild;

use crate::{Effect, GradientStop, Paint, Primitive, RenderTree, StrokeCap, StrokeJoin, Q32};
pub use genome::{probe_genome, GENOME_LEN};

#[cfg(test)]
mod decoder_tests;
#[cfg(test)]
mod generation_tests;

/// Which shipped generation of the wild generator draws a creature.
///
/// A stored `creature.wild` row names its version, and it must render at that
/// version forever: a fix that changed what an existing version draws would
/// silently change an agent's face. So each fix is a new variant here, and
/// every older variant keeps drawing exactly what it drew when it shipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Generation {
    /// `creature.wild` version 1. Its mood lids and squints are drawn at the
    /// unscaled eye centres, while the eyes themselves are scaled about the
    /// face axis by the creature's eye aspect and the mood's eye scale. On a
    /// creature whose eyes are scaled, the lid or squint therefore sits beside
    /// the eye. Kept unchanged because rows stored at version 1 must keep the
    /// creature their agent already has.
    V1,
    /// `creature.wild` version 2. Identical to version 1 except that the lid
    /// and squint overlays get the same face-axis transform as the eyes, so
    /// they land on the eye that was actually drawn.
    V2,
}

impl Generation {
    /// The generation new rows are minted at and the playground reviews.
    pub const LATEST: Generation = Generation::V2;
}

pub(crate) fn render_tree(
    bytes: &[u8; GENOME_LEN],
    mood: crate::Mood,
    generation: Generation,
) -> RenderTree {
    RenderTree {
        canvas: 240,
        background: None,
        primitives: draw_full(&decode_genome(bytes), mood, 120.0, 120.0, generation),
    }
}
/// Everything one genome decodes to on the full pipeline.
pub struct Creature {
    pub wb: wild::WildBody,
    pub exts: Vec<wild::Extremity>,
    pub ext_fills: Vec<u8>,
    pub sites: Vec<surface::Site>,
    pub recipe: usize,
    pub hue: f64,
    pub eyes: face::EyeKind,
    pub mouth: face::MouthKind,
    /// 0 = merged silhouette, 1 = layered tone fills, 2 = layered accent.
    pub topology: u8,
    /// The wild↔tame axis is pinned at 0.32, not decoded. Keeping it here lets
    /// the playground compare pinned and varied smoothing without a new renderer.
    pub dom: f64,
    pub voronoi: bool,
    pub belly_variant: u8,
    /// Bounded proportions and posture distinguish creatures that share the
    /// same eye and mouth archetypes without enlarging their placed features.
    pub face_style: FaceStyle,
}

#[derive(Clone, Copy)]
pub struct FaceStyle {
    /// Above 1 is taller, below 1 is wider. Applied as min-scaling on both
    /// axes so an eye never grows past the placed radius the invariants
    /// were proved for.
    pub eye_aspect: f64,
    /// Signed one-eye shrink (negative = left, positive = right). Shrinking
    /// cannot violate clearance or separation, while enlarging could. Live:
    /// the archetype renderers take a per-side radius, so mismatched eyes
    /// are available to every archetype instead of only to `Uneven`.
    pub eye_wonk: f64,
    /// Mouth scale; feeds the placement search's mouth-fit test, so a wide
    /// mouth changes where the face can sit rather than escaping the body.
    pub mouth_w: f64,
    pub mouth_d: f64,
    /// Resting brow: 0 none, 1 flat, 2 raised, 3 angled. Mood brows still
    /// override, so this is the creature's face at rest.
    pub brow: u8,
}

pub fn decode_full(seed: u64) -> Creature {
    decode_full_counted(seed).0
}

/// Report active genome bytes, excluding generated site randomness and padding.
pub fn decode_full_counted(seed: u64) -> (Creature, usize) {
    let creature = decode_genome(&genome::probe_genome(seed));
    let consumed = 1 + 3 * creature.wb.spokes.len() + 2 + 6 * creature.exts.len() + 4 + 7 + 4;
    (creature, consumed)
}

pub fn decode_genome(bytes: &[u8; genome::GENOME_LEN]) -> Creature {
    let wb = wild::decode(&bytes[..64]);
    let exts = wild::decode_extremities(&bytes[64..160]);
    let sites = surface::decode_sites(&bytes[240..304]);
    let rsel = bytes[160] % 8;
    let recipe = match rsel {
        0..=2 => 0,
        3..=4 => 2,
        5..=6 => 4,
        _ => 3,
    };
    let hue = (bytes[161] as f64 / 255.0 + bytes[162] as f64 / 65535.0) * 360.0;
    let eyes = match bytes[176] % 5 {
        0 => face::EyeKind::DotShine,
        1 => face::EyeKind::Lidded,
        2 => face::EyeKind::HappyArc,
        3 => face::EyeKind::Uneven,
        _ => face::EyeKind::HalfLid,
    };
    let mouth = match bytes[179] % 6 {
        0 => face::MouthKind::Arc,
        1 => face::MouthKind::CatW,
        2 => face::MouthKind::OpenHappy,
        3 => face::MouthKind::Snaggle,
        4 => face::MouthKind::Flat,
        _ => face::MouthKind::Fangs,
    };
    let ext_fills: Vec<u8> = (0..exts.len())
        .map(|i| bytes[64 + 4 + 10 * i + 5] % 3)
        .collect();
    // Topology: merged is a real character class, weighted 1/3.
    let topology = bytes[65] % 3;
    let dom = PINNED_DOM;
    // Voronoi mottle on ~40% of the population.
    let voronoi = bytes[240] % 5 < 2;
    let belly_variant = bytes[163] % 4;
    let face_style = FaceStyle {
        eye_aspect: genome::lin(bytes[177], 0.80, 1.28),
        // SIGNED: negative shrinks the left eye, positive the right, so the
        // smaller eye is not always on the same side fleet-wide.
        eye_wonk: genome::lin(bytes[178], -0.20, 0.20),
        mouth_w: genome::lin(bytes[180], 0.75, 1.30),
        mouth_d: genome::lin(bytes[181], 0.70, 1.35),
        // Most creatures rest brow-less; a resting brow is a strong
        // personality mark and reads as permanent attitude.
        brow: match bytes[182] % 8 {
            0 | 1 => 1,
            2 => 2,
            3 => 3,
            _ => 0,
        },
    };
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
        dom,
        voronoi,
        belly_variant,
        face_style,
    }
}

/// Belly patch: the body's width profile walked down from `top_y`, centred
/// on the mirror axis `axis_x`. Following the silhouette keeps the patch
/// organic on any body shape, and the inward scale guarantees it never
/// touches the outline.
pub fn belly_patch(outline: &[(f64, f64)], axis_x: f64, top_y: f64, scale: f64) -> Vec<(f64, f64)> {
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
    //
    // The walk stays on the span that contains the mirror axis and stops
    // where none does, which is where the body splits into legs. Every body
    // is mirror-symmetric, so its belly must be too. An earlier walk followed
    // "the span nearest the previous one" below the split; the two legs are
    // mirror images, so that was an exact tie settled by floating-point
    // noise, and the belly ran down one leg of a symmetric creature. Bridging
    // the gap instead paints belly over background, which is the smear the
    // profile walk replaced.
    let on_axis = |y: f64| {
        wild::extents_at(outline, y)
            .into_iter()
            .find(|&(l, r)| l <= axis_x && axis_x <= r)
    };
    // End the band where the legs split, then fit the whole ramp into what
    // remains. Stopping the walk mid-ramp instead would leave a wide flat
    // cut across the crotch, and on short torsos a sliver.
    let probes = 80;
    let mut end = top_y;
    for i in 0..=probes {
        let y = top_y + (bottom - top_y) * (i as f64 / probes as f64);
        if on_axis(y).is_none() {
            break;
        }
        end = y;
    }
    let bottom = end;
    if bottom <= top_y {
        return Vec::new();
    }
    let steps = 40;
    let mut left = Vec::with_capacity(steps + 1);
    let mut right = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let y = top_y + (bottom - top_y) * t;
        let Some((l, r)) = on_axis(y) else {
            break;
        };
        // The narrower side bounds the half-width, so the patch stays inside
        // the body on both sides even when rounding leaves the span a hair
        // off-centre.
        let half = (axis_x - l).min(r - axis_x);
        // sqrt ramp: rounded crown. Cubic pull-in at the base keeps the
        // patch clear of the silhouette where the body narrows to feet.
        let hw = half * scale * t.sqrt() * (1.0 - 0.3 * t * t * t);
        left.push((axis_x - hw, y));
        right.push((axis_x + hw, y));
    }
    if left.len() < 3 {
        return Vec::new();
    }
    left.extend(right.into_iter().rev());
    // Suppress patches too small to read. Tight thresholds preserve useful
    // belly shapes rather than hiding colour-contrast problems by removing them.
    let (bx0, by0, bx1, by1) = wild::bbox(&left);
    let (fx0, fy0, fx1, fy1) = wild::bbox(outline);
    if bx1 - bx0 < (fx1 - fx0) * 0.10 || by1 - by0 < (fy1 - fy0) * 0.08 {
        return Vec::new();
    }
    left
}

#[derive(Clone, Copy)]
pub struct FacePlacement {
    pub fx: f64,
    pub eye_y: f64,
    pub half_w: f64,
    /// Absolute half inter-pupil distance CHOSEN by the search — the fit may
    /// have pulled the eyes inward off thin side masses, so consumers must
    /// not re-derive spacing from half_w.
    pub ipd: f64,
    pub r: f64,
    /// Whether the winning candidate also fits a mouth. False only on the
    /// fallback path, where the mouth is then NOT drawn.
    pub mouth_fits: bool,
}

/// Fixed smoothing strength for stored wild genomes; the reserved byte is unread.
pub const PINNED_DOM: f64 = 0.32;

/// Drawn mouth height, also used by the playground's Rust/JavaScript comparison.
/// The face box sits at eye_y + 0.08h and the mouth at +0.15h within it,
/// where h is r/0.085. Placement tests a slightly lower 2.71r conservatively.
pub fn mouth_y_for(eye_y: f64, r: f64) -> f64 {
    eye_y + (r / 0.085) * 0.23
}

/// Belly-patch start shared by drawing and the playground's geometry probe,
/// so a comparison never measures an independently recomputed placement.
pub fn belly_top_for(eye_y: f64, r: f64) -> f64 {
    eye_y + r * 3.4
}

/// SDF-verified face placement. The widest-scanline approach checked fit
/// only HORIZONTALLY at one y — eye discs crossed the silhouette vertically
/// on notched crowns and landed outside thin masses. Each candidate is
/// scored by the signed distance of both
/// eye centers to the outline: an eye exists only where the body has
/// interior DEPTH for it. Search over height band x eye spacing; radius is
/// what the interior affords, capped by the cute proportions.
pub fn place_face(outline: &[(f64, f64)], mouth_w: f64, mouth_d: f64) -> Option<FacePlacement> {
    let (_, min_y, _, max_y) = wild::bbox(outline);
    let h = max_y - min_y;
    let poly = merge::Poly::new(outline, 2);
    // The eye disc must clear the outline by 22% of its radius.
    const MARGIN: f64 = 1.22;
    let r_cap = h * 0.085 * 1.15;
    let r_floor = h * 0.028;
    // Prefer candidates fitting both eyes and mouth. The widest mouth reaches
    // ±0.8r and the deepest reaches 0.62r below the tested line at 2.71r.
    // If none fits a mouth, retain the best eyes-only placement and draw no
    // mouth: omitting it is safer than emitting geometry outside the body.
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
            // Eyes must never touch each other: the
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
                mouth_fits: true,
            };
            if best_eyes_only
                .as_ref()
                .map(|(s, _)| score > *s)
                .unwrap_or(true)
            {
                best_eyes_only = Some((
                    score,
                    FacePlacement {
                        mouth_fits: false,
                        ..candidate
                    },
                ));
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

pub fn draw_full(
    c: &Creature,
    m: crate::Mood,
    cx: f64,
    cy: f64,
    generation: Generation,
) -> Vec<Primitive> {
    let colors = palette::resolve(c.recipe, c.hue);
    let dom = c.dom;
    let spec = mood::spec(m);

    let raw = wild::build_raw(&c.wb, dom);
    let mean_r = c.wb.spokes.iter().map(|s| s.r).sum::<f64>() / c.wb.spokes.len() as f64;
    // Root radius measured on the ACTUAL post-domestication outline, not on
    // the nearest raw spoke: smoothing moves the curve inward, and roots
    // that trusted the raw radius floated outside the body (seed 5012's
    // detached limbs).
    let ring_r_at = |theta: f64| -> f64 { wild::radius_along(&raw.pts, theta).unwrap_or(mean_r) };

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

    let mut shadow = render::filled(
        render::ellipse(cx, max_y + 4.0, (max_x - min_x) * 0.42, 6.0),
        "rgba(40,30,50,0.14)",
        1.0,
        None,
        "wild-shadow",
    );
    shadow.role = Some(crate::Role::GroundShadow);
    let mut primitives = vec![shadow];

    if !merged_mode {
        for (ei, pts) in &ext_outlines {
            let fill = match (c.topology, c.ext_fills[*ei]) {
                (2, _) => colors.part.clone(),
                (_, 0) => palette::tone(c.recipe, c.hue, 1),
                (_, 1) => palette::tone(c.recipe, c.hue, -1),
                _ => colors.body.clone(),
            };
            let points: Vec<(f64, f64)> = pts.iter().map(|point| t(*point)).collect();
            primitives.push(render::filled(
                render::polygon(&points),
                &fill,
                1.0,
                Some(render::stroke(
                    &colors.outline,
                    1.5,
                    StrokeCap::Butt,
                    StrokeJoin::Round,
                )),
                "wild-extremity",
            ));
        }
    }

    let outline_shape = render::polygon(&outline);
    primitives.push(render::filled(
        outline_shape.clone(),
        &colors.body,
        1.0,
        Some(render::stroke(
            &colors.outline,
            1.5,
            StrokeCap::Butt,
            StrokeJoin::Miter,
        )),
        "wild-body",
    ));

    // Placement first: the Voronoi face-fade mask needs to know where the
    // face lives, while the body silhouette masks every inside-body feature.
    let placement = place_face(&outline, c.face_style.mouth_w, c.face_style.mouth_d);
    let body_mask = vec![render::filled(
        outline_shape,
        "white",
        1.0,
        None,
        "wild-body-mask",
    )];
    let face_fade_mask = placement.map(|p| {
        let cap_top = p.eye_y - p.r * 2.2;
        let cap_bot = min_y + h * 0.62 + h * 0.29;
        let cap_cy = (cap_top + cap_bot) / 2.0;
        let cap_ry = (cap_bot - cap_top) / 2.0;
        let cap_rx = (p.ipd + p.r * 2.6).max(p.half_w * 0.66);
        let mut fade = render::filled(
            render::ellipse(p.fx, cap_cy, cap_rx, cap_ry),
            "white",
            1.0,
            None,
            "wild-voronoi-face-fade",
        );
        fade.paint = Paint::RadialGradient {
            stops: vec![
                GradientStop {
                    offset_q32: Q32((0.62 * (1_u64 << 32) as f64).round() as i64),
                    color: render::css_rgb("#000000"),
                },
                GradientStop {
                    offset_q32: Q32::ONE,
                    color: render::css_rgb("white"),
                },
            ],
        };
        vec![
            render::filled(
                render::rect(
                    min_x - 20.0,
                    min_y - 20.0,
                    max_x - min_x + 40.0,
                    max_y - min_y + 40.0,
                ),
                "white",
                1.0,
                None,
                "wild-voronoi-mask-field",
            ),
            fade,
        ]
    });

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
            let mut primitive = render::filled(
                render::rounded_polygon(&cell, 0.8),
                fill,
                1.0,
                None,
                "wild-voronoi-cell",
            );
            primitive.effects.push(Effect::Masked {
                mask: body_mask.clone(),
            });
            if let Some(mask) = &face_fade_mask {
                primitive
                    .effects
                    .push(Effect::Masked { mask: mask.clone() });
            }
            primitives.push(primitive);
        }
    }

    // Mood layers add body-colour lids and squints, brows, mouth overrides,
    // eye scaling and small icons to the creature's placed face.
    if let Some(p) = placement {
        // Belly follows the BODY, not an ellipse: the outline shrunk toward
        // a lower anchor and cut off above the belly line. A fixed ellipse
        // fits only ellipse-shaped bodies — it read as a ball stuck on a
        // lumpy body (seed 5001) and as a smear across the gap between two
        // legs (seed 5022). Shrinking the silhouette itself cannot produce
        // either: the patch is always the creature's own shape.
        let belly_top = belly_top_for(p.eye_y, p.r);
        let belly = belly_patch(&outline, cx, belly_top, 0.72);
        if belly.len() >= 3 {
            let mut primitive = render::filled(
                render::polygon(&belly),
                &palette::belly_fill(c.recipe, c.hue, c.belly_variant),
                1.0,
                None,
                "wild-belly",
            );
            primitive.effects.push(Effect::Masked {
                mask: body_mask.clone(),
            });
            primitives.push(primitive);
        }
        for blush_x in [p.fx - p.ipd * 1.28, p.fx + p.ipd * 1.28] {
            let mut primitive = render::filled(
                render::ellipse(blush_x, p.eye_y + p.r * 1.3, p.r * 0.82, p.r * 0.5),
                &colors.blush,
                0.8,
                None,
                "wild-blush",
            );
            primitive.effects.push(Effect::Masked {
                mask: body_mask.clone(),
            });
            primitives.push(primitive);
        }
        // Derive the shim's reference width from the selected half eye spacing.
        // The face frame then applies its width profile to that reference.
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
        let mut eyes = face::eyes_inked(
            c.eyes,
            &shim,
            p.fx,
            face_cy,
            0.0,
            face::EyeInk {
                accent: &colors.outline,
                ink: &colors.ink,
                pupil: &colors.pupil,
                sclera: colors.sclera.as_deref(),
            },
            c.face_style.eye_wonk,
        );
        // Per-creature eye shape, bounded so the invariants still hold:
        // both axis factors are <= 1, so an eye can only shrink inside the
        // placed radius that clearance and separation were proved against.
        // Aspect is applied to the pair; signed wonk shrinks one eye only.
        let st = c.face_style;
        let (ax, ay) = if st.eye_aspect >= 1.0 {
            (1.0 / st.eye_aspect, 1.0)
        } else {
            (1.0, st.eye_aspect)
        };
        let sx = ax * spec.eye_scale;
        let sy = ay * spec.eye_scale;
        let eye_transform = ((sx - 1.0).abs() > 0.005 || (sy - 1.0).abs() > 0.005).then(|| {
            [
                render::translate(p.fx, f.y),
                render::scale(sx, sy),
                render::translate(-p.fx, -f.y),
            ]
        });
        if let Some(operations) = &eye_transform {
            render::prepend_transform(&mut eyes, operations);
        }
        append_mask(&mut eyes, &body_mask);
        primitives.extend(eyes);

        // Mood layers depend on eye archetype: lids and squints suit discs,
        // but stacked on line eyes they read as stray lines. Each archetype decides what
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
        // Lids and squints are built at the unscaled eye centres, so they
        // need the eyes' face-axis transform to land on the drawn eye.
        // Version 1 shipped without it and must keep drawing that way.
        let overlay_transform = match generation {
            Generation::V1 => None,
            Generation::V2 => eye_transform.as_ref(),
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
                let mut overlays =
                    mood::squint_overlay(x, ey, er, spec.squint, &colors.body, &colors.outline);
                if let Some(operations) = overlay_transform {
                    render::prepend_transform(&mut overlays, operations);
                }
                append_mask(&mut overlays, &body_mask);
                primitives.extend(overlays);
            }
            if do_lid {
                let mut overlays = mood::lid_overlay(
                    x,
                    ey,
                    er,
                    spec.lid,
                    spec.lid_angle * side,
                    &colors.body,
                    &colors.outline,
                );
                if let Some(operations) = overlay_transform {
                    render::prepend_transform(&mut overlays, operations);
                }
                append_mask(&mut overlays, &body_mask);
                primitives.extend(overlays);
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
            let mut brows = mood::brow(brows, x, f.y, f.r, side, &colors.ink);
            append_mask(&mut brows, &body_mask);
            primitives.extend(brows);
        }

        // Mouth: mood override or the pack's own.
        let mouth_kind = match spec.mouth {
            mood::MouthMood::Keep => c.mouth,
            mood::MouthMood::Grin => face::MouthKind::OpenHappy,
            mood::MouthMood::O => face::MouthKind::OMouth,
            mood::MouthMood::Pressed => face::MouthKind::Flat,
            mood::MouthMood::Down => face::MouthKind::DownArc,
        };
        // No mouth on the fallback path: the placement search could not fit
        // one inside the silhouette, so drawing it would put geometry
        // outside the body.
        let mut mouth = if p.mouth_fits {
            face::mouth_inked(
                mouth_kind,
                &shim,
                p.fx,
                face_cy,
                &colors.outline,
                &colors.ink,
            )
        } else {
            Vec::new()
        };
        // Per-creature mouth proportion. The placement search already tested
        // fit at exactly these factors, so a wide mouth was accounted for
        // when the face was positioned rather than overflowing afterwards.
        let mouth_y = mouth_y_for(p.eye_y, p.r);
        if (st.mouth_w - 1.0).abs() > 0.02 || (st.mouth_d - 1.0).abs() > 0.02 {
            render::prepend_transform(
                &mut mouth,
                &[
                    render::translate(p.fx, mouth_y),
                    render::scale(st.mouth_w, st.mouth_d),
                    render::translate(-p.fx, -mouth_y),
                ],
            );
        }
        append_mask(&mut mouth, &body_mask);
        primitives.extend(mouth);

        // Adjuncts live OUTSIDE the body clip (Zz floats beside the crown).
        primitives.extend(mood::adjunct(
            spec.adjunct,
            cx,
            min_y,
            h / 200.0,
            &colors.ink,
        ));
    }
    primitives
}

fn append_mask(primitives: &mut [Primitive], mask: &[Primitive]) {
    for primitive in primitives {
        primitive.effects.push(Effect::Masked {
            mask: mask.to_vec(),
        });
    }
}

/// Rebuilds exactly the outline draw_full renders (same transforms), for
/// the eyes-inside checker. Kept beside draw_full in the same file so the
/// two can never drift silently.
pub fn outline_for(c: &Creature, cx: f64, cy: f64) -> Vec<(f64, f64)> {
    let dom = c.dom;
    let raw = wild::build_raw(&c.wb, dom);
    let mean_r = c.wb.spokes.iter().map(|s| s.r).sum::<f64>() / c.wb.spokes.len() as f64;
    // Must match draw_full exactly — a checker that rebuilds the geometry
    // differently is measuring a creature nobody renders.
    let ring_r_at = |theta: f64| -> f64 { wild::radius_along(&raw.pts, theta).unwrap_or(mean_r) };
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
