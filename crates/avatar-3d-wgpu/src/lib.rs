//! Draws the 3D small robots offscreen with wgpu.
//!
//! Everything a robot looks like comes from [`avatar_3d`] as plain data,
//! used unchanged: meshes, materials with their finishes, the status colour
//! table, textures with their bindings, the skeleton, the stage camera and
//! the poses an [`avatar_3d::motion::Actor`] gives. This crate adds only the
//! drawing:
//!
//! - the shading model in WGSL ([`SHADER`]): a wrapped key light fixed to
//!   the camera blending from each colour's own dark tone to the colour, a
//!   sky term, a fill, darkening at grazing angles and a highlight per
//!   finish, with no tone mapping;
//! - the ground shadow, a soft disc under each robot;
//! - offscreen frames of one robot or of many (the plaza), read back as
//!   RGBA8, with anchors projected to pixels for labels ([`view`]).
//!
//! # Colour
//!
//! Base colours are sRGB and shading runs in linear light into an
//! [`TARGET_FORMAT`] target, which encodes to sRGB on write. Textures are
//! converted at upload ([`upload`]) so that sampling the [`TEXTURE_FORMAT`]
//! texture yields linear colour premultiplied by alpha, and they blend
//! premultiplied. Face and stalk-eye textures are drawn unlit, at the
//! brightness the 2D mood bakes in, and the glow does not touch them; marks
//! are shaded like the paint under them.
//!
//! # Per frame
//!
//! [`frame`] works out on the CPU what a frame shows: skin matrices, each
//! material's colour (a status light's from the table) and emission, the
//! face textures the pose's face set and blink select, anchor positions and
//! the shadow. [`Renderer::render`] uploads those and draws.

pub mod colour;
pub mod frame;
mod gpu;
pub mod upload;
pub mod view;

pub use gpu::{vertex_data, GpuRobot, Image, Item, RenderError, Renderer, Scene};

/// The WGSL source of the robot and shadow pipelines.
pub const SHADER: &str = include_str!("shader.wgsl");

/// The format of every robot texture: sampling decodes sRGB to linear.
pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The format frames are drawn into: shading writes linear light, which the
/// target encodes to sRGB.
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The depth buffer's format.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Samples per pixel: frames are drawn multisampled and resolved before
/// readback, so silhouettes are antialiased.
pub const SAMPLE_COUNT: u32 = 4;
