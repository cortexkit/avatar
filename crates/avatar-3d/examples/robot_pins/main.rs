//! Prints the labelled SHA-256 digests of the 3D small robots' rest meshes.
//!
//! ```text
//! cargo run -p cortexkit-avatar-3d --example robot_pins [-- --write | --check]
//! ```
//!
//! Standard output is the pins file's content: the coverage seed and
//! prefix, then one `label<TAB>sha256` line per case (see `cases.rs`). Each
//! robot's triangle count and texture bytes go to standard error.
//! `--write` runs the coverage search and writes the pins to
//! `vectors/robots-3d/digests.tsv`, the only way that file is written.
//! `--check` exits non-zero unless the digests equal the committed pins.

mod cases;

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
    for line in &lines {
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
            let path = cases::vectors().join(cases::PINS);
            fs::create_dir_all(path.parent().expect("the pins file has a directory"))
                .expect("create vectors/robots-3d");
            let text = format!("{}\n{}\n", cases::HEADER, lines.join("\n"));
            fs::write(&path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
            eprintln!("robot_pins: wrote {}", path.display());
        }
        "--check" if lines != cases::committed() => {
            eprintln!("robot_pins: the digests differ from the committed pins");
            return ExitCode::FAILURE;
        }
        _ => {}
    }
    ExitCode::SUCCESS
}
