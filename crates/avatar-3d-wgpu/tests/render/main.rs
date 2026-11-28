//! Tests of the offscreen renderer. Everything but the `gpu` module runs
//! without a GPU: the shader is validated with naga, and every per-frame
//! choice is checked on the CPU. The modules that use the renderer's types
//! build only with the default `gpu` feature; the rest also build without
//! it.

mod faces;
mod genomes;
#[cfg(feature = "gpu")]
mod gpu;
mod layers;
mod plaza;
mod shader;
#[cfg(feature = "gpu")]
mod targets;
mod upload;
mod view;
