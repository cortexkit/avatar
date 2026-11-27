//! Compare a reference run of the fidelity harness with a port's run.
//!
//! ```text
//! cargo run --release -p cortexkit-avatar-raster --example fidelity_compare -- \
//!     --manifest BASE --reference DIR --candidate DIR --report FILE \
//!     [--probes PROBES] [--corpus-manifest CORPUS --corpus-choices DIR] [--size PX]
//! ```
//!
//! `scripts/fidelity/` writes the manifests and the reference side: for every
//! case, `<case>.svg` and `<case>.choices.json`, plus `domains.json`. A
//! generator crate's `examples/fidelity_port.rs` writes the same two files
//! per case for the port. `scripts/fidelity/README.md` describes the formats.
//!
//! The comparison:
//!
//! - The manifests are checked first. A repeated case ID, or a probe ID that
//!   is also a base ID, stops the run before anything is written.
//! - Each directory must hold exactly the cases of the base and probe
//!   manifests, each as an SVG and a choices file (`domains.json` may sit
//!   beside them). A missing file and an unexpected file are failures, so a
//!   case missing from both sides fails too.
//! - Both SVGs are drawn with [`avatar_raster::rasterize_svg`] at `--size`
//!   pixels (256 by default), premultiplied, on a transparent background, and
//!   every RGBA channel is compared. A pixel fails when any channel differs by
//!   more than 8/255; an image fails when more than 1% of its pixels fail.
//! - The two choices files must have the same keys and equal values. A
//!   choices file whose values are not integers, strings or booleans, or
//!   whose `body_rgb` is not three integers from 0 to 255, is a harness
//!   error, not a mismatch, and so is a reference choice that its
//!   `domains.json` does not list.
//! - Branch coverage: every key and value in `domains.json` is a branch. The
//!   report counts how many base cases, probe cases and committed-corpus
//!   cases reach each one on the reference side, and marks a branch the base
//!   and probe cases miss as missed and one the corpus misses as unpinned.
//!
//! The report is written to `--report` as Markdown and summarised on
//! standard output. The exit status is 0 when every case passes, 1 when a
//! case fails, and 2 for a harness error, in which case no report is written.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};

use avatar_raster::{rasterize_svg, Alpha};

const USAGE: &str =
    "usage: fidelity_compare --manifest BASE --reference DIR --candidate DIR --report FILE \
[--probes PROBES] [--corpus-manifest CORPUS --corpus-choices DIR] [--size PX]";
const MANIFEST_FORMAT: &str = "avatar-fidelity-manifest/1";
const MOODS: [&str; 7] = [
    "neutral", "working", "happy", "waiting", "blocked", "sad", "sleepy",
];
/// The base profile's case count: 265 genomes in seven moods.
const BASE_CASES: usize = 1_855;
const GENOME_HEX_DIGITS: usize = 2 * 16_384;
/// A pixel fails when any channel differs by more than this.
const CHANNEL_TOLERANCE: u8 = 8;
/// An image fails when more than this percentage of its pixels fail.
const FAILING_PIXEL_PERCENT: usize = 1;

fn main() {
    let options = Options::parse(std::env::args().skip(1).collect())
        .unwrap_or_else(|message| harness_error(&format!("{message}\n{USAGE}")));
    match run(&options) {
        Ok(outcome) => process::exit(if outcome.failed { 1 } else { 0 }),
        Err(message) => harness_error(&message),
    }
}

fn harness_error(message: &str) -> ! {
    eprintln!("fidelity_compare: {message}");
    process::exit(2)
}

struct Options {
    manifest: PathBuf,
    probes: Option<PathBuf>,
    reference: PathBuf,
    candidate: PathBuf,
    corpus: Option<(PathBuf, PathBuf)>,
    report: PathBuf,
    size_px: u32,
}

impl Options {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut flags: BTreeMap<String, String> = BTreeMap::new();
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            let name = flag
                .strip_prefix("--")
                .filter(|name| {
                    [
                        "manifest",
                        "probes",
                        "reference",
                        "candidate",
                        "corpus-manifest",
                        "corpus-choices",
                        "report",
                        "size",
                    ]
                    .contains(name)
                })
                .ok_or_else(|| format!("unknown argument {flag:?}"))?;
            let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
            if flags.insert(name.to_owned(), value).is_some() {
                return Err(format!("{flag} is given twice"));
            }
        }
        let mut take = |name: &str| flags.remove(name).map(PathBuf::from);
        let required = |value: Option<PathBuf>, name: &str| {
            value.ok_or_else(|| format!("--{name} is required"))
        };
        let manifest = required(take("manifest"), "manifest")?;
        let reference = required(take("reference"), "reference")?;
        let candidate = required(take("candidate"), "candidate")?;
        let report = required(take("report"), "report")?;
        let probes = take("probes");
        let corpus = match (take("corpus-manifest"), take("corpus-choices")) {
            (Some(manifest), Some(choices)) => Some((manifest, choices)),
            (None, None) => None,
            _ => return Err("--corpus-manifest and --corpus-choices go together".into()),
        };
        let size_px = match flags.remove("size") {
            None => 256,
            Some(text) => text
                .parse::<u32>()
                .ok()
                .filter(|size| (1..=avatar_raster::MAX_SIZE_PX).contains(size))
                .ok_or_else(|| {
                    format!(
                        "--size must be a whole number of pixels from 1 to {}, not {text:?}",
                        avatar_raster::MAX_SIZE_PX
                    )
                })?,
        };
        Ok(Self {
            manifest,
            probes,
            reference,
            candidate,
            corpus,
            report,
            size_px,
        })
    }
}

// ------------------------------------------------------------------ manifests

/// A case manifest: its case IDs in order, and what identifies the file.
#[derive(Debug)]
struct Manifest {
    path: PathBuf,
    sha256: String,
    avatar_type: String,
    cases: Vec<String>,
}

/// Reads a manifest and checks its structure: the format, the kind the flag
/// expects, unique genome names with full genomes, and unique case IDs of
/// the form `<genome>__<mood>`, each naming a listed genome.
fn read_manifest(path: &Path, kind: &str) -> Result<Manifest, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    parse_manifest(&bytes, path, kind)
}

