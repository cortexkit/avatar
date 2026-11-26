//! Version 2 of the wild generator moves mood lids onto the eyes that were
//! drawn. These tests measure drawn positions, with every transform applied,
//! at version 2 and at version 1, which must keep its old placement.

use super::*;
use crate::{Mood, Segment, Shape};

/// A committed corpus genome with DotShine eyes and an eye aspect of about
/// 1.13, so its eyes are drawn noticeably narrower than their frame.
const CORPUS_0022: &str = include_str!("../../../../vectors/wild/wild-port-0022.hex");

fn hex_genome(hex: &str) -> [u8; GENOME_LEN] {
    let hex = hex.trim().as_bytes();
    assert_eq!(hex.len(), GENOME_LEN * 2);
    std::array::from_fn(|i| {
        u8::from_str_radix(std::str::from_utf8(&hex[i * 2..i * 2 + 2]).unwrap(), 16).unwrap()
    })
}

/// Maps a point in a primitive's local tree units onto the 240-unit canvas
/// through the primitive's whole transform chain, as a renderer would.
fn drawn(primitive: &Primitive, x: i64, y: i64) -> (f64, f64) {
    let (x, y) = (x as f64, y as f64);
    let Some(transform) = &primitive.transform else {
        return (x / 100.0, y / 100.0);
    };
    let m = transform.affine();
    let q = |value: Q32| value.0 as f64 / (1_u64 << 32) as f64;
    (
        (q(m.a) * x + q(m.c) * y + m.tx as f64) / 100.0,
        (q(m.b) * x + q(m.d) * y + m.ty as f64) / 100.0,
    )
}

/// Horizontal positions for one eye in a sleepy tree, in canvas units.
#[derive(Debug)]
struct EyeLid {
    /// The pupil centre before any transform.
    pupil_local: f64,
    /// The pupil centre where it is drawn.
    pupil: f64,
    /// The drawn centre of the body-coloured lid cover.
    cover: f64,
    /// The drawn midpoint of the lid's definition line.
    line: f64,
}

fn eye_lids(genome: &[u8; GENOME_LEN], generation: Generation) -> Vec<EyeLid> {
    let tree = render_tree(genome, Mood::Sleepy, generation);
    let labelled = |label: &str| -> Vec<&Primitive> {
        tree.primitives
            .iter()
            .filter(|primitive| primitive.label == label)
            .collect()
    };
    let (pupils, covers, lines) = (
        labelled("wild-eye-pupil"),
        labelled("wild-mood-lid-cover"),
        labelled("wild-mood-lid-line"),
    );
    // Left then right in each list, so the lists pair up by index.
    assert_eq!((pupils.len(), covers.len(), lines.len()), (2, 2, 2));
    pupils
        .iter()
        .zip(&covers)
        .zip(&lines)
        .map(|((pupil, cover), line)| {
            let Shape::Circle { cx, cy, .. } = pupil.shape else {
                panic!("a DotShine pupil is a circle")
            };
            let Shape::Rect { x, y, w, h } = cover.shape else {
                panic!("a lid cover is a rectangle")
            };
            let Shape::Path { segments, .. } = &line.shape else {
                panic!("a lid line is a path")
            };
            let ends: Vec<f64> = segments
                .iter()
                .map(|segment| match *segment {
                    Segment::MoveTo { x, y } | Segment::LineTo { x, y } => drawn(line, x, y).0,
                    Segment::CubicTo { .. } => panic!("a lid line is straight"),
                })
                .collect();
            EyeLid {
                pupil_local: cx as f64 / 100.0,
                pupil: drawn(pupil, cx, cy).0,
                cover: drawn(cover, x + w / 2, y + h / 2).0,
                line: ends.iter().sum::<f64>() / ends.len() as f64,
            }
        })
        .collect()
}

#[test]
fn version_2_draws_sleepy_lids_on_aspect_scaled_eyes() {
    // All zero bytes except the maximum eye aspect: DotShine eyes drawn at
    // 1/1.28 of their width about the face axis.
    let mut widest_aspect = [0; GENOME_LEN];
    widest_aspect[177] = 255;
    for (name, genome) in [
        ("byte 177 = 255", widest_aspect),
        ("wild-port-0022", hex_genome(CORPUS_0022)),
    ] {
        // The check needs disc eyes that are visibly narrowed; a genome that
        // drifted out of that shape would pass without measuring anything.
        let creature = decode_genome(&genome);
        assert!(matches!(creature.eyes, face::EyeKind::DotShine), "{name}");
        assert!(creature.face_style.eye_aspect > 1.1, "{name}");

        for lid in eye_lids(&genome, Generation::V2) {
            println!("{name} v2: {lid:?}");
            assert!((lid.cover - lid.pupil).abs() <= 0.05, "{name} v2: {lid:?}");
            assert!((lid.line - lid.pupil).abs() <= 0.05, "{name} v2: {lid:?}");
        }
        // Version 1 still builds the lid at the unscaled eye centre, away
        // from the narrowed eye; rows stored at version 1 keep that drawing.
        for lid in eye_lids(&genome, Generation::V1) {
            println!(
                "{name} v1: {lid:?}, lid offset {:.2}",
                lid.cover - lid.pupil
            );
            assert!(
                (lid.cover - lid.pupil_local).abs() <= 0.05,
                "{name} v1: {lid:?}"
            );
            assert!(
                (lid.line - lid.pupil_local).abs() <= 0.05,
                "{name} v1: {lid:?}"
            );
            assert!((lid.cover - lid.pupil).abs() > 1.0, "{name} v1: {lid:?}");
        }
    }
}
