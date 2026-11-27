//! The WGSL is valid without a GPU, and the texture and target formats are
//! the stated sRGB ones.

use avatar_3d_wgpu::{SHADER, TARGET_FORMAT, TEXTURE_FORMAT};

#[test]
fn the_wgsl_parses_and_validates_with_naga() {
    let module = naga::front::wgsl::parse_str(SHADER)
        .unwrap_or_else(|e| panic!("the WGSL does not parse:\n{}", e.emit_to_string(SHADER)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("the WGSL does not validate:\n{}", e.emit_to_string(SHADER)));
    let mut entries: Vec<(&str, naga::ShaderStage)> = module
        .entry_points
        .iter()
        .map(|e| (e.name.as_str(), e.stage))
        .collect();
    entries.sort_by_key(|e| e.0);
    assert_eq!(
        entries,
        [
            ("fs_robot", naga::ShaderStage::Fragment),
            ("fs_shadow", naga::ShaderStage::Fragment),
            ("vs_robot", naga::ShaderStage::Vertex),
            ("vs_shadow", naga::ShaderStage::Vertex),
        ]
    );
}

#[test]
fn textures_and_targets_are_rgba8_srgb() {
    assert_eq!(TEXTURE_FORMAT, wgpu::TextureFormat::Rgba8UnormSrgb);
    assert_eq!(TARGET_FORMAT, wgpu::TextureFormat::Rgba8UnormSrgb);
    assert!(TEXTURE_FORMAT.is_srgb() && TARGET_FORMAT.is_srgb());
}
