//! Ground at rest, and the pinned rest-mesh digests.

use std::collections::BTreeSet;

use avatar_3d::robot::{build_with, BuildOptions};

use crate::cases;

/// For each gear kind, the roles of the parts that stand on the floor at
/// rest, and whether the gear hovers. Written here from the 2D type's gear,
/// not read from the builder.
fn on_floor(kind: &str) -> (&'static [&'static str], bool) {
    match kind {
        "biped" | "piston" | "spring" | "quad" | "hexa" | "tripod" | "pogo" | "hoses" => {
            (&["foot"], false)
        }
        "treads" => (&["tread"], false),
        "track" => (&["track"], false),
        "wheels" | "discs" | "mono" | "ball" => (&["wheel"], false),
        "jet" | "rings" => (&[], true),
        other => panic!("no ground rule for gear {other}"),
    }
}

#[test]
fn grounded_parts_match_the_gear_table_and_nothing_sinks() {
    for (label, genome) in cases::cases(cases::committed_prefix()) {
        let robot = build_with(&genome, BuildOptions::untextured());
        let (roles, hover) = on_floor(robot.gear.kind);
        assert_eq!(
            robot.gear.hover, hover,
            "{label}: {} hover",
            robot.gear.kind
        );
        let lowest = |i: usize| {
            robot.parts[i]
                .mesh
                .positions
                .iter()
                .map(|p| p[1])
                .fold(f64::INFINITY, f64::min)
        };
        let want: BTreeSet<usize> = (0..robot.parts.len())
            .filter(|&i| {
                let p = &robot.parts[i];
                roles.contains(&p.role)
                    && (p.entry.0 == "gear" || p.entry.0 == "foot")
                    && lowest(i) <= 1e-9
            })
            .collect();
        let got: BTreeSet<usize> = robot.grounded.iter().copied().collect();
        assert_eq!(got, want, "{label}: grounded parts of {}", robot.gear.kind);
        assert_eq!(
            got.is_empty(),
            hover,
            "{label}: {} grounding",
            robot.gear.kind
        );
        for &i in &got {
            assert!(
                lowest(i).abs() <= 1e-9,
                "{label}: {} does not touch the floor",
                robot.parts[i].name
            );
        }
        for (i, part) in robot.parts.iter().enumerate() {
            assert!(
                lowest(i) >= -1e-9,
                "{label}: {} goes below the floor",
                part.name
            );
        }
    }
}

#[test]
fn rest_mesh_digests_equal_the_pins() {
    let committed = cases::committed();
    let lines = cases::pin_lines(cases::committed_prefix());
    let labels = |lines: &[String]| -> Vec<String> {
        lines
            .iter()
            .map(|l| l.split('\t').next().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        labels(&lines),
        labels(&committed),
        "the label inventory differs from the pins"
    );
    for (got, want) in lines.iter().zip(&committed) {
        assert_eq!(
            got, want,
            "rerun the robot_pins mint if the change is intended"
        );
    }
}
