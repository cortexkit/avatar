//! Layout and standalone-layer contracts against the original emitters.

use std::collections::{BTreeMap, BTreeSet};

use avatar_svg::{Primitive, Segment, Shape, Transform, TransformOp};

use crate::body::chassis_rivets;
use crate::draw::{El, Out};
use crate::face_draw::{face, lens_eye};
use crate::geom::{tree_ops, Op, Path};
use crate::marks::head_marks;
use crate::mood::Shut;
use crate::parts::{pod_eye, stalk_at};
use crate::{decode, Creature, Mood, Tech, FULL_DETAIL_PX, GENOME_LEN, MOODS};

use super::bounds::Bounds;
use super::layers::{detail, shift};
use super::{Layer, Layout, CATALOGUES};

fn corpus() -> Vec<Box<[u8; GENOME_LEN]>> {
    crate::tests::corpus_ids()
        .map(|id| crate::tests::genome(&id))
        .collect()
}

#[test]
fn layout_ignores_reserved_bytes_in_every_corpus_mood() {
    for bytes in corpus() {
        let original = decode(&bytes);
        let baseline = MOODS.map(|m| original.render_tree(m, Some(FULL_DETAIL_PX)));
        for slots in [6144..6154, 14336..14346] {
            for value in [0x00, 0xff] {
                let mut genome = bytes.clone();
                genome[slots.clone()].fill(value);
                let variant = decode(&genome);
                for (i, mood) in MOODS.into_iter().enumerate() {
                    assert_eq!(
                        variant.render_tree(mood, Some(FULL_DETAIL_PX)),
                        baseline[i],
                        "bytes {slots:?} := {value:#04x}, {mood:?}"
                    );
                }
            }
        }
    }
}

fn unshift(layer: &Layer) -> Vec<Primitive> {
    let mut prims = layer.tree.primitives.clone();
    for p in &mut prims {
        shift(p, layer.region.origin_grid);
    }
    prims
}

fn prefix(prims: &[Primitive], ops: &[TransformOp]) -> Vec<Primitive> {
    prims
        .iter()
        .map(|p| {
            let mut p = p.clone();
            let mut chain = ops.to_vec();
            if let Some(local) = &p.transform {
                chain.extend_from_slice(&local.ops);
            }
            p.transform = (!chain.is_empty()).then_some(Transform { ops: chain });
            p
        })
        .collect()
}

fn contains_run(full: &[Primitive], run: &[Primitive], what: &str, mood: Mood) {
    if !run.is_empty() {
        assert!(
            full.windows(run.len()).any(|w| w == run),
            "missing {what} under the fit/pose/local chain in {mood:?}"
        );
    }
}

#[test]
fn layout_face_and_marks_match_emitter_and_full_tree_transform_chain() {
    for bytes in corpus() {
        let creature = decode(&bytes);
        let c = &creature.robot;
        let layout = Layout::new(&creature);
        let ctx = detail();
        let mut head = Out::new();
        head.group("rb-head-shell", &[], |out| head_marks(out, &ctx, c));
        let mut chassis = Out::new();
        chassis.group("rb-chassis-shell", &[], |out| chassis_rivets(out, &ctx, c));
        let marks: Vec<_> = head
            .prims
            .iter()
            .filter(|p| p.label != "rb-lid")
            .cloned()
            .collect();
        assert_eq!(
            creature.head_marks().map_or_else(Vec::new, |l| unshift(&l)),
            marks
        );
        assert_eq!(
            creature
                .chassis_marks()
                .map_or_else(Vec::new, |l| unshift(&l)),
            chassis.prims
        );
        for mood in MOODS {
            let m = layout.mood(mood);
            let fit = c.fit;
            let mut upper_ops = tree_ops(&[Op::T(fit.x, fit.y), Op::S(fit.s, fit.s)]);
            upper_ops.extend(tree_ops(&[
                Op::T(0.0, -(layout.rise + m.lift)),
                Op::R(m.lean),
            ]));
            let mut head_ops = upper_ops.clone();
            if layout.chassis.is_some() {
                head_ops.extend(tree_ops(&[
                    Op::T(0.0, layout.head.origin[1] + m.drop),
                    Op::R(m.tilt),
                ]));
            }
            let mut bare = Out::new();
            face(&mut bare, &ctx, &c.f, &c.p, &c.head, c.mood(mood));
            assert_eq!(
                unshift(&creature.face_layer(mood, false).unwrap()),
                bare.prims
            );
            let full = creature.render_tree(mood, Some(FULL_DETAIL_PX));
            contains_run(
                &full.primitives,
                &prefix(&bare.prims, &head_ops),
                "face",
                mood,
            );
            contains_run(
                &full.primitives,
                &prefix(&head.prims, &head_ops),
                "head shell marks",
                mood,
            );
            contains_run(
                &full.primitives,
                &prefix(&chassis.prims, &upper_ops),
                "chassis rivets",
                mood,
            );
        }
    }
}

