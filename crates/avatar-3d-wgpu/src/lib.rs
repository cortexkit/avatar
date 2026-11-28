//! Draws the 3D small robots offscreen with wgpu, or onto a window.
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
//! - offscreen frames of one robot or of many (the plaza, laid out by
//!   [`plaza`] so no robot hides another's face), read back as RGBA8, with
//!   anchors projected to pixels for labels ([`view`]).
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
//!
//! [`Renderer::draw`] draws the same frame without reading it back and
//! returns once it is submitted, so a frame's CPU time can be measured on
//! its own; [`Submitted::wait`] then gives the GPU's time for it from
//! timestamp queries, where the device has them.
//!
//! # Textures on the GPU
//!
//! A robot's textures share one mipmapped array texture, a layer for each
//! resident texture ([`layers`]). [`Renderer::upload`] keeps every texture
//! resident at its own index and leaves the robot's pixels with it, as the
//! `review_sheets` example needs; [`Renderer::upload_consuming`] uploads
//! only the textures with pixels and frees those pixels.
//! [`Renderer::restock`] changes which textures are resident, or their
//! edge, without waiting for the GPU, and the replaced array texture is
//! freed once the frames that sample it are done.
//!
//! # On a window
//!
//! [`Renderer::for_surface`] starts a renderer that can present to a
//! window's surface, [`Renderer::configure`] sets the surface up, and
//! [`Renderer::present`] shows a frame on it. The frame is drawn offscreen
//! as any other, then copied onto the surface's texture by one triangle
//! that covers it ([`BLIT_SHADER`]), so the robot pipelines keep their one
//! target format whatever format the window takes, and nothing is read
//! back.
//!
//! # Transparent frames
//!
//! A scene with a [`Background::Transparent`] background clears to
//! nothing, and its robots and their shadows are drawn as linear colour
//! premultiplied by coverage. The copy onto a window keeps that alpha, for
//! a surface configured to composite premultiplied alpha. Read back, the
//! frame comes as straight or premultiplied sRGB colour ([`Alpha`]).
//!
//! # Without a GPU
//!
//! The renderer sits behind the default `gpu` feature. Without it, the
//! crate keeps only the modules that work on the CPU ([`colour`],
//! [`frame`], [`layers`], [`plaza`], [`upload`] and [`view`]) and the WGSL
//! sources, and depends on no wgpu or naga.

pub mod colour;
pub mod frame;
#[cfg(feature = "gpu")]
mod gpu;
pub mod layers;
pub mod plaza;
pub mod upload;
pub mod view;

#[cfg(feature = "gpu")]
pub use gpu::{
    target_bytes, transparent_image_bytes, vertex_data, Alpha, Background, GpuRobot, Image, Item,
    RenderError, Renderer, Scene, Submitted, SurfaceSetup,
};

/// The WGSL source of the robot and shadow pipelines.
pub const SHADER: &str = include_str!("shader.wgsl");

/// The WGSL source of the pipelines that copy a drawn frame onto a
/// window's surface, or into the image a transparent frame is read back
/// from.
pub const BLIT_SHADER: &str = include_str!("blit.wgsl");

/// The format of every robot texture: sampling decodes sRGB to linear.
#[cfg(feature = "gpu")]
pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The format frames are drawn into: shading writes linear light, which the
/// target encodes to sRGB.
#[cfg(feature = "gpu")]
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The depth buffer's format.
#[cfg(feature = "gpu")]
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Samples per pixel: frames are drawn multisampled and resolved before
/// readback, so silhouettes are antialiased.
#[cfg(feature = "gpu")]
pub const SAMPLE_COUNT: u32 = 4;
