//! Deterministic image export for stored avatars.
//!
//! Image exports take a stored genome with its type and generation version,
//! so they depict the current creature even after its genome has been re-rolled.
//!
//! Classic renders through its legacy SVG renderer; wild renders through
//! the render tree serializer. Both rasterize with resvg (pure Rust, with
//! no system fonts needed because the SVG contains no text). Consumers
//! re-run exports when an agent's stored genome, type or version changes;
//! this tool keeps no cache.
//!
//! Usage:
//!   avatar-export --genome <hex> --type creature.wild --version 2 --out avatar.png
//!   avatar-export --genome-file genome.hex --type creature.wild --version 2 --out avatar.png
//!     --genome-file reads the same hex from a file ("-" reads standard
//!     input). A wild genome is 32,768 hex characters, which does not fit on
//!     a Windows command line (CreateProcess allows 32,767 characters including
//!     its terminating NUL, and the program name counts too), so callers that
//!     must work on every operating system pass genomes this way.
//!     The --out extension picks the format: .png or no extension (PNG),
//!     .svg (the drawing itself; a wild SVG is a 240-unit square canvas with
//!     no background), or .json (canonical render-tree JSON, which clients
//!     decode and draw). Any other extension is refused. --size applies to
//!     PNG only.
//!     Options: --size N (default 400), --mood M (default neutral).
//!   avatar-export --mint-genomes <id,id,...|@file> [--type <type>] [--out <file>]
//!     With @file, reads one agent ID per line, ignoring blank lines and # comments.
//!     Writes a JSON genome manifest, to standard output if --out is absent.

use avatar::{render_tree_for_bytes, serialize_tree, serialize_tree_json, Mood};
use avatar_genome::{decode, derive_genome_bytes, Genome, SeatIdentity};
// Classic SVG and PNG exports keep avatar-svg's direct classic renderer,
// whose bytes differ from the generic tree serializer's.
use avatar_svg::{render_svg, RenderParams, Q32};

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

fn classic_svg(genome: &Genome, mood: Mood) -> Result<Vec<u8>, String> {
    let phenotype = decode(genome);
    let params = RenderParams {
        mood,
        growth: Q32::ONE,
        canvas: 10_000,
    };
    render_svg(&phenotype, &params).map_err(|e| e.to_string())
}

/// Checks a stored genome against its type and version, in that order, and
/// returns its bytes. Every export of a stored genome goes through this, so a
/// malformed row is refused the same way whatever is being written.
fn stored_genome_bytes(hex: &str, type_name: &str, version: u32) -> Result<Vec<u8>, String> {
    let (_, len, versions) =
        mintable(type_name).ok_or_else(|| format!("unknown avatar type {type_name:?}"))?;
    if !versions.contains(&version) {
        let supported: Vec<String> = versions.iter().map(u32::to_string).collect();
        return Err(format!(
            "unsupported version {version} for {type_name:?}; supported: {}",
            supported.join(", ")
        ));
    }
    let hex = hex.trim();
    if hex.len() != len * 2 {
        return Err(format!(
            "expected {} hex characters, got {}",
            len * 2,
            hex.len()
        ));
    }
    hex.as_bytes()
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
        .collect()
}

/// Renders a stored genome to SVG and hands it to `encode`, which rasterizes
/// it to PNG or, for an SVG export, returns it unchanged.
fn export_from_hex(
    hex: &str,
    type_name: &str,
    version: u32,
    mood: Mood,
    size: u32,
    encode: impl FnOnce(&[u8], u32) -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    let bytes = stored_genome_bytes(hex, type_name, version)?;
    let svg = if type_name == AVATAR_TYPE {
        // Keep classic SVG bytes unchanged; tree serialization has a different byte format.
        let genome = Genome::from_hex(hex.trim()).map_err(|e| e.to_string())?;
        classic_svg(&genome, mood)?
    } else {
        let tree =
            render_tree_for_bytes(&bytes, type_name, version, mood).map_err(|e| e.to_string())?;
        serialize_tree(&tree)
    };
    encode(&svg, size)
}

/// Renders a stored genome to canonical render-tree JSON, the transport
/// clients decode and draw. Any type has a tree, including classic.
fn tree_json_from_hex(
    hex: &str,
    type_name: &str,
    version: u32,
    mood: Mood,
) -> Result<Vec<u8>, String> {
    let bytes = stored_genome_bytes(hex, type_name, version)?;
    let tree =
        render_tree_for_bytes(&bytes, type_name, version, mood).map_err(|e| e.to_string())?;
    Ok(serialize_tree_json(&tree).into_bytes())
}

