//! Body geometry: mirroring, spline construction, flattening, bounds,
//! rescale, and SVG path serialization (design note 004, stages 1–5).
//!
//! Everything here is integer-only: i64 milliunit coordinates, Q32.32
//! tensions, i128 intermediates, and `div_round_half_away` as the single
//! narrowing primitive.

use avatar_phenotype::{Body, Phenotype};
use avatar_tables::div_round_half_away;

use crate::fixed::{q32_ratio, CANVAS_MILLI, CENTER, Q32_ONE};
#[cfg(test)]
use crate::format::format_milli;
use crate::tree::{FillRule, Segment, Shape};

/// Samples per Bézier segment when flattening the body outline.
const FLATTEN_STEPS: i64 = 20;

/// Cubic body outline plus the flattened landmarks and derived metrics used
/// by later render-tree assembly.
///
/// All lengths are milliunits on the 1_000_000 canonical canvas. `all_points`
/// holds every flattened sample in segment order (segment endpoints appear
/// twice: once as the previous segment's last sample, once as the next
/// segment's first), which the face pass needs for mouth fitting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyGeometry {
    pub shape: Shape,
    pub body_width: i64,
    pub body_height: i64,
    pub top: i64,
    pub bottom: i64,
    pub max_width_y: i64,
    pub min_width_y: i64,
    pub all_points: Vec<(i64, i64)>,
}

/// Computes the body outline for a phenotype (growth = 1 adult shape).
pub fn body_geometry(phenotype: &Phenotype) -> BodyGeometry {
    let ring = mirror_ring(&phenotype.body);
    let mut spline = build_spline(&ring);
    let mut bounds = flatten(&spline);
    let body_width = rescale(&mut spline, &mut bounds);
    let top = bounds.min_y;
    let bottom = bounds.max_y;
    let body_height = bottom - top;
    BodyGeometry {
        shape: spline.shape(),
        body_width,
        body_height,
        top,
        bottom,
        max_width_y: bounds.max_width_y,
        min_width_y: min_width_y(&bounds.samples, top, body_height),
        all_points: bounds.samples,
    }
}

/// One point of the mirrored closed ring, carrying its spline tension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RingPoint {
    pub x: i64,
    pub y: i64,
    pub tension: i64,
}

/// Stage 1 — mirrors the half-side control points into a closed ring.
///
/// `count` half-side points become a `2·count` ring: the left side walks
/// points `0..count` at `center − x/2`, the right side walks the same points
/// in reverse (`count−1` down to `0`) at `center + x/2`, matching the mirror
/// loop in TOG's `type1.ts`. Every half-side point appears exactly once per
/// side, which is what makes the closed shape horizontally symmetric — a
/// ring that dropped the reversed index 0 would leave that point unpaired
/// and skew the whole body.
pub(crate) fn mirror_ring(body: &Body) -> Vec<RingPoint> {
    // The decoder contract is 2..=16 half-side points; clamping keeps the
    // renderer total for hand-built phenotypes outside that range.
    let count = usize::from(body.point_count.clamp(2, 16));
    let points = &body.points[..count];
    let max_y = points.iter().map(|point| point.y.0).max().unwrap_or(0);
    let vertical_shift = CENTER - div2(max_y);

    let mut ring = Vec::with_capacity(2 * count);
    for point in points {
        ring.push(RingPoint {
            x: CENTER - div2(point.x.0),
            y: point.y.0 + vertical_shift,
            tension: point.tension.0,
        });
    }
    for index in 1..=count {
        let point = points[count - index];
        ring.push(RingPoint {
            x: CENTER + div2(point.x.0),
            y: point.y.0 + vertical_shift,
            tension: point.tension.0,
        });
    }
    ring
}

/// The pinned ÷2 used by the mirror: half away from zero, never native `/`.
fn div2(value: i64) -> i64 {
    div_round_half_away(i128::from(value), 2) as i64
}

