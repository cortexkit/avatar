//! Face vocabulary for grid 2: eye archetypes x mouth families, plus the
//! additive life layer (cheek blush, belly patch).
//!
//! Coordinates are the blob frame (origin at blob center) with the cell
//! center passed in; every function returns SVG fragments.

use crate::body::{width_at, BodyParams};

#[derive(Clone, Copy, Debug)]
pub enum EyeKind {
    /// Grid-1 baseline: black dot + white shine.
    DotShine,
    /// Big lidded ovals: white sclera, iris dot, upper lid line.
    Lidded,
    /// Closed happy arcs.
    HappyArc,
    /// Gremlin: one big dot, one small dot (deliberate asymmetry).
    Uneven,
    /// Half-lidded flat-top eyes: dignified/unimpressed.
    HalfLid,
}

#[derive(Clone, Copy, Debug)]
pub enum MouthKind {
    /// Small downturned-then-up arc (grid-1 baseline smile).
    Arc,
    /// Cat mouth: two bumps (omega).
    CatW,
    /// Open rounded-rect happy mouth.
    OpenHappy,
    /// Crooked line with one tooth poking up (ugly-cute).
    Snaggle,
    /// Flat short line.
    Flat,
    /// Tiny w-fangs: two down-triangles under a short line.
    Fangs,
    /// Small round o (surprise/yawn) — mood vocabulary.
    OMouth,
    /// Downturned arc (sad) — mood vocabulary.
    DownArc,
}

pub struct FacePack {
    pub name: &'static str,
    pub eyes: EyeKind,
    pub mouth: MouthKind,
}

pub const PACKS: [FacePack; 6] = [
    FacePack { name: "baseline", eyes: EyeKind::DotShine, mouth: MouthKind::Arc },
    FacePack { name: "kitten", eyes: EyeKind::Lidded, mouth: MouthKind::CatW },
    FacePack { name: "joy", eyes: EyeKind::HappyArc, mouth: MouthKind::OpenHappy },
    FacePack { name: "gremlin", eyes: EyeKind::Uneven, mouth: MouthKind::Snaggle },
    FacePack { name: "deadpan", eyes: EyeKind::HalfLid, mouth: MouthKind::Flat },
    FacePack { name: "imp", eyes: EyeKind::DotShine, mouth: MouthKind::Fangs },
];

/// Eye geometry shared by all archetypes: large, wide-set, low.
pub struct EyeFrame {
    pub lx: f64,
    pub rx: f64,
    pub y: f64,
    pub r: f64,
}

/// Public frame accessor so mood overlays (lids, squint, brows) land exactly
/// on the archetype geometry without re-deriving it.
pub fn frame(bp: &BodyParams, cx: f64, cy: f64) -> EyeFrame {
    eye_frame(bp, cx, cy, 0.0)
}

fn eye_frame(bp: &BodyParams, cx: f64, cy: f64, tilt: f64) -> EyeFrame {
    let ey = -bp.height * 0.08;
    let w = width_at(bp, ey);
    let ex = w * 0.42;
    let r = (bp.height * 0.085).min(w * 0.3);
    EyeFrame {
        lx: cx - ex,
        rx: cx + ex,
        y: cy + ey + tilt,
        r,
    }
}

