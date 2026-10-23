//! Deterministic PNG export for agent avatars, including GitHub App images.
//!
//! Inputs are either a registry's opaque stable ID (never a display name)
//! or a stored genome with its type and generation version. Stable IDs
//! reproduce the original classic creature; stored genomes depict the
//! current creature even after its genome has been re-rolled.
//!
//! Classic renders through its legacy SVG renderer; wild renders through
//! the render tree serializer. Both rasterize with resvg (pure Rust, with
//! no system fonts needed because the SVG contains no text). Consumers
//! re-run exports when an agent's stored genome, type or version changes;
//! this tool keeps no cache.
//!
//! Usage:
//!   avatar-export --stable-id pj-0000000000000001 --out ava.png
//!   avatar-export --genome <hex> --type creature.wild --version 1 --out ava.png
//!   avatar-export --genome-file ava.hex --type creature.wild --version 1 --out ava.png
//!     --genome-file reads the same hex from a file ("-" reads standard
//!     input). A wild genome is 32,768 hex characters, which does not fit on
//!     a Windows command line (CreateProcess allows 32,767 characters including
//!     its terminating NUL, and the program name counts too), so callers that
//!     must work on every operating system pass genomes this way.
//!   avatar-export --manifest agents.json --out-dir avatars/
//!     manifest: JSON array of {"stable_id": "pj-…"} (unknown keys ignored,
//!     so callers can pass richer records); output files are <stable_id>.png.
//!   Options: --size N (default 400), --mood M (default neutral).

use avatar_genome::{decode, derive_genome, derive_genome_bytes, Genome, SeatIdentity};
use avatar_svg::{render_svg, render_tree_for_bytes, serialize_tree, Mood, RenderParams, Q32};

/// The avatar TYPE: which procedural representation renders the genome.
/// Types are different representations OF THE SAME GENOME behind one
/// shared interface — the same bits depicted in another style, not another
/// creature. The type also pins the genome LAYOUT, since a representation
/// reading different byte ranges could not be a depiction of those bits.
///
/// Named for its character rather than a number on purpose: a row reading
/// `type=creature.v1, version=2` invites the reader to assume one of the
/// two is wrong, when they are independent axes. The creature redesign in
/// flight becomes its own type (`creature.wild`), not a version bump.
const AVATAR_TYPE: &str = "creature.classic";

/// The generation VERSION of that type, which exists for one purpose: fixing
/// bugs in procedural generation without silently changing anybody's face.
/// A stored version is authoritative for rendering — a renderer must draw a
/// row at the version the row names, not at its own newest — and moving a
/// row to a newer version is an explicit, consented act. That is what makes
/// old generation paths a permanent obligation rather than dead code.
const AVATAR_VERSION: u32 = 2;

fn mood_from(name: &str) -> Result<Mood, String> {
    Ok(match name {
        "neutral" => Mood::Neutral,
        "working" => Mood::Working,
        "happy" => Mood::Happy,
        "waiting" => Mood::Waiting,
        "blocked" => Mood::Blocked,
        "sad" => Mood::Sad,
        "sleepy" => Mood::Sleepy,
        other => return Err(format!("unknown mood {other:?}")),
    })
}

/// Renders a genome that already exists, as opposed to deriving one.
///
/// This is the correct path for anything depicting a LIVE agent, because
/// the registry row is the only truth once genomes can be re-rolled:
/// deriving from an id reproduces the agent's ORIGINAL creature, which
/// after a re-roll is a face that agent no longer wears. A GitHub App
/// avatar minted by derivation and a phone rendering the stored row would
/// then disagree permanently, each looking correct.
fn export_png_from_genome(genome: &Genome, mood: Mood, size: u32) -> Result<Vec<u8>, String> {
    rasterize(&classic_svg(genome, mood)?, size)
}

fn classic_svg(genome: &Genome, mood: Mood) -> Result<Vec<u8>, String> {
    let phenotype = decode(genome);
    let params = RenderParams {
        mood,
        growth: Q32::ONE,
        canvas: 10_000,
    };
    render_svg(&phenotype, &params).map_err(|e| e.to_string())
}

