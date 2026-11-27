//! Avatar creatures in 3D as plain data, for a renderer to draw.
//!
//! Nothing here touches a GPU. Every result is plain data: f64 vertex
//! attributes, u32 triangle indices and flags, so a renderer in any language
//! can take over the drawing alone. Geometry uses only the operations IEEE 754
//! makes exact on every platform (`+ - * /`, `sqrt`, comparisons, `round`) and
//! the portable sine and cosine of [`avatar_svg::wild::math`], so the same
//! inputs give the same bytes everywhere.
//!
//! [`mesh`] makes the solids the creatures are built from; [`robot`]
//! builds the small robots from their 2D type, and [`motion`] poses them.

pub mod mesh;
pub mod motion;
pub mod robot;
