//! Outline diagnostics kept outside the shipped generation path.

/// Symmetry probe: max distance from any outline point to the nearest point
/// of the x-mirrored outline (0 = perfectly symmetric). Diagnostic only.
pub fn asymmetry(outline: &[(f64, f64)], axis_x: f64) -> f64 {
    let mut worst = 0.0_f64;
    for &(x, y) in outline {
        let mx = 2.0 * axis_x - x;
        let mut best = f64::MAX;
        for &(ox, oy) in outline {
            let d = ((ox - mx).powi(2) + (oy - y).powi(2)).sqrt();
            if d < best {
                best = d;
            }
        }
        worst = worst.max(best);
    }
    worst
}