fn parse_manifest(bytes: &[u8], path: &Path, kind: &str) -> Result<Manifest, String> {
    let what = path.display().to_string();
    let text = std::str::from_utf8(bytes).map_err(|_| format!("{what}: not UTF-8"))?;
    let root = parse_json(text).map_err(|error| format!("{what}: {error}"))?;
    let fail = |message: String| format!("{what}: {message}");
    let members = object(&root, "the manifest").map_err(fail)?;
    let keys: Vec<&str> = members.iter().map(|(key, _)| key.as_str()).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    if sorted != ["cases", "format", "genomes", "kind", "type"] {
        return Err(fail(format!(
            "expected exactly the keys cases, format, genomes, kind and type, found {keys:?}"
        )));
    }
    let format = string_field(members, "format").map_err(fail)?;
    if format != MANIFEST_FORMAT {
        return Err(fail(format!(
            "format is {format:?}, not {MANIFEST_FORMAT:?}"
        )));
    }
    let found_kind = string_field(members, "kind").map_err(fail)?;
    if found_kind != kind {
        return Err(fail(format!(
            "is a {found_kind:?} manifest where a {kind:?} manifest was expected"
        )));
    }
    let avatar_type = string_field(members, "type").map_err(fail)?.to_owned();
    if !avatar_type.starts_with("creature.") {
        return Err(fail(format!("type {avatar_type:?} is not a creature type")));
    }

    let mut genomes = BTreeSet::new();
    for genome in array_field(members, "genomes").map_err(fail)? {
        let entry = object(genome, "a genome").map_err(fail)?;
        let name = string_field(entry, "name").map_err(fail)?;
        if !is_name(name) {
            return Err(fail(format!(
                "genome name {name:?} is not lowercase letters, digits and hyphens"
            )));
        }
        string_field(entry, "source").map_err(fail)?;
        let hex = string_field(entry, "hex").map_err(fail)?;
        if hex.len() != GENOME_HEX_DIGITS
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(fail(format!(
                "genome {name} is not {GENOME_HEX_DIGITS} lowercase hex digits"
            )));
        }
        if !genomes.insert(name.to_owned()) {
            return Err(fail(format!("genome {name} is listed twice")));
        }
    }

    let mut cases = Vec::new();
    let mut seen = BTreeSet::new();
    for case in array_field(members, "cases").map_err(fail)? {
        let entry = object(case, "a case").map_err(fail)?;
        let id = string_field(entry, "id").map_err(fail)?;
        let genome = string_field(entry, "genome").map_err(fail)?;
        let mood = string_field(entry, "mood").map_err(fail)?;
        if !seen.insert(id.to_owned()) {
            return Err(fail(format!("case ID {id} is repeated")));
        }
        if !genomes.contains(genome) {
            return Err(fail(format!(
                "case {id} names genome {genome:?}, which the manifest does not list"
            )));
        }
        if !MOODS.contains(&mood) {
            return Err(fail(format!("case {id} has unknown mood {mood:?}")));
        }
        if id != format!("{genome}__{mood}") {
            return Err(fail(format!("case ID {id} is not {genome}__{mood}")));
        }
        cases.push(id.to_owned());
    }
    if cases.is_empty() {
        return Err(fail("lists no cases".into()));
    }
    Ok(Manifest {
        path: path.to_owned(),
        sha256: hex(&sha256(bytes)),
        avatar_type,
        cases,
    })
}

/// A genome or case name: lowercase letters, digits and hyphens, starting
/// with a letter or digit, so it is a safe file name.
fn is_name(name: &str) -> bool {
    name.bytes().next().is_some_and(|b| b != b'-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

// ----------------------------------------------------------------------- JSON

/// A JSON value. Numbers are kept apart by kind: an integer that fits `i64`,
/// or the text of any other number (a fraction or an exponent), which the
/// harness treats as an error wherever it appears.
enum Json {
    Null,
    Bool(bool),
    Integer(i64),
    OtherNumber(String),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    fn describe(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(value) => value.to_string(),
            Self::Integer(value) => value.to_string(),
            Self::OtherNumber(text) => format!("the non-integer number {text}"),
            Self::String(text) => format!("{text:?}"),
            Self::Array(_) => "an array".into(),
            Self::Object(_) => "an object".into(),
        }
    }
}

fn parse_json(text: &str) -> Result<Json, String> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        at: 0,
    };
    let value = parser.value()?;
    parser.skip_whitespace();
    if parser.at != parser.bytes.len() {
        return Err(format!("unexpected text at byte {}", parser.at));
    }
    Ok(value)
}

fn object<'a>(value: &'a Json, what: &str) -> Result<&'a [(String, Json)], String> {
    match value {
        Json::Object(members) => Ok(members),
        _ => Err(format!("{what} is not a JSON object")),
    }
}

fn field<'a>(members: &'a [(String, Json)], key: &str) -> Result<&'a Json, String> {
    members
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
        .ok_or_else(|| format!("missing key {key:?}"))
}

fn string_field<'a>(members: &'a [(String, Json)], key: &str) -> Result<&'a str, String> {
    match field(members, key)? {
        Json::String(text) => Ok(text),
        other => Err(format!("{key:?} is {}, not a string", other.describe())),
    }
}

