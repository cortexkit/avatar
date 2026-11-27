//! The part-mapping table covers every catalogue value, and each kind of
//! entry holds on the robots that show it.

use std::collections::BTreeSet;

use avatar_3d::robot::mapping::{entry, Kind, Texture, MAPPING};
use avatar_3d::robot::{build_with, BuildOptions, TextureContent, FINISHES};
use avatar_small_robots::layout::{Layout, CATALOGUES};
use avatar_small_robots::{decode, Mood};

use crate::cases;

/// The hull fields whose own entries build the hulls a plan arranges.
const HULLS: [&str; 3] = ["head", "chassis", "pelvis"];

#[test]
fn every_catalogue_value_has_exactly_one_entry() {
    for c in CATALOGUES {
        let distinct: BTreeSet<&str> = c.values.iter().copied().collect();
        for value in distinct {
            let n = MAPPING
                .iter()
                .filter(|e| e.field == c.field && e.value == value)
                .count();
            assert_eq!(n, 1, "{}={value} has {n} entries", c.field);
        }
    }
    for e in MAPPING {
        let known = e.field == "component"
            || CATALOGUES
                .iter()
                .any(|c| c.field == e.field && c.values.contains(&e.value));
        assert!(known, "{}={} is not a catalogue value", e.field, e.value);
        match e.kind {
            Kind::Drawable { finish, .. } | Kind::Finish { finish } => {
                assert!(
                    FINISHES.contains(&finish),
                    "{}={}: no finish {finish}",
                    e.field,
                    e.value
                );
            }
            Kind::Assembly { members } => {
                assert!(!members.is_empty());
                for m in members {
                    let defined = HULLS.contains(m)
                        || matches!(
                            entry("component", m).map(|e| e.kind),
                            Some(Kind::Drawable { .. })
                        );
                    assert!(
                        defined,
                        "{}={}: member {m} has no drawable entry",
                        e.field, e.value
                    );
                }
            }
            _ => {}
        }
    }
}

#[test]
fn every_entry_holds_on_the_robots_that_show_it() {
    let reachable = cases::reachable();
    let mut seen = BTreeSet::new();
    for (label, genome) in cases::cases(cases::committed_prefix()) {
        let creature = decode(&genome);
        let layout = Layout::new(&creature);
        let robot = build_with(&genome, BuildOptions::untextured());
        let shown: BTreeSet<(&str, &str)> = cases::values(&layout).into_iter().collect();
        for part in &robot.parts {
            let e = entry(part.entry.0, part.entry.1)
                .unwrap_or_else(|| panic!("{label}: part {} has no entry", part.name));
            assert!(
                matches!(e.kind, Kind::Drawable { .. } | Kind::Assembly { .. }),
                "{label}: part {} comes from a {:?} entry",
                part.name,
                e.kind
            );
            assert!(
                shown.contains(&part.entry),
                "{label}: part {} realises {:?}, which the robot does not show",
                part.name,
                part.entry
            );
        }
        for &(field, value) in &shown {
            seen.insert((field, value));
            let e = entry(field, value)
                .unwrap_or_else(|| panic!("{label}: {field}={value} has no entry"));
            let parts: Vec<_> = robot
                .parts
                .iter()
                .filter(|p| p.entry == (field, value))
                .collect();
            match e.kind {
                Kind::Drawable { .. } => {
                    assert!(
                        !parts.is_empty(),
                        "{label}: drawable {field}={value} has no part"
                    );
                    assert!(parts.iter().all(|p| !p.mesh.triangles.is_empty()));
                }
                Kind::Assembly { members } => {
                    for p in &parts {
                        assert!(
                            members.contains(&p.role),
                            "{label}: {field}={value} has a part {} of role {} outside its members",
                            p.name,
                            p.role
                        );
                    }
                    for m in members {
                        assert!(
                            robot
                                .parts
                                .iter()
                                .any(|p| p.role == *m && !p.mesh.triangles.is_empty()),
                            "{label}: {field}={value} lacks its member {m}"
                        );
                    }
                }
                Kind::Surface { texture } => {
                    assert!(
                        parts.is_empty(),
                        "{label}: surface {field}={value} has geometry"
                    );
                    match texture {
                        Texture::Face => {
                            let faces = robot
                                .textures
                                .iter()
                                .filter(|t| {
                                    matches!(t.binding.content, TextureContent::Face { .. })
                                })
                                .count();
                            assert_eq!(
                                faces, 14,
                                "{label}: {field}={value} needs every face texture"
                            );
                            let drawn = creature
                                .face_layer(Mood::Neutral, false)
                                .expect("a face layer");
                            assert!(
                                !drawn.tree.primitives.is_empty(),
                                "{label}: empty face layer"
                            );
                        }
                        Texture::HeadMarks => {
                            let head = robot.parts.iter().position(|p| p.name == "head").unwrap();
                            assert!(
                                robot.textures.iter().any(|t| t.binding.part == head
                                    && t.binding.content == TextureContent::Marks),
                                "{label}: {field}={value} is not in a head marks texture"
                            );
                        }
                        _ => unreachable!("no catalogue value paints the chassis"),
                    }
                }
                Kind::Finish { finish } => {
                    let metal = robot.materials.iter().find(|m| m.name == "metal");
                    if let Some(metal) = metal {
                        assert_eq!(metal.finish.name, finish, "{label}: metal {value}");
                    }
                }
                Kind::Absence => assert!(
                    parts.is_empty(),
                    "{label}: absent {field}={value} has geometry"
                ),
                _ => unreachable!("every kind is checked"),
            }
        }
    }
    let missed: Vec<_> = reachable.difference(&seen).collect();
    assert!(
        missed.is_empty(),
        "no corpus or coverage robot shows {missed:?}"
    );
}
