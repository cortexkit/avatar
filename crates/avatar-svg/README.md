# cortexkit-avatar-svg

> [!WARNING]
> **Alpha.** cortexkit-avatar-svg is in early development and is not yet released for general use.
> Breaking changes will happen between versions, including changes to its settings,
> stored data and interfaces. Pin an exact version if you depend on it.

The creature generators, the render tree they produce, and its JSON and SVG serializers. Most hosts should depend on [`cortexkit-avatar`](https://crates.io/crates/cortexkit-avatar) instead, which routes a stored avatar to the right generator and re-exports the tree types.

This crate is part of [avatar](https://github.com/cortexkit/avatar), which draws deterministic creatures from a stored genome. Licensed under MIT.