fn array_field<'a>(members: &'a [(String, Json)], key: &str) -> Result<&'a [Json], String> {
    match field(members, key)? {
        Json::Array(items) => Ok(items),
        other => Err(format!("{key:?} is {}, not an array", other.describe())),
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn skip_whitespace(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.bytes.get(self.at) {
            self.at += 1;
        }
    }

    /// Consumes `byte` if it comes next, after any whitespace.
    fn eat(&mut self, byte: u8) -> bool {
        self.skip_whitespace();
        let found = self.bytes.get(self.at) == Some(&byte);
        if found {
            self.at += 1;
        }
        found
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.eat(byte) {
            Ok(())
        } else {
            Err(format!("expected {:?} at byte {}", byte as char, self.at))
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(format!("unexpected text at byte {}", self.at))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_whitespace();
        match self.bytes.get(self.at) {
            Some(b'{') => {
                self.at += 1;
                let mut members: Vec<(String, Json)> = Vec::new();
                if !self.eat(b'}') {
                    loop {
                        self.skip_whitespace();
                        let start = self.at;
                        let key = self.string()?;
                        // A repeated key would make the file mean whatever the
                        // reader keeps, so it is refused.
                        if members.iter().any(|(name, _)| *name == key) {
                            return Err(format!("repeated key {key:?} at byte {start}"));
                        }
                        self.expect(b':')?;
                        members.push((key, self.value()?));
                        if self.eat(b'}') {
                            break;
                        }
                        self.expect(b',')?;
                    }
                }
                Ok(Json::Object(members))
            }
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                if !self.eat(b']') {
                    loop {
                        items.push(self.value()?);
                        if self.eat(b']') {
                            break;
                        }
                        self.expect(b',')?;
                    }
                }
                Ok(Json::Array(items))
            }
            Some(b'"') => Ok(Json::String(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(format!("unexpected text at byte {}", self.at)),
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.at;
        while let Some(b'0'..=b'9') = self.bytes.get(self.at) {
            self.at += 1;
        }
        self.at - start
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        if self.bytes.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        let first = self.at;
        let whole = self.digits();
        if whole == 0 || (whole > 1 && self.bytes[first] == b'0') {
            return Err(format!("malformed number at byte {start}"));
        }
        let mut integer = true;
        if self.bytes.get(self.at) == Some(&b'.') {
            self.at += 1;
            integer = false;
            if self.digits() == 0 {
                return Err(format!("malformed number at byte {start}"));
            }
        }
        if let Some(b'e' | b'E') = self.bytes.get(self.at) {
            self.at += 1;
            integer = false;
            if let Some(b'+' | b'-') = self.bytes.get(self.at) {
                self.at += 1;
            }
            if self.digits() == 0 {
                return Err(format!("malformed number at byte {start}"));
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).expect("ASCII digits");
        if !integer {
            return Ok(Json::OtherNumber(text.to_owned()));
        }
        text.parse()
            .map(Json::Integer)
            .map_err(|_| format!("the integer at byte {start} is out of range"))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.at..self.at + 4)
            .and_then(|digits| std::str::from_utf8(digits).ok())
            .and_then(|digits| u32::from_str_radix(digits, 16).ok())
            .ok_or_else(|| format!("malformed \\u escape at byte {}", self.at))?;
        self.at += 4;
        Ok(digits)
    }

    fn string(&mut self) -> Result<String, String> {
        let start = self.at;
        if self.bytes.get(self.at) != Some(&b'"') {
            return Err(format!("expected a string at byte {start}"));
        }
        self.at += 1;
        let mut out = Vec::new();
        loop {
            let byte = *self
                .bytes
                .get(self.at)
                .ok_or_else(|| format!("unterminated string at byte {start}"))?;
            self.at += 1;
            match byte {
                b'"' => break,
                b'\\' => {
                    let escape = *self
                        .bytes
                        .get(self.at)
                        .ok_or_else(|| format!("unterminated string at byte {start}"))?;
                    self.at += 1;
                    let decoded = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let high = self.hex4()?;
                            let code = if (0xd800..0xdc00).contains(&high) {
                                if !self.bytes[self.at..].starts_with(b"\\u") {
                                    return Err(format!("unpaired surrogate at byte {}", self.at));
                                }
                                self.at += 2;
                                let low = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&low) {
                                    return Err(format!("unpaired surrogate at byte {}", self.at));
                                }
                                0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00)
                            } else {
                                high
                            };
                            char::from_u32(code)
                                .ok_or_else(|| format!("invalid \\u escape at byte {}", self.at))?
                        }
                        _ => return Err(format!("unknown escape at byte {}", self.at - 1)),
                    };
                    let mut buffer = [0; 4];
                    out.extend_from_slice(decoded.encode_utf8(&mut buffer).as_bytes());
                }
                0..=0x1f => return Err(format!("control character in the string at byte {start}")),
                _ => out.push(byte),
            }
        }
        String::from_utf8(out).map_err(|_| format!("the string at byte {start} is not UTF-8"))
    }
}

// ------------------------------------------------------------------- choices

/// One discrete choice: an integer, a string or a boolean.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Choice {
    Bool(bool),
    Integer(i64),
    Text(String),
}

impl std::fmt::Display for Choice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(value) => write!(formatter, "{value}"),
            Self::Integer(value) => write!(formatter, "{value}"),
            Self::Text(text) => write!(formatter, "{text:?}"),
        }
    }
}

/// A value in a choices file: a choice, or the body colour.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ChoiceValue {
    One(Choice),
    Rgb([u8; 3]),
}

impl std::fmt::Display for ChoiceValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::One(choice) => write!(formatter, "{choice}"),
            Self::Rgb([r, g, b]) => write!(formatter, "[{r},{g},{b}]"),
        }
    }
}

type Choices = BTreeMap<String, ChoiceValue>;

/// The body colour's key: the one value that is an array.
const BODY_RGB: &str = "body_rgb";

fn choice_of(value: &Json) -> Option<Choice> {
    match value {
        Json::Bool(value) => Some(Choice::Bool(*value)),
        Json::Integer(value) => Some(Choice::Integer(*value)),
        Json::String(text) => Some(Choice::Text(text.clone())),
        _ => None,
    }
}

/// Reads a choices file: one flat JSON object of integers, strings and
/// booleans, plus `body_rgb` as three integers from 0 to 255. Anything else
/// is a harness error.
fn read_choices(path: &Path) -> Result<Choices, String> {
    let what = path.display().to_string();
    let text = fs::read_to_string(path).map_err(|error| format!("read {what}: {error}"))?;
    parse_choices(&text, &what)
}

fn parse_choices(text: &str, what: &str) -> Result<Choices, String> {
    let root = parse_json(text).map_err(|error| format!("{what}: {error}"))?;
    let members = object(&root, "the choices").map_err(|error| format!("{what}: {error}"))?;
    let mut choices = Choices::new();
    for (key, value) in members {
        let parsed = if key == BODY_RGB {
            let channels = match value {
                Json::Array(items) if items.len() == 3 => items
                    .iter()
                    .map(|item| match item {
                        Json::Integer(channel) => u8::try_from(*channel).ok(),
                        _ => None,
                    })
                    .collect::<Option<Vec<u8>>>(),
                _ => None,
            };
            let [r, g, b] = channels.as_deref().unwrap_or(&[]) else {
                return Err(format!(
                    "{what}: body_rgb is not three integers from 0 to 255"
                ));
            };
            ChoiceValue::Rgb([*r, *g, *b])
        } else {
            ChoiceValue::One(choice_of(value).ok_or_else(|| {
                format!(
                    "{what}: choice {key:?} is {}; a choice is an integer, a string or a boolean",
                    value.describe()
                )
            })?)
        };
        choices.insert(key.clone(), parsed);
    }
    Ok(choices)
}

