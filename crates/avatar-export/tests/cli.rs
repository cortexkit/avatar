use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

const WILD_HEX: &str = include_str!("../../../vectors/wild/wild-port-0005.hex");
/// A corpus genome with narrowed disc eyes, pinned at wild version 2.
const WILD_V2_HEX: &str = include_str!("../../../vectors/wild/wild-port-0022.hex");

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
fn both_wild_versions_render_their_own_golden_trees() {
    // Stored rows render at the version they name, so the CLI must keep
    // drawing version 1 after new rows move to version 2. This genome's
    // narrowed eyes make the two versions' sleepy lids differ.
    let genome = GenomeFile::new(WILD_V2_HEX.trim());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut trees = Vec::new();
    for (version, golden) in [
        ("2", Some("vectors/tree/wild-port-0022__v2__sleepy.json")),
        ("1", None),
    ] {
        let dir = ExportDir::new();
        let output = dir.run(&[
            "--genome-file",
            &genome.path,
            "--type",
            "creature.wild",
            "--version",
            version,
            "--mood",
            "sleepy",
            "--out",
            "wild.json",
        ]);
        assert!(output.status.success(), "{output:?}");
        let tree = std::fs::read(dir.0.join("wild.json")).unwrap();
        if let Some(golden) = golden {
            assert_eq!(tree, std::fs::read(root.join(golden)).unwrap(), "{golden}");
        }
        trees.push(tree);
    }
    assert_ne!(trees[0], trees[1], "versions 1 and 2 drew the same lids");
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
        (WILD_HEX.trim(), "creature.wild", "3", "unsupported version"),
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
fn cli_rejects_missing_option_values_before_writing() {
    let hex = "00".repeat(1024);
    for (args, missing) in [
        (
            vec!["--genome", &hex, "--out", "result.json", "--version"],
            "--version",
        ),
        (
            vec!["--genome", &hex, "--out", "result.json", "--size"],
            "--size",
        ),
        (
            vec![
                "--genome",
                &hex,
                "--out",
                "result.json",
                "--mood",
                "--type",
                "creature.classic",
            ],
            "--mood",
        ),
        (vec!["--mint-genomes", "agent", "--out"], "--out"),
    ] {
        let dir = ExportDir::new();
        let output = dir.run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(&format!("{missing}: missing value"))
        );
        assert!(
            output.stdout.is_empty(),
            "{args:?} unexpectedly wrote to stdout"
        );
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
    }
}

#[test]
fn cli_reports_missing_mint_file_without_panicking() {
    let dir = ExportDir::new();
    let output = dir.run(&["--mint-genomes", "@missing.txt", "--out", "manifest.json"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("--mint-genomes: read missing.txt"));
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
}

#[test]
fn cli_mint_round_trips_a_quoted_id() {
    let dir = ExportDir::new();
    let id = "agent\"quote";
    let output = dir.run(&["--mint-genomes", id]);
    assert!(output.status.success(), "{output:?}");
    let records: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["agent_id"], id);
    let expected = avatar_genome::derive_genome(&avatar_genome::SeatIdentity::Registered {
        project_id: id.to_owned(),
    });
    assert_eq!(records[0]["avatar_genome"], expected.to_hex());
}

#[test]
fn cli_rejects_retired_backfill_options() {
    for args in [
        ["--stable-id", "agent", "--out", "image.png"],
        ["--manifest", "missing.json", "--out-dir", "images"],
    ] {
        let dir = ExportDir::new();
        let output = dir.run(&args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains(&format!("unknown option {:?}", args[0])));
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
    }
}

/// The six types that draw the wild genome in other styles, with the
/// directory under `vectors/` that holds each one's golden trees.
const NEW_TYPES: [(&str, &str); 6] = [
    ("creature.wild-plus", "wild-plus"),
    ("creature.sprouts", "sprouts"),
    ("creature.pixel-critters", "pixel-critters"),
    ("creature.knitted-toys", "knitted-toys"),
    ("creature.small-robots", "small-robots"),
    ("creature.drawn-heads", "drawn-heads"),
];

#[test]
fn new_types_export_stored_genomes_at_version_1() {
    let genome = GenomeFile::new(WILD_HEX.trim());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (type_name, stem) in NEW_TYPES {
        // Without --version a stored genome renders at version 1, the only
        // one these types have.
        let dir = ExportDir::new();
        let output = dir.run(&[
            "--genome-file",
            &genome.path,
            "--type",
            type_name,
            "--mood",
            "happy",
            "--out",
            "tree.json",
        ]);
        assert!(output.status.success(), "{type_name}: {output:?}");
        assert_eq!(
            std::fs::read(dir.0.join("tree.json")).unwrap(),
            std::fs::read(root.join(format!("vectors/{stem}/tree/wild-port-0005__happy.json")))
                .unwrap(),
            "{type_name}"
        );

        let dir = ExportDir::new();
        let output = dir.run(&[
            "--genome-file",
            &genome.path,
            "--type",
            type_name,
            "--version",
            "1",
            "--size",
            "64",
            "--out",
            "image.png",
        ]);
        assert!(output.status.success(), "{type_name}: {output:?}");
        let png = std::fs::read(dir.0.join("image.png")).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "{type_name}");
        assert_eq!(&png[16..24], &[0, 0, 0, 64, 0, 0, 0, 64], "{type_name}");

        let dir = ExportDir::new();
        let output = dir.run(&[
            "--genome-file",
            &genome.path,
            "--type",
            type_name,
            "--version",
            "2",
            "--out",
            "tree.json",
        ]);
        assert_eq!(output.status.code(), Some(1), "{type_name}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("unsupported version 2"),
            "{type_name}: {output:?}"
        );
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
    }
}

#[test]
fn new_types_mint_genomes_at_version_1() {
    let dir = ExportDir::new();
    let wild = dir.run(&["--mint-genomes", "agent_x", "--type", "creature.wild"]);
    assert!(wild.status.success(), "{wild:?}");
    let wild: Vec<serde_json::Value> = serde_json::from_slice(&wild.stdout).unwrap();
    for (type_name, _) in NEW_TYPES {
        let output = dir.run(&["--mint-genomes", "agent_x", "--type", type_name]);
        assert!(output.status.success(), "{type_name}: {output:?}");
        let records: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(records.len(), 1, "{type_name}");
        assert_eq!(records[0]["agent_id"], "agent_x");
        assert_eq!(records[0]["avatar_type"], type_name);
        assert_eq!(records[0]["avatar_version"], 1, "{type_name}");
        // The same agent id mints the same 16,384 bytes under each type as
        // under wild.
        assert_eq!(
            records[0]["avatar_genome"], wild[0]["avatar_genome"],
            "{type_name}"
        );
    }
}
