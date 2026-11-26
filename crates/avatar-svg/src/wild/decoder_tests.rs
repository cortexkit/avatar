use super::*;

#[test]
fn region_offsets_and_slot_strides_match_the_byte_table() {
    let mut bytes = [0; genome::GENOME_LEN];
    bytes[0] = 11;
    for i in 0..16 {
        bytes[2 + 3 * i] = 255;
        bytes[3 + 3 * i] = 0;
        bytes[4 + 3 * i] = 255;
    }
    bytes[64] = 4;
    bytes[65] = 2;
    for i in 0..4 {
        let slot = 68 + 10 * i;
        bytes[slot..slot + 5].copy_from_slice(&[255, 0, 255, 0, 255]);
        bytes[slot + 5] = i as u8;
    }
    bytes[160..164].copy_from_slice(&[7, 255, 255, 3]);
    bytes[176..183].copy_from_slice(&[4, 255, 0, 5, 255, 0, 3]);
    bytes[240..244].copy_from_slice(&[2, 5, 0x12, 0x34]);
    let c = decode_genome(&bytes);
    assert_eq!(c.wb.spokes.len(), 16);
    for (i, spoke) in c.wb.spokes.iter().enumerate() {
        let expected_theta =
            (i as f64 / 16.0) * std::f64::consts::TAU + 0.45 * std::f64::consts::TAU / 16.0;
        assert_eq!(spoke.theta, expected_theta);
        assert_eq!(spoke.r, 0.12);
        assert_eq!(spoke.tension, 2.6);
    }
    assert_eq!(c.exts.len(), 4);
    assert_eq!(c.ext_fills, [0, 1, 2, 0]);
    for ext in &c.exts {
        assert_eq!(ext.theta, std::f64::consts::TAU * 1.02);
        assert_eq!(ext.len, 0.35);
        assert_eq!(ext.width, 0.30);
        assert_eq!(ext.curl, -1.3);
        assert_eq!(ext.taper, 0.85);
    }
    assert_eq!(c.topology, 2);
    assert_eq!(c.recipe, 3);
    assert_eq!(c.hue, (1.0 + 255.0 / 65535.0) * 360.0);
    assert!(c.hue > 360.0);
    assert_eq!(c.belly_variant, 3);
    assert!(matches!(c.eyes, face::EyeKind::HalfLid));
    assert!(matches!(c.mouth, face::MouthKind::Fangs));
    assert_eq!(c.face_style.eye_aspect, 1.28);
    assert_eq!(c.face_style.eye_wonk, -0.20);
    assert_eq!(c.face_style.mouth_w, 1.30);
    assert_eq!(c.face_style.mouth_d, 0.70);
    assert_eq!(c.face_style.brow, 3);
    assert_eq!(c.dom, 0.32);
    assert!(!c.voronoi);
    assert_eq!(c.sites.len(), 26);
    let mut source = genome::Bits::new(0x1234 ^ 0x5175_7266_6163_6501);
    assert_eq!(c.sites[0].x, source.lin(0.04, 1.05));
    assert_eq!(c.sites[0].y, source.lin(-1.05, 1.05));
    assert_eq!(c.sites[1].x, -c.sites[0].x);
}

#[test]
fn ignored_bytes_and_inactive_slots_do_not_change_the_tree() {
    let bytes = [0; genome::GENOME_LEN];
    let json = |bytes: &[u8; genome::GENOME_LEN]| {
        crate::serialize_tree_json(&RenderTree {
            canvas: 240,
            background: None,
            pixel_grid: None,
            primitives: draw_full(
                &decode_genome(bytes),
                mood::Mood::Neutral,
                120.0,
                120.0,
                Generation::LATEST,
            ),
        })
    };
    let expected = json(&bytes);
    let mut changed = bytes;
    let mut active = vec![0, 64, 65, 160, 161, 162, 163, 240, 241, 242, 243];
    active.extend(2..17);
    active.extend(176..183);
    for (i, byte) in changed.iter_mut().enumerate() {
        if !active.contains(&i) {
            *byte = 255;
        }
    }
    assert_eq!(json(&changed), expected);
}

