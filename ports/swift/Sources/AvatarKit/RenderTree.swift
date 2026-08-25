// Drawable geometry and resolved transforms in painter's order, allowing
// consumers to render an avatar without rebuilding the SVG scene graph.

/// A complete avatar drawing in painter's order.
public struct RenderTree: Equatable {
    /// The square canvas edge in SVG USER UNITS — the serializer's viewBox.
    ///
    /// **Not the unit primitive coordinates are expressed in.** Those are
    /// 100x finer; normalise with ``extent``, never with this. Dividing
    /// geometry by `canvas` produces a drawing 100x too large, which looks
    /// like a plausible zoomed-in avatar rather than an error.
    public var canvas: UInt32
    public var background: Rgb?
    public var primitives: [Primitive]

    /// The coordinate extent primitives span, on both axes, from zero.
    ///
    /// The divisor a geometry consumer wants: multiply a coordinate by the
    /// target edge length and divide by this to place it in a view. Exposed
    /// as a value so no consumer has to rediscover the factor by reading the
    /// SVG serializer.
    public var extent: Int64 { Int64(canvas) * RenderTree.coordinatesPerCanvasUnit }

    /// Primitive coordinates carry two more decimal places than the canvas.
    public static let coordinatesPerCanvasUnit: Int64 = 100

    public init(canvas: UInt32, background: Rgb?, primitives: [Primitive]) {
        self.canvas = canvas
        self.background = background
        self.primitives = primitives
    }
}

/// One independently drawable item. Transforms are resolved from the SVG group
/// stack so consumers can draw the array without traversing a scene graph.
public struct Primitive: Equatable {
    public var shape: Shape
    public var transform: Transform?
    public var paint: Paint
    public var stroke: Stroke?
    public var opacity: Q32?
    public var clip: Shape?
    public var effects: [Effect]
    public var label: String

    public init(
        shape: Shape,
        transform: Transform? = nil,
        paint: Paint,
        stroke: Stroke? = nil,
        opacity: Q32? = nil,
        clip: Shape? = nil,
        effects: [Effect] = [],
        label: String
    ) {
        self.shape = shape
        self.transform = transform
        self.paint = paint
        self.stroke = stroke
        self.opacity = opacity
        self.clip = clip
        self.effects = effects
        self.label = label
    }
}

public enum FillRule: Equatable {
    case nonZero
    case evenOdd
}

public enum Segment: Equatable {
    case moveTo(x: Int64, y: Int64)
    case cubicTo(cp1x: Int64, cp1y: Int64, cp2x: Int64, cp2y: Int64, x: Int64, y: Int64)
    case lineTo(x: Int64, y: Int64)
}

public enum Shape: Equatable {
    case path(segments: [Segment], closed: Bool, fill_rule: FillRule)
    case circle(cx: Int64, cy: Int64, r: Int64)
    case ellipse(cx: Int64, cy: Int64, rx: Int64, ry: Int64)
    case rect(x: Int64, y: Int64, w: Int64, h: Int64)
}

public struct GradientStop: Equatable {
    public var offset_q32: Q32
    public var color: Rgb

    public init(offset_q32: Q32, color: Rgb) {
        self.offset_q32 = offset_q32
        self.color = color
    }
}

public enum Paint: Equatable {
    case solid(color: Rgb)
    case radialGradient(stops: [GradientStop])
    case none
}

public enum StrokeCap: Equatable {
    case butt
    case round
    case square
}

public enum StrokeJoin: Equatable {
    case miter
    case round
    case bevel
}

public struct Stroke: Equatable {
    public var color: Rgb
    public var width: Int64
    public var cap: StrokeCap
    public var join: StrokeJoin

    public init(color: Rgb, width: Int64, cap: StrokeCap = .butt, join: StrokeJoin = .miter) {
        self.color = color
        self.width = width
        self.cap = cap
        self.join = join
    }
}

public indirect enum Effect: Equatable {
    /// Membership in the body's lighting-filter group. Carries the resolved
    /// point-light y so the serializer never re-derives it from the shape:
    /// the generator computes it from body geometry that passed through
    /// rescale's per-value rounding, and a re-flattening of the final shape
    /// can land one milliunit away — a byte-parity break, not a visual one.
    case lit(lightY: Int64)
    case masked(mask: [Primitive])
    case turbulence(seed: UInt8, freq_h: Q32, freq_v: Q32, octaves: UInt8, scale: Q32)
}

public enum TransformOp: Equatable {
    case translate(tx: Int64, ty: Int64)
    case rotate(angle: Bam16, cx: Int64, cy: Int64)
    case scale(sx: Q32, sy: Q32)
}

/// Q32.32 affine coefficients with milliunit translation.
public struct Affine: Equatable {
    public var a: Q32
    public var b: Q32
    public var c: Q32
    public var d: Q32
    public var tx: Int64
    public var ty: Int64

    public init(a: Q32, b: Q32, c: Q32, d: Q32, tx: Int64, ty: Int64) {
        self.a = a
        self.b = b
        self.c = c
        self.d = d
        self.tx = tx
        self.ty = ty
    }
}

public struct Transform: Equatable {
    public var ops: [TransformOp]

    public init(ops: [TransformOp]) {
        self.ops = ops
    }

    /// Composes SVG transform operations in their serialized order. Matrix
    /// products narrow once per Q32 multiply, matching the geometry helpers.
    public func affine() -> Affine {
        var result = Affine(
            a: .one, b: Q32(0), c: Q32(0), d: .one,
            tx: 0, ty: 0)
        for op in ops {
            let next: Affine
            switch op {
            case .translate(let tx, let ty):
                next = Affine(a: .one, b: Q32(0), c: Q32(0), d: .one, tx: tx, ty: ty)
            case .scale(let sx, let sy):
                next = Affine(a: sx, b: Q32(0), c: Q32(0), d: sy, tx: 0, ty: 0)
            case .rotate(let angle, let cx, let cy):
                let cosine = Int64(cosQ30(angle.value)) * 4
                let sine = Int64(sinQ30(angle.value)) * 4
                next = Affine(
                    a: Q32(cosine), b: Q32(sine), c: Q32(-sine), d: Q32(cosine),
                    tx: cx - q32Mul(cx, cosine) + q32Mul(cy, sine),
                    ty: cy - q32Mul(cx, sine) - q32Mul(cy, cosine))
            }
            result = composeAffine(result, next)
        }
        return result
    }
}

private func composeAffine(_ lhs: Affine, _ rhs: Affine) -> Affine {
    Affine(
        a: Q32(q32Mul(lhs.a.value, rhs.a.value) + q32Mul(lhs.c.value, rhs.b.value)),
        b: Q32(q32Mul(lhs.b.value, rhs.a.value) + q32Mul(lhs.d.value, rhs.b.value)),
        c: Q32(q32Mul(lhs.a.value, rhs.c.value) + q32Mul(lhs.c.value, rhs.d.value)),
        d: Q32(q32Mul(lhs.b.value, rhs.c.value) + q32Mul(lhs.d.value, rhs.d.value)),
        tx: lhs.tx + q32Mul(rhs.tx, lhs.a.value) + q32Mul(rhs.ty, lhs.c.value),
        ty: lhs.ty + q32Mul(rhs.tx, lhs.b.value) + q32Mul(rhs.ty, lhs.d.value))
}
