//! Draws review sheets of `creature.knitted-toys` for a person to look at.
//!
//! ```text
//! cargo run --release -p cortexkit-avatar-knitted-toys --example review_sheet -- \
//!     --manifest BASE.json --out DIR [--size PX] [--moods NAME,NAME,...]
//! ```
//!
//! `BASE.json` is a fidelity base manifest (see `scripts/fidelity/README.md`).
//! The sheet draws the kit's fleet-shaped genomes (`fleet-*`) and random
//! genomes (`random-*`) in neutral in `review-neutral.png`, and the genomes
//! named by `--moods` in all seven moods, one row each, in
//! `review-moods.png`. Every toy is rasterized with `avatar_raster`, as a
//! finished image is, and composited over white. `review-sheet.md` says
//! which tile is which genome. The sheets show real stored genomes, so they
//! are never committed.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use avatar_knitted_toys::render_tree;
use avatar_raster::{rasterize_tree, Alpha, GroundShadow, ImageOptions};
use avatar_svg::Mood;
use serde_json::Value;

const MOODS: [(Mood, &str); 7] = [
    (Mood::Neutral, "neutral"),
    (Mood::Working, "working"),
    (Mood::Happy, "happy"),
    (Mood::Waiting, "waiting"),
    (Mood::Blocked, "blocked"),
    (Mood::Sad, "sad"),
    (Mood::Sleepy, "sleepy"),
];

/// Six genomes of different shapes, drawn in every mood unless `--moods`
/// names others.
const DEFAULT_MOOD_GENOMES: &str = "fleet-00,fleet-05,random-00,random-07,variety-03,dice-r1-01";

/// An RGB image composited over white.
struct Sheet {
    width: usize,
    height: usize,
    rgb: Vec<u8>,
}

impl Sheet {
    fn new(columns: usize, rows: usize, tile: usize) -> Self {
        let (width, height) = (columns * tile, rows * tile);
        Sheet {
            width,
            height,
            rgb: vec![255; width * height * 3],
        }
    }

    /// Puts one premultiplied RGBA tile over white at a grid cell.
    fn put(&mut self, column: usize, row: usize, tile: usize, rgba: &[u8]) {
        for y in 0..tile {
            for x in 0..tile {
                let src = &rgba[(y * tile + x) * 4..][..4];
                let at = ((row * tile + y) * self.width + column * tile + x) * 3;
                for channel in 0..3 {
                    // Premultiplied over white: c + 255 * (1 - a).
                    self.rgb[at + channel] = src[channel] + (255 - src[3]);
                }
            }
        }
    }

    /// A PNG with stored (uncompressed) deflate blocks.
    fn png(&self) -> Vec<u8> {
        let mut raw = Vec::with_capacity((self.width * 3 + 1) * self.height);
        for row in self.rgb.chunks(self.width * 3) {
            raw.push(0);
            raw.extend_from_slice(row);
        }
        let mut z = vec![0x78, 0x01];
        let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
        for (i, block) in blocks.iter().enumerate() {
            z.push(u8::from(i + 1 == blocks.len()));
            let len = block.len() as u16;
            z.extend_from_slice(&len.to_le_bytes());
            z.extend_from_slice(&(!len).to_le_bytes());
            z.extend_from_slice(block);
        }
        z.extend_from_slice(&adler32(&raw).to_be_bytes());
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(self.width as u32).to_be_bytes());
        ihdr.extend_from_slice(&(self.height as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        out
    }
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = !0_u32;
    for part in parts {
        for &byte in *part {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
    }
    !crc
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
}

fn genome(hex: &str) -> Result<Box<[u8; 16_384]>, String> {
    if hex.len() != 32_768 {
        return Err(format!("a genome is 32,768 hex digits, not {}", hex.len()));
    }
    let mut out = Box::new([0_u8; 16_384]);
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)
            .map_err(|e| format!("genome hex at byte {i}: {e}"))?;
    }
    Ok(out)
}

fn tile(bytes: &[u8; 16_384], mood: Mood, size: u32) -> Result<Vec<u8>, String> {
    let tree = render_tree(bytes, mood, Some(size));
    let options = ImageOptions {
        ground_shadow: GroundShadow::Keep,
        alpha: Alpha::Premultiplied,
    };
    rasterize_tree(&tree, size, &options)
        .map(|image| image.pixels)
        .map_err(|e| format!("rasterize: {e:?}"))
}

