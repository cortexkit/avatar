//! Pixel checks for `avatar_raster`, through its public API only.
//!
//! Expected pixels never come from the code under test: they are computed
//! here from the SVG definition of each feature, written out cell by cell, or
//! rendered directly with resvg from a committed golden SVG.

mod corpus;
mod features;
mod grid;
mod options;

use std::path::PathBuf;

use avatar_svg::{Paint, Primitive, Rgb, Shape};

pub const BLUE: Rgb = Rgb {
    r: 0,
    g: 96,
    b: 224,
};
pub const RED: Rgb = Rgb {
    r: 240,
    g: 32,
    b: 16,
};
pub const WHITE: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};

pub const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

/// An opaque pixel of the given colour; premultiplied and straight agree.
pub fn opaque(color: Rgb) -> [u8; 4] {
    [color.r, color.g, color.b, 255]
}

pub fn primitive(shape: Shape, paint: Paint) -> Primitive {
    Primitive {
        shape,
        paint,
        transform: None,
        stroke: None,
        opacity: None,
        clip: None,
        effects: vec![],
        label: String::new(),
        role: None,
    }
}

/// The repository's `vectors/` directory.
pub fn vectors() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vectors")
}

/// One pixel of a square RGBA8 image.
pub fn pixel(image: &avatar_raster::Image, x: u32, y: u32) -> [u8; 4] {
    pixels(&image.pixels)[(y * image.size_px + x) as usize]
}

/// RGBA8 bytes as pixels.
pub fn pixels(bytes: &[u8]) -> &[[u8; 4]] {
    let (pixels, rest) = bytes.as_chunks::<4>();
    assert!(rest.is_empty(), "RGBA8 length must be a multiple of 4");
    pixels
}

/// Asserts every channel is within `tolerance` of the expected value.
pub fn assert_near(actual: [u8; 4], expected: [u8; 4], tolerance: u8, context: &str) {
    let close = actual
        .iter()
        .zip(expected)
        .all(|(&got, want)| got.abs_diff(want) <= tolerance);
    assert!(
        close,
        "{context}: got {actual:?}, expected {expected:?} within {tolerance}"
    );
}

/// The straight-alpha formula from the image contract, applied to one
/// premultiplied pixel.
pub fn straight(premultiplied: [u8; 4]) -> [u8; 4] {
    let alpha = u32::from(premultiplied[3]);
    if alpha == 0 {
        return TRANSPARENT;
    }
    let channel = |c: u8| ((u32::from(c) * 255 + alpha / 2) / alpha) as u8;
    [
        channel(premultiplied[0]),
        channel(premultiplied[1]),
        channel(premultiplied[2]),
        premultiplied[3],
    ]
}
