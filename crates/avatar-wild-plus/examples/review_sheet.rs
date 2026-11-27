//! A review sheet for `creature.wild-plus`: drawings composited over white,
//! tiled into one PNG for a person to look at full size.
//!
//! The sheet draws the base fidelity manifest's fleet-shaped genomes
//! (`fleet-*`) and random genomes (`random-*`) in neutral, one row of eight
//! after another, then six genomes in every mood, one genome per row with
//! the moods in the order neutral, working, happy, waiting, blocked, sad,
//! sleepy. Each tile is the finished image `rasterize_tree` draws, so the
//! pixels reviewed are the pixels shipped.
//!
//! ```sh
//! cargo run --release -p cortexkit-avatar-wild-plus --example review_sheet -- \
//!     --manifest base.json --out sheet.png [--tile 256] \
//!     [--mood-genomes fleet-00,fleet-01,fleet-02,random-00,random-01,random-02]
//! ```

use std::collections::BTreeMap;
use std::process::ExitCode;

use avatar_raster::{rasterize_tree, ImageOptions};
use avatar_svg::wild::GENOME_LEN;
use avatar_svg::Mood;
use avatar_wild_plus::render_tree;
use serde_json::Value;

const MOODS: [Mood; 7] = [
    Mood::Neutral,
    Mood::Working,
    Mood::Happy,
    Mood::Waiting,
    Mood::Blocked,
    Mood::Sad,
    Mood::Sleepy,
];
const COLUMNS: usize = 8;

fn genome(hex: &str) -> Result<[u8; GENOME_LEN], String> {
    if hex.len() != 2 * GENOME_LEN || !hex.is_ascii() {
        return Err(format!("a genome must be {} hex digits", 2 * GENOME_LEN));
    }
    let mut out = [0u8; GENOME_LEN];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// A white RGB canvas the tiles are composited onto.
struct Canvas {
    width: usize,
    height: usize,
    rgb: Vec<u8>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Canvas {
            width,
            height,
            rgb: vec![255; width * height * 3],
        }
    }

    /// Composites premultiplied RGBA over white at `(x0, y0)`.
    fn put(&mut self, x0: usize, y0: usize, size: usize, rgba: &[u8]) {
        for y in 0..size {
            for x in 0..size {
                let s = &rgba[(y * size + x) * 4..][..4];
                let d = ((y0 + y) * self.width + x0 + x) * 3;
                for c in 0..3 {
                    self.rgb[d + c] = s[c].saturating_add(255 - s[3]);
                }
            }
        }
    }

    /// The canvas as an 8-bit RGB PNG, stored without compression.
    fn png(&self) -> Vec<u8> {
        let mut raw = Vec::with_capacity(self.height * (1 + self.width * 3));
        for row in self.rgb.chunks(self.width * 3) {
            raw.push(0);
            raw.extend_from_slice(row);
        }
        let mut zlib = vec![0x78, 0x01];
        let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
        for (i, block) in blocks.iter().enumerate() {
            zlib.push(u8::from(i + 1 == blocks.len()));
            let len = block.len() as u16;
            zlib.extend_from_slice(&len.to_le_bytes());
            zlib.extend_from_slice(&(!len).to_le_bytes());
            zlib.extend_from_slice(block);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in &raw {
            a = (a + u32::from(byte)) % 65_521;
            b = (b + a) % 65_521;
        }
        zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());

        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(self.width as u32).to_be_bytes());
        ihdr.extend_from_slice(&(self.height as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        for (kind, data) in [(b"IHDR", &ihdr), (b"IDAT", &zlib), (b"IEND", &Vec::new())] {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let start = out.len();
            out.extend_from_slice(kind);
            out.extend_from_slice(data);
            let crc = crc32(&out[start..]);
            out.extend_from_slice(&crc.to_be_bytes());
        }
        out
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn run() -> Result<String, String> {
    let mut manifest = None;
    let mut out = None;
    let mut tile = 256u32;
    let mut mood_genomes: Vec<String> = [
        "fleet-00",
        "fleet-01",
        "fleet-02",
        "random-00",
        "random-01",
        "random-02",
    ]
    .map(str::to_owned)
    .to_vec();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--manifest" => manifest = Some(value()?),
            "--out" => out = Some(value()?),
            "--tile" => {
                tile = value()?
                    .parse()
                    .map_err(|_| "--tile takes a whole number")?
            }
            "--mood-genomes" => mood_genomes = value()?.split(',').map(str::to_owned).collect(),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let manifest = manifest.ok_or("--manifest is required")?;
    let out = out.ok_or("--out is required")?;
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{manifest}: {e}"))?;
    let m: Value = serde_json::from_str(&text).map_err(|e| format!("{manifest}: {e}"))?;
    let mut genomes = BTreeMap::new();
    let mut neutral = Vec::new();
    for g in m["genomes"].as_array().ok_or("manifest has no genomes")? {
        let name = g["name"].as_str().ok_or("a genome has no name")?.to_owned();
        let bytes = genome(g["hex"].as_str().ok_or("a genome has no hex")?)?;
        if name.starts_with("fleet-") || name.starts_with("random-") {
            neutral.push(name.clone());
        }
        genomes.insert(name, bytes);
    }
    let size = tile as usize;
    let neutral_rows = neutral.len().div_ceil(COLUMNS);
    let mut canvas = Canvas::new(COLUMNS * size, (neutral_rows + mood_genomes.len()) * size);
    let draw = |canvas: &mut Canvas,
                name: &str,
                mood: Mood,
                col: usize,
                row: usize|
     -> Result<(), String> {
        let bytes = genomes.get(name).ok_or(format!("no genome {name}"))?;
        let image = rasterize_tree(
            &render_tree(bytes, mood, Some(tile)),
            tile,
            &ImageOptions::default(),
        )
        .map_err(|e| format!("{name}: {e}"))?;
        canvas.put(col * size, row * size, size, &image.pixels);
        Ok(())
    };
    for (i, name) in neutral.iter().enumerate() {
        draw(&mut canvas, name, Mood::Neutral, i % COLUMNS, i / COLUMNS)?;
    }
    for (r, name) in mood_genomes.iter().enumerate() {
        for (c, mood) in MOODS.into_iter().enumerate() {
            draw(&mut canvas, name, mood, c, neutral_rows + r)?;
        }
    }
    std::fs::write(&out, canvas.png()).map_err(|e| format!("{out}: {e}"))?;
    Ok(format!(
        "{} genomes in neutral ({} rows of {COLUMNS}), then {} in every mood: {}",
        neutral.len(),
        neutral_rows,
        mood_genomes.len(),
        mood_genomes.join(", ")
    ))
}

fn main() -> ExitCode {
    match run() {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("review_sheet: {e}");
            ExitCode::from(2)
        }
    }
}
