# cortexkit-avatar

> [!WARNING]
> **Alpha.** cortexkit-avatar is in early development and is not yet released for general use.
> Breaking changes will happen between versions, including changes to its settings,
> stored data and interfaces. Pin an exact version if you depend on it.

Render a stored avatar. A stored avatar is a genome (bytes), a creature type and a generator version. `render_tree_for_bytes` routes them to the generator that draws them and returns a render tree, a flat list of shapes in painter order. `serialize_tree` writes it as SVG, and `serialize_tree_json` as canonical JSON.

This is the one crate a Rust host needs to render avatars:

```toml
[dependencies]
avatar = { package = "cortexkit-avatar", version = "=0.1.0" }
```

```rust
use avatar::{render_tree_for_bytes, serialize_tree, Mood, WILD};

let genome = vec![0u8; 16_384]; // a stored creature.wild genome
let tree = render_tree_for_bytes(&genome, WILD, 2, Mood::Happy).expect("wild version 2 is supported");
let svg: Vec<u8> = serialize_tree(&tree);
```

An unknown type or version is a distinct error, so a host can draw nothing rather than a wrong creature. The same genome, type, version and mood always produce the same bytes on every platform.

This crate is part of [avatar](https://github.com/cortexkit/avatar), which draws deterministic creatures from a stored genome. Licensed under MIT.