pub fn eyes(kind: EyeKind, bp: &BodyParams, cx: f64, cy: f64, tilt: f64, accent: &str) -> String {
    let f = eye_frame(bp, cx, cy, tilt);
    let mut s = String::new();
    match kind {
        EyeKind::DotShine => {
            for (x, dy) in [(f.lx, -tilt), (f.rx, tilt)] {
                let y = f.y + dy;
                s.push_str(&format!(
                    "<circle cx='{x:.1}' cy='{y:.1}' r='{:.1}' fill='#241f26'/><circle cx='{:.1}' cy='{:.1}' r='{:.1}' fill='white' opacity='0.9'/>",
                    f.r, x + f.r * 0.32, y - f.r * 0.35, f.r * 0.3,
                ));
            }
        }
        EyeKind::Lidded => {
            for (x, dy) in [(f.lx, -tilt), (f.rx, tilt)] {
                let y = f.y + dy;
                let rw = f.r * 1.1;
                let rh = f.r * 1.25;
                // Iris centered and large (looking AT you, not down), lid a
                // thin top sliver instead of a heavy shade.
                s.push_str(&format!(
                    "<ellipse cx='{x:.1}' cy='{y:.1}' rx='{rw:.1}' ry='{rh:.1}' fill='white' stroke='{accent}' stroke-width='2'/><circle cx='{x:.1}' cy='{:.1}' r='{:.1}' fill='#241f26'/><circle cx='{:.1}' cy='{:.1}' r='{:.1}' fill='white' opacity='0.9'/><path d='M{:.1} {:.1} A{rw:.1} {rh:.1} 0 0 1 {:.1} {:.1} L{:.1} {:.1} A{rw:.1} {:.1} 0 0 0 {:.1} {:.1} Z' fill='{accent}' opacity='0.5'/>",
                    y + rh * 0.08, f.r * 0.78,
                    x + f.r * 0.28, y - f.r * 0.2, f.r * 0.22,
                    x - rw, y - rh * 0.1, x + rw, y - rh * 0.1,
                    x + rw, y - rh * 0.28, rh * 0.9, x - rw, y - rh * 0.28,
                ));
            }
        }
        EyeKind::HappyArc => {
            for (x, dy) in [(f.lx, -tilt), (f.rx, tilt)] {
                let y = f.y + dy;
                s.push_str(&format!(
                    "<path d='M{:.1} {:.1} Q{x:.1} {:.1} {:.1} {:.1}' stroke='#241f26' stroke-width='3.4' fill='none' stroke-linecap='round'/>",
                    x - f.r, y + f.r * 0.2, y - f.r * 0.9, x + f.r, y + f.r * 0.2,
                ));
            }
        }
        EyeKind::Uneven => {
            s.push_str(&format!(
                "<circle cx='{:.1}' cy='{:.1}' r='{:.1}' fill='#241f26'/><circle cx='{:.1}' cy='{:.1}' r='{:.1}' fill='white' opacity='0.9'/>",
                f.lx, f.y - tilt, f.r * 1.25, f.lx + f.r * 0.4, f.y - tilt - f.r * 0.44, f.r * 0.36,
            ));
            s.push_str(&format!(
                "<circle cx='{:.1}' cy='{:.1}' r='{:.1}' fill='#241f26'/><circle cx='{:.1}' cy='{:.1}' r='{:.1}' fill='white' opacity='0.9'/>",
                f.rx, f.y + tilt + f.r * 0.15, f.r * 0.62, f.rx + f.r * 0.2, f.y + tilt - f.r * 0.08, f.r * 0.18,
            ));
        }
        EyeKind::HalfLid => {
            for (x, dy) in [(f.lx, -tilt), (f.rx, tilt)] {
                let y = f.y + dy;
                // Flat-topped circle: dot clipped by a lid rectangle drawn in body fill
                // is overkill here; a chord-topped path sells it.
                s.push_str(&format!(
                    "<path d='M{:.1} {:.1} A{:.1} {:.1} 0 1 0 {:.1} {:.1} Z' fill='#241f26'/><line x1='{:.1}' y1='{:.1}' x2='{:.1}' y2='{:.1}' stroke='#241f26' stroke-width='2.6' stroke-linecap='round'/>",
                    x - f.r * 0.92, y - f.r * 0.25,
                    f.r, f.r,
                    x + f.r * 0.92, y - f.r * 0.25,
                    x - f.r * 1.05, y - f.r * 0.3, x + f.r * 1.05, y - f.r * 0.3,
                ));
            }
        }
    }
    s
}