#[test]
fn wild_totality_extremes_and_64_probe_genomes_at_every_mood() {
    let genomes = [[0; GENOME_LEN], [255; GENOME_LEN]]
        .into_iter()
        .chain((0..64).map(probe_genome));
    for (index, bytes) in genomes.enumerate() {
        for mood in mood::ALL {
            let tree = crate::render_tree_for_bytes(&bytes, crate::WILD, 1, mood)
                .unwrap_or_else(|error| panic!("genome {index}, mood {mood:?}: {error}"));
            assert_eq!(tree.canvas, 240);
            assert_eq!(tree.background, None);
            assert!(!tree.primitives.is_empty());
        }
    }
}

/// Prints a tree hash and render timing for comparing signed distance field implementations.
#[test]
#[ignore = "comparison probe; hashes 301 genomes in all seven moods"]
fn weld_byte_identity_and_timing() {
    use std::{hint::black_box, time::Instant};

    let mut weld = [0; GENOME_LEN];
    weld[0] = 11;
    weld[64] = 4;
    weld[240] = 2;
    for i in 0..4 {
        weld[68 + i * 10] = (63 * i) as u8;
        weld[69 + i * 10] = 200;
    }
    let render =
        |bytes: &[u8], mood| crate::render_tree_for_bytes(bytes, crate::WILD, 1, mood).unwrap();
    let mut hash = blake3::Hasher::new();
    let mut count = 0;
    for genome in (0..300).map(probe_genome).chain(std::iter::once(weld)) {
        for mood in mood::ALL {
            let json = crate::serialize_tree_json(&render(&genome, mood));
            hash.update(&(json.len() as u64).to_le_bytes());
            hash.update(json.as_bytes());
            count += 1;
        }
    }
    assert_eq!(count, 2_107);
    println!("{count} canonical trees: {}", hash.finalize().to_hex());

    black_box(render(&weld, mood::Mood::Neutral));
    let mut ms = [0.0; 101];
    for time in &mut ms {
        let start = Instant::now();
        let tree = black_box(render(black_box(&weld), mood::Mood::Neutral));
        *time = start.elapsed().as_secs_f64() * 1e3;
        drop(tree);
    }
    ms.sort_by(f64::total_cmp);
    println!("four-pair weld, median of 101 renders: {:.3} ms", ms[50]);
}

#[test]
fn comparison_preserves_finite_order_and_equal_signed_zeros() {
    use std::cmp::Ordering;
    for a in [-f64::MAX, -1.0, -0.0, 0.0, 1.0, f64::MAX] {
        for b in [-f64::MAX, -1.0, -0.0, 0.0, 1.0, f64::MAX] {
            assert_eq!(math::compare(a, b), a.partial_cmp(&b).unwrap());
        }
    }
    assert_eq!(math::compare(-0.0, 0.0), Ordering::Equal);
    assert_eq!(math::compare(f64::NAN, 0.0), f64::NAN.total_cmp(&0.0));
}

/// Every wild body is mirror-symmetric, so its belly must be centred on the
/// mirror axis. The walk once followed "the span nearest the previous one"
/// below a leg split, an exact tie between mirror-image legs that floating
/// point settled at random, so about four bellies in ten ran down one leg.
/// The sample covers many split-leg bodies: in the first 28 probe seeds, 12
/// split inside the belly band.
#[test]
fn bellies_are_centred_on_the_mirror_axis() {
    let mut checked = 0;
    for seed in 0..300u64 {
        let c = decode_genome(&probe_genome(seed));
        let outline = outline_for(&c, 120.0, 120.0);
        let Some(p) = place_face(&outline, c.face_style.mouth_w, c.face_style.mouth_d) else {
            continue;
        };
        let belly = belly_patch(&outline, 120.0, belly_top_for(p.eye_y, p.r), 0.72);
        if belly.len() < 3 {
            continue;
        }
        let (x0, _, x1, _) = shape::bbox(&belly);
        let offset = ((x0 + x1) / 2.0 - 120.0).abs();
        assert!(
            offset < 1e-9,
            "seed {seed}: belly centre is {offset} off the axis"
        );
        checked += 1;
    }
    // A collapse of the sample, such as every belly being suppressed, must
    // fail rather than pass on nothing.
    assert!(checked >= 200, "only {checked} bellies drawn in 300 seeds");
}
