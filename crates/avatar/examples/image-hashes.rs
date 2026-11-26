//! Print the SHA-256 of every golden image as this machine draws it.
//!
//! ```text
//! cargo run -p cortexkit-avatar --example image-hashes [-- --dump DIR]
//! ```
//!
//! Reads `vectors/image/manifest.json`, draws each entry with
//! [`avatar::render_image`] from its committed genome, and prints one line per
//! entry in manifest order: the SHA-256 of the image's raw RGBA pixels
//! (`Image::pixels`), two spaces and the entry's file name. The hash covers
//! the pixels only, so it does not depend on how a PNG encoder compresses, and
//! it equals the SHA-256 of the golden PNG's decoded pixels when this machine
//! draws the same image.
//!
//! With `--dump DIR`, each image is also written to `DIR/<file>` in the golden
//! format: an 8-bit RGBA PNG holding the pixels verbatim, so a premultiplied
//! image stays premultiplied. The deflate stream is stored uncompressed, so
//! the PNG's pixels match the golden's but its bytes do not.
//!
//! `scripts/wild-cross-platform.sh image` runs this natively and in Linux
//! containers and compares the output with the goldens. It reads the
//! repository this example was compiled from and uses nothing but `avatar`
//! and the standard library, so the same command measures any platform the
//! crate builds on, Windows included.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process,
};

use avatar::{render_image, Alpha, GroundShadow, ImageOptions, Mood};

const USAGE: &str = "usage: image-hashes [--dump DIR]";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dump = match args.as_slice() {
        [] => None,
        [flag, directory] if flag == "--dump" => Some(PathBuf::from(directory)),
        _ => fail(USAGE),
    };
    if let Some(directory) = &dump {
        fs::create_dir_all(directory)
            .unwrap_or_else(|error| fail(&format!("create {}: {error}", directory.display())));
    }

    let vectors = vectors_directory();
    let manifest_path = vectors.join("image").join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| fail(&format!("read {}: {error}", manifest_path.display())));
    let cases = parse_manifest(&manifest)
        .unwrap_or_else(|error| fail(&format!("{}: {error}", manifest_path.display())));

    let stdout = io::stdout();
    let mut out = stdout.lock();
    for case in &cases {
        let genome = read_genome(&vectors.join(&case.genome_hex));
        let image = render_image(
            &genome,
            &case.avatar_type,
            case.version,
            case.mood,
            case.size_px,
            &case.options,
        )
        .unwrap_or_else(|error| fail(&format!("{}: {error}", case.file)));
        writeln!(out, "{}  {}", hex(&sha256(&image.pixels)), case.file)
            .unwrap_or_else(|error| fail(&format!("write hashes: {error}")));
        if let Some(directory) = &dump {
            let path = directory.join(&case.file);
            fs::write(&path, png(image.size_px, &image.pixels))
                .unwrap_or_else(|error| fail(&format!("write {}: {error}", path.display())));
        }
    }
    out.flush()
        .unwrap_or_else(|error| fail(&format!("write hashes: {error}")));
    eprintln!("hashed {} images", cases.len());
}

fn fail(message: &str) -> ! {
    eprintln!("image-hashes: {message}");
    process::exit(1)
}

/// The repository's `vectors/`, found from where this example was compiled.
fn vectors_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/avatar sits two levels below the repository root")
        .join("vectors")
}

/// A genome file holds the genome bytes as hexadecimal on one line.
fn read_genome(path: &Path) -> Vec<u8> {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| fail(&format!("read {}: {error}", path.display())));
    let digits = text.trim().as_bytes();
    let (pairs, rest) = digits.as_chunks::<2>();
    if pairs.is_empty() || !rest.is_empty() || !digits.iter().all(u8::is_ascii_hexdigit) {
        fail(&format!(
            "{}: not one line of hex byte pairs",
            path.display()
        ));
    }
    let nibble = |digit: u8| (digit as char).to_digit(16).expect("checked above") as u8;
    pairs
        .iter()
        .map(|&[high, low]| nibble(high) << 4 | nibble(low))
        .collect()
}

