//! The fidelity comparison draws each SVG with `avatar_raster::rasterize_svg`,
//! which knows nothing of pixel grids, so the port writes its trees with
//! `placed_svg`. These tests pin that the placed SVG draws exactly the
//! pixels apps get from `rasterize_tree`, so the comparison checks what
//! ships.

use std::fs;
use std::path::Path;

use avatar_pixel_critters::{placed_svg, render_tree, GENOME_LEN};
use avatar_raster::{rasterize_svg, rasterize_tree, Alpha, ImageOptions};
use avatar_svg::Mood;

fn read_genome(project_id: &str) -> [u8; GENOME_LEN] {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vectors/wild")
        .join(format!("{project_id}.hex"));
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut genome = [0u8; GENOME_LEN];
    for (i, byte) in genome.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text.trim()[2 * i..2 * i + 2], 16).expect("hex genome");
    }
    genome
}

#[test]
fn placed_svg_draws_the_pixels_rasterize_tree_draws() {
    let cases = [
        ("wild-port-0005", Mood::Neutral),
        ("wild-port-0005", Mood::Blocked),
        ("wild-port-0005", Mood::Sleepy),
        ("wild-port-0022", Mood::Happy),
        ("wild-port-0077", Mood::Waiting),
    ];
    // 256 px leaves an 8 px margin; 96 and 48 px divide evenly; 100 px
    // leaves a 2 px margin.
    for size in [256, 96, 48, 100] {
        for (id, mood) in cases {
            let tree = render_tree(&read_genome(id), mood);
            let placed = rasterize_svg(&placed_svg(&tree, size), size, Alpha::Premultiplied)
                .expect("the placed SVG parses");
            let shipped =
                rasterize_tree(&tree, size, &ImageOptions::default()).expect("the tree rasterizes");
            assert!(
                placed.pixels == shipped.pixels,
                "{id} {mood:?} at {size} px: the placed SVG draws other pixels"
            );
            // The margin is transparent and the creature is drawn.
            assert_eq!(placed.pixels[3], 0, "{id} {mood:?} at {size} px: corner");
            assert!(placed.pixels.chunks(4).any(|p| p[3] == 255));
        }
    }
}

#[test]
fn placed_svg_only_replaces_the_root_element() {
    let tree = render_tree(&read_genome("wild-port-0009"), Mood::Neutral);
    let plain = String::from_utf8(avatar_svg::serialize_tree(&tree)).unwrap();
    let placed = String::from_utf8(placed_svg(&tree, 256)).unwrap();
    assert!(placed.starts_with(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"256\" height=\"256\" viewBox=\"-0.8 -0.8 25.6 25.6\" shape-rendering=\"crispEdges\">"
    ));
    let body = |svg: &str| svg[svg.find('>').unwrap()..].to_owned();
    assert_eq!(body(&placed), body(&plain));
}