/// `domains.json`: for every choices key except `body_rgb`, the values the
/// reference's own selector can give, in the order the shim lists them.
type Domains = BTreeMap<String, Vec<Choice>>;

fn read_domains(path: &Path) -> Result<Domains, String> {
    let what = path.display().to_string();
    let text = fs::read_to_string(path).map_err(|error| format!("read {what}: {error}"))?;
    parse_domains(&text, &what)
}

fn parse_domains(text: &str, what: &str) -> Result<Domains, String> {
    let root = parse_json(text).map_err(|error| format!("{what}: {error}"))?;
    let members = object(&root, "domains.json").map_err(|error| format!("{what}: {error}"))?;
    let mut domains = Domains::new();
    for (key, values) in members {
        if key == BODY_RGB {
            return Err(format!("{what}: body_rgb has no domain"));
        }
        let Json::Array(items) = values else {
            return Err(format!("{what}: the domain of {key:?} is not an array"));
        };
        let mut domain: Vec<Choice> = Vec::new();
        for item in items {
            let choice = choice_of(item).ok_or_else(|| {
                format!("{what}: the domain of {key:?} holds {}", item.describe())
            })?;
            if domain.contains(&choice) {
                return Err(format!(
                    "{what}: the domain of {key:?} lists {choice} twice"
                ));
            }
            domain.push(choice);
        }
        if domain.is_empty() {
            return Err(format!("{what}: the domain of {key:?} is empty"));
        }
        domains.insert(key.clone(), domain);
    }
    Ok(domains)
}

/// Every reference choice must lie in its domain: the shim checks this as
/// it writes, so a value outside means the files do not belong together.
fn check_domains(choices: &Choices, domains: &Domains, path: &Path) -> Result<(), String> {
    for (key, value) in choices {
        let ChoiceValue::One(choice) = value else {
            continue;
        };
        let domain = domains.get(key).ok_or_else(|| {
            format!(
                "{}: choice {key:?} has no domain in domains.json",
                path.display()
            )
        })?;
        if !domain.contains(choice) {
            return Err(format!(
                "{}: choice {key:?} = {choice} is outside its domain",
                path.display()
            ));
        }
    }
    Ok(())
}

/// The differences between a case's reference and candidate choices, one
/// line each.
fn compare_choices(reference: &Choices, candidate: &Choices) -> Vec<(String, String)> {
    let keys: BTreeSet<&String> = reference.keys().chain(candidate.keys()).collect();
    keys.into_iter()
        .filter_map(|key| {
            match (reference.get(key), candidate.get(key)) {
                (Some(a), Some(b)) if a == b => None,
                (Some(a), Some(b)) => Some(format!("reference {a}, candidate {b}")),
                (Some(a), None) => Some(format!("missing from the candidate (reference {a})")),
                (None, Some(b)) => Some(format!("not in the reference (candidate {b})")),
                (None, None) => None,
            }
            .map(|line| (key.clone(), line))
        })
        .collect()
}

// ------------------------------------------------------------------- listings

/// What a run directory holds: the case IDs that have an SVG, those that
/// have a choices file, and every other entry except `domains.json`.
struct Listing {
    svg: BTreeSet<String>,
    choices: BTreeSet<String>,
    other: Vec<String>,
}

fn list(directory: &Path) -> Result<Listing, String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("read the directory {}: {error}", directory.display()))?;
    let mut listing = Listing {
        svg: BTreeSet::new(),
        choices: BTreeSet::new(),
        other: Vec::new(),
    };
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("read the directory {}: {error}", directory.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_file = entry.file_type().is_ok_and(|kind| kind.is_file());
        if name == "domains.json" && is_file {
            continue;
        }
        if let (Some(id), true) = (name.strip_suffix(".choices.json"), is_file) {
            listing.choices.insert(id.to_owned());
        } else if let (Some(id), true) = (name.strip_suffix(".svg"), is_file) {
            listing.svg.insert(id.to_owned());
        } else {
            listing.other.push(name);
        }
    }
    listing.other.sort();
    Ok(listing)
}

impl Listing {
    /// Case IDs with any file here that `expected` does not hold.
    fn unexpected(&self, expected: &BTreeSet<&str>) -> BTreeSet<String> {
        self.svg
            .iter()
            .chain(&self.choices)
            .filter(|id| !expected.contains(id.as_str()))
            .cloned()
            .collect()
    }
}

// ---------------------------------------------------------------- comparison

#[derive(Clone, Copy, PartialEq, Eq)]
enum Set {
    Base,
    Probe,
}

impl Set {
    fn name(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Probe => "probe",
        }
    }
}

/// How two images of one case compare.
#[derive(Clone, Copy)]
struct Pixels {
    /// Pixels with a channel more than the tolerance apart.
    failing: usize,
    /// Pixels with any channel apart.
    differing: usize,
    max_difference: u8,
    failed: bool,
}

struct Case {
    id: String,
    set: Set,
    /// Missing files, an SVG that did not draw: what stopped a comparison.
    problems: Vec<String>,
    /// Keys whose values differ, with what each side holds.
    mismatches: Vec<(String, String)>,
    pixels: Option<Pixels>,
}

impl Case {
    fn failed(&self) -> bool {
        !self.problems.is_empty()
            || !self.mismatches.is_empty()
            || self.pixels.is_some_and(|pixels| pixels.failed)
    }
}

fn compare_pixels(reference: &[u8], candidate: &[u8], size_px: u32) -> Pixels {
    let mut pixels = Pixels {
        failing: 0,
        differing: 0,
        max_difference: 0,
        failed: false,
    };
    let (reference, _) = reference.as_chunks::<4>();
    let (candidate, _) = candidate.as_chunks::<4>();
    for (a, b) in reference.iter().zip(candidate) {
        let difference = a
            .iter()
            .zip(b)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        pixels.max_difference = pixels.max_difference.max(difference);
        pixels.differing += usize::from(difference > 0);
        pixels.failing += usize::from(difference > CHANNEL_TOLERANCE);
    }
    let total = size_px as usize * size_px as usize;
    pixels.failed = pixels.failing * 100 > total * FAILING_PIXEL_PERCENT;
    pixels
}

