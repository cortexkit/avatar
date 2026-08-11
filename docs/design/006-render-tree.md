# Design note 006 — Render tree: geometry-primary API v1

Status: DRAFT, sketch circulated to CKIOS (the first geometry consumer)
before the Swift part-2 implementation. Shape agreed in the integration
thread: iOS has no native SVG renderer, so the port's PRIMARY API is a
typed, PRE-FLATTENED primitive list SwiftUI can draw in a straight loop;
SVG becomes one serializer over the same list. The byte-parity gate
stays on the SVG serializer output.

## Shape

```
RenderTree {
  canvas: u32,                 // 10_000 centi-units (viewBox side)
  background: Rgb | None,      // full-canvas rect
  transform: Transform2D,      // THE one resolved transform (centering
                               // translate + uniform scale) — shared by
                               // every primitive below; identity when the
                               // body fits. No nesting exists to flatten:
                               // this is the tree's entire transform state.
  primitives: [Primitive],     // draw in list order, painter's algorithm
}

Transform2D { tx: i64, ty: i64, scale: Q32 }   // milliunits + Q32.32

Primitive {
  shape: Shape,
  paint: Paint,
  stroke: Stroke | None,
  effects: [Effect],           // consumer-optional annotations, see below
}

Shape =
  | Path { segments: [Segment], closed: bool }   // milliunit coordinates
  | Circle { cx, cy, r }                         // milliunits
  | Ellipse { cx, cy, rx, ry }
  | Rect { x, y, w, h }

Segment =
  | MoveTo { x, y }
  | CubicTo { c1x, c1y, c2x, c2y, x, y }
  | LineTo { x, y }

Paint =
  | Solid { color: Rgb }
  | RadialGradient { stops: [(offset_q32, Rgb)] }   // centered on shape
                                                    // bounds, r = 50% —
                                                    // the only radial
                                                    // form the generator
                                                    // emits
  | None                                            // stroke-only shapes

Stroke { color: Rgb, width: i64 }                   // milliunits

Effect =
  | Lit          // member of the body's lighting group (SVG:
                 // feDiffuseLighting+feComposite). iOS fallback:
                 // ignore — at 28–44pt the lighting is subtle; the
                 // SVG serializer reproduces the exact filter.
  | Masked { mask: [Primitive] }  // pattern mask; SwiftUI: .mask with
                                  // the mask primitives; SVG: <mask>.
}
```

## Rules

1. **Pre-flattened**: the list is depth-first paint order; no groups, no
   inherited state. `transform` is the single resolved transform (the
   generator has exactly one — the centering group), carried once at the
   tree root; consumers apply it per-shape or once around the loop,
   equivalently. If a future generator version ever introduces a second
   transform scope, primitives gain a per-primitive `transform` field
   and the root one is deprecated in the same version — flatness is the
   contract, not the current luck.
2. **Coordinates are pre-transform milliunits**, identical to the values
   the SVG serializer formats. Byte parity forces this: SVG emits
   untransformed coordinates inside a transformed group, so a tree that
   baked the transform in could not reproduce the bytes.
3. **Effects are annotations, not structure.** A consumer that ignores
   every effect still draws a correct creature (lighting is subtle;
   an ignored mask means the pattern overlay is skipped entirely rather
   than drawn unmasked — the Masked primitive's paint is only correct
   under its mask).
4. **The SVG serializer is a pure function of the tree** and reproduces
   today's byte-exact output: element order = list order, defs emitted
   for the effects present, attribute formatting per note 001 §2.5.
5. Fixed-point out the door: i64 milliunits and Q32.32 exactly as the
   phenotype carries them. Scaling to points/pixels is the consumer's
   one division, in its own coordinate space.

## Implementation order

1. Swift part 2 implements the tree and its SVG serializer, gated by
   boundary-2 byte parity (the serializer must reproduce all 30 corpus
   SVGs from the tree).
2. Rust follows in the same shape when the GPUI desktop consumer
   materializes (render_svg becomes serialize(build_tree(...));
   corpus unchanged, pure refactor gated by the existing vectors).
3. TS stays string-first until a TS consumer wants geometry (web draws
   SVG natively; no forced work).
