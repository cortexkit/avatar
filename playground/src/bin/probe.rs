//! Symmetry probe: measures body-outline and extremity-pair asymmetry per
//! seed, in ring/cell units. Evidence tool for the grid-8 symmetry review.
use ava_playground::{genome, wild};

fn main() {
    for seed in 3000u64..3006 {
        let mut b = genome::Bits::new(seed);
        let wb = wild::decode(&mut b);
        let exts = wild::decode_extremities(&mut b);
        let outline = wild::outline(&wb, 0.32, 120.0, 120.0, 172.0);
        let body_asym = wild::asymmetry(&outline, 120.0);
        let mean_r = wb.spokes.iter().map(|s| s.r).sum::<f64>() / wb.spokes.len() as f64;
        let mut ext_asym = 0.0_f64;
        for e in &exts {
            let right = wild::extremity_outline(e, mean_r, mean_r, false);
            let left = wild::extremity_outline(e, mean_r, mean_r, true);
            let mirrored: Vec<(f64, f64)> = right.iter().map(|p| (-p.0, p.1)).collect();
            let mut worst = 0.0_f64;
            for &(x, y) in &mirrored {
                let mut best = f64::MAX;
                for &(ox, oy) in &left {
                    let d = ((ox - x).powi(2) + (oy - y).powi(2)).sqrt();
                    if d < best {
                        best = d;
                    }
                }
                worst = worst.max(best);
            }
            ext_asym = ext_asym.max(worst);
        }
        println!("seed {seed}: body_asym={body_asym:.3}cell ext_asym={ext_asym:.4}ring n_ext={}", exts.len());
    }
}
