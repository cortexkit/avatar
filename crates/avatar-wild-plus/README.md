# cortexkit-avatar-wild-plus

> [!WARNING]
> **Alpha.** cortexkit-avatar-wild-plus is in early development and is not yet released for general use.
> Breaking changes will happen between versions, including changes to its settings,
> stored data and interfaces. Pin an exact version if you depend on it.

Draws the `creature.wild-plus` avatar type: a wider, stranger branch of the wild creatures: more anatomy, surfaces and faces read from the same genome. It turns a stored 16,384-byte genome and a mood into a render tree. The same genome, version and mood always give the same tree on every platform.

Most hosts should depend on [`cortexkit-avatar`](https://crates.io/crates/cortexkit-avatar) instead, which routes a stored avatar's type and version to this generator and can return a finished image.

This crate is part of [avatar](https://github.com/cortexkit/avatar), which draws deterministic creatures from a stored genome. Licensed under MIT.
