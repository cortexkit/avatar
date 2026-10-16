use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const WILD_HEX: &str = include_str!("../../../vectors/wild/wild-port-0005.hex");

struct ExportDir(PathBuf);

impl ExportDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "avatar-export-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_avatar-export"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}

impl Drop for ExportDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn wild_cli_renders_a_corpus_genome() {
    let dir = ExportDir::new();
    let output = dir.run(&[
        "--genome",
        WILD_HEX.trim(),
        "--type",
        "creature.wild",
        "--version",
        "1",
        "--mood",
        "happy",
        "--size",
        "64",
        "--out",
        "wild.png",
    ]);
    assert!(output.status.success(), "{output:?}");
    let png = std::fs::read(dir.0.join("wild.png")).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&png[16..24], &[0, 0, 0, 64, 0, 0, 0, 64]);
}

#[test]
fn wild_length_without_wild_type_is_refused_without_writing_a_file() {
    for type_args in [vec![], vec!["--type", "creature.classic"]] {
        let dir = ExportDir::new();
        let mut args = vec!["--genome", WILD_HEX.trim(), "--out", "refused.png"];
        args.extend(type_args);
        let output = dir.run(&args);
        assert!(!output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("hex"));
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
    }
}

#[test]
fn invalid_wild_metadata_or_hex_writes_no_file() {
    let bad_hex = "zz".repeat(16_384);
    for (hex, type_name, version, error) in [
        (
            WILD_HEX.trim(),
            "creature.unknown",
            "1",
            "unknown avatar type",
        ),
        (WILD_HEX.trim(), "creature.wild", "2", "unsupported version"),
        (WILD_HEX.trim(), "creature.wild", "no", "--version"),
        ("00", "creature.wild", "1", "hex characters"),
        (bad_hex.as_str(), "creature.wild", "1", "invalid hex"),
    ] {
        let dir = ExportDir::new();
        let output = dir.run(&["--genome", hex, "--type", type_name, "--version", version]);
        assert!(!output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains(error));
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
    }
}

#[test]
fn classic_stable_id_and_stored_genome_cli_outputs_agree() {
    let dir = ExportDir::new();
    let id = "pj-0000000000000001";
    let genome = avatar_genome::derive_genome(&avatar_genome::SeatIdentity::Registered {
        project_id: id.to_owned(),
    });
    let output = dir.run(&["--stable-id", id, "--size", "64", "--out", "stable.png"]);
    assert!(output.status.success(), "{output:?}");
    let expected = std::fs::read(dir.0.join("stable.png")).unwrap();
    for type_args in [vec![], vec!["--type", "creature.classic", "--version", "2"]] {
        let hex = genome.to_hex();
        let mut args = vec!["--genome", &hex, "--size", "64", "--out", "stored.png"];
        args.extend(type_args);
        let output = dir.run(&args);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(std::fs::read(dir.0.join("stored.png")).unwrap(), expected);
    }
}
