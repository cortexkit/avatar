//! Body blobs: superellipse-quadrant closed shapes, sampled densely.
//!
//! The body IS the head (baby-schema compactness). Four quadrants share a
//! width profile (top vs bottom half-width), a per-half squircle exponent,
//! and a ground-sit flattening; sampling density does the smoothing.

/// Proportion parameters for one blob. All lengths relative to a unit cell;
/// the renderer scales.
#[derive(Clone, Copy, Debug)]
pub struct BodyParams {
    /// Total height of the blob.
    pub height: f64,
    /// Half-width of the upper mass.
    pub w_top: f64,
    /// Half-width of the lower mass.
    pub w_bot: f64,
    /// Squircle exponent for the top half (2 = ellipse, higher = squarer).
    pub n_top: f64,
    /// Squircle exponent for the bottom half (high = flat ground-sit).
    pub n_bot: f64,
    /// Vertical position of the widest point, 0..1 from top (0.5 = middle).
    pub waist: f64,
}

/// One sampled outline point.
#[derive(Clone, Copy, Debug)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

/// Smootherstep: C2 ease used to blend the top/bottom width targets so the
/// outline has no tangent break at the waist (grid-1 lesson: hard-switched
/// superellipse halves put a corner where the widths differ — ghost skirt).
fn smoother(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Continuous half-width profile W(y): blended base width × superellipse
/// caps. Both caps equal 1 at the waist with zero slope (n > 1), and the
/// base blend is C2, so the profile is smooth everywhere.
fn profile(p: &BodyParams, y: f64) -> f64 {
    let half_h = p.height / 2.0;
    let waist_y = -half_h + p.waist * p.height;
    let base = p.w_top + (p.w_bot - p.w_top) * smoother((y + half_h) / p.height);
    let cap = if y <= waist_y {
        let extent = waist_y + half_h;
        let u = ((waist_y - y) / extent).clamp(0.0, 1.0);
        (1.0 - u.powf(p.n_top)).max(0.0).powf(1.0 / p.n_top)
    } else {
        let extent = half_h - waist_y;
        let u = ((y - waist_y) / extent).clamp(0.0, 1.0);
        (1.0 - u.powf(p.n_bot)).max(0.0).powf(1.0 / p.n_bot)
    };
    base * cap
}

/// The "alive" layer: deliberate imperfection applied to a sampled body.
/// Zeroed, the outline is the pure symmetric profile.
#[derive(Clone, Copy, Debug, Default)]
pub struct Wonk {
    /// Horizontal shear: x += y * lean (ground contact stays put, mass tips).
    pub lean: f64,
    /// Per-side width bias: right *= 1+bias, left *= 1-bias.
    pub side_bias: f64,
    /// Eye-line tilt in px carried to the face layer (positive = right eye lower).
    pub eye_tilt: f64,
}

/// Samples the closed outline by scanline: down the right side, up the left.
pub fn sample_outline(p: &BodyParams, steps: usize) -> Vec<Pt> {
    sample_outline_wonk(p, steps, &Wonk::default())
}

/// Outline with the wonk layer applied.
pub fn sample_outline_wonk(p: &BodyParams, steps: usize, w: &Wonk) -> Vec<Pt> {
    let total = 2 * steps;
    let mut points = Vec::with_capacity(2 * total);
    let half_h = p.height / 2.0;
    for i in 0..=total {
        let y = -half_h + p.height * (i as f64 / total as f64);
        let x = profile(p, y) * (1.0 + w.side_bias) + y * w.lean;
        points.push(Pt { x, y });
    }
    for i in (0..=total).rev() {
        let y = -half_h + p.height * (i as f64 / total as f64);
        let x = -profile(p, y) * (1.0 - w.side_bias) + y * w.lean;
        points.push(Pt { x, y });
    }
    points
}

/// Converts an outline to an SVG path string (M + L chain, closed).
pub fn outline_path(points: &[Pt], cx: f64, cy: f64) -> String {
    let mut d = String::new();
    for (i, p) in points.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        d.push_str(&format!("{cmd}{:.2} {:.2} ", cx + p.x, cy + p.y));
    }
    d.push('Z');
    d
}

/// Width of the blob at a given y (positive down, blob frame). Used to place
/// eyes and parts on the surface.
pub fn width_at(p: &BodyParams, y: f64) -> f64 {
    profile(p, y)
}
