import Foundation

// Drawable geometry and resolved transforms in painter's order, allowing
// consumers to render an avatar without rebuilding the SVG scene graph.

/// A complete avatar drawing in painter's order.
public struct RenderTree: Equatable, Decodable, Sendable {
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
/// Labels are diagnostic names without a stability promise; roles are the
/// semantic contract consumers can use to change drawing behavior.
public struct Primitive: Equatable, Decodable, Sendable {
    public var shape: Shape
    public var transform: Transform?
    public var paint: Paint
    public var stroke: Stroke?
    public var opacity: Q32?
    public var clip: Shape?
    public var effects: [Effect]
    public var label: String
    public var role: Role?

    public init(
        shape: Shape,
        transform: Transform? = nil,
        paint: Paint,
        stroke: Stroke? = nil,
        opacity: Q32? = nil,
        clip: Shape? = nil,
        effects: [Effect] = [],
        label: String,
        role: Role? = nil
    ) {
        self.shape = shape
        self.transform = transform
        self.paint = paint
        self.stroke = stroke
        self.opacity = opacity
        self.clip = clip
        self.effects = effects
        self.label = label
        self.role = role
    }
}

public enum Role: String, Equatable, Decodable, Sendable {
    case groundShadow = "ground_shadow"
}

public enum FillRule: String, Equatable, Decodable, Sendable {
    case nonZero = "non_zero"
    case evenOdd = "even_odd"
}

public enum Segment: Equatable, Decodable, Sendable {
    case moveTo(x: Int64, y: Int64)
    case cubicTo(cp1x: Int64, cp1y: Int64, cp2x: Int64, cp2y: Int64, x: Int64, y: Int64)
    case lineTo(x: Int64, y: Int64)
}

public enum Shape: Equatable, Decodable, Sendable {
    case path(segments: [Segment], closed: Bool, fill_rule: FillRule)
    case circle(cx: Int64, cy: Int64, r: Int64)
    case ellipse(cx: Int64, cy: Int64, rx: Int64, ry: Int64)
    case rect(x: Int64, y: Int64, w: Int64, h: Int64)
}

public struct GradientStop: Equatable, Decodable, Sendable {
    public var offset_q32: Q32
    public var color: Rgb

    public init(offset_q32: Q32, color: Rgb) {
        self.offset_q32 = offset_q32
        self.color = color
    }
}

public enum Paint: Equatable, Decodable, Sendable {
    case solid(color: Rgb)
    case radialGradient(stops: [GradientStop])
    case none
}

public enum StrokeCap: String, Equatable, Decodable, Sendable {
    case butt
    case round
    case square
}

public enum StrokeJoin: String, Equatable, Decodable, Sendable {
    case miter
    case round
    case bevel
}

public struct Stroke: Equatable, Decodable, Sendable {
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

public indirect enum Effect: Equatable, Decodable, Sendable {
    /// Membership in the body's lighting-filter group. Carries the resolved
    /// point-light y so the serializer never re-derives it from the shape:
    /// the generator computes it from body geometry that passed through
    /// rescale's per-value rounding, and a re-flattening of the final shape
    /// can land one milliunit away — a byte-parity break, not a visual one.
    case lit(lightY: Int64)
    case masked(mask: [Primitive])
    case turbulence(seed: UInt8, freq_h: Q32, freq_v: Q32, octaves: UInt8, scale: Q32)
}

public enum TransformOp: Equatable, Decodable, Sendable {
    case translate(tx: Int64, ty: Int64)
    case rotate(angle: Bam16, cx: Int64, cy: Int64)
    case scale(sx: Q32, sy: Q32)
}

/// Q32.32 affine coefficients with milliunit translation.
public struct Affine: Equatable, Decodable, Sendable {
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

public struct Transform: Equatable, Decodable, Sendable {
    public var ops: [TransformOp]

