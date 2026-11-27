//! A sheet image: frames laid out in cells, labelled in pixel digits so no
//! font is needed, written to PNG.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use avatar_3d_wgpu::Image;

/// The digits 0 to 9 on a 3 by 5 pixel grid. Each row is three bits, the
/// left pixel highest, and the top row is the highest three bits.
const DIGITS: [u16; 10] = [
    0b111_101_101_101_111,
    0b010_110_010_010_111,
    0b111_001_111_100_111,
    0b111_001_111_001_111,
    0b101_101_111_001_001,
    0b111_100_111_001_111,
    0b111_100_111_101_111,
    0b111_001_001_001_001,
    0b111_101_111_101_111,
    0b111_101_111_001_111,
];

/// The grey of the lines between cells.
const SEPARATOR: [u8; 3] = [0xd0, 0xd0, 0xd0];
/// A label's paper and ink.
const PAPER: [u8; 3] = [0xf8, 0xf8, 0xf8];
const INK: [u8; 3] = [0x30, 0x30, 0x30];

/// An RGBA8 image being composed.
pub struct Sheet {
    pub width: u32,
    pub height: u32,
    pixels: Vec<u8>,
}

impl Sheet {
    /// A sheet of `columns × rows` cells of `cell_w × cell_h` pixels with
    /// 1 px separators between and around them.
    pub fn grid(columns: u32, rows: u32, cell_w: u32, cell_h: u32) -> Self {
        let width = columns * (cell_w + 1) + 1;
        let height = rows * (cell_h + 1) + 1;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..width * height {
            pixels.extend_from_slice(&[SEPARATOR[0], SEPARATOR[1], SEPARATOR[2], 255]);
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    /// A sheet holding one frame as it is.
    pub fn of(image: &Image) -> Self {
        Self {
            width: image.width,
            height: image.height,
            pixels: image.pixels.clone(),
        }
    }

    /// The top-left pixel of cell `(column, row)` of a grid of
    /// `cell_w × cell_h` cells.
    pub fn cell_origin(column: u32, row: u32, cell_w: u32, cell_h: u32) -> (u32, u32) {
        (1 + column * (cell_w + 1), 1 + row * (cell_h + 1))
    }

    /// Copies a frame in with its top-left corner at `(x, y)`.
    pub fn blit(&mut self, image: &Image, x: u32, y: u32) {
        for row in 0..image.height {
            let src =
                &image.pixels[(row * image.width * 4) as usize..][..(image.width * 4) as usize];
            let at = (((y + row) * self.width + x) * 4) as usize;
            self.pixels[at..at + src.len()].copy_from_slice(src);
        }
    }

    fn fill(&mut self, x: i64, y: i64, w: i64, h: i64, rgb: [u8; 3]) {
        for py in y.max(0)..(y + h).min(i64::from(self.height)) {
            for px in x.max(0)..(x + w).min(i64::from(self.width)) {
                let at = ((py * i64::from(self.width) + px) * 4) as usize;
                self.pixels[at..at + 3].copy_from_slice(&rgb);
            }
        }
    }

    /// The pixel size of a label of `text` at `scale` pixels per digit
    /// pixel, paper margin included.
    pub fn label_size(text: &str, scale: u32) -> (u32, u32) {
        let n = text.len() as u32;
        ((4 * n + 1) * scale, 7 * scale)
    }

    /// Draws `text` (digits only) on a paper box whose top-left corner is
    /// `(x, y)`, `scale` pixels per digit pixel.
    pub fn label(&mut self, text: &str, x: i64, y: i64, scale: u32) {
        let (w, h) = Self::label_size(text, scale);
        let s = i64::from(scale);
        self.fill(x, y, i64::from(w), i64::from(h), PAPER);
        for (i, ch) in text.chars().enumerate() {
            let digit = DIGITS[ch.to_digit(10).expect("labels are digits") as usize];
            let left = x + s + 4 * s * i as i64;
            for row in 0..5 {
                for col in 0..3 {
                    if digit >> (14 - (row * 3 + col)) & 1 == 1 {
                        self.fill(left + col * s, y + s + row * s, s, s, INK);
                    }
                }
            }
        }
    }

    /// Writes the sheet as an 8-bit RGBA PNG.
    pub fn write_png(&self, path: &Path) -> Result<(), String> {
        let file = File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut w| w.write_image_data(&self.pixels))
            .map_err(|e| format!("write {}: {e}", path.display()))
    }
}