/// One entry of the golden image manifest: what to draw and the file name.
struct Case {
    avatar_type: String,
    version: u32,
    mood: Mood,
    size_px: u32,
    options: ImageOptions,
    /// The committed genome, relative to `vectors/`.
    genome_hex: String,
    file: String,
}

fn parse_manifest(text: &str) -> Result<Vec<Case>, String> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        at: 0,
    };
    let root = parser.value()?;
    parser.skip_whitespace();
    if parser.at != parser.bytes.len() {
        return Err(format!("unexpected text at byte {}", parser.at));
    }
    let Json::Array(entries) = field(&root, "entries")? else {
        return Err("entries is not an array".into());
    };
    if entries.is_empty() {
        return Err("the manifest lists no images".into());
    }
    entries.iter().map(case).collect()
}

fn case(entry: &Json) -> Result<Case, String> {
    let file = string(entry, "file")?;
    // The name becomes a path under the dump directory, so it must be a bare
    // file name.
    if file.is_empty() || file.contains(['/', '\\']) || file.starts_with('.') {
        return Err(format!("file {file:?} is not a bare file name"));
    }
    let mood = match string(entry, "mood")? {
        "neutral" => Mood::Neutral,
        "working" => Mood::Working,
        "happy" => Mood::Happy,
        "waiting" => Mood::Waiting,
        "blocked" => Mood::Blocked,
        "sad" => Mood::Sad,
        "sleepy" => Mood::Sleepy,
        other => return Err(format!("{file}: unknown mood {other:?}")),
    };
    let ground_shadow = match string(entry, "ground_shadow")? {
        "keep" => GroundShadow::Keep,
        "drop" => GroundShadow::Drop,
        other => return Err(format!("{file}: unknown ground_shadow {other:?}")),
    };
    let alpha = match string(entry, "alpha")? {
        "premultiplied" => Alpha::Premultiplied,
        "straight" => Alpha::Straight,
        other => return Err(format!("{file}: unknown alpha {other:?}")),
    };
    Ok(Case {
        avatar_type: string(entry, "type")?.to_owned(),
        version: number(entry, "version")?,
        mood,
        size_px: number(entry, "size_px")?,
        options: ImageOptions {
            ground_shadow,
            alpha,
        },
        genome_hex: string(entry, "genome_hex")?.to_owned(),
        file: file.to_owned(),
    })
}

/// The JSON the manifest uses: objects, arrays, strings and non-negative
/// integers. Anything else is refused, so a format change fails loudly.
enum Json {
    Object(Vec<(String, Json)>),
    Array(Vec<Json>),
    String(String),
    Number(u64),
}

fn field<'a>(value: &'a Json, key: &str) -> Result<&'a Json, String> {
    let Json::Object(members) = value else {
        return Err(format!("expected an object holding {key:?}"));
    };
    let mut found = members.iter().filter(|(name, _)| name == key);
    match (found.next(), found.next()) {
        (Some((_, value)), None) => Ok(value),
        (None, _) => Err(format!("missing key {key:?}")),
        (Some(_), Some(_)) => Err(format!("repeated key {key:?}")),
    }
}

fn string<'a>(value: &'a Json, key: &str) -> Result<&'a str, String> {
    match field(value, key)? {
        Json::String(text) => Ok(text),
        _ => Err(format!("{key:?} is not a string")),
    }
}