/// One cubic Bézier segment of the spline (control points + endpoint).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Curve {
    pub cp1x: i64,
    pub cp1y: i64,
    pub cp2x: i64,
    pub cp2y: i64,
    pub x2: i64,
    pub y2: i64,
}

/// A closed spline: its start point plus one Bézier segment per ring point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Spline {
    pub start_x: i64,
    pub start_y: i64,
    pub curves: Vec<Curve>,
}

impl Spline {
    pub(crate) fn shape(&self) -> Shape {
        let mut segments = Vec::with_capacity(self.curves.len() + 1);
        segments.push(Segment::MoveTo {
            x: self.start_x,
            y: self.start_y,
        });
        segments.extend(self.curves.iter().map(|curve| Segment::CubicTo {
            cp1x: curve.cp1x,
            cp1y: curve.cp1y,
            cp2x: curve.cp2x,
            cp2y: curve.cp2y,
            x: curve.x2,
            y: curve.y2,
        }));
        Shape::Path {
            segments,
            closed: false,
            fill_rule: FillRule::NonZero,
        }
    }
}

/// Stage 2 — turns the closed ring into cubic Béziers.
///
/// Catmull-Rom-style with per-point tensions. The cursor starts at
/// `len − 2` (the phase TOG's `spline.ts` uses, kept so curve and tension
/// alignment match the mirrored ring), and each segment reads a four-point
/// window with wraparound. `spline.ts`'s final-window guard
/// (`i == points.length + 3`) can never fire for a window walk of `len`
/// segments; its effect is plain wraparound, which is what happens here.
///
/// Control points are `p1 + (p2 − p0)·t/6` and `p2 − (p3 − p1)·t/6`. The
/// milliunit delta × Q32 tension product is divided by `6·2^32` in ONE
/// `div_round_half_away` call per coordinate — a single rounding, not a
/// ÷6-then-×t chain.
pub(crate) fn build_spline(ring: &[RingPoint]) -> Spline {
    let len = ring.len();
    let mut tension_iter = 0usize;
    let start = ring[len - 1];
    let mut spline = Spline {
        start_x: start.x,
        start_y: start.y,
        curves: Vec::with_capacity(len),
    };

    for segment in 0..len {
        let cursor = len - 2 + segment;
        let p0 = ring[cursor % len];
        let p1 = ring[(cursor + 1) % len];
        let p2 = ring[(cursor + 2) % len];
        let p3 = ring[(cursor + 3) % len];
        let tension = ring[tension_iter].tension;
        spline.curves.push(Curve {
            cp1x: p1.x + tension_offset(p2.x - p0.x, tension),
            cp1y: p1.y + tension_offset(p2.y - p0.y, tension),
            cp2x: p2.x - tension_offset(p3.x - p1.x, tension),
            cp2y: p2.y - tension_offset(p3.y - p1.y, tension),
            x2: p2.x,
            y2: p2.y,
        });
        // The tension walk advances every segment except the seam segment
        // (`len / 2`, integer division): skipping there phase-locks the
        // tensions of the two segments that straddle the mirrored side.
        if segment != len / 2 {
            tension_iter += 1;
        }
    }
    spline
}

/// `delta · tension / (6 · 2^32)` as one pinned narrowing.
fn tension_offset(delta: i64, tension: i64) -> i64 {
    div_round_half_away(
        i128::from(delta) * i128::from(tension),
        6 * i128::from(Q32_ONE),
    ) as i64
}

/// Outline extremes plus every flattened sample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Bounds {
    pub min_x: i64,
    pub min_x_y: i64,
    pub max_x: i64,
    pub min_y: i64,
    pub max_y: i64,
    pub max_width_y: i64,
    pub samples: Vec<(i64, i64)>,
}

