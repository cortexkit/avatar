use std::collections::BTreeSet;
use std::path::Path;

use avatar_phenotype::{from_canonical_json, MoodResponse, Phenotype, Q32};
use avatar_svg::{build_render_tree, Affine, Effect, Mood, Primitive, RenderParams, TransformOp};
use avatar_tables::{cos_q30, div_round_half_away, sin_q30};

const Q32_ONE: i64 = Q32::ONE.0;

fn params(mood: Mood) -> RenderParams {
    RenderParams {
        mood,
        growth: Q32::ONE,
        canvas: 10_000,
    }
}

fn moods() -> [Mood; 7] {
    [
        Mood::Neutral,
        Mood::Working,
        Mood::Happy,
        Mood::Waiting,
        Mood::Blocked,
        Mood::Sad,
        Mood::Sleepy,
    ]
}

fn corpus_phenotypes() -> Vec<(String, Phenotype)> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors/phenotype");
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .expect("read phenotype corpus")
        .map(|entry| entry.expect("read phenotype entry").path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("json"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "phenotype corpus must not be empty");
    paths
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .expect("phenotype filename")
                .to_string_lossy()
                .into_owned();
            let bytes = std::fs::read(&path).expect("read phenotype JSON");
            let phenotype = from_canonical_json(&bytes).expect("parse phenotype JSON");
            (name, phenotype)
        })
        .collect()
}

#[test]
fn corpus_trees_are_flat_and_every_primitive_is_labeled() {
    for (name, phenotype) in corpus_phenotypes() {
        for mood in moods() {
            let tree = build_render_tree(&phenotype, &params(mood)).expect("build render tree");
            assert!(!tree.primitives.is_empty(), "{name} {mood:?}");
            let root_ops = tree.primitives[0]
                .transform
                .as_ref()
                .expect("body transform")
                .ops
                .clone();
            for primitive in &tree.primitives {
                assert_resolved(primitive, &root_ops, &name, mood);
            }
        }
    }
}

fn assert_resolved(primitive: &Primitive, root_ops: &[TransformOp], name: &str, mood: Mood) {
    assert!(
        !primitive.label.is_empty(),
        "{name} {mood:?}: unlabeled primitive"
    );
    let transform = primitive
        .transform
        .as_ref()
        .unwrap_or_else(|| panic!("{name} {mood:?}: {} has no transform", primitive.label));
    assert!(
        transform.ops.starts_with(root_ops),
        "{name} {mood:?}: {} lacks the resolved root transform",
        primitive.label
    );
    for effect in &primitive.effects {
        if let Effect::Masked { mask } = effect {
            assert!(!mask.is_empty(), "{name} {mood:?}: empty mask");
            for item in mask {
                assert!(
                    item.effects
                        .iter()
                        .all(|effect| !matches!(effect, Effect::Masked { .. })),
                    "{name} {mood:?}: nested mask"
                );
                assert_resolved(item, root_ops, name, mood);
            }
        }
    }
}

#[test]
fn corpus_transforms_affine_equals_naive_operation_composition() {
    for (name, phenotype) in corpus_phenotypes() {
        for mood in moods() {
            let tree = build_render_tree(&phenotype, &params(mood)).expect("build render tree");
            for primitive in &tree.primitives {
                assert_affines(primitive, &name, mood);
            }
        }
    }
}

fn assert_affines(primitive: &Primitive, name: &str, mood: Mood) {
    if let Some(transform) = &primitive.transform {
        assert_eq!(
            transform.affine(),
            naive_affine(&transform.ops),
            "{name} {mood:?}: {}",
            primitive.label
        );
    }
    for effect in &primitive.effects {
        if let Effect::Masked { mask } = effect {
            for item in mask {
                assert_affines(item, name, mood);
            }
        }
    }
}

#[test]
fn neutral_tree_equals_every_mood_with_zeroed_response() {
    for (name, phenotype) in corpus_phenotypes() {
        let neutral = build_render_tree(&phenotype, &params(Mood::Neutral))
            .expect("build neutral render tree");
        let mut zeroed = phenotype.clone();
        zeroed.mood_response = MoodResponse {
            lid_reactivity: Q32(0),
            brow_reactivity: Q32(0),
            eye_rotation_reactivity: Q32(0),
            mouth_reactivity: Q32(0),
        };
        for mood in moods() {
            let actual =
                build_render_tree(&zeroed, &params(mood)).expect("build zero-response render tree");
            assert_eq!(neutral, actual, "{name} {mood:?}");
        }
    }
}

#[test]
fn every_corpus_tree_has_exactly_one_resolved_light_y_value() {
    for (name, phenotype) in corpus_phenotypes() {
        for mood in moods() {
            let tree = build_render_tree(&phenotype, &params(mood)).expect("build render tree");
            let values: BTreeSet<_> = tree
                .primitives
                .iter()
                .flat_map(|primitive| primitive.effects.iter())
                .filter_map(|effect| match effect {
                    Effect::Lit { light_y } => Some(*light_y),
                    _ => None,
                })
                .collect();
            assert_eq!(values.len(), 1, "{name} {mood:?}: {values:?}");
        }
    }
}

fn naive_affine(ops: &[TransformOp]) -> Affine {
    let mut result = Affine {
        a: Q32::ONE,
        b: Q32(0),
        c: Q32(0),
        d: Q32::ONE,
        tx: 0,
        ty: 0,
    };
    for operation in ops {
        let next = match *operation {
            TransformOp::Translate { tx, ty } => Affine {
                a: Q32::ONE,
                b: Q32(0),
                c: Q32(0),
                d: Q32::ONE,
                tx,
                ty,
            },
            TransformOp::Scale { sx, sy } => Affine {
                a: sx,
                b: Q32(0),
                c: Q32(0),
                d: sy,
                tx: 0,
                ty: 0,
            },
            TransformOp::Rotate { angle, cx, cy } => {
                let cosine = i64::from(cos_q30(angle.0)) * 4;
                let sine = i64::from(sin_q30(angle.0)) * 4;
                Affine {
                    a: Q32(cosine),
                    b: Q32(sine),
                    c: Q32(-sine),
                    d: Q32(cosine),
                    tx: cx - mul_q32(cx, cosine) + mul_q32(cy, sine),
                    ty: cy - mul_q32(cx, sine) - mul_q32(cy, cosine),
                }
            }
        };
        result = multiply(result, next);
    }
    result
}

fn multiply(lhs: Affine, rhs: Affine) -> Affine {
    Affine {
        a: Q32(mul_q32(lhs.a.0, rhs.a.0) + mul_q32(lhs.c.0, rhs.b.0)),
        b: Q32(mul_q32(lhs.b.0, rhs.a.0) + mul_q32(lhs.d.0, rhs.b.0)),
        c: Q32(mul_q32(lhs.a.0, rhs.c.0) + mul_q32(lhs.c.0, rhs.d.0)),
        d: Q32(mul_q32(lhs.b.0, rhs.c.0) + mul_q32(lhs.d.0, rhs.d.0)),
        tx: lhs.tx + mul_q32(rhs.tx, lhs.a.0) + mul_q32(rhs.ty, lhs.c.0),
        ty: lhs.ty + mul_q32(rhs.tx, lhs.b.0) + mul_q32(rhs.ty, lhs.d.0),
    }
}

fn mul_q32(value: i64, factor: i64) -> i64 {
    div_round_half_away(i128::from(value) * i128::from(factor), i128::from(Q32_ONE)) as i64
}