fn export_png(stable_id: &str, mood: Mood, size: u32) -> Result<Vec<u8>, String> {
    let genome = derive_genome(&SeatIdentity::Registered {
        project_id: stable_id.to_owned(),
    });
    export_png_from_genome(&genome, mood, size)
}

fn export_png_from_hex(
    hex: &str,
    type_name: &str,
    version: u32,
    mood: Mood,
    size: u32,
    rasterizer: impl FnOnce(&[u8], u32) -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    let (_, len, supported_version) =
        mintable(type_name).ok_or_else(|| format!("unknown avatar type {type_name:?}"))?;
    if version != supported_version {
        return Err(format!(
            "unsupported version {version} for {type_name:?}; supported: {supported_version}"
        ));
    }
    let hex = hex.trim();
    let svg = if type_name == AVATAR_TYPE {
        // Keep classic SVG bytes unchanged; tree serialization has a different byte format.
        let genome = Genome::from_hex(hex).map_err(|e| e.to_string())?;
        classic_svg(&genome, mood)?
    } else {
        if hex.len() != len * 2 {
            return Err(format!(
                "expected {} hex characters, got {}",
                len * 2,
                hex.len()
            ));
        }
        let bytes: Result<Vec<u8>, String> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .enumerate()
            .map(|(i, pair)| {
                let hi = char::from(pair[0]).to_digit(16);
                let lo = char::from(pair[1]).to_digit(16);
                match (hi, lo) {
                    (Some(hi), Some(lo)) => Ok(((hi << 4) | lo) as u8),
                    _ => Err(format!("invalid hex at byte {i}")),
                }
            })
            .collect();
        let tree =
            render_tree_for_bytes(&bytes?, type_name, version, mood).map_err(|e| e.to_string())?;
        serialize_tree(&tree)
    };
    rasterizer(&svg, size)
}

fn rasterize(svg: &[u8], size: u32) -> Result<Vec<u8>, String> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default())
        .map_err(|e| format!("svg parse: {e}"))?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)
        .ok_or_else(|| "pixmap alloc failed".to_owned())?;
    let scale = size as f32 / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| format!("png encode: {e}"))
}