fn number(value: &Json, key: &str) -> Result<u32, String> {
    match field(value, key)? {
        Json::Number(number) => {
            u32::try_from(*number).map_err(|_| format!("{key:?} {number} is out of range"))
        }
        _ => Err(format!("{key:?} is not a number")),
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

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        self.skip_whitespace();
        if self.bytes.get(self.at) == Some(&byte) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("expected {:?} at byte {}", byte as char, self.at))
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

    fn value(&mut self) -> Result<Json, String> {
        self.skip_whitespace();
        match self.bytes.get(self.at) {
            Some(b'{') => {
                self.at += 1;
                let mut members = Vec::new();
                if !self.eat(b'}') {
                    loop {
                        self.skip_whitespace();
                        let key = self.string()?;
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
            Some(b'0'..=b'9') => {
                let start = self.at;
                while let Some(b'0'..=b'9') = self.bytes.get(self.at) {
                    self.at += 1;
                }
                let digits = std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or("");
                digits
                    .parse()
                    .map(Json::Number)
                    .map_err(|_| format!("number at byte {start} is out of range"))
            }
            _ => Err(format!("unsupported JSON value at byte {}", self.at)),
        }
    }

    /// A string without escapes: the manifest's names and paths need none.
    fn string(&mut self) -> Result<String, String> {
        let start = self.at;
        if self.bytes.get(self.at) != Some(&b'"') {
            return Err(format!("expected a string at byte {start}"));
        }
        let length = self.bytes[start + 1..]
            .iter()
            .position(|&byte| byte == b'"' || byte == b'\\' || byte < 0x20)
            .ok_or_else(|| format!("unterminated string at byte {start}"))?;
        let end = start + 1 + length;
        if self.bytes[end] != b'"' {
            return Err(format!(
                "unsupported character in the string at byte {start}"
            ));
        }
        self.at = end + 1;
        String::from_utf8(self.bytes[start + 1..end].to_vec())
            .map_err(|_| format!("the string at byte {start} is not UTF-8"))
    }
}

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

/// An 8-bit RGBA PNG of a square image, holding `pixels` verbatim. Every
/// scanline uses filter type 0 (none), and the zlib stream uses stored
/// deflate blocks, so no compressor is needed.
fn png(size_px: u32, pixels: &[u8]) -> Vec<u8> {
    let row = size_px as usize * 4;
    let mut scanlines = Vec::with_capacity(pixels.len() + size_px as usize);
    for line in pixels.chunks_exact(row) {
        scanlines.push(0);
        scanlines.extend_from_slice(line);
    }

    // zlib header: deflate with a 32 KiB window; 0x7801 is a multiple of 31,
    // as the header check requires.
    let mut zlib = vec![0x78, 0x01];
    let mut blocks = scanlines.chunks(usize::from(u16::MAX)).peekable();
    while let Some(block) = blocks.next() {
        let last = blocks.peek().is_none();
        let length = block.len() as u16;
        zlib.push(u8::from(last));
        zlib.extend_from_slice(&length.to_le_bytes());
        zlib.extend_from_slice(&(!length).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&scanlines).to_be_bytes());

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&size_px.to_be_bytes());
    header.extend_from_slice(&size_px.to_be_bytes());
    // Bit depth 8, colour type 6 (RGBA), deflate, adaptive filtering, no
    // interlace.
    header.extend_from_slice(&[8, 6, 0, 0, 0]);

    let mut file = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, data) in [
        (b"IHDR", header.as_slice()),
        (b"IDAT", zlib.as_slice()),
        (b"IEND", &[][..]),
    ] {
        let length = u32::try_from(data.len()).expect("a PNG chunk is under 4 GiB");
        file.extend_from_slice(&length.to_be_bytes());
        let start = file.len();
        file.extend_from_slice(kind);
        file.extend_from_slice(data);
        let crc = crc32(&file[start..]);
        file.extend_from_slice(&crc.to_be_bytes());
    }
    file
}

fn adler32(data: &[u8]) -> u32 {
    const MODULUS: u32 = 65_521;
    let (mut a, mut b) = (1_u32, 0_u32);
    for &byte in data {
        a = (a + u32::from(byte)) % MODULUS;
        b = (b + a) % MODULUS;
    }
    b << 16 | a
}

/// The CRC-32 PNG chunks carry (the zlib and Ethernet polynomial).
fn crc32(data: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut table = [0_u32; 256];
        let mut index = 0;
        while index < 256 {
            let mut value = index as u32;
            let mut bit = 0;
            while bit < 8 {
                value = if value & 1 == 1 {
                    0xedb8_8320 ^ (value >> 1)
                } else {
                    value >> 1
                };
                bit += 1;
            }
            table[index] = value;
            index += 1;
        }
        table
    };
    let mut crc = !0_u32;
    for &byte in data {
        crc = TABLE[((crc ^ u32::from(byte)) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}
