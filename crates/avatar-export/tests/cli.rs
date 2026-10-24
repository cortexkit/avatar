use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
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

    fn run_with_stdin(&self, args: &[&str], stdin: &str) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_avatar-export"))
            .current_dir(&self.0)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
}

/// A hex genome written to a file in its own directory, so tests that assert
/// the export directory stays empty are not confused by their input. Wild
/// genomes go through a file because 32,768 hex characters do not fit on a
/// Windows command line.
struct GenomeFile {
    dir: ExportDir,
    path: String,
}

impl GenomeFile {
    fn new(hex: &str) -> Self {
        let dir = ExportDir::new();
        let path = dir.0.join("genome.hex");
        // A trailing newline, as an editor or `echo` would leave, must be fine.
        std::fs::write(&path, format!("{hex}\n")).unwrap();
        let path = path.to_str().unwrap().to_owned();
        Self { dir, path }
    }
}

impl Drop for ExportDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const WILD_ARGS: [&str; 10] = [
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
];

fn assert_wild_png(dir: &ExportDir, output: &Output) {
    assert!(output.status.success(), "{output:?}");
    let png = std::fs::read(dir.0.join("wild.png")).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&png[16..24], &[0, 0, 0, 64, 0, 0, 0, 64]);
}

#[test]
fn wild_cli_renders_a_corpus_genome_from_a_file() {
    let genome = GenomeFile::new(WILD_HEX.trim());
    let dir = ExportDir::new();
    let mut args = vec!["--genome-file", genome.path.as_str()];
    args.extend(WILD_ARGS);
    let output = dir.run(&args);
    assert_wild_png(&dir, &output);
}

#[test]
fn the_out_extension_selects_svg_or_tree_json() {
    let genome = GenomeFile::new(WILD_HEX.trim());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (out, golden) in [
        ("wild.svg", "vectors/tree-svg/wild-port-0005__happy.svg"),
        ("wild.json", "vectors/tree/wild-port-0005__happy.json"),
    ] {
        let dir = ExportDir::new();
        let output = dir.run(&[
            "--genome-file",
            &genome.path,
            "--type",
            "creature.wild",
            "--mood",
            "happy",
            "--out",
            out,
        ]);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            std::fs::read(dir.0.join(out)).unwrap(),
            std::fs::read(root.join(golden)).unwrap(),
            "{out}"
        );
    }
}

#[test]
fn an_unsupported_out_extension_is_refused_without_writing_a_file() {
    let genome = GenomeFile::new(WILD_HEX.trim());
    let dir = ExportDir::new();
    let output = dir.run(&[
        "--genome-file",
        &genome.path,
        "--type",
        "creature.wild",
        "--out",
        "wild.jpg",
    ]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported extension"));
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
}

#[test]
fn wild_cli_reads_the_genome_from_standard_input() {
    let dir = ExportDir::new();
    let mut args = vec!["--genome-file", "-"];
    args.extend(WILD_ARGS);
    let output = dir.run_with_stdin(&args, WILD_HEX);
    assert_wild_png(&dir, &output);
}

#[test]
fn genome_and_genome_file_together_or_a_missing_file_write_no_file() {
    let genome = GenomeFile::new(WILD_HEX.trim());
    let missing = genome.dir.0.join("missing.hex");
    let missing = missing.to_str().unwrap();
    for (args, error) in [
        (
            vec!["--genome", "00", "--genome-file", genome.path.as_str()],
            "alternatives",
        ),
        (vec!["--genome-file", missing], "--genome-file"),
    ] {
        let dir = ExportDir::new();
        let output = dir.run(&args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains(error));
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
    }
}

#[test]
fn wild_length_without_wild_type_is_refused_without_writing_a_file() {
    let genome = GenomeFile::new(WILD_HEX.trim());
    for type_args in [vec![], vec!["--type", "creature.classic"]] {
        let dir = ExportDir::new();
        let mut args = vec![
            "--genome-file",
            genome.path.as_str(),
            "--out",
            "refused.png",
        ];
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
        let genome = GenomeFile::new(hex);
        let dir = ExportDir::new();
        let output = dir.run(&[
            "--genome-file",
            &genome.path,
            "--type",
            type_name,
            "--version",
            version,
        ]);
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
