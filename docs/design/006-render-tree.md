# Design note 006 — Render tree: geometry-primary API v1 (rev 2)

Status: rev 2 after CKIOS's corpus-first review. Rev 1 was written from
the generator's intent and the corpus proved it wrong in four places;
this revision was derived by inventorying every attribute the 30 golden
SVGs actually emit. Rule zero for future revisions: **the corpus is the
spec of record for what the tree must express — diff against it, not
against the renderer source.**

The API shape: a typed, flat primitive list the consumer draws in a
straight loop; SVG is one serializer over the same list; the byte-parity
gate stays on the serializer output.

## Corpus inventory (what the tree must express)

From the 30 committed SVGs: paths/circles/ellipses/rects; solid fills,
one radial-gradient form, stroke color+width with **per-element
linecap/linejoin (212 round caps, 30 round joins — mixed within one
creature)**; **per-element opacity** (240 uses); **per-group transforms**
in four syntactic forms (translate / rotate(a,cx,cy) / translate+rotate /
translate+scale) — the face pass emits per-eye and per-brow transforms,
so rev 1's "single root transform" was FALSE; `<use href>` body reuse
(3–4 per creature); eye **clip-paths** (2 per creature); the lighting
filter group; the iris turbulence-displacement filter; semantic `class`
labels and `data-family` on mouths.

## Shape

```
RenderTree {
  canvas: u32,                       // 10_000 centi-units
  background: Rgb | None,
  primitives: [Primitive],           // flat, painter's order
}

Primitive {
  shape: Shape,
  transform: Transform | None,       // RESOLVED per primitive (composed
                                     //   chain from the SVG group stack);
                                     //   None = identity
  paint: Paint,
  stroke: Stroke | None,
  opacity: Q32 | None,               // None = 1.0
  clip: Shape | None,                // structural: consumers MUST honour
                                     //   (an unclipped iris bleeds outside
                                     //   the eye) — maps to .clipShape
  effects: [Effect],                 // ignorable annotations
  label: str,                        // diagnostic SVG class ("av-pupil", ...)
  role: Role | None,                 // semantic contract for consumer behavior
}

Role = GroundShadow

Transform {                          // typed as the serializer emits it —
  ops: [TransformOp],                //   order preserved for byte parity
}
TransformOp =
  | Translate { tx: i64, ty: i64 }
  | Rotate { angle: Bam16, cx: i64, cy: i64 }
  | Scale { sx: Q32, sy: Q32 }
// Consumers: fold ops into one affine matrix (helper provided:
// Transform.affine() -> (a,b,c,d,tx,ty) in milliunit space).

Shape =
  | Path { segments: [Segment], closed: bool, fill_rule: NonZero|EvenOdd }
  | Circle { cx, cy, r } | Ellipse { cx, cy, rx, ry } | Rect { x, y, w, h }
Segment = MoveTo | CubicTo | LineTo   // milliunit fields as rev 1

Paint = Solid { color: Rgb }
      | RadialGradient { stops: [(offset_q32, Rgb)] }
      | None

Stroke {
  color: Rgb, width: i64,
  cap: Butt | Round | Square,        // 1:1 CGLineCap
  join: Miter | Round | Bevel,       // 1:1 CGLineJoin
}

Effect =
  | Lit { light_y: i64 }             // body lighting filter group; carries
                                     //   the RESOLVED point-light y — the
                                     //   generator computes it from rescaled
                                     //   body geometry, and re-deriving it
                                     //   from the final shape can round one
                                     //   milliunit differently (found as a
                                     //   byte-parity break on a degenerate
                                     //   body; resolved values ride the tree,
                                     //   consumers never re-derive)
  | Masked { mask: [Primitive] }     // pattern mask
  | Turbulence { seed: u8, freq_h: Q32, freq_v: Q32,
                 octaves: u8, scale: Q32 }   // iris shade displacement
```

## Rules

1. **Flat list, resolved transforms.** Each primitive carries its full
   composed transform chain (the SVG group nesting is flattened at tree
   build). Consumers never traverse. `fill_rule` today is always
   NonZero (the corpus emits no fill-rule attribute); the field exists
   so a future hole-cutting shape cannot silently diverge on renderer
   defaults — "agreeing by accident" is not a contract.
2. **Body reuse is inlined.** The SVG's `<use href="#av-bp">` instances
   appear in the tree as separate primitives sharing one Path value
   (same segments, different paint/effects). The SERIALIZER re-collapses
   identical body paths into one def + uses to reproduce the bytes —
   dedup is a serialization concern; the primitive list is one-to-one
   with DRAWN shapes, which is what a renderer loop wants. (Reader
   warning: list length ≠ SVG element count for exactly this reason.)
3. **Coordinates are pre-transform milliunits** — byte parity requires
   serializing untransformed coordinates inside transformed groups.
4. **clip is structural; effects are ignorable.** Skipping Lit loses
   subtle lighting; skipping Masked skips the overlay entirely;
   skipping Turbulence renders the iris shade flat. All acceptable at
   28–44pt. Skipping clip is NOT acceptable (geometry bleeds), hence
   its position outside effects.
5. **The SVG serializer is a pure function of the tree**, reproducing
   all 30 corpus files byte-exactly — that gate is what proves the tree
   expresses everything (any inexpressible attribute fails parity).
6. Fixed-point out the door: i64 milliunits, Q32.32, BAM16. The
   consumer's scale-to-points is one division in its own space.

## Primitive labels and roles

`label` is a diagnostic name (also used for SVG classes and ids), with no
stability promise. Consumers must use `role`, not label spelling, when changing
behavior based on a primitive's meaning. The first semantic role is
`ground_shadow`: a floating-creature view can omit it without depending on the
generator's diagnostic names. Classic has no ground shadow; wild has exactly
one. Roles do not affect SVG serialization or geometry.

Roles form a closed enum: an unknown string fails decoding rather than silently
losing meaning. Like the other optional primitive fields, `role` is always
present in canonical JSON, with `null` for primitives without a role. Strict
consumers accept that null but reject a missing key. Object keys remain
byte-sorted, so `role` appears between `paint` and `shape`.

## Implementation status

1. Swift: DONE — build_tree + SVG serializer, boundary-2 parity from
   the tree (all 30 corpus files).
2. Rust: DONE — same reshape as a corpus-gated zero-diff refactor
   (`build_render_tree` / `serialize_svg` public; `render_svg` is the
   composition). Both tree implementations share the structural shape,
   so generator changes land once per language in the same seam.
3. TS: stays string-first until a geometry consumer exists (web draws
   SVG natively; no forced work).
