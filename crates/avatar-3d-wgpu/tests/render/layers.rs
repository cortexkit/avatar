//! Which textures a robot has on the GPU, worked out without one: the
//! resident sets an upload and a restock lay out, and the bindings buffer
//! whose fourth lane gives the shader each texture's layer.

use avatar_3d::robot::{build_with, BuildOptions, Robot, TextureContent};
use avatar_3d_wgpu::layers::{binding_data, layer_bytes, Residency};
use avatar_small_robots::Mood;

use crate::genomes;

/// The four f32 values of each entry of a bindings buffer.
fn entries(data: &[u8]) -> Vec<[f32; 4]> {
    data.as_chunks::<16>()
        .0
        .iter()
        .map(|e| {
            [0, 1, 2, 3]
                .map(|k| f32::from_le_bytes([e[4 * k], e[4 * k + 1], e[4 * k + 2], e[4 * k + 3]]))
        })
        .collect()
}

/// The layer of each of `bindings` textures in a resident set.
fn layers(r: &Residency, bindings: usize) -> Vec<Option<u32>> {
    (0..bindings).map(|b| r.layer(b)).collect()
}

/// The indices of a robot's textures whose content passes `want`.
fn textures(robot: &Robot, want: impl Fn(TextureContent) -> bool) -> Vec<usize> {
    robot
        .textures
        .iter()
        .enumerate()
        .filter(|(_, t)| want(t.binding.content))
        .map(|(b, _)| b)
        .collect()
}

#[test]
fn a_borrowing_upload_keeps_every_texture_at_its_own_layer() {
    let r = Residency::every(5, 128);
    assert_eq!(
        layers(&r, 6),
        [Some(0), Some(1), Some(2), Some(3), Some(4), None]
    );
    assert_eq!(r.resident(), [0, 1, 2, 3, 4]);
    assert_eq!((r.px(), r.bindings(), r.layer_count()), (128, 5, 5));
    assert_eq!(r.texture_bytes(), 5 * layer_bytes(128));
}

#[test]
fn packed_layers_follow_binding_order_whatever_order_they_are_named_in() {
    let r = Residency::packed(8, 256, &[6, 2, 3]);
    assert_eq!(
        layers(&r, 8),
        [None, None, Some(0), Some(1), None, None, Some(2), None]
    );
    assert_eq!(r.resident(), [2, 3, 6]);
    assert_eq!(r.layer_count(), 3);
    assert_eq!(r.resident_bytes(), 3 * layer_bytes(256));
    assert_eq!(r.texture_bytes(), r.resident_bytes());
}

#[test]
fn an_empty_set_still_takes_one_transparent_layer() {
    let r = Residency::packed(4, 64, &[]);
    assert_eq!(layers(&r, 4), [None; 4]);
    assert_eq!((r.layer_count(), r.texture_layers()), (0, 1));
    assert_eq!(r.texture_bytes(), layer_bytes(64));
    assert_eq!(r.resident_bytes(), 0);
}

#[test]
fn a_layer_holds_every_mip_level() {
    // 256² + 128² + … + 1² texels, and 64² + … + 1², of four bytes each.
    assert_eq!(layer_bytes(256), 4 * 87_381);
    assert_eq!(layer_bytes(64), 4 * 5_461);
}

#[test]
fn a_restock_at_the_same_edge_copies_what_it_keeps_and_uploads_the_rest() {
    // Textures 0, 1, 6 and 7 sit at layers 0 to 3.
    let current = Residency::packed(8, 256, &[0, 1, 6, 7]);
    let plan = current.plan(256, &[7, 6], &[2]);
    // The new set 2, 6, 7 is packed in binding order.
    assert_eq!(plan.residency, Residency::packed(8, 256, &[2, 6, 7]));
    assert_eq!(
        layers(&plan.residency, 8),
        [None, None, Some(0), None, None, None, Some(1), Some(2)]
    );
    // Texture 7 moves from layer 3 to 2 and texture 6 from 2 to 1; texture
    // 2 is uploaded into layer 0.
    assert_eq!(plan.copies, [(3, 2), (2, 1)]);
    assert_eq!(plan.uploads, [0]);
}

