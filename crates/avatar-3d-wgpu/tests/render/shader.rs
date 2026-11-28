//! The WGSL is valid without a GPU, and the texture and target formats are
//! the stated sRGB ones.

use avatar_3d_wgpu::{BLIT_SHADER, SHADER};
#[cfg(feature = "gpu")]
use avatar_3d_wgpu::{TARGET_FORMAT, TEXTURE_FORMAT};

/// Parses and validates WGSL with naga, and returns its entry points by
/// name.
fn entry_points(source: &str) -> Vec<(String, naga::ShaderStage)> {
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|e| panic!("the WGSL does not parse:\n{}", e.emit_to_string(source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("the WGSL does not validate:\n{}", e.emit_to_string(source)));
    let mut entries: Vec<(String, naga::ShaderStage)> = module
        .entry_points
        .iter()
        .map(|e| (e.name.clone(), e.stage))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

#[test]
fn the_wgsl_parses_and_validates_with_naga() {
    let entries = entry_points(SHADER);
    let names: Vec<(&str, naga::ShaderStage)> =
        entries.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    assert_eq!(
        names,
        [
            ("fs_robot", naga::ShaderStage::Fragment),
            ("fs_shadow", naga::ShaderStage::Fragment),
            ("vs_robot", naga::ShaderStage::Vertex),
            ("vs_shadow", naga::ShaderStage::Vertex),
        ]
    );
}

#[test]
fn the_blit_wgsl_parses_and_validates_with_naga() {
    let entries = entry_points(BLIT_SHADER);
    let names: Vec<(&str, naga::ShaderStage)> =
        entries.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    assert_eq!(
        names,
        [
            ("fs_encode", naga::ShaderStage::Fragment),
            ("fs_linear", naga::ShaderStage::Fragment),
            ("fs_premultiplied", naga::ShaderStage::Fragment),
            ("fs_straight", naga::ShaderStage::Fragment),
            ("vs_blit", naga::ShaderStage::Vertex),
        ]
    );
}

#[test]
#[cfg(feature = "gpu")]
fn textures_and_targets_are_rgba8_srgb() {
    assert_eq!(TEXTURE_FORMAT, wgpu::TextureFormat::Rgba8UnormSrgb);
    assert_eq!(TARGET_FORMAT, wgpu::TextureFormat::Rgba8UnormSrgb);
    assert!(TEXTURE_FORMAT.is_srgb() && TARGET_FORMAT.is_srgb());
}