const SEED: u64 = 1;
const LIMIT: u64 = 4_096;
const COVERAGE_PREFIX: u64 = 70;

fn sheet_genome(index: u64) -> Box<[u8; GENOME_LEN]> {
    // Same derivation as the random_sheet example; the keyed hash is valid
    // for any genome length, including the small-robots genome used here.
    let mut key = [0_u8; 32];
    key[..8].copy_from_slice(&SEED.to_le_bytes());
    key[8..16].copy_from_slice(&index.to_le_bytes());
    let mut hasher = blake3::Hasher::new_keyed(&key);
    hasher.update(b"avatar random_sheet genome");
    let mut bytes = Box::new([0; GENOME_LEN]);
    hasher.finalize_xof().fill(bytes.as_mut_slice());
    bytes
}

fn note_choices(c: &Creature, seen: &mut BTreeMap<&'static str, BTreeSet<&'static str>>) {
    let r = &c.robot;
    let cases = [
        ("plan", Some(c.plan)),
        ("head", Some(c.hull)),
        ("chassis", c.chassis),
        ("pelvis", r.pelvis.as_ref().map(|p| p.kind)),
        ("neck", c.neck),
        ("face", Some(c.face.name())),
        ("screen_eye", c.screen.map(|(_, style)| style)),
        ("mouth", Some(c.mouth.name())),
        ("top", Some(c.top)),
        ("tip", Some(r.tip).filter(|s| !s.is_empty())),
        ("side", Some(c.side)),
        ("arms", Some(c.arms)),
        ("hand", Some(c.hand)),
        ("hand", Some(c.hand_left)),
        ("gear", Some(c.gear)),
        ("foot", Some(r.gear.foot_kind).filter(|s| !s.is_empty())),
        ("chest", Some(c.chest)),
        (
            "decal",
            Some(r.decor.decal.as_ref().map_or("none", |p| p.kind)),
        ),
        ("decor", Some(r.decor.scheme)),
        (
            "wear",
            Some(r.decor.wear.as_ref().map_or("none", |p| p.kind)),
        ),
        ("back", Some(c.back)),
        ("metal", Some(c.metal)),
    ];
    for (field, value) in cases {
        if let Some(value) = value {
            let catalogue = CATALOGUES.iter().find(|cat| cat.field == field).unwrap();
            assert!(
                catalogue.is_reachable(value),
                "decoder produced {field}={value} outside reachable catalogue"
            );
            seen.entry(field).or_default().insert(value);
        }
    }
}

fn missing(seen: &BTreeMap<&str, BTreeSet<&str>>) -> Vec<(&'static str, &'static str)> {
    CATALOGUES
        .iter()
        .flat_map(|cat| {
            cat.values.iter().filter_map(|&kind| {
                (cat.is_reachable(kind) && !seen.get(cat.field).is_some_and(|s| s.contains(kind)))
                    .then_some((cat.field, kind))
            })
        })
        .collect()
}

