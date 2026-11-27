//! The memory the renderer's frame targets take, as the numbers report
//! it.

use avatar_3d_wgpu::target_bytes;

#[test]
fn frame_targets_count_multisampled_colour_and_depth_the_resolve_and_padded_readback() {
    // 1600 × 1000: 4 samples of 4-byte colour and of 4-byte depth, a
    // 4-byte resolve, and readback rows of 6400 bytes, already a multiple
    // of the 256-byte copy alignment.
    let pixels = 1600 * 1000;
    assert_eq!(
        target_bytes(1600, 1000),
        16 * pixels + 16 * pixels + 4 * pixels + 6400 * 1000
    );
    // 288 × 288: rows of 1152 bytes are padded to 1280 for the readback.
    let pixels = 288 * 288;
    assert_eq!(
        target_bytes(288, 288),
        16 * pixels + 16 * pixels + 4 * pixels + 1280 * 288
    );
}
