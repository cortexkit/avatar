//! Turn an avatar render tree, or any SVG, into a finished RGBA image.
//!
//! [`rasterize_tree`] is the route a finished avatar image takes. It applies
//! the [`ImageOptions`], projects the tree to SVG with
//! [`avatar_svg::serialize_tree`] and draws that SVG with resvg through
//! [`rasterize_svg`]. Fidelity reports and review sheets call the same
//! functions, so the pixels reviewed are the pixels shipped.
//!
//! Classic trees draw through that generic projection too, which ignores the
//! classic-only `Lit` and `Turbulence` effects exactly as tree clients do.
//!
//! Every call is a pure function of its arguments: nothing is cached, no
//! global state is read or written, and the SVG parser is never allowed to
//! load a file. Calls are safe to make from several threads at once.
//!
//! ```
//! use avatar_raster::{rasterize_svg, Alpha};
//!
//! let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
//! <rect x="5" y="0" width="5" height="10" fill="#ff0000"/>
//! </svg>"##;
//! let image = rasterize_svg(svg, 4, Alpha::Straight).expect("the SVG parses");
//!
//! assert_eq!(image.pixels.len(), 4 * 4 * 4);
//! // The left half is transparent and the right half opaque red.
//! assert_eq!(image.pixels[..4], [0, 0, 0, 0]);
//! assert_eq!(image.pixels[12..16], [255, 0, 0, 255]);
//! ```

use std::borrow::Cow;

use avatar_svg::{serialize_tree, RenderTree, Role};
use resvg::{tiny_skia, usvg};

/// The largest image edge, in pixels, either function accepts. The smallest
/// is 1.
pub const MAX_SIZE_PX: u32 = 2_048;

/// Whether the primitives that draw the creature's ground shadow are kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GroundShadow {
    /// Draw every primitive, the ground shadow included.
    #[default]
    Keep,
    /// Leave out each top-level primitive whose role is
    /// [`Role::GroundShadow`], for a host that draws its own shadow. Only the
    /// role decides: a primitive that merely looks like a shadow stays.
    Drop,
}

/// How the colour channels of [`Image::pixels`] relate to its alpha channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Alpha {
    /// Each colour channel is already multiplied by alpha, as the rasterizer
    /// produces it.
    #[default]
    Premultiplied,
    /// Colour channels do not depend on alpha. A fully transparent pixel is
    /// `[0, 0, 0, 0]`; any other pixel's colour channel is
    /// `(c × 255 + a / 2) / a` in integers, where `c` is the premultiplied
    /// channel and `a` the alpha.
    Straight,
}

/// How [`rasterize_tree`] turns a tree into an image. The default keeps the
/// ground shadow and returns premultiplied pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ImageOptions {
    pub ground_shadow: GroundShadow,
    pub alpha: Alpha,
}

/// A square sRGB image with 8-bit RGBA pixels.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Image {
    /// The edge length in pixels, on both axes.
    pub size_px: u32,
    /// Whether `pixels` is premultiplied or straight.
    pub alpha: Alpha,
    /// Exactly `size_px² × 4` bytes: R, G, B, A for each pixel, rows top
    /// first, each row left to right, with no padding.
    pub pixels: Vec<u8>,
}

/// Why an image could not be produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RasterError {
    /// The requested edge length was outside `1..=MAX_SIZE_PX`.
    ImageSize { requested: u32 },
    /// The SVG did not parse. The message is the parser's own.
    InvalidSvg { message: String },
}

impl std::fmt::Display for RasterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ImageSize { requested } => write!(
                formatter,
                "image size must be 1 to {MAX_SIZE_PX} pixels, received {requested}"
            ),
            Self::InvalidSvg { message } => write!(formatter, "SVG did not parse: {message}"),
        }
    }
}

impl std::error::Error for RasterError {}

/// Draws an SVG document onto a transparent square image.
///
/// The document is scaled uniformly by `size_px` divided by its width, with
/// its origin at the image's top-left corner. Nothing outside the scaled
/// document's area is painted.
pub fn rasterize_svg(svg: &[u8], size_px: u32, alpha: Alpha) -> Result<Image, RasterError> {
    check_size(size_px)?;
    let document = parse(svg)?;
    let scale = size_px as f32 / document.size().width();
    Ok(draw(
        &document,
        size_px,
        tiny_skia::Transform::from_scale(scale, scale),
        alpha,
    ))
}