fn assert_on_canvas(layer: &Layer) {
    assert_eq!(
        layer.tree.extent() as f64,
        f64::from(layer.region.extent) * super::GRID
    );
    assert_eq!(layer.region.scale, 1.0);
    let c = f64::from(layer.tree.canvas);
    for p in &layer.tree.primitives {
        if let Some(b) = Bounds::primitive(p).nonempty() {
            for i in 0..2 {
                assert!(
                    b.min[i] >= 0.0 && b.max[i] <= c,
                    "{} on axis {i}: {b:?} not inside canvas {c}",
                    p.label
                );
                assert!(
                    b.max[i] <= layer.region.size[i],
                    "{} on axis {i}: {b:?} not inside unpadded region {:?}",
                    p.label,
                    layer.region
                );
            }
        }
    }
}

fn check_canvases(c: &Creature) {
    let layout = Layout::new(c);
    for mood in MOODS {
        let m = layout.mood(mood);
        for blink in [false, true] {
            let face = c.face_layer(mood, blink).unwrap();
            assert_eq!(layout.face.region, Some(face.region));
            for e in &layout.face.eyes {
                assert!(face.region.contains(e.centre));
            }
            assert!(face.region.contains(layout.face.mouth_centre));
            assert_on_canvas(&face);
            for (i, stalk) in m.stalks.iter().enumerate() {
                let eye = c.stalk_eye_layer(i, mood, blink).unwrap();
                assert_eq!(layout.top.stalks[i].region, eye.region);
                assert!(eye.region.contains(stalk.eye_centre));
                assert_on_canvas(&eye);
            }
        }
    }
    for marks in [c.head_marks(), c.chassis_marks()].into_iter().flatten() {
        assert_on_canvas(&marks);
    }
}

#[test]
fn layout_catalogue_coverage_and_canvas_bounds() {
    let mut seen = BTreeMap::new();
    for genome in corpus() {
        let c = decode(&genome);
        note_choices(&c, &mut seen);
        check_canvases(&c);
    }
    for index in 0..LIMIT {
        if missing(&seen).is_empty() {
            assert_eq!(index, COVERAGE_PREFIX, "coverage prefix changed");
            println!("coverage: random_sheet seed {SEED}, prefix length {index}");
            return;
        }
        let c = decode(&sheet_genome(index));
        note_choices(&c, &mut seen);
        check_canvases(&c);
    }
    assert!(
        missing(&seen).is_empty(),
        "missing catalogue values after seed {SEED}, {LIMIT} genomes: {:?}",
        missing(&seen)
    );
}

#[test]
fn layout_bounds_include_cubic_extremum_and_stroke() {
    let mut path = Path::new();
    path.move_to([-5.0, 0.0]);
    path.cubic_to([-5.0, 20.0], [5.0, 20.0], [5.0, 0.0]);
    let mut out = Out::new();
    out.put(El::path(&path).stroke(crate::kit::hex(0), 2.0).round());
    let b = Bounds::primitive(&out.prims[0]);
    assert_eq!(b.min, [-6.0, -1.0]);
    // The cubic reaches y = 15 at t = 1/2, not y = 20 at its controls.
    assert_eq!(b.max, [6.0, 16.0]);
}

fn fixture(pred: impl Fn(&Creature) -> bool) -> Creature {
    corpus()
        .into_iter()
        .chain((0..256).map(sheet_genome))
        .map(|g| decode(&g))
        .find(pred)
        .expect("fixture in corpus or first 256 sheet genomes")
}