    public init(ops: [TransformOp]) {
        self.ops = ops
    }
}

// MARK: - Canonical tree JSON decoding

/// Decodes a render tree after enforcing the integer-only wire grammar.
///
/// Foundation's `JSONDecoder` accepts integral decimal tokens such as `1.0`
/// for integer properties. The canonical tree format does not: validating the
/// token grammar first keeps alternate numeric spellings out of the identity
/// boundary while `Decodable` handles the typed schema.
public func decodeRenderTreeJson(_ bytes: [UInt8]) throws -> RenderTree {
    var parser = JsonParser(bytes: bytes)
    let document = try parser.parseDocument()
    try requireIntegerNumbers(document)
    let tree = try JSONDecoder().decode(RenderTree.self, from: Data(bytes))
    try validateTransforms(tree.primitives)
    return tree
}

private func validateTransforms(_ primitives: [Primitive]) throws {
    for primitive in primitives {
        if let transform = primitive.transform {
            _ = try transform.checkedAffine()
        }
        for effect in primitive.effects {
            if case .masked(let mask) = effect {
                try validateTransforms(mask)
            }
        }
    }
}

private func requireIntegerNumbers(_ value: JsonValue) throws {
    switch value {
    case .object(let fields):
        for value in fields.values {
            try requireIntegerNumbers(value)
        }
    case .array(let values):
        for value in values {
            try requireIntegerNumbers(value)
        }
    case .floatNumber:
        throw ParseError("render-tree numbers must use integer JSON tokens")
    case .integer, .string, .boolean, .null:
        break
    }
}

private struct TreeJSONKey: CodingKey, Hashable {
    let stringValue: String
    let intValue: Int? = nil

    init?(stringValue: String) {
        self.stringValue = stringValue
    }