/// Draws a render tree onto a transparent square image.
///
/// With [`GroundShadow::Drop`] the tree's top-level ground-shadow primitives
/// are removed first. The tree is then serialized with
/// [`avatar_svg::serialize_tree`] and drawn as [`rasterize_svg`] draws it,
/// except for pixel-grid trees.
///
/// A tree whose `pixel_grid` is `Some(cells)` is pixel art: each cell must
/// cover a whole number of device pixels, with hard edges. Each cell is
/// therefore drawn `cell_px = size_px / cells` pixels wide (rounded down), and
/// the `cell_px × cells` square drawing is centred in the image. Leftover
/// pixels form a transparent margin; when their count is odd, the extra pixel
/// goes to the right and bottom margins. Anti-aliasing is off because the
/// serializer marks a pixel-grid document `shape-rendering="crispEdges"`.
///
/// When `size_px` is smaller than `cells`, a whole pixel per cell does not
/// fit. The tree is then drawn like any other tree, the whole canvas scaled
/// to `size_px` with anti-aliasing on, so a small icon still shows a
/// smoothly downsampled creature rather than nothing.
///
/// # Panics
///
/// Panics when the tree breaks an invariant the serializer asserts, such as
/// `pixel_grid` being `Some(0)`.
pub fn rasterize_tree(
    tree: &RenderTree,
    size_px: u32,
    options: &ImageOptions,
) -> Result<Image, RasterError> {
    check_size(size_px)?;
    let mut drawn = Cow::Borrowed(tree);
    if options.ground_shadow == GroundShadow::Drop && tree.primitives.iter().any(is_ground_shadow) {
        drawn
            .to_mut()
            .primitives
            .retain(|primitive| !is_ground_shadow(primitive));
    }

    let Some(cells) = tree.pixel_grid else {
        return rasterize_svg(&serialize_tree(&drawn), size_px, options.alpha);
    };
    assert!(cells > 0, "pixel grid must have at least one cell");
    let cell_px = size_px / cells;
    if cell_px == 0 {
        // Without the grid the serializer leaves out crispEdges, so this
        // draws anti-aliased.
        drawn.to_mut().pixel_grid = None;
        return rasterize_svg(&serialize_tree(&drawn), size_px, options.alpha);
    }

    let document = parse(&serialize_tree(&drawn))?;
    let drawing_px = cell_px * cells;
    let margin = ((size_px - drawing_px) / 2) as f32;
    let scale = drawing_px as f32 / document.size().width();
    Ok(draw(
        &document,
        size_px,
        tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, margin, margin),
        options.alpha,
    ))
}

fn is_ground_shadow(primitive: &avatar_svg::Primitive) -> bool {
    primitive.role == Some(Role::GroundShadow)
}

fn check_size(size_px: u32) -> Result<(), RasterError> {
    if (1..=MAX_SIZE_PX).contains(&size_px) {
        Ok(())
    } else {
        Err(RasterError::ImageSize { requested: size_px })
    }
}

fn parse(svg: &[u8]) -> Result<usvg::Tree, RasterError> {
    let mut options = usvg::Options::default();
    // By default the parser reads any file an <image> names. Refusing every
    // such reference keeps the image a function of the SVG bytes alone; an
    // image embedded as a data URL still travels inside them.
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    usvg::Tree::from_data(svg, &options).map_err(|error| RasterError::InvalidSvg {
        message: error.to_string(),
    })
}

fn draw(
    document: &usvg::Tree,
    size_px: u32,
    transform: tiny_skia::Transform,
    alpha: Alpha,
) -> Image {
    let mut pixmap = tiny_skia::Pixmap::new(size_px, size_px)
        .expect("a square pixmap of 1 to MAX_SIZE_PX pixels always allocates");
    resvg::render(document, transform, &mut pixmap.as_mut());
    let mut pixels = pixmap.take();
    if alpha == Alpha::Straight {
        unpremultiply(&mut pixels);
    }
    Image {
        size_px,
        alpha,
        pixels,
    }
}

/// Converts premultiplied RGBA8 to straight alpha in place, in integers, so
/// the result is the same on every platform.
fn unpremultiply(pixels: &mut [u8]) {
    let (pixels, _) = pixels.as_chunks_mut::<4>();
    for pixel in pixels {
        let alpha = u32::from(pixel[3]);
        if alpha == 0 {
            pixel.fill(0);
            continue;
        }
        for channel in &mut pixel[..3] {
            // A premultiplied channel never exceeds alpha, which keeps the
            // quotient within 255; the clamp only guards malformed input.
            let straight = (u32::from(*channel) * 255 + alpha / 2) / alpha;
            *channel = straight.min(255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::unpremultiply;

    #[test]
    fn unpremultiply_rounds_to_nearest_and_clears_transparent_pixels() {
        let mut pixels = [
            128, 64, 0, 128, // half alpha: 255.5 rounds down, 128 exactly
            10, 5, 15, 64, // quarter alpha
            7, 7, 7, 0, // a transparent pixel with stray colour
            255, 0, 1, 255, // opaque pixels are unchanged
        ];
        unpremultiply(&mut pixels);
        assert_eq!(
            pixels,
            [255, 128, 0, 128, 40, 20, 60, 64, 0, 0, 0, 0, 255, 0, 1, 255]
        );
    }
}