#[test]
fn layout_fractional_canvas_round_trips_shape_and_stripe_clip() {
    let c = fixture(|c| c.robot.decor.scheme == "stripe");
    let layer = c.head_marks().unwrap();
    let region = layer.region;
    assert!(region.origin.into_iter().any(|v| v.fract() != 0.0));
    assert_ne!(region.size[0], region.size[1]);
    assert_eq!(
        region.extent,
        region.size[0].max(region.size[1]).ceil() as u32
    );
    let mut raw = Out::new();
    raw.group("rb-head-shell", &[], |o| head_marks(o, &detail(), &c.robot));
    raw.prims.retain(|p| p.label != "rb-lid");
    assert_eq!(unshift(&layer), raw.prims);

    let stripe = layer
        .tree
        .primitives
        .iter()
        .find(|p| p.label == "rb-stripe")
        .unwrap();
    let source = raw.prims.iter().find(|p| p.label == "rb-stripe").unwrap();
    let tx = stripe.transform.as_ref().unwrap().affine();
    assert_eq!([tx.tx, tx.ty], region.origin_grid.map(|x| -x));
    assert_eq!(stripe.shape, source.shape);
    assert_eq!(stripe.clip.as_ref().unwrap(), source.clip.as_ref().unwrap());
    assert!(source.transform.is_none());
}

#[test]
fn layout_unmarked_hulls_have_no_layers() {
    let c = fixture(|c| {
        let r = &c.robot;
        r.body.is_some()
            && !r.body_rivets
            && r.side_vents.is_none()
            && r.decor.scheme == "plain"
            && r.decor.rows.is_empty()
            && r.decor.decal.is_none()
            && r.decor.wear.is_none()
    });
    assert_eq!(c.chassis_marks(), None);
    assert_eq!(c.head_marks(), None);
}

fn assert_flat_blink(prims: &[Primitive], label: &str, count: usize) {
    let lines: Vec<_> = prims.iter().filter(|p| p.label == label).collect();
    assert_eq!(lines.len(), count);
    for p in lines {
        assert!(p.stroke.is_some());
        let Shape::Path {
            segments,
            closed: false,
            ..
        } = &p.shape
        else {
            panic!("expected one open path")
        };
        let [Segment::MoveTo { x: x0, y: y0 }, Segment::LineTo { x: x1, y: y1 }] =
            segments.as_slice()
        else {
            panic!("expected one segment, got {segments:?}");
        };
        assert_eq!(y0, y1);
        assert!(x0 < x1);
    }
}

#[test]
fn layout_blink_changes_only_eyes_and_preserves_mood_closed_forms() {
    let c = fixture(|c| {
        c.face == Tech::Lens
            && c.robot
                .f
                .eyes
                .iter()
                .all(|e| c.robot.mood(Mood::Neutral).eye_for(e.side).closed.is_none())
    });
    let open = c.face_layer(Mood::Neutral, false).unwrap();
    let blink = c.face_layer(Mood::Neutral, true).unwrap();
    assert_eq!(open.region, blink.region);
    assert_ne!(open.tree, blink.tree);
    assert_flat_blink(&blink.tree.primitives, "rb-lid-edge", c.robot.f.eyes.len());

    let mut without_eyes = c.robot.f.clone();
    without_eyes.eyes.clear();
    // Use the mouth drawing as an independent non-eye comparison. Eyebrows
    // are checked when comparing the two layers below.
    let mut m = c.robot.mood(Mood::Neutral).clone();
    m.brows = None;
    let mut bare = Out::new();
    face(
        &mut bare,
        &detail(),
        &without_eyes,
        &c.robot.p,
        &c.robot.head,
        &m,
    );
    let non_eye = |layer: &Layer| {
        unshift(layer)
            .into_iter()
            .filter(|p| {
                !matches!(
                    p.label.as_str(),
                    "rb-eye-rim"
                        | "rb-eye-glass"
                        | "rb-eye-core"
                        | "rb-iris"
                        | "rb-pupil"
                        | "rb-glint"
                        | "rb-lid-upper"
                        | "rb-lid-lower"
                        | "rb-lid-edge"
                )
            })
            .collect::<Vec<_>>()
    };
    let as_drawn = non_eye(&open);
    assert_eq!(as_drawn, non_eye(&blink));
    assert!(as_drawn
        .windows(bare.prims.len())
        .any(|run| run == bare.prims));

    let c = fixture(|c| {
        c.face != Tech::Stalk
            && c.robot.f.eyes.iter().all(|e| {
                c.robot
                    .mood(Mood::Sleepy)
                    .eye_for(e.side)
                    .closed
                    .is_some_and(|s| s == Shut::ArcDown || s == Shut::ArcUp)
            })
    });
    assert_eq!(
        c.face_layer(Mood::Sleepy, false),
        c.face_layer(Mood::Sleepy, true)
    );
}