fn run() -> Result<(), String> {
    let usage = "usage: review_sheet --manifest FILE --out DIR [--size PX] [--moods NAME,NAME,...]";
    let mut manifest = None;
    let mut out = None;
    let mut size = 256_u32;
    let mut mood_genomes = DEFAULT_MOOD_GENOMES.to_owned();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| format!("{arg} needs a value\n{usage}"))
        };
        match arg.as_str() {
            "--manifest" => manifest = Some(PathBuf::from(value()?)),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--size" => {
                size = value()?
                    .parse()
                    .ok()
                    .filter(|s| (16..=1024).contains(s))
                    .ok_or("--size must be 16 to 1024 pixels")?;
            }
            "--moods" => mood_genomes = value()?,
            _ => return Err(format!("unknown argument {arg:?}\n{usage}")),
        }
    }
    let manifest = manifest.ok_or_else(|| format!("--manifest is required\n{usage}"))?;
    let out = out.ok_or_else(|| format!("--out is required\n{usage}"))?;
    let text =
        fs::read_to_string(&manifest).map_err(|e| format!("read {}: {e}", manifest.display()))?;
    let doc: Value =
        serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", manifest.display()))?;
    let mut genomes = Vec::new();
    for g in doc["genomes"]
        .as_array()
        .ok_or("the manifest has no genomes")?
    {
        let name = g["name"].as_str().ok_or("a genome has no name")?.to_owned();
        let bytes = genome(g["hex"].as_str().ok_or("a genome has no hex")?)?;
        genomes.push((name, bytes));
    }
    fs::create_dir_all(&out).map_err(|e| format!("create {}: {e}", out.display()))?;
    let px = size as usize;
    let mut index = String::from("# Knitted toys review sheets\n\n");
    index.push_str(&format!(
        "Each tile is {size} px, rasterized with avatar_raster and composited over white.\n\n"
    ));

    // The fleet-shaped and random genomes in neutral, eight to a row.
    let neutral: Vec<&(String, Box<[u8; 16_384]>)> = genomes
        .iter()
        .filter(|(name, _)| name.starts_with("fleet-") || name.starts_with("random-"))
        .collect();
    let columns = 8;
    let rows = neutral.len().div_ceil(columns);
    let mut sheet = Sheet::new(columns, rows, px);
    index.push_str("## review-neutral.png\n\nRow by row, left to right:\n\n");
    for (i, (name, bytes)) in neutral.iter().enumerate() {
        sheet.put(
            i % columns,
            i / columns,
            px,
            &tile(bytes, Mood::Neutral, size)?,
        );
        index.push_str(&format!(
            "- row {}, column {}: {name}\n",
            i / columns + 1,
            i % columns + 1
        ));
    }
    fs::write(out.join("review-neutral.png"), sheet.png())
        .map_err(|e| format!("write review-neutral.png: {e}"))?;

    // The chosen genomes in every mood, one row each.
    let chosen: Vec<&str> = mood_genomes.split(',').collect();
    let mut sheet = Sheet::new(MOODS.len(), chosen.len(), px);
    index.push_str("\n## review-moods.png\n\nColumns: ");
    index.push_str(&MOODS.map(|(_, name)| name).join(", "));
    index.push_str(".\n\n");
    for (row, name) in chosen.iter().enumerate() {
        let (_, bytes) = genomes
            .iter()
            .find(|(n, _)| n == name)
            .ok_or_else(|| format!("no genome {name} in the manifest"))?;
        for (column, (mood, _)) in MOODS.iter().enumerate() {
            sheet.put(column, row, px, &tile(bytes, *mood, size)?);
        }
        index.push_str(&format!("- row {}: {name}\n", row + 1));
    }
    fs::write(out.join("review-moods.png"), sheet.png())
        .map_err(|e| format!("write review-moods.png: {e}"))?;
    fs::write(out.join("review-sheet.md"), index)
        .map_err(|e| format!("write review-sheet.md: {e}"))?;
    println!(
        "wrote {} neutral tiles and {} mood rows to {}",
        neutral.len(),
        chosen.len(),
        out.display()
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("review_sheet: {message}");
            ExitCode::from(2)
        }
    }
}