/// Stage 3 — flattens every segment at `t = j/20, j in 0..=20` and tracks
/// the outline extremes.
///
/// The comparison chain of `spline.ts`'s `getBezierTipPoint` is transcribed
/// with its exact tie behavior: strict `<`/`>` in an else-if chain, so the
/// FIRST sample at a tied extreme keeps its companion y. The ordering is
/// observable (it selects `max_width_y` / `min_x_y`) and therefore
/// specified, not incidental.
pub(crate) fn flatten(spline: &Spline) -> Bounds {
    let mut bounds = Bounds {
        min_x: i64::MAX,
        min_x_y: 0,
        max_x: 0,
        min_y: i64::MAX,
        max_y: 0,
        max_width_y: 0,
        samples: Vec::with_capacity(spline.curves.len() * (FLATTEN_STEPS as usize + 1)),
    };
    let mut anchor_x = spline.start_x;
    let mut anchor_y = spline.start_y;
    for curve in &spline.curves {
        for step in 0..=FLATTEN_STEPS {
            let x = bezier_sample(anchor_x, curve.cp1x, curve.cp2x, curve.x2, step);
            let y = bezier_sample(anchor_y, curve.cp1y, curve.cp2y, curve.y2, step);
            if x < bounds.min_x {
                bounds.min_x = x;
                bounds.min_x_y = y;
            } else if x > bounds.max_x {
                bounds.max_x = x;
                bounds.max_width_y = y;
            }
            if y < bounds.min_y {
                bounds.min_y = y;
            } else if y > bounds.max_y {
                bounds.max_y = y;
            }
            bounds.samples.push((x, y));
        }
        anchor_x = curve.x2;
        anchor_y = curve.y2;
    }
    bounds
}

/// Integer De Casteljau evaluation of one cubic at `t = step/FLATTEN_STEPS`.
///
/// Three lerp levels, each `a + (b − a)·step/20` with one pinned rounding,
/// staying in i64 milliunits. This is the v1 specification for curve
/// sampling; it replaces the float Bernstein polynomial of TOG's
/// `math.ts` (`bezierPoint`). The endpoints (`step` 0 and 20) evaluate
/// exactly, so consecutive segments share their junction sample
/// bit-for-bit.
fn bezier_sample(p0: i64, p1: i64, p2: i64, p3: i64, step: i64) -> i64 {
    let q0 = lerp_step(p0, p1, step);
    let q1 = lerp_step(p1, p2, step);
    let q2 = lerp_step(p2, p3, step);
    let r0 = lerp_step(q0, q1, step);
    let r1 = lerp_step(q1, q2, step);
    lerp_step(r0, r1, step)
}

fn lerp_step(a: i64, b: i64, step: i64) -> i64 {
    a + div_round_half_away(
        (i128::from(b) - i128::from(a)) * i128::from(step),
        i128::from(FLATTEN_STEPS),
    ) as i64
}