/// What a stored-genome export writes, chosen by the `--out` extension.
/// An extension that names no supported format is refused (`None`), so a
/// caller asking for, say, `face.jpg` does not silently get PNG bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum OutputFormat {
    Png,
    Svg,
    TreeJson,
}

fn output_format(out: &str) -> Option<OutputFormat> {
    let extension = std::path::Path::new(out)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    match extension.as_deref() {
        None | Some("png") => Some(OutputFormat::Png),
        Some("svg") => Some(OutputFormat::Svg),
        Some("json") => Some(OutputFormat::TreeJson),
        Some(_) => None,
    }
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
fn read_ids(arg: &str) -> Result<Vec<String>, String> {
    let text;
    let source = match arg.strip_prefix('@') {
        Some(path) => {
            text = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
            text.as_str()
        }
        None => arg,
    };
    let split: Box<dyn Iterator<Item = &str>> = if arg.starts_with('@') {
        Box::new(source.lines())
    } else {
        Box::new(source.split(','))
    };
    Ok(split
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .map(str::to_owned)
        .collect())
}

/// A genome manifest holds the initial genomes minted for a batch of agents.
///
/// The genome is STORED on the agent's record and served from there, so
/// derivation runs once per agent, when the agent is created or backfilled,
/// rather than on every render. A manifest is one auditable artifact for such
/// a batch, which the host that owns the records imports into its store.
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
///
/// The last field lists every generation version this tool renders for the
/// type, oldest first. A stored row renders at the version it names, so a
/// type keeps its older versions here; new rows are minted at the last one,
/// the current generation.
const MINTABLE: &[(&str, usize, &[u32])] = &[
    (AVATAR_TYPE, 1_024, &[AVATAR_VERSION]),
    // Version 2 puts mood lids and squints on aspect-scaled eyes; version 1
    // drew them beside the eye and stays for rows that still store it.
    ("creature.wild", 16_384, &[1, 2]),
];

fn mintable(type_name: &str) -> Option<(&'static str, usize, &'static [u32])> {
    MINTABLE.iter().copied().find(|(t, _, _)| *t == type_name)
}

/// The current generation of a type: what new rows are minted at, and what a
/// stored-genome export renders when no version is given.
fn current_version(versions: &[u32]) -> u32 {
    *versions
        .last()
        .expect("every mintable type renders at least one version")
}

/// One JSON object per agent, each carrying its own type and version.
///
/// Deliberately NOT a map of id to genome with the type stated once at the
/// top: an entry that carries its own type cannot be separated from the
/// genome it describes by a later edit, a merge, or a partial copy. A
/// genome is uninterpretable without knowing which layout minted it, so
/// the two must travel together.
fn mint_manifest(ids: &[String], type_name: &str) -> String {
    let (type_name, len, versions) = mintable(type_name).expect("caller validated the type");
    let version = current_version(versions);
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
        let record = serde_json::json!({
            "agent_id": id,
            "avatar_genome": hex,
            "avatar_type": type_name,
            "avatar_version": version,
        });
        out.push_str(&format!("  {record}{comma}\n"));
    }
    out.push_str("]\n");
    out
}

/// Pixel size for the raster, defaulting only when the flag is ABSENT.
///
/// This read `.parse().ok().unwrap_or(400)`, so `--size 51w` silently
/// minted 400px images: a batch export would produce every image at the
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

#[derive(Default)]
struct Options {
    size: Option<String>,
    mood: Option<String>,
    genome: Option<String>,
    genome_file: Option<String>,
    type_name: Option<String>,
    version: Option<String>,
    out: Option<String>,
    mint_genomes: Option<String>,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<Self, String> {
        let mut options = Self::default();
        while let Some(flag) = args.next() {
            let flag = flag.into_string().map_err(|_| "option name is not UTF-8")?;
            let slot = match flag.as_str() {
                "--size" => &mut options.size,
                "--mood" => &mut options.mood,
                "--genome" => &mut options.genome,
                "--genome-file" => &mut options.genome_file,
                "--type" => &mut options.type_name,
                "--version" => &mut options.version,
                "--out" => &mut options.out,
                "--mint-genomes" => &mut options.mint_genomes,
                _ => return Err(format!("unknown option {flag:?}")),
            };
            let value = args
                .next()
                .ok_or_else(|| format!("{flag}: missing value"))?
                .into_string()
                .map_err(|_| format!("{flag}: value is not UTF-8"))?;
            if value.is_empty() || value.starts_with("--") {
                return Err(format!("{flag}: missing value"));
            }
            if slot.replace(value).is_some() {
                return Err(format!("{flag}: passed more than once"));
            }
        }
        Ok(options)
    }
}

