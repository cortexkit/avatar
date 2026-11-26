//! Render a stored avatar.
//!
//! This is the one crate a Rust host depends on. A registry row stores an
//! avatar's genome bytes, its type (such as [`WILD`] or [`CLASSIC`]) and the
//! generation version that produced it. [`render_tree_for_bytes`] routes that
//! row to the generator that draws it and returns the generic [`RenderTree`];
//! [`serialize_tree`] turns the tree into SVG and [`serialize_tree_json`] into
//! its canonical JSON.
//!
//! Every item is re-exported unchanged from the crate that implements it, so
//! a value from here is the same type the lower-level crates use.
//!
//! ```
//! use avatar::{render_tree_for_bytes, serialize_tree, Mood, Shape, WILD};
//!
//! // A wild genome is 16,384 bytes; any byte pattern decodes to a creature.
//! let genome: Vec<u8> = (0..16_384_u32).map(|index| (index * 7 % 251) as u8).collect();
//! let tree = render_tree_for_bytes(&genome, WILD, 2, Mood::Happy).expect("wild version 2 renders");
//!
//! assert_eq!(tree.canvas, 240);
//! assert!(!tree.primitives.is_empty());
//! assert!(tree
//!     .primitives
//!     .iter()
//!     .any(|primitive| matches!(primitive.shape, Shape::Path { .. })));
//!
//! let svg = String::from_utf8(serialize_tree(&tree)).expect("SVG is UTF-8");
//! assert!(svg.starts_with("<svg"));
//! assert!(svg.trim_end().ends_with("</svg>"));
//! ```
//!
//! A classic genome is 1,024 bytes and is passed as a [`Genome`]:
//!
//! ```
//! use avatar::{render_tree_for, serialize_svg, Genome, Mood, CLASSIC};
//!
//! let genome = Genome::from_bytes([0; 1_024]);
//! let tree = render_tree_for(&genome, CLASSIC, 2, Mood::Neutral).expect("classic version 2 renders");
//!
//! let svg = String::from_utf8(serialize_svg(&tree)).expect("SVG is UTF-8");
//! assert!(svg.starts_with("<svg"));
//! ```

pub use avatar_genome::Genome;
pub use avatar_svg::{render_tree_for, render_tree_for_bytes, DispatchError, CLASSIC, WILD};
pub use avatar_svg::{
    serialize_svg, serialize_tree, serialize_tree_json, serialize_tree_with_id_prefix,
};
pub use avatar_svg::{
    Affine, Effect, FillRule, GradientStop, GradientUnits, Paint, Primitive, RenderTree, Role,
    Segment, Shape, Stroke, StrokeCap, StrokeJoin, Subpath, Transform, TransformOp,
};
pub use avatar_svg::{Bam16, Mood, RenderError, Rgb, Q32};