/// Stage 4 — pulls an overflowing body back inside the canvas.
///
/// Width and height extents are measured against the canvas with a
/// `canvas/6` margin (the branchy logic of `spline.ts`, exact i64 compares);
/// if the larger ratio exceeds 1, every coordinate moves through
/// `adjust_point` toward the canvas center by `1/ratio`. Returns the final
/// body width.
pub(crate) fn rescale(spline: &mut Spline, bounds: &mut Bounds) -> i64 {
    let margin = div_round_half_away(i128::from(CANVAS_MILLI), 6) as i64;

    let mut max_width = bounds.max_x - bounds.min_x;
    if bounds.min_x - margin < 0 {
        max_width = if bounds.min_x < 0 {
            -bounds.min_x + bounds.max_x + margin
        } else {
            bounds.max_x - bounds.min_x + margin
        };
    }

    // Height must be checked against both canvas edges: a body can overflow
    // the top, the bottom, or both, and the larger demand wins.
    let mut max_height1 = bounds.max_y - bounds.min_y;
    let mut max_height2 = bounds.max_y - bounds.min_y;
    if bounds.min_y - margin < 0 {
        max_height1 = if bounds.min_y < 0 {
            2 * (-bounds.min_y + CENTER) + margin
        } else {
            2 * (CENTER - bounds.min_y) + margin
        };
    }
    if bounds.max_y + margin > CANVAS_MILLI {
        max_height2 = 2 * (bounds.max_y - CENTER) + margin;
    }
    let max_height = if max_height1 > max_height2 {
        max_height1
    } else {
        max_height2
    };

    let width_ratio = q32_ratio(max_width);
    let height_ratio = q32_ratio(max_height);
    let final_ratio = if width_ratio > height_ratio {
        width_ratio
    } else {
        height_ratio
    };

    if final_ratio > Q32_ONE {
        spline.start_x = adjust_point(spline.start_x, final_ratio);
        spline.start_y = adjust_point(spline.start_y, final_ratio);
        for curve in &mut spline.curves {
            curve.cp1x = adjust_point(curve.cp1x, final_ratio);
            curve.cp1y = adjust_point(curve.cp1y, final_ratio);
            curve.cp2x = adjust_point(curve.cp2x, final_ratio);
            curve.cp2y = adjust_point(curve.cp2y, final_ratio);
            curve.x2 = adjust_point(curve.x2, final_ratio);
            curve.y2 = adjust_point(curve.y2, final_ratio);
        }
        bounds.max_width_y = adjust_point(bounds.max_width_y, final_ratio);
        bounds.min_x = adjust_point(bounds.min_x, final_ratio);
        bounds.min_x_y = adjust_point(bounds.min_x_y, final_ratio);
        bounds.min_y = adjust_point(bounds.min_y, final_ratio);
        bounds.max_y = adjust_point(bounds.max_y, final_ratio);
        max_width = div_round_half_away(
            i128::from(max_width) * i128::from(Q32_ONE),
            i128::from(final_ratio),
        ) as i64;
        for (x, y) in &mut bounds.samples {
            *x = adjust_point(*x, final_ratio);
            *y = adjust_point(*y, final_ratio);
        }
    }
    max_width
}

/// Moves one coordinate toward the canvas center by `1/ratio`.
///
/// The three branches cover `p` above the center, below it but positive, and
/// non-positive (mirrored coordinates can cross zero); each scaled offset is
/// one pinned division.
fn adjust_point(p: i64, ratio: i64) -> i64 {
    let scaled = |delta: i64| {
        div_round_half_away(i128::from(delta) * i128::from(Q32_ONE), i128::from(ratio)) as i64
    };
    if p > CENTER {
        CENTER + scaled(p - CENTER)
    } else if p > 0 {
        CENTER - scaled(CENTER - p)
    } else if p < 0 {
        CENTER - scaled(-p + CENTER)
    } else {
        p
    }
}

/// The y of the band with the largest left-side inset.
///
/// Transcribed from `spline.ts`'s `getMinWidthY` 10-band scan because face
/// placement depends on its exact behavior: bands
/// `top + i·h/10 < y ≤ top + (i+1)·h/10` for `i in 1..9` over left-side
/// samples (`x < center`), selecting the midpoint y of the band whose
/// minimum x is largest. An empty band keeps `i64::MAX` as its minimum and
/// therefore outranks every real inset — a quirk preserved verbatim.
pub(crate) fn min_width_y(samples: &[(i64, i64)], top: i64, height: i64) -> i64 {
    let mut best_inset_x = 0_i64;
    let mut best_y = 0_i64;
    for band in 1..9 {
        let band_top = top + height_tenths(height, band);
        let band_bottom = top + height_tenths(height, band + 1);
        let mut min_x_in_band = i64::MAX;
        for &(x, y) in samples {
            if x < CENTER && y > band_top && y <= band_bottom && x < min_x_in_band {
                min_x_in_band = x;
            }
        }
        if min_x_in_band > best_inset_x {
            best_inset_x = min_x_in_band;
            best_y = band_top + height_twentieth(height);
        }
    }
    best_y
}

fn height_tenths(height: i64, count: i64) -> i64 {
    div_round_half_away(i128::from(height) * i128::from(count), 10) as i64
}