#[test]
fn layout_stalk_eyes_draw_separately_from_mouth() {
    let c = fixture(|c| c.face == Tech::Stalk);
    let r = &c.robot;
    let m = r.mood(Mood::Neutral);
    let face = c.face_layer(Mood::Neutral, false).unwrap();
    assert_eq!(Some(&face), c.face_layer(Mood::Neutral, true).as_ref());
    let mut mouth_only = Out::new();
    crate::face_draw::mouth(&mut mouth_only, &detail(), &r.f, &r.p, m);
    assert!(!mouth_only.is_empty());
    assert_eq!(unshift(&face), mouth_only.prims);
    for (i, side) in [-1.0, 1.0].into_iter().enumerate() {
        let open = c.stalk_eye_layer(i, Mood::Neutral, false).unwrap();
        let shut = c.stalk_eye_layer(i, Mood::Neutral, true).unwrap();
        assert_ne!(open.tree, shut.tree);
        let (_, centre) = stalk_at(r, side, m);
        let mut e = pod_eye(r, side);
        (e.x, e.y) = (centre[0], centre[1]);
        let mut raw = Out::new();
        lens_eye(&mut raw, &detail(), &r.f, &r.p, &e, &m.eye_for(side), m);
        assert_eq!(unshift(&open), raw.prims);
        assert_flat_blink(&shut.tree.primitives, "rb-lid-edge", 1);
    }
    assert!(c.stalk_eye_layer(2, Mood::Neutral, false).is_none());
    let non_stalk = fixture(|c| c.face != Tech::Stalk);
    assert!(non_stalk.stalk_eye_layer(0, Mood::Neutral, false).is_none());
}

#[test]
fn layout_catalogue_exclusions_follow_decoder_weights_and_overrides() {
    let expected = BTreeSet::from([
        ("arms", "none"),
        ("screen_eye", "wide"),
        ("mouth", "none"),
        ("decor", "dipped"),
    ]);
    let actual = CATALOGUES
        .iter()
        .flat_map(|cat| {
            assert!(cat
                .exclusions
                .iter()
                .all(|(v, reason)| cat.values.contains(v) && !reason.is_empty()));
            cat.values
                .iter()
                .filter_map(|value| (!cat.is_reachable(value)).then_some((cat.field, *value)))
        })
        .collect();
    assert_eq!(expected, actual);

    let mut bytes = corpus().into_iter().next().unwrap();
    let mut arms = BTreeMap::new();
    for value in 0..32 {
        bytes[1072] = value;
        *arms.entry(decode(&bytes).arms).or_insert(0) += 1;
    }
    assert_eq!(
        arms,
        BTreeMap::from([
            ("rod", 9),
            ("shell", 8),
            ("hose", 7),
            ("telescope", 5),
            ("stub", 3)
        ])
    );

    let mut bytes = corpus()
        .into_iter()
        .chain((0..256).map(sheet_genome))
        .find(|g| decode(g).face == Tech::Screen)
        .unwrap();
    let mut eyes = BTreeMap::new();
    for value in 0..32 {
        bytes[1050] = value;
        *eyes.entry(decode(&bytes).screen.unwrap().1).or_insert(0) += 1;
    }
    assert_eq!(
        eyes,
        BTreeMap::from([
            ("dot", 12),
            ("pill", 8),
            ("square", 5),
            ("ring", 4),
            ("pupil", 3)
        ])
    );
}
