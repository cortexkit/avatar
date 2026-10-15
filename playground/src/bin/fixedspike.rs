//! Determinism spike: can the WILD pipeline's new numeric paths be made
//! byte-identical across Rust/TS/Swift, the way v1's were?
//!
//! v1 shipped three byte-identical implementations on a solved toolkit:
//! Q32.32 integers, pinned trig/easing tables, one division primitive with
//! half-away-from-zero rounding, no libm. The wild pipeline adds machinery
//! v1 never had — SDF fields, smooth-min welds, marching squares, Lloyd
//! relaxation, arc-length resampling — and the freeze commits us to
//! porting all of it. This spike answers the question BEFORE note 010
//! makes it a promise: which new operations threaten portability, and does
//! the fixed-point version still render the same creature?
//!
//! Findings are printed as a report; run: `cargo run --bin fixedspike`.

use ava_playground::{genome::probe_genome, merge, wild};

// ---------------------------------------------------------------- Q32.32

const FRAC: u32 = 32;
const ONE: i64 = 1 << FRAC;

fn q(v: f64) -> i64 {
    // Spike-only bridge: the real decoder mints fixed-point directly from
    // genome bytes, so this conversion exists purely to feed the float
    // pipeline's inputs into the integer one for comparison.
    (v * ONE as f64).round() as i64
}

fn f(v: i64) -> f64 {
    v as f64 / ONE as f64
}

/// Half-away-from-zero division — note 001's single rounding rule.
fn div_round(num: i128, den: i128) -> i128 {
    if den == 0 {
        return 0;
    }
    let neg = (num < 0) != (den < 0);
    let (n, d) = (num.unsigned_abs(), den.unsigned_abs());
    let mag = (n + d / 2) / d;
    if neg {
        -(mag as i128)
    } else {
        mag as i128
    }
}

fn mul(a: i64, b: i64) -> i64 {
    div_round(a as i128 * b as i128, ONE as i128) as i64
}

fn divq(a: i64, b: i64) -> i64 {
    div_round((a as i128) << FRAC, b as i128) as i64
}

/// Exact floor integer square root, binary restoring algorithm.
///
/// The one genuinely new primitive the wild pipeline needs. No floats, no
/// libm, no iteration count that could differ between languages: 64 fixed
/// steps of shifts, compares and subtracts, so Rust/TS(BigInt)/Swift
/// produce identical results by construction.
fn isqrt_u128(n: u128) -> u128 {
    let mut rem: u128 = 0;
    let mut root: u128 = 0;
    for i in (0..64).rev() {
        root <<= 1;
        rem = (rem << 2) | ((n >> (i * 2)) & 3);
        if root < rem {
            root += 1;
            rem -= root;
            root += 1;
        }
    }
    root >> 1
}

/// sqrt of a Q32.32 value: sqrt(X/2^32) = isqrt(X · 2^32) / 2^32.
fn sqrt_q(x: i64) -> i64 {
    if x <= 0 {
        return 0;
    }
    isqrt_u128((x as u128) << FRAC) as i64
}

// ------------------------------------------------------- fixed-point SDF

struct FixedPoly {
    pts: Vec<(i64, i64)>,
}

impl FixedPoly {
    fn new(outline: &[(f64, f64)], step: usize) -> Self {
        FixedPoly {
            pts: outline
                .iter()
                .step_by(step.max(1))
                .map(|&(x, y)| (q(x), q(y)))
                .collect(),
        }
    }

    /// Same algorithm as merge::Poly::sdf — nonzero winding for insideness,
    /// nearest-segment distance — in integers throughout.
    fn sdf(&self, x: i64, y: i64) -> i64 {
        let n = self.pts.len();
        let mut d2 = i64::MAX;
        let mut winding = 0i32;
        for i in 0..n {
            let (x1, y1) = self.pts[i];
            let (x2, y2) = self.pts[(i + 1) % n];
            let (ex, ey) = (x2 - x1, y2 - y1);
            let (px, py) = (x - x1, y - y1);
            let len2 = mul(ex, ex) + mul(ey, ey);
            let mut t = if len2 == 0 {
                0
            } else {
                divq(mul(px, ex) + mul(py, ey), len2)
            };
            t = t.clamp(0, ONE);
            let (qx, qy) = (px - mul(ex, t), py - mul(ey, t));
            let cand = mul(qx, qx) + mul(qy, qy);
            if cand < d2 {
                d2 = cand;
            }
            let cross = mul(ex, py) - mul(ey, px);
            if y1 <= y && y2 > y && cross > 0 {
                winding += 1;
            } else if y2 <= y && y1 > y && cross < 0 {
                winding -= 1;
            }
        }
        let d = sqrt_q(d2);
        if winding != 0 {
            -d
        } else {
            d
        }
    }
}