fn height_twentieth(height: i64) -> i64 {
    div_round_half_away(i128::from(height), 20) as i64
}

/// Test-only formatter used to verify that the tree serializer preserves the
/// established SVG body-path syntax.
#[cfg(test)]
pub(crate) fn path_string(spline: &Spline) -> String {
    let mut path = format!(
        "M{},{}",
        format_milli(spline.start_x),
        format_milli(spline.start_y)
    );
    for curve in &spline.curves {
        path.push_str(&format!(
            "C{} {}, {} {}, {} {}",
            format_milli(curve.cp1x),
            format_milli(curve.cp1y),
            format_milli(curve.cp2x),
            format_milli(curve.cp2y),
            format_milli(curve.x2),
            format_milli(curve.y2),
        ));
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use avatar_phenotype::{BodyPoint, Milli, Q32};

    fn point(x: i64, y: i64, tension: i64) -> BodyPoint {
        BodyPoint {
            x: Milli(x),
            y: Milli(y),
            tension: Q32(tension),
        }
    }

    fn body_with(points: &[BodyPoint]) -> Body {
        let mut body = Body {
            point_count: points.len() as u8,
            size: Milli(CANVAS_MILLI),
            points: [BodyPoint {
                x: Milli(0),
                y: Milli(0),
                tension: Q32(0),
            }; 16],
        };
        body.points[..points.len()].copy_from_slice(points);
        body
    }

    #[test]
    fn mirror_of_three_points_is_a_six_point_ring() {
        // max_y = 400_000 → vertical shift = 500_000 − 200_000 = 300_000.
        let ring = mirror_ring(&body_with(&[
            point(100_000, 200_000, Q32_ONE),
            point(300_000, 400_000, 2 * Q32_ONE),
            point(500_000, 100_000, Q32_ONE / 2),
        ]));
        assert_eq!(
            ring,
            vec![
                // Left side, i = 0..3: x = center − x/2.
                RingPoint {
                    x: 450_000,
                    y: 500_000,
                    tension: Q32_ONE
                },
                RingPoint {
                    x: 350_000,
                    y: 700_000,
                    tension: 2 * Q32_ONE
                },
                RingPoint {
                    x: 250_000,
                    y: 400_000,
                    tension: Q32_ONE / 2
                },
                // Right side mirrors points[2], points[1], then points[0]:
                // every half-side point appears once per side.
                RingPoint {
                    x: 750_000,
                    y: 400_000,
                    tension: Q32_ONE / 2
                },
                RingPoint {
                    x: 650_000,
                    y: 700_000,
                    tension: 2 * Q32_ONE
                },
                RingPoint {
                    x: 550_000,
                    y: 500_000,
                    tension: Q32_ONE
                },
            ]
        );
    }

    #[test]
    fn mirror_halving_rounds_half_away_from_zero() {
        let ring = mirror_ring(&body_with(&[
            point(100_001, 0, Q32_ONE),
            point(-100_001, 10, Q32_ONE),
        ]));
        // +100_001/2 = 50_000.5 → 50_001; −100_001/2 = −50_000.5 → −50_001.
        assert_eq!(ring[0].x, 500_000 - 50_001);
        assert_eq!(ring[1].x, 500_000 + 50_001);
    }

    #[test]
    fn spline_closes_the_ring() {
        let ring = mirror_ring(&body_with(&[
            point(100_000, 200_000, Q32_ONE),
            point(300_000, 400_000, Q32_ONE),
            point(500_000, 100_000, Q32_ONE),
        ]));
        let spline = build_spline(&ring);
        assert_eq!(spline.curves.len(), ring.len());
        // The final segment ends where the path started.
        let last = *spline.curves.last().unwrap();
        assert_eq!((last.x2, last.y2), (spline.start_x, spline.start_y));
        // The first segment starts at the ring's last point (cursor len−2).
        assert_eq!(
            (spline.start_x, spline.start_y),
            (ring[ring.len() - 1].x, ring[ring.len() - 1].y)
        );
    }

    #[test]
    fn tension_walk_skips_the_seam_segment() {
        // Five ring points on a line with distinct tensions. Expected control
        // points are hand-computed with the pinned arithmetic (all products
        // divide evenly, so no rounding is hidden in the literals):
        //   offset = delta · tension / (6 · 2^32)
        // Walk: segments take ring tensions [t0, t1, t2, t2, t3] — the
        // increment is skipped after segment 2 (len/2 = 2), so segment 3
        // reuses t2 and t4 is never read. Without the skip, segment 3's cp1x
        // would be 180 and segment 4's cp1x 190 instead of 140 and 240.
        let one = Q32_ONE;
        let ring = vec![
            RingPoint {
                x: 0,
                y: 0,
                tension: one,
            },
            RingPoint {
                x: 60,
                y: 0,
                tension: 3 * one,
            },
            RingPoint {
                x: 120,
                y: 0,
                tension: one,
            },
            RingPoint {
                x: 180,
                y: 0,
                tension: 3 * one,
            },
            RingPoint {
                x: 240,
                y: 0,
                tension: one / 2,
            },
        ];
        let spline = build_spline(&ring);
        assert_eq!((spline.start_x, spline.start_y), (240, 0));
        let expected: [(i64, i64, i64, i64, i64, i64); 5] = [
            // seg0: window P3 P4 P0 P1, tension t0 = 1.
            (210, 0, 30, 0, 0, 0),
            // seg1: window P4 P0 P1 P2, tension t1 = 3.
            (-90, 0, 0, 0, 60, 0),
            // seg2: window P0 P1 P2 P3, tension t2 = 1.
            (80, 0, 100, 0, 120, 0),
            // seg3: window P1 P2 P3 P4, tension t2 again (the skip).
            (140, 0, 160, 0, 180, 0),
            // seg4: window P2 P3 P4 P0, tension t3 = 3.
            (240, 0, 330, 0, 240, 0),
        ];
        for (curve, (cp1x, cp1y, cp2x, cp2y, x2, y2)) in spline.curves.iter().zip(expected) {
            assert_eq!(
                (curve.cp1x, curve.cp1y, curve.cp2x, curve.cp2y, curve.x2, curve.y2),
                (cp1x, cp1y, cp2x, cp2y, x2, y2)
            );
        }
    }

    #[test]
    fn flatten_samples_every_segment_and_hits_endpoints_exactly() {
        let ring = mirror_ring(&body_with(&[
            point(100_000, 200_000, Q32_ONE),
            point(300_000, 400_000, Q32_ONE),
            point(500_000, 100_000, Q32_ONE),
        ]));
        let spline = build_spline(&ring);
        let bounds = flatten(&spline);
        assert_eq!(bounds.samples.len(), spline.curves.len() * 21);
        assert_eq!(bounds.samples[0], (spline.start_x, spline.start_y));
        for (index, curve) in spline.curves.iter().enumerate() {
            assert_eq!(bounds.samples[index * 21 + 20], (curve.x2, curve.y2));
        }
        assert!(bounds.min_x <= bounds.max_x);
        assert!(bounds.min_y <= bounds.max_y);
    }

    #[test]
    fn flatten_tracks_extremes_first_wins_on_ties() {
        // A straight horizontal Bézier: every sample has the same y, so the
        // first sample must own both y extremes (strict comparisons).
        let spline = Spline {
            start_x: 0,
            start_y: 100,
            curves: vec![Curve {
                cp1x: 100,
                cp1y: 100,
                cp2x: 200,
                cp2y: 100,
                x2: 300,
                y2: 100,
            }],
        };
        let bounds = flatten(&spline);
        assert_eq!(bounds.min_y, 100);
        assert_eq!(bounds.max_y, 100);
        assert_eq!(bounds.min_x, 0);
        assert_eq!(bounds.max_x, 300);
        assert_eq!(bounds.max_width_y, 100);
    }

    #[test]
    fn bezier_endpoints_are_exact() {
        assert_eq!(bezier_sample(10, 20, 30, 40, 0), 10);
        assert_eq!(bezier_sample(10, 20, 30, 40, FLATTEN_STEPS), 40);
        assert_eq!(bezier_sample(-7, 3, 5, 11, 0), -7);
        assert_eq!(bezier_sample(-7, 3, 5, 11, FLATTEN_STEPS), 11);
    }

    #[test]
    fn adjust_point_pulls_toward_center_by_ratio() {
        let ratio = 2 * Q32_ONE;
        assert_eq!(adjust_point(750_000, ratio), 625_000);
        assert_eq!(adjust_point(250_000, ratio), 375_000);
        assert_eq!(adjust_point(-100_000, ratio), 200_000);
        assert_eq!(adjust_point(CENTER, ratio), CENTER);
    }

    #[test]
    fn rescale_leaves_fitting_bodies_untouched() {
        let ring = mirror_ring(&body_with(&[
            point(100_000, 200_000, Q32_ONE),
            point(300_000, 400_000, Q32_ONE),
            point(500_000, 100_000, Q32_ONE),
        ]));
        let mut spline = build_spline(&ring);
        let mut bounds = flatten(&spline);
        let before = spline.clone();
        let width = rescale(&mut spline, &mut bounds);
        assert_eq!(spline, before);
        assert_eq!(width, bounds.max_x - bounds.min_x);
    }

    #[test]
    fn rescale_shrinks_an_overflowing_body() {
        // Points far outside the canvas force final_ratio > 1.
        let ring = mirror_ring(&body_with(&[
            point(2_000_000, 0, Q32_ONE),
            point(2_400_000, 3_000_000, Q32_ONE),
            point(1_800_000, 100_000, Q32_ONE),
        ]));
        let mut spline = build_spline(&ring);
        let mut bounds = flatten(&spline);
        let width = rescale(&mut spline, &mut bounds);
        // After rescale every sample must sit inside the canvas again.
        for &(x, y) in &bounds.samples {
            assert!((-1..=CANVAS_MILLI + 1).contains(&x), "x out of bounds: {x}");
            assert!((-1..=CANVAS_MILLI + 1).contains(&y), "y out of bounds: {y}");
        }
        assert!(width <= CANVAS_MILLI);
    }

    #[test]
    fn min_width_y_picks_the_most_inset_band_midpoint() {
        // Every band 1..=8 carries samples so the empty-band quirk stays out
        // of play; band 3 has the largest left inset (its min x is the
        // greatest), so its midpoint must win.
        let samples = vec![
            (50, 150),
            (40, 250),
            (100, 350),
            (30, 450),
            (35, 550),
            (45, 650),
            (55, 750),
            (60, 850),
        ];
        // Band 3 midpoint: top + 3·h/10 + h/20 = 300 + 50 = 350.
        assert_eq!(min_width_y(&samples, 0, 1_000), 350);
    }

    #[test]
    fn min_width_y_empty_first_band_wins() {
        // Reference quirk: an empty band keeps i64::MAX as its inset and
        // outranks every real sample, so the first empty band's midpoint is
        // returned.
        let samples = vec![(100, 750)];
        assert_eq!(min_width_y(&samples, 0, 1_000), 150);
    }

    #[test]
    fn path_string_layout_matches_the_pinned_shape() {
        let spline = Spline {
            start_x: 500_000,
            start_y: 250_000,
            curves: vec![Curve {
                cp1x: 500_150,
                cp1y: 250_000,
                cp2x: 500_105,
                cp2y: 250_100,
                x2: 500_000,
                y2: 250_200,
            }],
        };
        assert_eq!(
            path_string(&spline),
            "M5000,2500C5001.5 2500, 5001.05 2501, 5000 2502"
        );
    }
}
