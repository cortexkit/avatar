//! Prints the labelled SHA-256 digests of the 3D small robots' rest meshes
//! and of their poses in the motion scripts, and the exact bits of their
//! stage cameras.
//!
//! ```text
//! cargo run -p cortexkit-avatar-3d --example robot_pins [-- --write | --check]
//! ```
//!
//! Standard output is the three pins files' content without their comments:
//! the coverage seed and prefix and one `label<TAB>sha256` line per rest
//! mesh (see `cases.rs`), then one line per pinned pose (see `scripts.rs`),
//! then one line per robot's stage camera (see `cases.rs`).
//! Each robot's triangle count and texture bytes go to standard error.
//! `--write` runs the coverage search and writes the pins to
//! `vectors/robots-3d/digests.tsv`, `vectors/robots-3d/poses.tsv` and
//! `vectors/robots-3d/cameras.tsv`, the only way those files are written.
//! `--check` exits non-zero unless the output equals the committed pins.

mod cases;
mod scripts;

use std::process::ExitCode;
use std::{env, fs};

use avatar_3d::robot::{build_with, BuildOptions};

fn main() -> ExitCode {
    let mode = env::args().nth(1).unwrap_or_default();
    let count = match mode.as_str() {
        "--write" => match cases::coverage_search(cases::COVERAGE_SEED, cases::COVERAGE_LIMIT) {
            Ok(count) => count,
            Err(message) => {
                eprintln!("robot_pins: {message}");
                return ExitCode::FAILURE;
            }
        },
        "" | "--check" => cases::committed_prefix(),
        other => {
            eprintln!("robot_pins: unknown argument {other:?}; use --write or --check");
            return ExitCode::FAILURE;
        }
    };
    let lines = cases::pin_lines(count);
    let poses = scripts::pose_lines();
    let cameras = cases::camera_lines(count);
    for line in lines.iter().chain(&poses).chain(&cameras) {
        println!("{line}");
    }
    for (label, genome) in cases::cases(count) {
        let robot = build_with(&genome, BuildOptions::untextured());
        eprintln!(
            "{label}\ttriangles {}\ttextures {}\ttexture bytes {}",
            robot.triangle_count(),
            robot.textures.len(),
            robot.texture_bytes()
        );
    }
    match mode.as_str() {
        "--write" => {
            for (file, header, lines) in [
                (cases::PINS, cases::HEADER, &lines),
                (scripts::POSE_PINS, scripts::POSE_HEADER, &poses),
                (cases::CAMERA_PINS, cases::CAMERA_HEADER, &cameras),
            ] {
                let path = cases::vectors().join(file);
                fs::create_dir_all(path.parent().expect("the pins file has a directory"))
                    .expect("create vectors/robots-3d");
                let text = format!("{header}\n{}\n", lines.join("\n"));
                fs::write(&path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
                eprintln!("robot_pins: wrote {}", path.display());
            }
        }
        "--check"
            if lines != cases::committed()
                || poses != scripts::committed_poses()
                || cameras != cases::committed_cameras() =>
        {
            eprintln!("robot_pins: the digests differ from the committed pins");
            return ExitCode::FAILURE;
        }
        _ => {}
    }
    ExitCode::SUCCESS
}