#[test]
fn fresh_pixels_may_replace_a_resident_texture() {
    let current = Residency::every(3, 128);
    let plan = current.plan(128, &[0, 2], &[1]);
    assert_eq!(plan.residency, current);
    assert_eq!(plan.copies, [(0, 0), (2, 2)]);
    assert_eq!(plan.uploads, [1]);
}

#[test]
fn a_change_of_edge_uploads_every_layer_fresh() {
    let current = Residency::packed(6, 256, &[1, 4]);
    let plan = current.plan(128, &[], &[4, 1, 5]);
    assert_eq!(plan.residency, Residency::packed(6, 128, &[1, 4, 5]));
    assert_eq!(plan.residency.px(), 128);
    assert!(plan.copies.is_empty());
    assert_eq!(plan.uploads, [1, 0, 2]);
}

#[test]
#[should_panic(expected = "uploads every layer fresh")]
fn a_change_of_edge_keeps_no_layer() {
    Residency::packed(6, 256, &[1, 4]).plan(512, &[1], &[4]);
}

#[test]
#[should_panic(expected = "kept but not resident")]
fn only_a_resident_texture_can_be_kept() {
    Residency::packed(6, 256, &[1]).plan(256, &[2], &[]);
}

#[test]
#[should_panic(expected = "named twice")]
fn a_texture_is_not_both_kept_and_uploaded() {
    Residency::every(3, 256).plan(256, &[1], &[1]);
}

#[test]
#[should_panic(expected = "a layer edge is a power of two")]
fn a_layer_edge_is_one_a_robot_can_be_built_at() {
    Residency::every(3, 96);
}

#[test]
fn the_bindings_buffer_gives_each_texture_its_canvas_and_its_layer_or_minus_one() {
    let robot = genomes::corpus()
        .iter()
        .map(|g| build_with(g, BuildOptions::untextured()))
        .find(|r| {
            r.textures
                .iter()
                .any(|t| t.binding.content == TextureContent::Marks)
        })
        .expect("a corpus robot has marks");
    let n = robot.textures.len();
    let sad = textures(&robot, |c| {
        matches!(
            c,
            TextureContent::Face {
                mood: Mood::Sad,
                ..
            } | TextureContent::StalkEye {
                mood: Mood::Sad,
                ..
            }
        )
    });
    let marks = textures(&robot, |c| c == TextureContent::Marks);
    let mut resident: Vec<usize> = marks.iter().chain(&sad).copied().collect();
    let residency = Residency::packed(n, 128, &resident);
    resident.sort_unstable();

    let packed = entries(&binding_data(&robot, &residency));
    assert_eq!(packed.len(), n, "one entry per texture");
    for (b, (entry, t)) in packed.iter().zip(&robot.textures).enumerate() {
        let canvas = [
            t.binding.origin[0] as f32,
            t.binding.origin[1] as f32,
            t.binding.extent as f32,
        ];
        assert_eq!(entry[..3], canvas, "texture {b}'s canvas");
        // Its rank among the resident textures, or -1.
        let layer = resident
            .iter()
            .position(|r| *r == b)
            .map_or(-1.0, |l| l as f32);
        assert_eq!(entry[3], layer, "texture {b}'s layer");
    }
    // Compacted: the sad faces and the marks lie below their own indices,
    // and the other moods' faces are not resident.
    assert!(sad.iter().chain(&marks).all(|&b| packed[b][3] < b as f32));
    assert!(packed.iter().any(|e| e[3] == -1.0));

    // A borrowing upload's lanes are the textures' own indices.
    let every = entries(&binding_data(&robot, &Residency::every(n, 256)));
    for (b, entry) in every.iter().enumerate() {
        assert_eq!(entry[3], b as f32, "texture {b}'s layer");
    }
}
