//! Separation: the 3D data crate never depends on a GPU crate.

use std::process::Command;

/// Whether a crate name is one the 3D data crate must never reach.
fn forbidden(name: &str) -> bool {
    name == "cortexkit-avatar-3d-wgpu"
        || name.starts_with("wgpu")
        || name == "naga"
        || name == "metal"
        || name == "ash"
}

#[test]
fn no_gpu_crate_on_any_target_feature_or_dependency_kind() {
    let cargo = std::env::var("CARGO")
        .ok()
        .or_else(|| option_env!("CARGO").map(str::to_owned))
        .unwrap_or_else(|| "cargo".to_owned());
    let output = Command::new(cargo)
        .args([
            "tree",
            "-p",
            "cortexkit-avatar-3d",
            "--target",
            "all",
            "--all-features",
            "-e",
            "normal,build,dev",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run cargo tree");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let names: Vec<&str> = stdout
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    // The listing must be the real tree, not an empty one.
    for expected in [
        "cortexkit-avatar-3d",
        "cortexkit-avatar-small-robots",
        "cortexkit-avatar-raster",
    ] {
        assert!(
            names.contains(&expected),
            "cargo tree does not list {expected}"
        );
    }
    let bad: Vec<&&str> = names.iter().filter(|n| forbidden(n)).collect();
    assert!(bad.is_empty(), "GPU crates in the tree: {bad:?}");
}