/// Draws both SVGs of a case and compares them.
fn draw_pair(reference: &Path, candidate: &Path, size_px: u32) -> Result<Pixels, String> {
    let draw = |path: &Path, side: &str| {
        let svg = fs::read(path).map_err(|error| format!("{side} SVG unreadable: {error}"))?;
        rasterize_svg(&svg, size_px, Alpha::Premultiplied)
            .map_err(|error| format!("{side} SVG did not draw: {error}"))
    };
    let a = draw(reference, "reference")?;
    let b = draw(candidate, "candidate")?;
    Ok(compare_pixels(&a.pixels, &b.pixels, size_px))
}

/// Draws every case that has both SVGs, on all available cores.
fn draw_all(cases: &mut [Case], options: &Options, listings: [&Listing; 2]) {
    let jobs: Vec<usize> = (0..cases.len())
        .filter(|&i| {
            listings
                .iter()
                .all(|listing| listing.svg.contains(&cases[i].id))
        })
        .collect();
    let ids: Vec<String> = cases.iter().map(|case| case.id.clone()).collect();
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, Result<Pixels, String>)>> = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(1, |count| count.get());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let job = next.fetch_add(1, Ordering::Relaxed);
                let Some(&i) = jobs.get(job) else {
                    break;
                };
                let file = format!("{}.svg", ids[i]);
                let result = draw_pair(
                    &options.reference.join(&file),
                    &options.candidate.join(&file),
                    options.size_px,
                );
                results
                    .lock()
                    .expect("no drawing thread panics while holding the lock")
                    .push((i, result));
            });
        }
    });
    let results = results
        .into_inner()
        .expect("every drawing thread has finished");
    for (i, result) in results {
        match result {
            Ok(pixels) => cases[i].pixels = Some(pixels),
            Err(problem) => cases[i].problems.push(problem),
        }
    }
}

// ------------------------------------------------------------------- the run

struct Outcome {
    failed: bool,
}

/// How many base, probe and corpus cases reach each branch.
type Coverage = BTreeMap<(String, Choice), [usize; 3]>;

fn count(coverage: &mut Coverage, choices: &Choices, column: usize) {
    for (key, value) in choices {
        if let ChoiceValue::One(choice) = value {
            coverage
                .entry((key.clone(), choice.clone()))
                .or_insert([0; 3])[column] += 1;
        }
    }
}

fn run(options: &Options) -> Result<Outcome, String> {
    // Every manifest is checked before anything else is read or written.
    let base = read_manifest(&options.manifest, "base")?;
    let probes = options
        .probes
        .as_deref()
        .map(|path| read_manifest(path, "probe"))
        .transpose()?;
    let corpus = options
        .corpus
        .as_ref()
        .map(|(path, _)| read_manifest(path, "corpus"))
        .transpose()?;
    for other in probes.iter().chain(&corpus) {
        if other.avatar_type != base.avatar_type {
            return Err(format!(
                "{} is for {}, but the base manifest is for {}",
                other.path.display(),
                other.avatar_type,
                base.avatar_type
            ));
        }
    }
    let base_ids: BTreeSet<&str> = base.cases.iter().map(String::as_str).collect();
    if let Some(probes) = &probes {
        if let Some(id) = probes
            .cases
            .iter()
            .find(|id| base_ids.contains(id.as_str()))
        {
            return Err(format!(
                "probe case ID {id} is also a base case ID; probes never replace a base case"
            ));
        }
    }

    let domains = read_domains(&options.reference.join("domains.json"))?;
    let reference = list(&options.reference)?;
    let candidate = list(&options.candidate)?;

    let mut cases: Vec<Case> = base
        .cases
        .iter()
        .map(|id| (id, Set::Base))
        .chain(
            probes
                .iter()
                .flat_map(|m| m.cases.iter().map(|id| (id, Set::Probe))),
        )
        .map(|(id, set)| Case {
            id: id.clone(),
            set,
            problems: Vec::new(),
            mismatches: Vec::new(),
            pixels: None,
        })
        .collect();
    let expected: BTreeSet<&str> = base
        .cases
        .iter()
        .chain(probes.iter().flat_map(|m| &m.cases))
        .map(String::as_str)
        .collect();

    let mut coverage = Coverage::new();
    for case in &mut cases {
        let sides = [
            ("reference", &options.reference, &reference),
            ("candidate", &options.candidate, &candidate),
        ];
        let mut read = [None, None];
        for (slot, (side, directory, listing)) in read.iter_mut().zip(sides) {
            if !listing.svg.contains(&case.id) {
                case.problems.push(format!("{side} SVG missing"));
            }
            if !listing.choices.contains(&case.id) {
                case.problems.push(format!("{side} choices missing"));
                continue;
            }
            let path = directory.join(format!("{}.choices.json", case.id));
            let choices = read_choices(&path)?;
            if side == "reference" {
                check_domains(&choices, &domains, &path)?;
            }
            *slot = Some(choices);
        }
        if let Some(choices) = &read[0] {
            count(
                &mut coverage,
                choices,
                if case.set == Set::Base { 0 } else { 1 },
            );
        }
        if let [Some(a), Some(b)] = &read {
            case.mismatches = compare_choices(a, b);
        }
    }

    if let (Some(manifest), Some((_, directory))) = (&corpus, &options.corpus) {
        count_corpus(manifest, directory, &domains, &mut coverage)?;
    }

    draw_all(&mut cases, options, [&reference, &candidate]);

    let mut unexpected = Vec::new();
    for (side, listing) in [("reference", &reference), ("candidate", &candidate)] {
        for id in listing.unexpected(&expected) {
            unexpected.push(format!("{side}: case {id}, which no manifest lists"));
        }
        for name in &listing.other {
            unexpected.push(format!("{side}: {name}, which is not a case file"));
        }
    }

    let report = Report {
        options,
        base: &base,
        probes: probes.as_ref(),
        corpus: corpus.as_ref(),
        cases: &cases,
        unexpected: &unexpected,
        domains: &domains,
        coverage: &coverage,
    };
    fs::write(&options.report, report.markdown())
        .map_err(|error| format!("write {}: {error}", options.report.display()))?;
    print!("{}", report.summary());
    Ok(Outcome {
        failed: cases.iter().any(Case::failed) || !unexpected.is_empty(),
    })
}