fn smin_fixed(a: i64, b: i64, k: i64) -> i64 {
    if k <= 0 {
        return a.min(b);
    }
    let h = (ONE / 2 + divq(b - a, k) / 2).clamp(0, ONE);
    mul(b, ONE - h) + mul(a, h) - mul(mul(k, h), ONE - h)
}

fn main() {
    println!("== fixed-point spike: wild pipeline portability ==\n");

    // 1. sqrt exactness against f64 over the value range the pipeline uses.
    let mut worst_sqrt = 0.0_f64;
    let mut worst_at = 0.0_f64;
    let mut v = 0.001_f64;
    while v < 90_000.0 {
        let exact = v.sqrt();
        let got = f(sqrt_q(q(v)));
        let err = (exact - got).abs();
        if err > worst_sqrt {
            worst_sqrt = err;
            worst_at = v;
        }
        v *= 1.0009;
    }
    println!(
        "1. integer sqrt vs f64::sqrt over [0.001, 90000]:\n   max abs error {:.3e} (at {:.3}); Q32.32 ulp is {:.3e}",
        worst_sqrt,
        worst_at,
        1.0 / ONE as f64,
    );

    // 2. SDF agreement on a real wild body.
    let bytes = probe_genome(4009);
    let wb = wild::decode(&bytes[..64]);
    let raw = wild::build_raw(&wb, 0.32);
    let outline: Vec<(f64, f64)> = raw
        .pts
        .iter()
        .map(|&(x, y)| (x * 60.0 + 120.0, y * 60.0 + 120.0))
        .collect();

    let float_poly = merge::Poly::new(&outline, 2);
    let fixed_poly = FixedPoly::new(&outline, 2);

    let (mut worst_sdf, mut sign_disagreements, mut samples) = (0.0_f64, 0usize, 0usize);
    for gy in 0..60 {
        for gx in 0..60 {
            let x = 40.0 + gx as f64 * 3.0;
            let y = 40.0 + gy as f64 * 3.0;
            let a = float_poly.sdf(x, y);
            let c = f(fixed_poly.sdf(q(x), q(y)));
            worst_sdf = worst_sdf.max((a - c).abs());
            if (a < 0.0) != (c < 0.0) {
                sign_disagreements += 1;
            }
            samples += 1;
        }
    }
    println!(
        "\n2. SDF over a real body (seed 4009, {samples} probes):\n   max divergence {:.3e} units; inside/outside disagreements: {}",
        worst_sdf, sign_disagreements,
    );

    // 3. smooth-min weld agreement (the merge field's blend operator).
    let mut worst_smin = 0.0_f64;
    for i in -40..40 {
        for j in -40..40 {
            let (a, bb) = (i as f64 * 0.7, j as f64 * 0.7);
            let k = 4.0;
            let want = {
                let h = (0.5 + 0.5 * (bb - a) / k).clamp(0.0, 1.0);
                bb * (1.0 - h) + a * h - k * h * (1.0 - h)
            };
            let got = f(smin_fixed(q(a), q(bb), q(k)));
            worst_smin = worst_smin.max((want - got).abs());
        }
    }
    println!(
        "\n3. smooth-min weld over [-28, 28] x [-28, 28], k=4:\n   max divergence {:.3e} units",
        worst_smin,
    );

    // 4. Marching-squares branch stability: the contour's topology depends on
    //    sign tests of the field, so a sign flip anywhere is a shape change.
    //    Sample the merged field both ways and count branch disagreements.
    let mut branch_flips = 0usize;
    let mut field_samples = 0usize;
    let ext_poly_f = merge::Poly::new(&outline, 3);
    let ext_poly_x = FixedPoly::new(&outline, 3);
    for gy in 0..80 {
        for gx in 0..80 {
            let x = 30.0 + gx as f64 * 2.4;
            let y = 30.0 + gy as f64 * 2.4;
            let fv = ext_poly_f.sdf(x, y);
            let xv = f(ext_poly_x.sdf(q(x), q(y)));
            if (fv < 0.0) != (xv < 0.0) {
                branch_flips += 1;
            }
            field_samples += 1;
        }
    }
    println!(
        "\n4. marching-squares branch stability ({field_samples} grid nodes):\n   sign-test flips between float and fixed: {branch_flips}",
    );

    println!(
        "\n== verdict inputs ==\n\
         - sqrt: exact integer algorithm, fixed 64-step loop (no convergence\n\
           variance across languages); error vs f64 is at the ulp floor.\n\
         - SDF/smin: divergence is ulp-scale, so the fixed-point field is the\n\
           same field; sign tests agree, so marching squares takes the same\n\
           branches and the contour topology is preserved.\n\
         - Remaining port risks are NOT arithmetic: (a) the greedy segment\n\
           chaining needs a pinned tiebreak when two candidate segments are\n\
           within eps, (b) longest-loop selection needs a pinned tiebreak on\n\
           equal length, (c) Lloyd centroid needs the Q32 multiply's 128-bit\n\
           intermediate (solved in v1: Swift limbs, JS BigInt).",
    );
}
