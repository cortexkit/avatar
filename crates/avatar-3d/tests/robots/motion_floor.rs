//! The floor under motion: no vertex of any part goes below the floor at
//! any sample of any script, and a landed hover robot's exhaust shrinks
//! instead of sinking.

use avatar_3d::motion::{Actor, Pose};
use avatar_3d::robot::{build_with, BuildOptions, Robot, Skin};
use avatar_small_robots::{Mood, MOODS};

use crate::cases;
use crate::motion::{corpus_robots, local, name, posed, V3};
use crate::scripts::{self, Script};

/// How far below the floor a vertex may round to.
const FLOOR: f64 = -1e-9;

/// For each bone, the bounds relative to its joint of every vertex it
/// carries or weights, and which parts those are. A bone's matrix carries
/// its bounds' corners to a height below every such vertex, so only a bone
/// whose bound dips below the floor needs its vertices posed.
struct Bounds {
    boxes: Vec<Option<[V3; 2]>>,
    parts: Vec<Vec<usize>>,
}

impl Bounds {
    fn new(robot: &Robot) -> Self {
        let n = robot.bones.len();
        let mut boxes: Vec<Option<[V3; 2]>> = vec![None; n];
        let mut parts = vec![Vec::new(); n];
        for (i, part) in robot.parts.iter().enumerate() {
            let mut add = |bone: usize, v: V3| {
                let l = local(robot, bone, v);
                let b = boxes[bone].get_or_insert([l, l]);
                for k in 0..3 {
                    b[0][k] = b[0][k].min(l[k]);
                    b[1][k] = b[1][k].max(l[k]);
                }
                if !parts[bone].contains(&i) {
                    parts[bone].push(i);
                }
            };
            match &part.skin {
                Skin::Rigid { bone } => {
                    for v in &part.mesh.positions {
                        add(*bone, *v);
                    }
                }
                Skin::Skinned { joints, weights } => {
                    for (v, (js, ws)) in part.mesh.positions.iter().zip(joints.iter().zip(weights))
                    {
                        for k in 0..4 {
                            if ws[k] > 0.0 {
                                add(js[k] as usize, *v);
                            }
                        }
                    }
                }
                _ => panic!("a part is rigid or skinned"),
            }
        }
        Self { boxes, parts }
    }

    /// Panics naming the first part with a vertex below the floor.
    fn check(&self, robot: &Robot, pose: &Pose, label: &str) {
        for (j, bx) in self.boxes.iter().enumerate() {
            let Some([lo, hi]) = bx else { continue };
            let m = &pose.bones[j];
            let low: f64 = (0..3)
                .map(|k| (m[4 * k + 1] * lo[k]).min(m[4 * k + 1] * hi[k]))
                .sum::<f64>()
                + m[13];
            if low >= FLOOR {
                continue;
            }
            for &i in &self.parts[j] {
                let y = posed(robot, pose, i)
                    .iter()
                    .map(|p| p[1])
                    .fold(f64::INFINITY, f64::min);
                assert!(
                    y >= FLOOR,
                    "{label}: {} goes {y} below the floor",
                    robot.parts[i].name
                );
            }
        }
    }
}

fn floor_holds(which: impl Fn(Script) -> bool) {
    for (id, robot) in corpus_robots() {
        let bounds = Bounds::new(&robot);
        for mood in MOODS {
            for script in scripts::scripts().into_iter().filter(|s| which(*s)) {
                let actor = script.actor(&robot, mood);
                for t in script.samples(&robot) {
                    let label = format!("{id} {} {} at {t}", name(mood), script.name());
                    bounds.check(&robot, &actor.evaluate(t), &label);
                }
            }
        }
    }
}

#[test]
fn the_floor_holds_through_idle_and_reactions() {
    floor_holds(|s| matches!(s, Script::Idle | Script::React(_)));
}

#[test]
fn the_floor_holds_through_status_loops() {
    floor_holds(|s| matches!(s, Script::Loop(_)));
}

#[test]
fn the_floor_holds_through_mood_changes() {
    floor_holds(|s| matches!(s, Script::MoodChange | Script::Composed));
}

/// The hover robots among the corpus and coverage genomes.
fn hover_robots() -> Vec<(String, Robot)> {
    let out: Vec<(String, Robot)> = cases::cases(cases::committed_prefix())
        .into_iter()
        .map(|(label, g)| (label, build_with(&g, BuildOptions::untextured())))
        .filter(|(_, r)| r.gear.hover)
        .collect();
    assert!(!out.is_empty(), "the cases include hover robots");
    out
}

#[test]
fn hover_robots_keep_the_floor_in_sleepy() {
    for (label, robot) in hover_robots() {
        if label.starts_with("mesh/corpus/") {
            // The corpus genomes run every mood above.
            continue;
        }
        let bounds = Bounds::new(&robot);
        for script in scripts::scripts() {
            let actor = script.actor(&robot, Mood::Sleepy);
            for t in script.samples(&robot) {
                let label = format!("{label} sleepy {} at {t}", script.name());
                bounds.check(&robot, &actor.evaluate(t), &label);
            }
        }
    }
}

#[test]
fn a_hover_exhaust_is_full_while_hovering_and_shrinks_on_landing() {
    for (label, robot) in hover_robots() {
        let jet = robot.bone("jet.0").expect("hover gear has a jet");
        let exhaust = robot.bone("exhaust.0").expect("hover gear has an exhaust");
        assert_eq!(robot.bones[exhaust].parent, Some(jet), "{label}");
        let on_exhaust: Vec<usize> = (0..robot.parts.len())
            .filter(|&i| robot.parts[i].skin == Skin::Rigid { bone: exhaust })
            .collect();
        assert!(!on_exhaust.is_empty(), "{label}: an empty exhaust");
        // Hovering in neutral, the exhaust bone carries the jet's axes
        // unscaled: it is full length.
        let hovering = Actor::new(&robot, Mood::Neutral);
        for k in 0..=360 {
            let pose = hovering.evaluate(f64::from(k) / 120.0);
            assert_eq!(
                pose.bones[exhaust][..12],
                pose.bones[jet][..12],
                "{label}: the exhaust is scaled while hovering"
            );
        }
        // Through a change to sleepy no exhaust vertex goes below the
        // floor, and once landed the exhaust is shorter.
        let mut landing = Actor::new(&robot, Mood::Neutral);
        landing.set_mood(0.5, Mood::Sleepy);
        for k in 0..=360 {
            let t = f64::from(k) / 120.0;
            let pose = landing.evaluate(t);
            for &i in &on_exhaust {
                let y = posed(&robot, &pose, i)
                    .iter()
                    .map(|p| p[1])
                    .fold(f64::INFINITY, f64::min);
                assert!(
                    y >= FLOOR,
                    "{label}: {} at {t} goes {y} below the floor",
                    robot.parts[i].name
                );
            }
        }
        let landed = landing.evaluate(3.0);
        let length = |m: &[f64; 16]| (m[4] * m[4] + m[5] * m[5] + m[6] * m[6]).sqrt();
        assert!(
            length(&landed.bones[exhaust]) < 0.5 * length(&landed.bones[jet]),
            "{label}: the landed exhaust keeps its length"
        );
    }
}
