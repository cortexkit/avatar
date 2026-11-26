# cortexkit-avatar-raster

> [!WARNING]
> **Alpha.** cortexkit-avatar-raster is in early development and is not yet released for general use.
> Breaking changes will happen between versions, including changes to its settings,
> stored data and interfaces. Pin an exact version if you depend on it.

Rasterizes avatar render trees, and their SVG, into square sRGB RGBA8 images at an exact pixel size, with premultiplied or straight alpha and the ground shadow kept or dropped. Most hosts should call `render_image` in [`cortexkit-avatar`](https://crates.io/crates/cortexkit-avatar) instead, which draws a stored avatar straight to pixels.

This crate is part of [avatar](https://github.com/cortexkit/avatar), which draws deterministic creatures from a stored genome. Licensed under MIT.