/// Counts the branches the committed corpus reaches, from a shim run on the
/// corpus manifest. That directory must hold exactly the corpus cases.
fn count_corpus(
    manifest: &Manifest,
    directory: &Path,
    domains: &Domains,
    coverage: &mut Coverage,
) -> Result<(), String> {
    let listing = list(directory)?;
    let what = directory.display();
    if let Some(name) = listing.other.first() {
        return Err(format!("{what}: {name} is not a case file"));
    }
    let expected: BTreeSet<&str> = manifest.cases.iter().map(String::as_str).collect();
    if let Some(id) = listing.unexpected(&expected).first() {
        return Err(format!(
            "{what}: case {id} is not in the corpus manifest {}",
            manifest.path.display()
        ));
    }
    let corpus_domains = directory.join("domains.json");
    if corpus_domains.exists() && read_domains(&corpus_domains)? != *domains {
        return Err(format!(
            "{what}: its domains.json differs from the reference run's"
        ));
    }
    for id in &manifest.cases {
        if !listing.choices.contains(id) {
            return Err(format!("{what}: corpus case {id} has no choices file"));
        }
        let path = directory.join(format!("{id}.choices.json"));
        let choices = read_choices(&path)?;
        check_domains(&choices, domains, &path)?;
        count(coverage, &choices, 2);
    }
    Ok(())
}

// -------------------------------------------------------------------- report

struct Report<'a> {
    options: &'a Options,
    base: &'a Manifest,
    probes: Option<&'a Manifest>,
    corpus: Option<&'a Manifest>,
    cases: &'a [Case],
    unexpected: &'a [String],
    domains: &'a Domains,
    coverage: &'a Coverage,
}

/// Counts for one set of cases.
struct Tally {
    cases: usize,
    failed: usize,
    failing_pixels: usize,
    image_failures: usize,
    mismatches: usize,
    mismatched_cases: usize,
    problems: usize,
}

