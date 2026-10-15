//! Width profile for the face's body-shaped placement shim. Wild faces use
//! equal half-widths, a centered waist and exponent 2 on both halves.

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
    /// Upper exponent retained in the shim layout; wild faces always use 2.
    pub n_top: f64,
    /// Lower exponent retained in the shim layout; wild faces always use 2.
    pub n_bot: f64,
    /// Vertical position of the widest point, 0..1 from top (0.5 = middle).
    pub waist: f64,
}

/// Smootherstep: C2 ease used to blend the top/bottom width targets so the
/// outline has no tangent break where the widths meet.
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
        (1.0 - u * u).max(0.0).sqrt()
    } else {
        let extent = half_h - waist_y;
        let u = ((y - waist_y) / extent).clamp(0.0, 1.0);
        (1.0 - u * u).max(0.0).sqrt()
    };
    base * cap
}

/// Width of the blob at a given y (positive down, blob frame). Used to place
/// eyes and parts on the surface.
pub fn width_at(p: &BodyParams, y: f64) -> f64 {
    profile(p, y)
}
