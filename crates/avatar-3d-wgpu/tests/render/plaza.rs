//! The plaza layout: from the projected boxes and anchors, no robot covers
//! another robot's face, and no label touches another label or another
//! robot.

use avatar_3d::motion::Actor;
use avatar_3d::robot::{build_with, BuildOptions};
use avatar_3d_wgpu::plaza::{self, Clash, Plaza};
use avatar_small_robots::Mood;

use crate::genomes;

/// The review sheet's plaza: genomes 0 to 32 of seed 503, neutral, idle
/// off, at t = 0, in a 1600 × 1000 frame.
const SEED: u64 = 503;

#[test]
fn the_plaza_hides_no_face_and_no_label_touches_anything() {
    let prints: Vec<_> = (0..33)
        .map(|i| {
            let robot = build_with(&genomes::random(SEED, i), BuildOptions::untextured());
            let pose = Actor::new(&robot, Mood::Neutral)
                .without_idle()
                .evaluate(0.0);
            plaza::footprint(&robot, &pose)
        })
        .collect();
    let placed = plaza::layout(&prints, 1600, 1000);
    assert_eq!(plaza::clashes(&placed, &prints), Vec::<Clash>::new());
    assert!(
        placed.label_scale >= 2,
        "labels at {} px",
        placed.label_scale
    );

    // The rules do catch a crowd: every robot on one spot covers faces and
    // stacks labels.
    let crowd = Plaza {
        offsets: vec![[0.0; 3]; prints.len()],
        ..placed
    };
    let found = plaza::clashes(&crowd, &prints);
    assert!(found.iter().any(|c| matches!(c, Clash::Face { .. })));
    assert!(found.iter().any(|c| matches!(c, Clash::Labels(..))));
}