    init?(intValue: Int) {
        return nil
    }
}

private func rejectUnknownKeys(_ decoder: Decoder, allowed: Set<String>) throws {
    let container = try decoder.container(keyedBy: TreeJSONKey.self)
    let unknown = container.allKeys.map(\.stringValue).filter { !allowed.contains($0) }.sorted()
    guard unknown.isEmpty else {
        throw DecodingError.dataCorrupted(.init(
            codingPath: decoder.codingPath,
            debugDescription: "unknown render-tree field(s): \(unknown.joined(separator: ", "))"))
    }
}

private func unknownTag<T>(_ tag: String, _ type: T.Type, decoder: Decoder) -> DecodingError {
    DecodingError.dataCorrupted(.init(
        codingPath: decoder.codingPath,
        debugDescription: "unknown \(String(describing: type)) tag \(tag.debugDescription)"))
}

private extension KeyedDecodingContainer {
    /// Unlike `decodeIfPresent`, this distinguishes a required explicit null
    /// from a missing field so schema drift cannot silently acquire defaults.
    func decodeRequiredOptional<T: Decodable>(_ type: T.Type, forKey key: Key) throws -> T? {
        guard contains(key) else {
            throw DecodingError.keyNotFound(key, .init(
                codingPath: codingPath,
                debugDescription: "required nullable render-tree field is missing"))
        }
        if try decodeNil(forKey: key) {
            return nil
        }
        return try decode(type, forKey: key)
    }
}

extension Q32: Decodable {
    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        self.init(try container.decode(Int64.self))
    }
}

extension Bam16: Decodable {
    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        self.init(try container.decode(UInt16.self))
    }
}

extension Rgb: Decodable {
    private enum CodingKeys: String, CodingKey {
        case b, g, r
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownKeys(decoder, allowed: ["b", "g", "r"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            r: try container.decode(UInt8.self, forKey: .r),
            g: try container.decode(UInt8.self, forKey: .g),
            b: try container.decode(UInt8.self, forKey: .b))
    }
}

extension RenderTree {
    private enum CodingKeys: String, CodingKey {
        case background, canvas, primitives
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownKeys(decoder, allowed: ["background", "canvas", "primitives"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            canvas: try container.decode(UInt32.self, forKey: .canvas),
            background: try container.decodeRequiredOptional(Rgb.self, forKey: .background),
            primitives: try container.decode([Primitive].self, forKey: .primitives))
    }
}

extension Primitive {
    private enum CodingKeys: String, CodingKey {
        case clip, effects, label, opacity, paint, role, shape, stroke, transform
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownKeys(
            decoder,
            allowed: ["clip", "effects", "label", "opacity", "paint", "role", "shape", "stroke", "transform"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            shape: try container.decode(Shape.self, forKey: .shape),
            transform: try container.decodeRequiredOptional(Transform.self, forKey: .transform),
            paint: try container.decode(Paint.self, forKey: .paint),
            stroke: try container.decodeRequiredOptional(Stroke.self, forKey: .stroke),
            opacity: try container.decodeRequiredOptional(Q32.self, forKey: .opacity),
            clip: try container.decodeRequiredOptional(Shape.self, forKey: .clip),
            effects: try container.decode([Effect].self, forKey: .effects),
            label: try container.decode(String.self, forKey: .label),
            role: try container.decodeRequiredOptional(Role.self, forKey: .role))
    }
}

extension Segment {
    private enum CodingKeys: String, CodingKey {
        case cp1x, cp1y, cp2x, cp2y, kind, x, y
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let kind = try container.decode(String.self, forKey: .kind)
        switch kind {
        case "move_to":
            try rejectUnknownKeys(decoder, allowed: ["kind", "x", "y"])
            self = .moveTo(
                x: try container.decode(Int64.self, forKey: .x),
                y: try container.decode(Int64.self, forKey: .y))
        case "cubic_to":
            try rejectUnknownKeys(
                decoder, allowed: ["cp1x", "cp1y", "cp2x", "cp2y", "kind", "x", "y"])
            self = .cubicTo(
                cp1x: try container.decode(Int64.self, forKey: .cp1x),
                cp1y: try container.decode(Int64.self, forKey: .cp1y),
                cp2x: try container.decode(Int64.self, forKey: .cp2x),
                cp2y: try container.decode(Int64.self, forKey: .cp2y),
                x: try container.decode(Int64.self, forKey: .x),
                y: try container.decode(Int64.self, forKey: .y))
        case "line_to":
            try rejectUnknownKeys(decoder, allowed: ["kind", "x", "y"])
            self = .lineTo(
                x: try container.decode(Int64.self, forKey: .x),
                y: try container.decode(Int64.self, forKey: .y))
        default:
            throw unknownTag(kind, Segment.self, decoder: decoder)
        }
    }
}

extension Shape {
    private enum CodingKeys: String, CodingKey {
        case closed, cx, cy, fill_rule, h, kind, r, rx, ry, segments, w, x, y
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let kind = try container.decode(String.self, forKey: .kind)
        switch kind {
        case "path":
            try rejectUnknownKeys(
                decoder, allowed: ["closed", "fill_rule", "kind", "segments"])
            self = .path(
                segments: try container.decode([Segment].self, forKey: .segments),
                closed: try container.decode(Bool.self, forKey: .closed),
                fill_rule: try container.decode(FillRule.self, forKey: .fill_rule))
        case "circle":
            try rejectUnknownKeys(decoder, allowed: ["cx", "cy", "kind", "r"])
            self = .circle(
                cx: try container.decode(Int64.self, forKey: .cx),
                cy: try container.decode(Int64.self, forKey: .cy),
                r: try container.decode(Int64.self, forKey: .r))
        case "ellipse":
            try rejectUnknownKeys(decoder, allowed: ["cx", "cy", "kind", "rx", "ry"])
            self = .ellipse(
                cx: try container.decode(Int64.self, forKey: .cx),
                cy: try container.decode(Int64.self, forKey: .cy),
                rx: try container.decode(Int64.self, forKey: .rx),
                ry: try container.decode(Int64.self, forKey: .ry))
        case "rect":
            try rejectUnknownKeys(decoder, allowed: ["h", "kind", "w", "x", "y"])
            self = .rect(
                x: try container.decode(Int64.self, forKey: .x),
                y: try container.decode(Int64.self, forKey: .y),
                w: try container.decode(Int64.self, forKey: .w),
                h: try container.decode(Int64.self, forKey: .h))
        default:
            throw unknownTag(kind, Shape.self, decoder: decoder)
        }
    }
}

extension GradientStop {
    private enum CodingKeys: String, CodingKey {
        case color, offset_q32
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownKeys(decoder, allowed: ["color", "offset_q32"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            offset_q32: try container.decode(Q32.self, forKey: .offset_q32),
            color: try container.decode(Rgb.self, forKey: .color))
    }
}

extension Paint {
    private enum CodingKeys: String, CodingKey {
        case color, kind, stops
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let kind = try container.decode(String.self, forKey: .kind)
        switch kind {
        case "solid":
            try rejectUnknownKeys(decoder, allowed: ["color", "kind"])
            self = .solid(color: try container.decode(Rgb.self, forKey: .color))
        case "radial_gradient":
            try rejectUnknownKeys(decoder, allowed: ["kind", "stops"])
            self = .radialGradient(stops: try container.decode([GradientStop].self, forKey: .stops))
        case "none":
            try rejectUnknownKeys(decoder, allowed: ["kind"])
            self = .none
        default:
            throw unknownTag(kind, Paint.self, decoder: decoder)
        }
    }
}

extension Stroke {
    private enum CodingKeys: String, CodingKey {
        case cap, color, join, width
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownKeys(decoder, allowed: ["cap", "color", "join", "width"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            color: try container.decode(Rgb.self, forKey: .color),
            width: try container.decode(Int64.self, forKey: .width),
            cap: try container.decode(StrokeCap.self, forKey: .cap),
            join: try container.decode(StrokeJoin.self, forKey: .join))
    }
}

extension Effect {
    private enum CodingKeys: String, CodingKey {
        case freq_h, freq_v, kind, light_y, mask, octaves, scale, seed
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let kind = try container.decode(String.self, forKey: .kind)
        switch kind {
        case "lit":
            try rejectUnknownKeys(decoder, allowed: ["kind", "light_y"])
            self = .lit(lightY: try container.decode(Int64.self, forKey: .light_y))
        case "masked":
            try rejectUnknownKeys(decoder, allowed: ["kind", "mask"])
            self = .masked(mask: try container.decode([Primitive].self, forKey: .mask))
        case "turbulence":
            try rejectUnknownKeys(
                decoder, allowed: ["freq_h", "freq_v", "kind", "octaves", "scale", "seed"])
            self = .turbulence(
                seed: try container.decode(UInt8.self, forKey: .seed),
                freq_h: try container.decode(Q32.self, forKey: .freq_h),
                freq_v: try container.decode(Q32.self, forKey: .freq_v),
                octaves: try container.decode(UInt8.self, forKey: .octaves),
                scale: try container.decode(Q32.self, forKey: .scale))
        default:
            throw unknownTag(kind, Effect.self, decoder: decoder)
        }
    }
}

extension TransformOp {
    private enum CodingKeys: String, CodingKey {
        case angle, cx, cy, kind, sx, sy, tx, ty
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let kind = try container.decode(String.self, forKey: .kind)
        switch kind {
        case "translate":
            try rejectUnknownKeys(decoder, allowed: ["kind", "tx", "ty"])
            self = .translate(
                tx: try container.decode(Int64.self, forKey: .tx),
                ty: try container.decode(Int64.self, forKey: .ty))
        case "rotate":
            try rejectUnknownKeys(decoder, allowed: ["angle", "cx", "cy", "kind"])
            self = .rotate(
                angle: try container.decode(Bam16.self, forKey: .angle),
                cx: try container.decode(Int64.self, forKey: .cx),
                cy: try container.decode(Int64.self, forKey: .cy))
        case "scale":
            try rejectUnknownKeys(decoder, allowed: ["kind", "sx", "sy"])
            self = .scale(
                sx: try container.decode(Q32.self, forKey: .sx),
                sy: try container.decode(Q32.self, forKey: .sy))
        default:
            throw unknownTag(kind, TransformOp.self, decoder: decoder)
        }
    }
}

extension Affine {
    private enum CodingKeys: String, CodingKey {
        case a, b, c, d, tx, ty
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownKeys(decoder, allowed: ["a", "b", "c", "d", "tx", "ty"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            a: try container.decode(Q32.self, forKey: .a),
            b: try container.decode(Q32.self, forKey: .b),
            c: try container.decode(Q32.self, forKey: .c),
            d: try container.decode(Q32.self, forKey: .d),
            tx: try container.decode(Int64.self, forKey: .tx),
            ty: try container.decode(Int64.self, forKey: .ty))
    }
}

extension Transform {
    private enum CodingKeys: String, CodingKey {
        case ops
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownKeys(decoder, allowed: ["ops"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(ops: try container.decode([TransformOp].self, forKey: .ops))
    }
}