fn input_error(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

fn main() {
    let options = Options::parse(std::env::args_os().skip(1)).unwrap_or_else(|e| input_error(&e));
    if options.mint_genomes.is_some() {
        if options.genome.is_some() || options.genome_file.is_some() {
            input_error("--mint-genomes cannot be combined with --genome or --genome-file");
        }
        if options.version.is_some() || options.size.is_some() || options.mood.is_some() {
            input_error("--version, --size and --mood are only for stored-genome exports");
        }
    }
    let type_name = options.type_name.as_deref().unwrap_or(AVATAR_TYPE);

    let size = match parse_size(options.size.as_deref()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let mood = match mood_from(options.mood.as_deref().unwrap_or("neutral")) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };

    // Renders a genome the caller already holds — the path anything
    // depicting a live agent must use, since after a re-roll the stored
    // genome and a derived one are different creatures.
    let genome_hex = match (options.genome, options.genome_file) {
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
        let default_version = mintable(type_name)
            .map_or(AVATAR_VERSION, |(_, _, versions)| current_version(versions));
        let version = match options.version {
            Some(value) => value.parse::<u32>().unwrap_or_else(|_| {
                eprintln!("--version: expected an unsigned integer, got {value:?}");
                std::process::exit(2);
            }),
            None => default_version,
        };
        let out = options.out.as_deref().unwrap_or("avatar.png");
        let Some(format) = output_format(out) else {
            eprintln!("--out {out}: unsupported extension; use .png, .svg or .json");
            std::process::exit(2);
        };
        let exported = match format {
            OutputFormat::Png => export_from_hex(&hex, type_name, version, mood, size, rasterize),
            OutputFormat::Svg => export_from_hex(&hex, type_name, version, mood, size, |svg, _| {
                Ok(svg.to_vec())
            }),
            OutputFormat::TreeJson => tree_json_from_hex(&hex, type_name, version, mood),
        };
        match exported {
            Ok(bytes) => {
                std::fs::write(out, bytes).expect("write output");
                println!("{out}");
            }
            Err(e) => {
                eprintln!("export: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(ids_arg) = options.mint_genomes {
        let ids =
            read_ids(&ids_arg).unwrap_or_else(|e| input_error(&format!("--mint-genomes: {e}")));
        if ids.is_empty() {
            eprintln!("--mint-genomes: no agent ids given");
            std::process::exit(2);
        }
        if mintable(type_name).is_none() {
            eprintln!("--type: unknown avatar type {type_name:?}");
            eprintln!("  known: {}", {
                let names: Vec<&str> = MINTABLE.iter().map(|(t, _, _)| *t).collect();
                names.join(", ")
            });
            std::process::exit(2);
        }
        let manifest = mint_manifest(&ids, type_name);
        match options.out {
            Some(path) => {
                std::fs::write(&path, &manifest).expect("write manifest");
                eprintln!("minted {} genomes -> {path}", ids.len());
            }
            None => print!("{manifest}"),
        }
        return;
    }

    input_error(
        "usage: avatar-export (--genome <hex> | --genome-file <path|->) [--type creature.classic|creature.wild] [--version N] [--out f.png|f.svg|f.json]\n       options: --size N (default 400, PNG only), --mood neutral|working|happy|waiting|blocked|sad|sleepy\n       stored-genome defaults: creature.classic; the newest version of the type (2 for classic, 2 for wild)\n       avatar-export --mint-genomes <id,id,...|@file> [--type creature.classic|creature.wild] [--out <file>]"
    );
}

#[cfg(test)]
mod tests {
    use avatar_genome::derive_genome;

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
    const WILD_V2_HEX: &str = include_str!("../../../vectors/wild/wild-port-0022.hex");

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
            let png = export_from_hex(WILD_HEX, "creature.wild", 1, mood, 64, |svg, size| {
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
        let png = export_from_hex(
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
        assert_eq!(png, rasterize(&expected, 64).unwrap());
    }

    #[test]
    fn invalid_stored_genomes_never_reach_rasterization() {
        let invalid_hex = "gg".repeat(16_384);
        let unicode_hex = "é".repeat(16_384);
        for (hex, type_name, version, error) in [
            (WILD_HEX, "creature.unknown", 1, "unknown avatar type"),
            (WILD_HEX, "creature.wild", 3, "unsupported version"),
            ("", "creature.wild", 3, "unsupported version"),
            (WILD_HEX, AVATAR_TYPE, 1, "unsupported version"),
            (WILD_HEX, AVATAR_TYPE, 2, "hex"),
            ("00", "creature.wild", 1, "expected 32768 hex characters"),
            (invalid_hex.as_str(), "creature.wild", 1, "invalid hex"),
            (unicode_hex.as_str(), "creature.wild", 1, "invalid hex"),
        ] {
            let result = export_from_hex(hex, type_name, version, Mood::Neutral, 64, |_, _| {
                panic!("invalid input reached the rasterizer")
            });
            assert!(result.unwrap_err().contains(error));
            let json = tree_json_from_hex(hex, type_name, version, Mood::Neutral);
            assert!(json.unwrap_err().contains(error));
        }
    }

    #[test]
    fn output_format_follows_the_out_extension() {
        assert_eq!(output_format("a.png"), Some(OutputFormat::Png));
        assert_eq!(output_format("a"), Some(OutputFormat::Png));
        assert_eq!(output_format("dir/a.SVG"), Some(OutputFormat::Svg));
        assert_eq!(output_format("a.svg"), Some(OutputFormat::Svg));
        assert_eq!(output_format("a.json"), Some(OutputFormat::TreeJson));
        // An unsupported extension is refused, never answered with PNG.
        assert_eq!(output_format("a.jpg"), None);
        assert_eq!(output_format("a.gif"), None);
    }

    #[test]
    fn wild_svg_and_tree_json_exports_match_the_golden_corpus() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (mood, name) in [
            (Mood::Neutral, "neutral"),
            (Mood::Working, "working"),
            (Mood::Happy, "happy"),
            (Mood::Waiting, "waiting"),
            (Mood::Blocked, "blocked"),
            (Mood::Sad, "sad"),
            (Mood::Sleepy, "sleepy"),
        ] {
            let svg = export_from_hex(WILD_HEX, "creature.wild", 1, mood, 64, |svg, _| {
                Ok(svg.to_vec())
            })
            .unwrap();
            let golden = root.join(format!("vectors/tree-svg/wild-port-0005__{name}.svg"));
            assert_eq!(svg, std::fs::read(golden).unwrap(), "SVG for {name}");
            let json = tree_json_from_hex(WILD_HEX, "creature.wild", 1, mood).unwrap();
            let golden = root.join(format!("vectors/tree/wild-port-0005__{name}.json"));
            assert_eq!(json, std::fs::read(golden).unwrap(), "tree JSON for {name}");

            // Version 2 has its own golden trees, for a creature whose
            // narrowed eyes make the two generations draw differently.
            let svg = export_from_hex(WILD_V2_HEX, "creature.wild", 2, mood, 64, |svg, _| {
                Ok(svg.to_vec())
            })
            .unwrap();
            let golden = root.join(format!("vectors/tree-svg/wild-port-0022__v2__{name}.svg"));
            assert_eq!(
                svg,
                std::fs::read(golden).unwrap(),
                "version 2 SVG for {name}"
            );
            let json = tree_json_from_hex(WILD_V2_HEX, "creature.wild", 2, mood).unwrap();
            let golden = root.join(format!("vectors/tree/wild-port-0022__v2__{name}.json"));
            assert_eq!(
                json,
                std::fs::read(golden).unwrap(),
                "version 2 tree JSON for {name}"
            );
        }
    }

    #[test]
    fn minted_manifest_holds_the_properties_a_host_imports_on() {
        // These are the properties a host relies on when it imports a
        // manifest into its store: reproducible, one genome per agent, and
        // every genome the declared layout size, so a store that checks the
        // length accepts it. A change that broke any of them would write
        // wrong data into every imported row.
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
        // New rows carry the current wild generation, not the first one.
        assert!(wild.contains("\"avatar_version\":2"));
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
        // A short id is as valid as a long one: the derivation hashes an
        // opaque string, so length must not matter.
        assert_eq!(genomes[2].len(), 2048);

        assert!(first.contains("\"avatar_type\":\"creature.classic\""));
        assert!(first.contains("\"avatar_version\":2"));
    }

    #[test]
    fn ids_parse_from_a_list_or_a_file() {
        assert_eq!(read_ids("a, b ,c").unwrap(), vec!["a", "b", "c"]);
        let path = std::env::temp_dir().join("avatar-export-ids-test.txt");
        std::fs::write(&path, "# roster\nagent_x\n\n  agent_y  \n").unwrap();
        assert_eq!(
            read_ids(&format!("@{}", path.display())).unwrap(),
            vec!["agent_x", "agent_y"]
        );
        let _ = std::fs::remove_file(&path);
    }
}
