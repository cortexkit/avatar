//! A review sheet of drawn heads, composited over white, for a person to
//! look at full size.
//!
//! ```text
//! cargo run --release -p cortexkit-avatar-drawn-heads --example review_sheet -- \
//!     --manifest BASE --out SHEET.png [--px 200]
//! ```
//!
//! `BASE` is the fidelity harness's base manifest (`scripts/fidelity/`).
//! The sheet's top part is the 33 fleet-shaped genomes (`fleet-00` to
//! `fleet-32`) and the 24 random ones (`random-00` to `random-23`) in
//! neutral, ten to a row in that order. Below it, six genomes (`fleet-00`,
//! `fleet-07`, `fleet-14`, `fleet-21`, `random-03`, `random-17`) each take a
//! row of all seven moods, in the order neutral, working, happy, waiting,
//! blocked, sad, sleepy. Each tile is drawn by `avatar_raster::rasterize_tree`,
//! the way finished images are drawn. The sheet is never committed: the
//! fleet-shaped genomes are real stored genomes.

use std::{collections::BTreeMap, fs, path::PathBuf, process};

use avatar_drawn_heads::{render_tree, GENOME_LEN, MOODS};
use avatar_raster::{rasterize_tree, Alpha, GroundShadow, ImageOptions};
use avatar_svg::Mood;
use serde_json::Value;

const COLUMNS: usize = 10;
const GAP: usize = 6;
const MOOD_ROWS: [&str; 6] = [
    "fleet-00",
    "fleet-07",
    "fleet-14",
    "fleet-21",
    "random-03",
    "random-17",
];

fn fail(message: &str) -> ! {
    eprintln!("review_sheet: {message}");
    process::exit(2)
}

fn genome_of(hex: &str) -> [u8; GENOME_LEN] {
    let mut out = [0u8; GENOME_LEN];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)
            .unwrap_or_else(|_| fail("a genome is not hex"));
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut manifest = None;
    let mut out = None;
    let mut px: usize = 200;
    for pair in args.chunks(2) {
        let [flag, value] = pair else {
            fail("every flag needs a value")
        };
        match flag.as_str() {
            "--manifest" => manifest = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--px" => px = value.parse().unwrap_or_else(|_| fail("--px is a number")),
            other => fail(&format!("unknown flag {other}")),
        }
    }
    let manifest = manifest.unwrap_or_else(|| fail("--manifest is required"));
    let out = out.unwrap_or_else(|| fail("--out is required"));
    let doc: Value = serde_json::from_str(
        &fs::read_to_string(&manifest).unwrap_or_else(|e| fail(&format!("read manifest: {e}"))),
    )
    .unwrap_or_else(|e| fail(&format!("parse manifest: {e}")));
    let genomes: BTreeMap<String, [u8; GENOME_LEN]> = doc["genomes"]
        .as_array()
        .unwrap_or_else(|| fail("no genomes"))
        .iter()
        .map(|g| {
            (
                g["name"].as_str().unwrap_or_default().to_owned(),
                genome_of(g["hex"].as_str().unwrap_or_default()),
            )
        })
        .collect();
    let get = |name: &str| {
        genomes
            .get(name)
            .unwrap_or_else(|| fail(&format!("the manifest has no genome {name}")))
    };

    // The tiles, row by row.
    let mut rows: Vec<Vec<(String, Mood)>> = Vec::new();
    let neutral: Vec<String> = (0..33)
        .map(|i| format!("fleet-{i:02}"))
        .chain((0..24).map(|i| format!("random-{i:02}")))
        .collect();
    for chunk in neutral.chunks(COLUMNS) {
        rows.push(chunk.iter().map(|n| (n.clone(), Mood::Neutral)).collect());
    }
    for name in MOOD_ROWS {
        rows.push(MOODS.iter().map(|&m| (name.to_owned(), m)).collect());
    }

    let width = COLUMNS * px + (COLUMNS + 1) * GAP;
    let height = rows.len() * px + (rows.len() + 1) * GAP;
    // A light grey ground with white tiles, so the tiles' edges show.
    let mut rgb = vec![214u8; width * height * 3];
    let options = ImageOptions {
        ground_shadow: GroundShadow::Keep,
        alpha: Alpha::Premultiplied,
    };
    for (r, row) in rows.iter().enumerate() {
        for (c, (name, mood)) in row.iter().enumerate() {
            let tree = render_tree(get(name), *mood, Some(px as u32));
            let image = rasterize_tree(&tree, px as u32, &options)
                .unwrap_or_else(|e| fail(&format!("rasterize {name}: {e}")));
            let x0 = GAP + c * (px + GAP);
            let y0 = GAP + r * (px + GAP);
            for y in 0..px {
                for x in 0..px {
                    let s = (y * px + x) * 4;
                    let a = image.pixels[s + 3];
                    let d = ((y0 + y) * width + x0 + x) * 3;
                    for k in 0..3 {
                        // Premultiplied over white: c + (255 - a).
                        rgb[d + k] = image.pixels[s + k] + (255 - a);
                    }
                }
            }
        }
    }
    fs::write(&out, png(width, height, &rgb))
        .unwrap_or_else(|e| fail(&format!("write {}: {e}", out.display())));
    println!(
        "review_sheet: {} tiles of {px} px, {width} x {height}, written to {}",
        rows.iter().map(Vec::len).sum::<usize>(),
        out.display()
    );
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in bytes {
        crc ^= u32::from(b);
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

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend(kind);
    out.extend(data);
    let crc = crc32(&out[start..]);
    out.extend(crc.to_be_bytes());
}

/// An 8-bit RGB PNG, its pixels in stored (uncompressed) deflate blocks.
fn png(width: usize, height: usize, rgb: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width * 3 + 1) * height);
    for row in rgb.chunks(width * 3) {
        raw.push(0);
        raw.extend(row);
    }
    let mut z = vec![0x78, 0x01];
    let blocks = raw.chunks(65_535).collect::<Vec<_>>();
    for (i, block) in blocks.iter().enumerate() {
        z.push(u8::from(i + 1 == blocks.len()));
        let len = block.len() as u16;
        z.extend(len.to_le_bytes());
        z.extend((!len).to_le_bytes());
        z.extend(*block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend((width as u32).to_be_bytes());
    ihdr.extend((height as u32).to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}