pub fn mouth(kind: MouthKind, bp: &BodyParams, cx: f64, cy: f64, accent: &str) -> String {
    let ey = -bp.height * 0.08;
    let r = (bp.height * 0.085).min(width_at(bp, ey) * 0.3);
    let my = cy + ey + bp.height * 0.15;
    match kind {
        MouthKind::Arc => format!(
            "<path d='M{:.1} {my:.1} Q{cx:.1} {:.1} {:.1} {my:.1}' stroke='{accent}' stroke-width='2.4' fill='none' stroke-linecap='round'/>",
            cx - r * 0.5, my + r * 0.45, cx + r * 0.5,
        ),
        MouthKind::CatW => format!(
            "<path d='M{:.1} {my:.1} Q{:.1} {:.1} {cx:.1} {my:.1} Q{:.1} {:.1} {:.1} {my:.1}' stroke='{accent}' stroke-width='2.4' fill='none' stroke-linecap='round'/>",
            cx - r * 0.8, cx - r * 0.4, my + r * 0.55, cx + r * 0.4, my + r * 0.55, cx + r * 0.8,
        ),
        // Tongue is INSCRIBED in the mouth's own quadratic: the mouth's
        // lower edge midpoint sits at +0.5r and reaches y=0.35r at x=±0.35r,
        // so the tongue chord (0.30r) and peak (mid 0.46r) stay strictly
        // inside — a tongue drawn deeper pokes out below the lip (Ufuk's
        // lab find; TOG clipped instead, geometry beats a clip here).
        MouthKind::OpenHappy => format!(
            "<path d='M{:.1} {:.1} Q{cx:.1} {:.1} {:.1} {:.1} Z' fill='#4a2f33' stroke='{accent}' stroke-width='2'/><path d='M{:.1} {:.1} Q{cx:.1} {:.1} {:.1} {:.1} Z' fill='#e88a93'/>",
            cx - r * 0.7, my - r * 0.1, my + r * 1.1, cx + r * 0.7, my - r * 0.1,
            cx - r * 0.35, my + r * 0.30, my + r * 0.62, cx + r * 0.35, my + r * 0.30,
        ),
        // Snaggletooth hangs DOWN from the lip. Base endpoints evaluated ON
        // the mouth quadratic (t=0.6 and t=0.8 of M(-0.75r,+0.15r)
        // Q(0,+0.5r) (0.7r,-0.1r)), apex dropped below — the previous
        // up-pointing triangle crossed the mouth line and read as a shard
        // through the face (Ufuk's lab find).
        MouthKind::Snaggle => format!(
            "<path d='M{:.1} {:.1} Q{cx:.1} {:.1} {:.1} {:.1}' stroke='{accent}' stroke-width='2.4' fill='none' stroke-linecap='round'/><path d='M{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} Z' fill='white' stroke='{accent}' stroke-width='1.4' stroke-linejoin='round'/>",
            cx - r * 0.75, my + r * 0.15, my + r * 0.5, cx + r * 0.7, my - r * 0.1,
            cx + r * 0.13, my + r * 0.23, cx + r * 0.42, my + r * 0.10, cx + r * 0.33, my + r * 0.55,
        ),
        MouthKind::Flat => format!(
            "<line x1='{:.1}' y1='{my:.1}' x2='{:.1}' y2='{my:.1}' stroke='{accent}' stroke-width='2.6' stroke-linecap='round'/>",
            cx - r * 0.55, cx + r * 0.55,
        ),
        MouthKind::OMouth => format!(
            "<ellipse cx='{cx:.1}' cy='{:.1}' rx='{:.1}' ry='{:.1}' fill='#4a2f33' stroke='{accent}' stroke-width='1.8'/>",
            my + r * 0.2, r * 0.3, r * 0.38,
        ),
        MouthKind::DownArc => format!(
            "<path d='M{:.1} {:.1} Q{cx:.1} {:.1} {:.1} {:.1}' stroke='{accent}' stroke-width='2.4' fill='none' stroke-linecap='round'/>",
            cx - r * 0.55, my + r * 0.35, my - r * 0.25, cx + r * 0.55, my + r * 0.35,
        ),
        MouthKind::Fangs => format!(
            "<path d='M{:.1} {my:.1} Q{cx:.1} {:.1} {:.1} {my:.1}' stroke='{accent}' stroke-width='2.4' fill='none' stroke-linecap='round'/><path d='M{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} Z' fill='white'/><path d='M{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} Z' fill='white'/>",
            cx - r * 0.6, my + r * 0.5, cx + r * 0.6,
            cx - r * 0.45, my + r * 0.12, cx - r * 0.15, my + r * 0.14, cx - r * 0.3, my + r * 0.62,
            cx + r * 0.15, my + r * 0.14, cx + r * 0.45, my + r * 0.12, cx + r * 0.3, my + r * 0.62,
        ),
    }
}