impl Report<'_> {
    fn tally(&self, set: Set) -> Tally {
        let cases: Vec<&Case> = self.cases.iter().filter(|case| case.set == set).collect();
        Tally {
            cases: cases.len(),
            failed: cases.iter().filter(|case| case.failed()).count(),
            failing_pixels: cases
                .iter()
                .filter_map(|case| case.pixels)
                .map(|pixels| pixels.failing)
                .sum(),
            image_failures: cases
                .iter()
                .filter(|case| case.pixels.is_some_and(|pixels| pixels.failed))
                .count(),
            mismatches: cases.iter().map(|case| case.mismatches.len()).sum(),
            mismatched_cases: cases
                .iter()
                .filter(|case| !case.mismatches.is_empty())
                .count(),
            problems: cases.iter().map(|case| case.problems.len()).sum(),
        }
    }

    fn tally_line(&self, set: Set) -> String {
        let t = self.tally(set);
        format!(
            "{}: {} cases, {} passed, {} failed; {} failing pixels in all, {} images over the limit; \
             {} choice mismatches in {} cases; {} missing files or undrawable SVGs",
            set.name(),
            t.cases,
            t.cases - t.failed,
            t.failed,
            t.failing_pixels,
            t.image_failures,
            t.mismatches,
            t.mismatched_cases,
            t.problems
        )
    }

    /// Each branch with its counts and whether the base and probe cases, or
    /// the corpus, miss it.
    fn branches(&self) -> Vec<(String, [usize; 3], bool, bool)> {
        let mut rows = Vec::new();
        for (key, domain) in self.domains {
            for value in domain {
                let counts = self
                    .coverage
                    .get(&(key.clone(), value.clone()))
                    .copied()
                    .unwrap_or([0; 3]);
                let missed = counts[0] == 0 && counts[1] == 0;
                let unpinned = self.corpus.is_some() && counts[2] == 0;
                rows.push((format!("{key} = {value}"), counts, missed, unpinned));
            }
        }
        rows
    }

    fn manifest_line(manifest: Option<&Manifest>) -> String {
        manifest.map_or_else(
            || "none".to_owned(),
            |m| {
                format!(
                    "{}, sha256 {}, {} cases",
                    m.path.display(),
                    m.sha256,
                    m.cases.len()
                )
            },
        )
    }

    fn failure_details(case: &Case, size_px: u32) -> String {
        let mut parts = case.problems.clone();
        if let Some(pixels) = case.pixels.filter(|pixels| pixels.failed) {
            let total = size_px as usize * size_px as usize;
            parts.push(format!(
                "{} of {total} pixels fail ({:.2}%), {} differ, largest channel difference {}",
                pixels.failing,
                pixels.failing as f64 * 100.0 / total as f64,
                pixels.differing,
                pixels.max_difference
            ));
        }
        for (key, line) in &case.mismatches {
            parts.push(format!("choice {key}: {line}"));
        }
        parts.join("; ")
    }

    fn markdown(&self) -> String {
        let o = self.options;
        let total = o.size_px as usize * o.size_px as usize;
        let mut r = String::new();
        let _ = writeln!(r, "# Fidelity report: {}\n", self.base.avatar_type);
        let _ = writeln!(
            r,
            "- Drawn with avatar_raster::rasterize_svg at {} px, premultiplied, on a transparent background.",
            o.size_px
        );
        let _ = writeln!(
            r,
            "- A pixel fails when any RGBA channel differs by more than {CHANNEL_TOLERANCE}/255. \
             An image fails when more than {FAILING_PIXEL_PERCENT}% of its {total} pixels fail."
        );
        let _ = writeln!(r, "- Reference: {}", o.reference.display());
        let _ = writeln!(r, "- Candidate: {}", o.candidate.display());
        let _ = writeln!(
            r,
            "- Base manifest: {}",
            Self::manifest_line(Some(self.base))
        );
        if self.base.cases.len() != BASE_CASES {
            let _ = writeln!(
                r,
                "- **The base manifest holds {} cases, not the {BASE_CASES} of the base profile.**",
                self.base.cases.len()
            );
        }
        let _ = writeln!(r, "- Probe manifest: {}", Self::manifest_line(self.probes));
        let _ = writeln!(r, "- Corpus manifest: {}", Self::manifest_line(self.corpus));
        if let Some((_, directory)) = &o.corpus {
            let _ = writeln!(r, "- Corpus choices: {}", directory.display());
        }

        let _ = writeln!(r, "\n## Result\n");
        let _ = writeln!(r, "- {}", self.tally_line(Set::Base));
        if self.probes.is_some() {
            let _ = writeln!(r, "- {}", self.tally_line(Set::Probe));
        }
        let _ = writeln!(r, "- Unexpected files: {}", self.unexpected.len());

        let failing: Vec<&Case> = self.cases.iter().filter(|case| case.failed()).collect();
        let _ = writeln!(r, "\n## Failing cases\n");
        if failing.is_empty() {
            let _ = writeln!(r, "None.");
        } else {
            let _ = writeln!(r, "| case | set | what failed |\n|---|---|---|");
            for case in failing {
                let _ = writeln!(
                    r,
                    "| {} | {} | {} |",
                    case.id,
                    case.set.name(),
                    Self::failure_details(case, o.size_px).replace('|', "\\|")
                );
            }
        }
        let _ = writeln!(r, "\n## Unexpected files\n");
        if self.unexpected.is_empty() {
            let _ = writeln!(r, "None.");
        }
        for line in self.unexpected {
            let _ = writeln!(r, "- {line}");
        }

        let _ = writeln!(r, "\n## Discrete comparison\n");
        let _ = writeln!(
            r,
            "| key | base cases that differ | probe cases that differ |\n|---|---|---|"
        );
        let keys: BTreeSet<&str> = self
            .domains
            .keys()
            .map(String::as_str)
            .chain([BODY_RGB])
            .chain(
                self.cases
                    .iter()
                    .flat_map(|case| case.mismatches.iter().map(|(key, _)| key.as_str())),
            )
            .collect();
        for key in keys {
            let differ = |set: Set| {
                self.cases
                    .iter()
                    .filter(|case| case.set == set && case.mismatches.iter().any(|(k, _)| k == key))
                    .count()
            };
            let _ = writeln!(
                r,
                "| {key} | {} | {} |",
                differ(Set::Base),
                differ(Set::Probe)
            );
        }

        let rows = self.branches();
        let _ = writeln!(r, "\n## Branch coverage\n");
        let _ = writeln!(
            r,
            "Each branch is a key and a value of domains.json. The counts are reference cases that \
             reach it. A branch neither the base nor the probe cases reach is missed; one the \
             committed corpus does not reach is unpinned.\n"
        );
        let _ = writeln!(r, "{}\n", self.coverage_line(&rows));
        let _ = writeln!(
            r,
            "| branch | base | probes | corpus | |\n|---|---|---|---|---|"
        );
        for (branch, counts, missed, unpinned) in &rows {
            let corpus = if self.corpus.is_some() {
                counts[2].to_string()
            } else {
                "-".to_owned()
            };
            let mark = match (missed, unpinned) {
                (true, true) => "missed, unpinned",
                (true, false) => "missed",
                (false, true) => "unpinned",
                (false, false) => "",
            };
            let _ = writeln!(
                r,
                "| {} | {} | {} | {corpus} | {mark} |",
                branch.replace('|', "\\|"),
                counts[0],
                counts[1]
            );
        }
        r
    }

    fn coverage_line(&self, rows: &[(String, [usize; 3], bool, bool)]) -> String {
        let by_base = rows.iter().filter(|row| row.1[0] > 0).count();
        let by_probes_only = rows
            .iter()
            .filter(|row| row.1[0] == 0 && row.1[1] > 0)
            .count();
        let missed = rows.iter().filter(|row| row.2).count();
        let mut line = format!(
            "{} branches: {by_base} reached by the base cases, {by_probes_only} only by probes, {missed} missed",
            rows.len()
        );
        if self.corpus.is_some() {
            let unpinned = rows.iter().filter(|row| row.3).count();
            let _ = write!(line, "; {unpinned} unpinned by the corpus");
        }
        line
    }

    fn summary(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{}", self.base.avatar_type);
        let _ = writeln!(s, "  {}", self.tally_line(Set::Base));
        if self.probes.is_some() {
            let _ = writeln!(s, "  {}", self.tally_line(Set::Probe));
        }
        let rows = self.branches();
        let _ = writeln!(s, "  {}", self.coverage_line(&rows));
        for (branch, _, missed, _) in &rows {
            if *missed {
                let _ = writeln!(s, "  missed branch: {branch}");
            }
        }
        let failing: Vec<&Case> = self.cases.iter().filter(|case| case.failed()).collect();
        const SHOWN: usize = 20;
        for case in failing.iter().take(SHOWN) {
            let _ = writeln!(
                s,
                "  FAIL {} ({}): {}",
                case.id,
                case.set.name(),
                Self::failure_details(case, self.options.size_px)
            );
        }
        if failing.len() > SHOWN {
            let _ = writeln!(s, "  ... and {} more failing cases", failing.len() - SHOWN);
        }
        for line in self.unexpected.iter().take(SHOWN) {
            let _ = writeln!(s, "  UNEXPECTED {line}");
        }
        if self.unexpected.len() > SHOWN {
            let _ = writeln!(
                s,
                "  ... and {} more unexpected files",
                self.unexpected.len() - SHOWN
            );
        }
        let _ = writeln!(s, "  report: {}", self.options.report.display());
        s
    }
}