/// Reads agent ids from a comma list or, with a leading `@`, from a file of
/// one id per line (`#` comments allowed).
fn read_ids(arg: &str) -> Vec<String> {
    let text;
    let source = match arg.strip_prefix('@') {
        Some(path) => {
            text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
            text.as_str()
        }
        None => arg,
    };
    let split: Box<dyn Iterator<Item = &str>> = if arg.starts_with('@') {
        Box::new(source.lines())
    } else {
        Box::new(source.split(','))
    };
    split
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// Mints the genome deposit manifest for a batch of agents.
///
/// The operator ruled that the genome is STORED on the agent row and served,
/// so derivation runs once per agent at creation or backfill rather than per
/// render. This is the backfill half: one auditable artifact for a batch,
/// deposited through prefrontal's gated set-avatar op.
///
/// Every record carries `avatar_type`, because a genome is positional and
/// uninterpretable without knowing which LAYOUT minted it. A row holding
/// genome bytes with no type is corrupt rather than defaulting.
/// The avatar types this tool can mint a genome for, with the genome
/// LENGTH each layout requires. Length belongs to the type, never to the
/// version: a version fixes generation bugs and never moves a field.
///
/// Derivation itself is type-independent (one XOF stream read to the
/// required length), so a seat's longer genome extends its classic one
/// rather than re-rolling it. That is what lets a type be a different
/// REPRESENTATION of the same creature rather than a different creature.
const MINTABLE: &[(&str, usize, u32)] = &[
    (AVATAR_TYPE, 1_024, AVATAR_VERSION),
    ("creature.wild", 16_384, 1),
];

fn mintable(type_name: &str) -> Option<(&'static str, usize, u32)> {
    MINTABLE.iter().copied().find(|(t, _, _)| *t == type_name)
}

/// One JSON object per agent, each carrying its own type and version.
///
/// Deliberately NOT a map of id to genome with the type stated once at the
/// top: an entry that carries its own type cannot be separated from the
/// genome it describes by a later edit, a merge, or a partial copy. A
/// genome is uninterpretable without knowing which layout minted it, so
/// the two must travel together.
fn mint_manifest(ids: &[String], type_name: &str) -> String {
    let (type_name, len, version) = mintable(type_name).expect("caller validated the type");
    let mut out = String::from("[\n");
    for (i, id) in ids.iter().enumerate() {
        let bytes = derive_genome_bytes(
            &SeatIdentity::Registered {
                project_id: id.clone(),
            },
            len,
        );
        let mut hex = String::with_capacity(len * 2);
        for byte in &bytes {
            hex.push_str(&format!("{byte:02x}"));
        }
        let comma = if i + 1 == ids.len() { "" } else { "," };
        out.push_str(&format!(
            "  {{\"agent_id\":\"{id}\",\"avatar_genome\":\"{hex}\",\"avatar_type\":\"{type_name}\",\"avatar_version\":{version}}}{comma}\n",
        ));
    }
    out.push_str("]\n");
    out
}

/// Minimal manifest reader: extracts every `"stable_id": "…"` string value.
/// Deliberately not a JSON library — the schema is one required key, and a
/// dependency-free scan keeps the export crate's supply chain equal to the
/// pipeline's own.
fn manifest_ids(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let key = b"\"stable_id\"";
    let mut i = 0;
    while let Some(pos) = text[i..].find(std::str::from_utf8(key).unwrap()) {
        let mut j = i + pos + key.len();
        // Skip whitespace and the colon, then read the string value.
        while j < bytes.len() && (bytes[j] as char).is_whitespace() {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == b':' {
            j += 1;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'"' {
                if let Some(end) = text[j + 1..].find('"') {
                    out.push(text[j + 1..j + 1 + end].to_owned());
                }
            }
        }
        i = j;
    }
    out
}

/// Pixel size for the raster, defaulting only when the flag is ABSENT.
///
/// This read `.parse().ok().unwrap_or(400)`, so `--size 51w` silently
/// minted 400px images: a ceremony would produce a whole fleet at the
/// wrong size with nothing reporting it. A default for a missing flag is
/// a default; a default for an unreadable one is a swallowed error, and
/// `--mood` on the next line had always failed closed.
fn parse_size(raw: Option<&str>) -> Result<u32, String> {
    const RANGE: std::ops::RangeInclusive<u32> = 16..=4096;
    match raw {
        None => Ok(400),
        Some(text) => match text.parse::<u32>() {
            Ok(v) if RANGE.contains(&v) => Ok(v),
            Ok(v) => Err(format!("--size {v} is outside 16..=4096")),
            Err(e) => Err(format!("--size {text}: {e}")),
        },
    }
}

/// Reads a hex genome from a file, or from standard input when `path` is "-".
/// The hex is trimmed before parsing, so a trailing newline or other
/// surrounding whitespace is accepted.
fn read_genome_file(path: &str) -> std::io::Result<String> {
    if path == "-" {
        std::io::read_to_string(std::io::stdin())
    } else {
        std::fs::read_to_string(path)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |flag: &str| -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1).cloned())
    };

    let size = match parse_size(get("--size").as_deref()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let mood = match mood_from(&get("--mood").unwrap_or_else(|| "neutral".into())) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };

    // Renders a genome the caller already holds — the path anything
    // depicting a live agent must use, since after a re-roll the stored
    // genome and a derived one are different creatures.
    let genome_hex = match (get("--genome"), get("--genome-file")) {
        (Some(_), Some(_)) => {
            eprintln!("--genome and --genome-file are alternatives; pass one");
            std::process::exit(2);
        }
        (Some(hex), None) => Some(hex),
        (None, Some(path)) => match read_genome_file(&path) {
            Ok(hex) => Some(hex),
            Err(e) => {
                eprintln!("--genome-file {path}: {e}");
                std::process::exit(2);
            }
        },
        (None, None) => None,
    };
    if let Some(hex) = genome_hex {
        let type_name = get("--type").unwrap_or_else(|| AVATAR_TYPE.to_owned());
        let default_version =
            mintable(&type_name).map_or(AVATAR_VERSION, |(_, _, version)| version);
        let version = match get("--version") {
            Some(value) => value.parse::<u32>().unwrap_or_else(|_| {
                eprintln!("--version: expected an unsigned integer, got {value:?}");
                std::process::exit(2);
            }),
            None => default_version,
        };
        let out = get("--out").unwrap_or_else(|| "avatar.png".to_owned());
        match export_png_from_hex(&hex, &type_name, version, mood, size, rasterize) {
            Ok(png) => {
                std::fs::write(&out, png).expect("write png");
                println!("{out}");
            }
            Err(e) => {
                eprintln!("export: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(ids_arg) = get("--mint-genomes") {
        let ids = read_ids(&ids_arg);
        if ids.is_empty() {
            eprintln!("--mint-genomes: no agent ids given");
            std::process::exit(2);
        }
        let type_name = get("--type").unwrap_or_else(|| AVATAR_TYPE.to_owned());
        if mintable(&type_name).is_none() {
            eprintln!("--type: unknown avatar type {type_name:?}");
            eprintln!("  known: {}", {
                let names: Vec<&str> = MINTABLE.iter().map(|(t, _, _)| *t).collect();
                names.join(", ")
            });
            std::process::exit(2);
        }
        let manifest = mint_manifest(&ids, &type_name);
        match get("--out") {
            Some(path) => {
                std::fs::write(&path, &manifest).expect("write manifest");
                eprintln!("minted {} genomes -> {path}", ids.len());
            }
            None => print!("{manifest}"),
        }
        return;
    }

    if let Some(stable_id) = get("--stable-id") {
        let out = get("--out").unwrap_or_else(|| format!("{stable_id}.png"));
        match export_png(&stable_id, mood, size) {
            Ok(png) => {
                std::fs::write(&out, png).expect("write png");
                println!("{out}");
            }
            Err(e) => {
                eprintln!("export {stable_id}: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(manifest_path) = get("--manifest") {
        let out_dir = get("--out-dir").unwrap_or_else(|| ".".into());
        std::fs::create_dir_all(&out_dir).expect("mkdir out-dir");
        let text = std::fs::read_to_string(&manifest_path).expect("read manifest");
        let ids = manifest_ids(&text);
        if ids.is_empty() {
            eprintln!("manifest contains no stable_id entries");
            std::process::exit(2);
        }
        let mut failures = 0;
        for id in &ids {
            let out = format!("{out_dir}/{id}.png");
            match export_png(id, mood, size) {
                Ok(png) => {
                    std::fs::write(&out, png).expect("write png");
                    println!("{out}");
                }
                Err(e) => {
                    eprintln!("export {id}: {e}");
                    failures += 1;
                }
            }
        }
        if failures > 0 {
            std::process::exit(1);
        }
        return;
    }

    eprintln!(
        "usage: avatar-export --stable-id <id> [--out f.png] | --manifest agents.json [--out-dir d]\n       avatar-export --genome <hex> | --genome-file <path|-> [--type creature.classic|creature.wild] [--version N] [--out f.png]\n       options: --size N (default 400), --mood neutral|working|happy|waiting|blocked|sad|sleepy\n       stored-genome defaults: creature.classic; version 2 for classic, 1 for wild"
    );
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    #[test]
    fn size_defaults_only_when_the_flag_is_absent() {
        use super::parse_size;
        // Absent is the only case that may default.
        assert_eq!(parse_size(None), Ok(400));
        assert_eq!(parse_size(Some("512")), Ok(512));
        // Present-but-unreadable must NOT become the default. Before this
        // was a Result, `--size 51w` rendered at 400 and said nothing.
        assert!(parse_size(Some("51w")).is_err());
        assert!(parse_size(Some("")).is_err());
        assert!(parse_size(Some("-1")).is_err());
        assert!(parse_size(Some("400px")).is_err());
        // Parseable but unrenderable is refused too, for the same reason:
        // a 0px PNG is a broken asset produced without complaint.
        assert!(parse_size(Some("0")).is_err());
        assert!(parse_size(Some("8")).is_err());
        assert!(parse_size(Some("999999")).is_err());
    }

    use super::*;

    const WILD_HEX: &str = include_str!("../../../vectors/wild/wild-port-0005.hex");

    #[test]
    fn wild_corpus_svg_is_passed_to_rasterizer() {
        let bytes: Vec<u8> = WILD_HEX
            .trim()
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(bytes.len(), 16_384);
        for mood in [
            Mood::Neutral,
            Mood::Working,
            Mood::Happy,
            Mood::Waiting,
            Mood::Blocked,
            Mood::Sad,
            Mood::Sleepy,
        ] {
            let expected =
                serialize_tree(&render_tree_for_bytes(&bytes, "creature.wild", 1, mood).unwrap());
            let mut calls = 0;
            let png = export_png_from_hex(WILD_HEX, "creature.wild", 1, mood, 64, |svg, size| {
                calls += 1;
                assert_eq!(svg, expected, "SVG at the rasterizer boundary for {mood:?}");
                assert_eq!(size, 64);
                rasterize(svg, size)
            })
            .unwrap();
            assert_eq!(calls, 1);
            assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        }
    }

    #[test]
    fn classic_genome_svg_stays_on_legacy_renderer() {
        let genome = derive_genome(&SeatIdentity::Registered {
            project_id: "pj-0000000000000001".to_owned(),
        });
        let expected = render_svg(
            &decode(&genome),
            &RenderParams {
                mood: Mood::Happy,
                growth: Q32::ONE,
                canvas: 10_000,
            },
        )
        .unwrap();
        let png = export_png_from_hex(
            &genome.to_hex(),
            AVATAR_TYPE,
            AVATAR_VERSION,
            Mood::Happy,
            64,
            |svg, size| {
                assert_eq!(svg, expected);
                rasterize(svg, size)
            },
        )
        .unwrap();
        assert_eq!(
            png,
            export_png_from_genome(&genome, Mood::Happy, 64).unwrap()
        );
    }

    #[test]
    fn invalid_stored_genomes_never_reach_rasterization() {
        let invalid_hex = "gg".repeat(16_384);
        let unicode_hex = "é".repeat(16_384);
        for (hex, type_name, version, error) in [
            (WILD_HEX, "creature.unknown", 1, "unknown avatar type"),
            (WILD_HEX, "creature.wild", 2, "unsupported version"),
            ("", "creature.wild", 2, "unsupported version"),
            (WILD_HEX, AVATAR_TYPE, 1, "unsupported version"),
            (WILD_HEX, AVATAR_TYPE, 2, "hex"),
            ("00", "creature.wild", 1, "expected 32768 hex characters"),
            (invalid_hex.as_str(), "creature.wild", 1, "invalid hex"),
            (unicode_hex.as_str(), "creature.wild", 1, "invalid hex"),
        ] {
            let result = export_png_from_hex(hex, type_name, version, Mood::Neutral, 64, |_, _| {
                panic!("invalid input reached the rasterizer")
            });
            assert!(result.unwrap_err().contains(error));
        }
    }

    #[test]
    fn export_is_deterministic_and_png_shaped() {
        let a = export_png("pj-0000000000000001", Mood::Neutral, 400).unwrap();
        let b = export_png("pj-0000000000000001", Mood::Neutral, 400).unwrap();
        assert_eq!(a, b, "same stableId must produce identical PNG bytes");
        assert_eq!(&a[..8], b"\x89PNG\r\n\x1a\n", "PNG magic");
    }

    #[test]
    fn different_ids_differ() {
        let a = export_png("pj-0000000000000001", Mood::Neutral, 64).unwrap();
        let b = export_png("pj-000000000000dead", Mood::Neutral, 64).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn minted_manifest_holds_the_properties_the_fleet_deposits_on() {
        // These are the exact claims made to the registry when handing over
        // the backfill: reproducible, one genome per agent, and every genome
        // the declared layout size so the op's length gate passes. A change
        // that broke any of them would deposit wrong data into 30 rows.
        let ids: Vec<String> = ["agent_aaa", "agent_bbb", "agent_00000003"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();

        let first = mint_manifest(&ids, AVATAR_TYPE);
        assert_eq!(
            first,
            mint_manifest(&ids, AVATAR_TYPE),
            "mint must be reproducible"
        );

        // A longer type mints the same seats at its own length, and the
        // wild genome EXTENDS the classic one rather than re-rolling it:
        // the hex prefix must match, or a seat's identity forks between
        // the two types with nothing reporting it.
        let wild = mint_manifest(&ids, "creature.wild");
        assert!(wild.contains("\"avatar_type\":\"creature.wild\""));
        assert!(wild.contains("\"avatar_version\":1"));
        let classic_hex = first
            .split("\"avatar_genome\":\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("classic genome hex");
        let wild_hex = wild
            .split("\"avatar_genome\":\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("wild genome hex");
        assert_eq!(classic_hex.len(), 2_048);
        assert_eq!(wild_hex.len(), 32_768);
        assert!(wild_hex.starts_with(classic_hex));

        let genomes: Vec<&str> = first
            .split("\"avatar_genome\":\"")
            .skip(1)
            .map(|rest| rest.split('"').next().expect("closing quote"))
            .collect();
        assert_eq!(genomes.len(), ids.len());
        for hex in &genomes {
            assert_eq!(hex.len(), 2048, "1024 bytes as hex");
            assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        }
        assert_ne!(genomes[0], genomes[1], "distinct ids, distinct genomes");
        // Short ids are real registry ids, not truncation: the derivation
        // hashes an opaque string, so length must not matter.
        assert_eq!(genomes[2].len(), 2048);

        assert!(first.contains("\"avatar_type\":\"creature.classic\""));
        assert!(first.contains("\"avatar_version\":2"));
    }

    #[test]
    fn stored_and_derived_paths_agree_until_a_reroll_separates_them() {
        // The two paths must coincide for an un-re-rolled agent, which is
        // what makes the stored path a safe drop-in today. They stop
        // coinciding the moment a genome is re-rolled: derivation still
        // reproduces the ORIGINAL creature while the row wears a new one.
        // Anything depicting a live agent must therefore render the stored
        // genome, or a GitHub avatar and a phone will disagree permanently,
        // each looking entirely correct.
        let id = "agent_0000000000000001";
        let derived = derive_genome(&SeatIdentity::Registered {
            project_id: id.to_owned(),
        });

        let from_id = export_png(id, Mood::Neutral, 64).expect("derived render");
        let from_genome =
            export_png_from_genome(&derived, Mood::Neutral, 64).expect("stored render");
        assert_eq!(from_id, from_genome);

        // A different genome must not render the same creature, or the
        // assertion above would pass for the wrong reason.
        let other = derive_genome(&SeatIdentity::Registered {
            project_id: "agent_somebody_else".to_owned(),
        });
        let from_other = export_png_from_genome(&other, Mood::Neutral, 64).expect("other render");
        assert_ne!(from_genome, from_other);
    }

    #[test]
    fn ids_parse_from_a_list_or_a_file() {
        assert_eq!(read_ids("a, b ,c"), vec!["a", "b", "c"]);
        let path = std::env::temp_dir().join("avatar-export-ids-test.txt");
        std::fs::write(&path, "# roster\nagent_x\n\n  agent_y  \n").unwrap();
        assert_eq!(
            read_ids(&format!("@{}", path.display())),
            vec!["agent_x", "agent_y"]
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn manifest_scan_reads_ids_and_ignores_unknown_keys() {
        let ids =
            manifest_ids(r#"[{"agent":"AVA","stable_id":"pj-1"},{"stable_id": "pj-2","x":1}]"#);
        assert_eq!(ids, vec!["pj-1", "pj-2"]);
    }
}