/// Ink-parameterized wrappers for grid 3: palettes may flip the face ink
/// (dark bodies need light features), so the archetype functions gain color
/// arguments while the grid-2 call sites keep their defaults.
///
/// Pupils are NOT ink: ink is line-work (mouth strokes, brows, closed-eye
/// arcs) and flips light on dark bodies, but a light "pupil" reads as a
/// blank ghost orb. Pupils stay dark everywhere; on dark bodies a light
/// sclera disc drawn BEHIND each open-eye pupil provides the contrast the
/// light body gives for free. Closed/arc eyes are line-work and keep ink.
pub fn eyes_inked(
    kind: EyeKind,
    bp: &BodyParams,
    cx: f64,
    cy: f64,
    tilt: f64,
    accent: &str,
    ink: &str,
    pupil: &str,
    sclera: Option<&str>,
) -> String {
    let f = eye_frame(bp, cx, cy, tilt);
    let mut s = String::new();
    if let Some(sc) = sclera {
        // Sclera per OPEN-eye archetype; HappyArc is a stroke, not a disc.
        // Radii track each archetype's drawn pupil footprint plus a rim.
        match kind {
            EyeKind::DotShine => {
                for x in [f.lx, f.rx] {
                    s.push_str(&format!(
                        "<circle cx='{x:.1}' cy='{:.1}' r='{:.1}' fill='{sc}'/>",
                        f.y, f.r * 1.22,
                    ));
                }
            }
            EyeKind::HalfLid => {
                for x in [f.lx, f.rx] {
                    s.push_str(&format!(
                        "<circle cx='{x:.1}' cy='{:.1}' r='{:.1}' fill='{sc}'/>",
                        f.y, f.r * 1.18,
                    ));
                }
            }
            EyeKind::Uneven => {
                s.push_str(&format!(
                    "<circle cx='{:.1}' cy='{:.1}' r='{:.1}' fill='{sc}'/>",
                    f.lx, f.y - tilt, f.r * 1.25 * 1.18,
                ));
                s.push_str(&format!(
                    "<circle cx='{:.1}' cy='{:.1}' r='{:.1}' fill='{sc}'/>",
                    f.rx, f.y + tilt + f.r * 0.15, f.r * 0.62 * 1.25,
                ));
            }
            // Lidded draws its own white sclera; HappyArc is line-work.
            EyeKind::Lidded | EyeKind::HappyArc => {}
        }
    }
    // Line-work archetypes (closed arcs) take ink; pupil fills take pupil.
    let body = match kind {
        EyeKind::HappyArc => eyes(kind, bp, cx, cy, tilt, accent).replace("#241f26", ink),
        _ => eyes(kind, bp, cx, cy, tilt, accent).replace("#241f26", pupil),
    };
    s.push_str(&body);
    s
}

pub fn mouth_inked(
    kind: MouthKind,
    bp: &BodyParams,
    cx: f64,
    cy: f64,
    accent: &str,
    ink: &str,
) -> String {
    mouth(kind, bp, cx, cy, accent).replace("#241f26", ink)
}

pub fn blush_colored(bp: &BodyParams, cx: f64, cy: f64, color: &str) -> String {
    blush(bp, cx, cy).replace("#f0808f' opacity='0.35", &format!("{color}' opacity='0.8"))
}

pub fn belly_colored(bp: &BodyParams, cx: f64, cy: f64, color: &str) -> String {
    belly(bp, cx, cy).replace("fill='white' opacity='0.42'", &format!("fill='{color}'"))
}

/// Cheek blush: soft rosy ellipses outer-low of each eye.
pub fn blush(bp: &BodyParams, cx: f64, cy: f64) -> String {
    let ey = -bp.height * 0.08;
    let w = width_at(bp, ey);
    let bx = w * 0.62;
    let by = cy + ey + bp.height * 0.09;
    let rw = w * 0.14;
    let rh = rw * 0.62;
    format!(
        "<ellipse cx='{:.1}' cy='{by:.1}' rx='{rw:.1}' ry='{rh:.1}' fill='#f0808f' opacity='0.35'/><ellipse cx='{:.1}' cy='{by:.1}' rx='{rw:.1}' ry='{rh:.1}' fill='#f0808f' opacity='0.35'/>",
        cx - bx, cx + bx,
    )
}

/// Belly patch: a lighter inner blob hugging the lower mass.
pub fn belly(bp: &BodyParams, cx: f64, cy: f64) -> String {
    let half_h = bp.height / 2.0;
    let top = -half_h * 0.05;
    let bot = half_h * 0.92;
    let steps = 24;
    let mut d = String::new();
    for i in 0..=steps {
        let y = top + (bot - top) * (i as f64 / steps as f64);
        let x = width_at(bp, y) * 0.58;
        let cmd = if i == 0 { 'M' } else { 'L' };
        d.push_str(&format!("{cmd}{:.1} {:.1} ", cx + x, cy + y));
    }
    for i in (0..=steps).rev() {
        let y = top + (bot - top) * (i as f64 / steps as f64);
        let x = width_at(bp, y) * 0.58;
        d.push_str(&format!("L{:.1} {:.1} ", cx - x, cy + y));
    }
    d.push('Z');
    format!("<path d='{d}' fill='white' opacity='0.42'/>")
}