// ------------------------------------------------------------------- hashing

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// SHA-256 (FIPS 180-4), written out so the example needs no dependency.
fn sha256(message: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    // Padding: a 1 bit, zeros up to 56 bytes into the last block, then the
    // message length in bits as a big-endian u64.
    let mut padded = message.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&((message.len() as u64) * 8).to_be_bytes());

    for block in padded.as_chunks::<64>().0 {
        let mut w = [0_u32; 64];
        for (word, bytes) in w.iter_mut().zip(block.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*bytes);
        }
        for t in 16..64 {
            let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
            let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
            w[t] = w[t - 16]
                .wrapping_add(s0)
                .wrapping_add(w[t - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for (constant, word) in K.into_iter().zip(w) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(constant)
                .wrapping_add(word);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (word, add) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *word = word.wrapping_add(add);
        }
    }

    let mut digest = [0_u8; 32];
    for (bytes, word) in digest.as_chunks_mut::<4>().0.iter_mut().zip(state) {
        *bytes = word.to_be_bytes();
    }
    digest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(size_px: u32, changed: usize, by: u8) -> Vec<u8> {
        let mut pixels = vec![0; size_px as usize * size_px as usize * 4];
        for pixel in pixels.as_chunks_mut::<4>().0.iter_mut().take(changed) {
            pixel[2] = by;
        }
        pixels
    }

    #[test]
    fn a_pixel_fails_above_8_and_an_image_above_one_percent() {
        let blank = image(256, 0, 0);
        // 1% of 65,536 pixels is 655.36: 655 failing pixels pass, 656 fail.
        let at_limit = compare_pixels(&blank, &image(256, 655, 9), 256);
        assert_eq!((at_limit.failing, at_limit.failed), (655, false));
        let over = compare_pixels(&blank, &image(256, 656, 9), 256);
        assert_eq!(
            (over.failing, over.failed, over.max_difference),
            (656, true, 9)
        );
        // A difference of 8 in every pixel differs but does not fail.
        let within = compare_pixels(&blank, &image(256, 65_536, 8), 256);
        assert_eq!(
            (within.differing, within.failing, within.failed),
            (65_536, 0, false)
        );
        // The limit scales with the size: one pixel in 100 passes, two fail.
        let small = image(10, 0, 0);
        assert!(!compare_pixels(&small, &image(10, 1, 255), 10).failed);
        assert!(compare_pixels(&small, &image(10, 2, 255), 10).failed);
    }

    #[test]
    fn choices_refuse_every_value_that_is_not_a_choice() {
        let ok = parse_choices(
            r#"{"a":-3,"b":"x\u00e9","c":true,"body_rgb":[0,128,255]}"#,
            "t",
        )
        .expect("a valid choices file");
        assert_eq!(ok["a"], ChoiceValue::One(Choice::Integer(-3)));
        assert_eq!(ok["b"], ChoiceValue::One(Choice::Text("x\u{e9}".into())));
        assert_eq!(ok[BODY_RGB], ChoiceValue::Rgb([0, 128, 255]));
        for (text, expected) in [
            (r#"{"a":0.5}"#, "non-integer number 0.5"),
            (r#"{"a":1e3}"#, "non-integer number 1e3"),
            (r#"{"a":[1]}"#, "an array"),
            (r#"{"a":null}"#, "null"),
            (r#"{"a":{}}"#, "an object"),
            (r#"{"a":1,"a":2}"#, "repeated key"),
            (
                r#"{"body_rgb":[1,2,256]}"#,
                "body_rgb is not three integers",
            ),
            (r#"{"body_rgb":[1,2]}"#, "body_rgb is not three integers"),
            (
                r#"{"body_rgb":[1,2,3.0]}"#,
                "body_rgb is not three integers",
            ),
            (r#"[1]"#, "not a JSON object"),
        ] {
            let error = parse_choices(text, "t").expect_err(text);
            assert!(error.contains(expected), "{text}: {error}");
        }
    }

    #[test]
    fn domains_refuse_body_rgb_repeats_and_empty_lists() {
        let domains = parse_domains(r#"{"k":[1,"1",true]}"#, "d").expect("valid domains");
        assert_eq!(
            domains["k"],
            [
                Choice::Integer(1),
                Choice::Text("1".into()),
                Choice::Bool(true)
            ]
        );
        for (text, expected) in [
            (r#"{"body_rgb":[1]}"#, "body_rgb has no domain"),
            (r#"{"k":[1,1]}"#, "lists 1 twice"),
            (r#"{"k":[]}"#, "is empty"),
            (r#"{"k":[0.5]}"#, "non-integer"),
        ] {
            let error = parse_domains(text, "d").expect_err(text);
            assert!(error.contains(expected), "{text}: {error}");
        }
    }

    #[test]
    fn choice_comparison_names_every_differing_key() {
        let reference =
            parse_choices(r#"{"a":1,"b":"x","c":true,"body_rgb":[1,2,3]}"#, "r").expect("valid");
        let candidate =
            parse_choices(r#"{"a":"1","c":true,"d":2,"body_rgb":[1,2,4]}"#, "c").expect("valid");
        let lines = compare_choices(&reference, &candidate);
        let keys: Vec<&str> = lines.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(keys, ["a", "b", "body_rgb", "d"]);
        assert!(compare_choices(&reference, &reference).is_empty());
    }

    fn manifest(kind: &str, cases: &[(&str, &str, &str)]) -> String {
        let hex = "0".repeat(GENOME_HEX_DIGITS);
        let cases: Vec<String> = cases
            .iter()
            .map(|(id, genome, mood)| {
                format!(r#"{{"id":"{id}","genome":"{genome}","mood":"{mood}"}}"#)
            })
            .collect();
        format!(
            r#"{{"format":"{MANIFEST_FORMAT}","type":"creature.test","kind":"{kind}","genomes":[{{"name":"g","source":"s","hex":"{hex}"}}],"cases":[{}]}}"#,
            cases.join(",")
        )
    }

    #[test]
    fn manifests_refuse_repeated_and_malformed_case_ids() {
        let path = Path::new("m.json");
        let good = manifest(
            "base",
            &[("g__neutral", "g", "neutral"), ("g__sad", "g", "sad")],
        );
        let parsed = parse_manifest(good.as_bytes(), path, "base").expect("a valid manifest");
        assert_eq!(parsed.cases, ["g__neutral", "g__sad"]);
        assert_eq!(parsed.sha256, hex(&sha256(good.as_bytes())));
        for (text, kind, expected) in [
            (
                manifest(
                    "base",
                    &[
                        ("g__neutral", "g", "neutral"),
                        ("g__neutral", "g", "neutral"),
                    ],
                ),
                "base",
                "case ID g__neutral is repeated",
            ),
            (good.clone(), "probe", "is a \"base\" manifest"),
            (
                manifest("base", &[("x__neutral", "g", "neutral")]),
                "base",
                "is not g__neutral",
            ),
            (
                manifest("base", &[("h__neutral", "h", "neutral")]),
                "base",
                "does not list",
            ),
            (
                manifest("base", &[("g__angry", "g", "angry")]),
                "base",
                "unknown mood",
            ),
            (manifest("base", &[]), "base", "lists no cases"),
        ] {
            let error = parse_manifest(text.as_bytes(), path, kind).expect_err(&text[..60]);
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn sha256_matches_the_standard_test_vector() {
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
