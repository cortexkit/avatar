//! Tests of the offscreen renderer. Everything but the `gpu` module runs
//! without a GPU: the shader is validated with naga, and every per-frame
//! choice is checked on the CPU.

mod faces;
mod genomes;
mod gpu;
mod shader;
mod upload;
mod view;
