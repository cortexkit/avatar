// Byte-stable SVG serialization ported from crates/avatar-svg/src/{svg,format}.rs.
import Foundation

/// Serializes only information carried by the render tree. Repeated body
/// geometry is emitted once and referenced with `<use>` elements to preserve
/// the corpus's compact, byte-stable SVG representation.
public func serializeSvg(_ tree: RenderTree) -> [UInt8] {
    let body = tree.primitives.filter { $0.label.hasPrefix("av-body-") }
    guard let base = body.first, let root = base.transform else { return [] }
    let rootOps = root.ops
    let rootValues = rootPlacement(rootOps)
    var output = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 \(tree.canvas) \(tree.canvas)\">\n"
    if let background = tree.background {
        output += "<rect width=\"\(tree.canvas)\" height=\"\(tree.canvas)\" fill=\"\(hex(background))\"/>\n"
    }
    output += "<g transform=\"translate(\(formatMilli(rootValues.tx)),\(formatMilli(rootValues.ty))) scale(\(formatQ32Decimal(rootValues.sx)), \(formatQ32Decimal(rootValues.sy)))\">\n"
    output += "<defs>\n"
    if let pattern = body.first(where: { $0.label == "av-body-pattern" }),
       let mask = maskedPrimitives(pattern.effects)
    {
        output += "  <mask id=\"av-pattern\">\(serializeMask(mask, dropping: rootOps))</mask>\n"
    }
    if let gradientPrimitive = body.first(where: { $0.label == "av-body-gradient" }),
       case .radialGradient(let stops) = gradientPrimitive.paint,
       stops.count >= 2
    {
        output += "  <radialGradient id=\"av-gradox\" cx=\"50%\" cy=\"50%\" r=\"50%\" fx=\"50%\" fy=\"50%\" spreadMethod=\"pad\">\n"
        output += "    <stop offset=\"89%\" stop-color=\"\(hex(stops[0].color))\"/><stop offset=\"100%\" stop-color=\"\(hex(stops[1].color))\"/>\n"
        output += "  </radialGradient>\n"
    }
    // The resolved light y rides the lit effect; falling back to the shape
    // re-derivation only for trees built without one (hand-built tests).
    let lightY = body.compactMap { primitive -> Int64? in
        for effect in primitive.effects {
            if case .lit(let y) = effect { return y }
        }
        return nil
    }.first ?? bodyLightY(base.shape)
    output += "  <filter id=\"av-light\">\n"
    output += "    <feDiffuseLighting in=\"SourceGraphic\" result=\"light\" lighting-color=\"white\">\n"
    output += "      <fePointLight x=\"\(formatMilli(CENTER))\" y=\"\(formatMilli(lightY))\" z=\"\(formatMilli(divI64(CANVAS_MILLI, 21)))\"/>\n"
    output += "    </feDiffuseLighting>\n"
    output += "    <feComposite in=\"SourceGraphic\" in2=\"light\" operator=\"arithmetic\" k1=\"0.9\" k2=\"1.1\" k3=\"0\" k4=\"0\"/>\n"
    output += "  </filter>\n"
    output += "  <path id=\"av-bp\" d=\"\(pathData(base.shape, label: base.label))\"/>\n"
    output += "</defs>\n"
    output += serializeBody(body)

    let face = tree.primitives.filter { !$0.label.hasPrefix("av-body-") }
    if !face.isEmpty {
        output += serializeFace(face, dropping: rootOps)
    }
    output += "</g>\n</svg>\n"
    return Array(output.utf8)
}

private func serializeBody(_ body: [Primitive]) -> String {
    guard body.count >= 3,
          let baseStroke = body[0].stroke,
          case .solid(let baseFill) = body[0].paint,
          case .solid(let litFill) = body[1].paint
    else { return "" }
    var output = "<use href=\"#av-bp\" stroke-width=\"\(formatMilli(baseStroke.width))\" stroke=\"\(hex(baseStroke.color))\" fill=\"\(hex(baseFill))\"/>\n"
    output += "<g filter=\"url(#av-light)\">\n"
    output += "  <use href=\"#av-bp\" fill=\"\(hex(litFill))\"/>\n"
    output += "  <use href=\"#av-bp\" fill=\"url(#av-gradox)\"/>\n"
    if body.count > 3, case .solid(let patternFill) = body[3].paint {
        output += "  <use href=\"#av-bp\" fill=\"\(hex(patternFill))\" mask=\"url(#av-pattern)\"/>\n"
    }
    output += "</g>\n"
    return output
}

private func serializeMask(_ primitives: [Primitive], dropping rootOps: [TransformOp]) -> String {
    guard let primitive = primitives.first else { return "" }
    let local = Array((primitive.transform?.ops ?? []).dropFirst(rootOps.count))
    var tx: Int64 = 0
    var ty: Int64 = 0
    var sx = Q32.one
    var sy = Q32.one
    var angle = Bam16(0)
    var cx = CENTER
    var cy = CENTER
    for op in local {
        switch op {
        case .translate(let x, let y): tx = x; ty = y
        case .scale(let x, let y): sx = x; sy = y
        case .rotate(let value, let x, let y): angle = value; cx = x; cy = y
        }
    }
    let degrees: String
    if primitive.label.hasPrefix("av-pattern-voronoi-") {
        degrees = String(divI64(Int64(angle.value) * 360, 65_536))
    } else if primitive.label == "av-pattern-cut-flipped" {
        degrees = "180"
    } else {
        degrees = "0"
    }
    return "<g><g transform=\"translate(\(formatMilli(tx)), \(formatMilli(ty)))\"><g transform=\"scale(\(formatQ32Decimal(sx.value)) \(formatQ32Decimal(sy.value))) rotate(\(degrees) \(formatMilli(cx)) \(formatMilli(cy)))\"><path d=\"\(pathData(primitive.shape, label: primitive.label))\" fill=\"\(paintValue(primitive.paint))\"/></g></g></g>"
}

private func serializeFace(_ primitives: [Primitive], dropping rootOps: [TransformOp]) -> String {
    var output = "<g id=\"av-face\">\n"
    let eyeballs = primitives.filter { $0.label == "av-eyeball" }
    guard eyeballs.count == 2,
          let shade = primitives.first(where: { $0.label == "av-iris-shade" }),
          let turbulence = turbulenceEffect(shade.effects)
    else { return output + "</g>\n" }
    let left = eyeballs[0]
    let right = eyeballs[1]
    output += "  <defs>\n"
    output += "    <clipPath id=\"av-eye-left-clip\"><path d=\"\(pathData(left.shape, label: "av-eye-clip"))\"/></clipPath>\n"
    output += "    <clipPath id=\"av-eye-right-clip\"><path d=\"\(pathData(right.shape, label: "av-eye-clip"))\"/></clipPath>\n"
    output += "    <filter id=\"av-eye-noise\"><feTurbulence type=\"fractalNoise\" stitchTiles=\"stitch\" baseFrequency=\"\(formatQ32Decimal(turbulence.freqH.value)) \(formatQ32Decimal(turbulence.freqV.value))\" numOctaves=\"\(turbulence.octaves)\" seed=\"\(turbulence.seed)\" result=\"warp\"/><feDisplacementMap xChannelSelector=\"R\" yChannelSelector=\"G\" scale=\"\(formatMilli(q32Mul(100, turbulence.scale.value)))\" in=\"SourceGraphic\" in2=\"warp\"/></filter>\n"
    output += "  </defs>\n"
    if let mouth = primitives.first(where: { $0.label.hasPrefix("av-mouth-") }) {
        output += serializeMouth(mouth)
    }
    output += serializeEye(.left, primitives, dropping: rootOps)
    output += serializeEye(.right, primitives, dropping: rootOps)
    for brow in primitives.filter({ $0.label.hasPrefix("av-brow-") }) {
        output += serializeBrow(brow, dropping: rootOps)
    }
    output += "</g>\n"
    return output
}

private func serializeMouth(_ primitive: Primitive) -> String {
    let family = primitive.label.split(separator: "-").last ?? "0"
    guard let stroke = primitive.stroke else { return "" }
    return "  <g id=\"av-mouth\" data-family=\"\(family)\"><path d=\"\(pathData(primitive.shape, label: primitive.label))\" fill=\"\(paintValue(primitive.paint))\" stroke=\"\(hex(stroke.color))\" stroke-width=\"\(formatMilli(stroke.width))\" stroke-linejoin=\"round\"/></g>\n"
}

private func serializeEye(
    _ side: EyeSide, _ primitives: [Primitive], dropping rootOps: [TransformOp]
) -> String {
    let eyeballs = primitives.filter { $0.label == "av-eyeball" }
    let eyeIndex = side == .left ? 0 : 1
    guard eyeballs.indices.contains(eyeIndex) else { return "" }
    let first = eyeballs[eyeIndex]
    let eye = primitives.filter { primitive in
        primitive.transform == first.transform && [
            "av-eyeball", "av-iris", "av-iris-shade", "av-pupil",
            "av-highlight", "av-upper-lid", "av-lower-lid", "av-lash",
        ].contains(primitive.label)
    }
    let local = Array((first.transform?.ops ?? []).dropFirst(rootOps.count))
    guard case .rotate(let angle, let cx, let cy)? = local.last else { return "" }
    var output = "  <g id=\"av-eye-\(side.rawValue)\" transform=\"rotate(\(formatQ32Decimal(bamDegreesQ32(angle))),\(formatMilli(cx)),\(formatMilli(cy)))\">\n"
    if let eyeball = eye.first(where: { $0.label == "av-eyeball" }) {
        output += "    <path class=\"av-eyeball\" d=\"\(pathData(eyeball.shape, label: eyeball.label))\" fill=\"\(paintValue(eyeball.paint))\" stroke=\"\(hex(eyeball.stroke!.color))\" stroke-width=\"\(formatMilli(eyeball.stroke!.width))\"/>\n"
    }
    output += "    <g clip-path=\"url(#av-eye-\(side.rawValue)-clip)\">\n"
    for primitive in eye {
        if primitive.label == "av-iris" {
            if case .circle(let cx, let cy, let radius) = primitive.shape, let stroke = primitive.stroke {
                output += "      <circle class=\"av-iris\" cx=\"\(formatMilli(cx))\" cy=\"\(formatMilli(cy))\" r=\"\(formatMilli(radius))\" fill=\"\(paintValue(primitive.paint))\" stroke=\"\(hex(stroke.color))\" stroke-width=\"\(formatMilli(stroke.width))\"/>\n"
            }
        } else if primitive.label == "av-iris-shade" {
            if case .ellipse(let cx, let cy, let rx, let ry) = primitive.shape {
                output += "      <ellipse class=\"av-iris-shade\" cx=\"\(formatMilli(cx))\" cy=\"\(formatMilli(cy))\" rx=\"\(formatMilli(rx))\" ry=\"\(formatMilli(ry))\" fill=\"\(paintValue(primitive.paint))\" opacity=\"\(formatQ32Decimal(primitive.opacity?.value ?? Q32_ONE))\" filter=\"url(#av-eye-noise)\"/>\n"
            }
        } else if primitive.label == "av-pupil" {
            if case .circle(let cx, let cy, let radius) = primitive.shape {
                output += "      <circle class=\"av-pupil\" cx=\"\(formatMilli(cx))\" cy=\"\(formatMilli(cy))\" r=\"\(formatMilli(radius))\" fill=\"\(paintValue(primitive.paint))\"/>\n"
            }
        } else if primitive.label == "av-highlight" {
            if case .circle(let cx, let cy, let radius) = primitive.shape {
                output += "      <circle class=\"av-highlight\" cx=\"\(formatMilli(cx))\" cy=\"\(formatMilli(cy))\" r=\"\(formatMilli(radius))\" fill=\"\(paintValue(primitive.paint))\" opacity=\"\(formatQ32Decimal(primitive.opacity?.value ?? Q32_ONE))\"/>\n"
            }
        }
    }
    output += "    </g>\n"
    for primitive in eye {
        if primitive.label == "av-upper-lid" || primitive.label == "av-lower-lid" {
            let cssClass = primitive.label.hasPrefix("av-upper") ? "av-upper-lid" : "av-lower-lid"
            let stroke = primitive.stroke!
            output += "    <path class=\"\(cssClass)\" d=\"\(pathData(primitive.shape, label: primitive.label))\" fill=\"\(paintValue(primitive.paint))\" stroke=\"\(hex(stroke.color))\" stroke-width=\"\(formatMilli(stroke.width))\"/>\n"
        } else if primitive.label == "av-lash" {
            let stroke = primitive.stroke!
            output += "    <path class=\"av-lash\" d=\"\(pathData(primitive.shape, label: primitive.label))\" stroke=\"\(hex(stroke.color))\" stroke-width=\"\(formatMilli(stroke.width))\" stroke-linecap=\"round\" fill=\"\(paintValue(primitive.paint))\"/>\n"
        }
    }
    output += "  </g>\n"
    return output
}

private func serializeBrow(_ primitive: Primitive, dropping rootOps: [TransformOp]) -> String {
    let side = primitive.label.hasSuffix("left") ? "left" : "right"
    let local = Array((primitive.transform?.ops ?? []).dropFirst(rootOps.count))
    var tx: Int64 = 0
    var ty: Int64 = 0
    var angle = Bam16(0)
    var cx: Int64 = 0
    var cy: Int64 = 0
    for op in local {
        switch op {
        case .translate(let x, let y): tx = x; ty = y
        case .rotate(let value, let x, let y): angle = value; cx = x; cy = y
        case .scale: break
        }
    }
    return "  <path id=\"av-brow-\(side)\" d=\"\(pathData(primitive.shape, label: primitive.label))\" transform=\"translate(\(formatMilli(tx)),\(formatMilli(ty))) rotate(\(formatQ32Decimal(bamDegreesQ32(angle))),\(formatMilli(cx)),\(formatMilli(cy)))\" fill=\"\(paintValue(primitive.paint))\"/>\n"
}

private func pathData(_ shape: Shape, label: String) -> String {
    if label.hasPrefix("av-pattern-voronoi-"),
       let index = Int(label.split(separator: "-").last ?? "")
    {
        return VORONOI_PATTERNS[index]
    }
    guard case .path(let segments, let closed, _) = shape else { return "" }
    if label == "av-pattern-cut" || label == "av-pattern-cut-flipped" {
        guard segments.count == 3,
              case .moveTo(_, let yBottom) = segments[0],
              case .cubicTo(let a, let b, let c, let d, let e, let f) = segments[1],
              case .cubicTo(let g, let h, let i, let j, let k, let l) = segments[2]
        else { return "" }
        return "M 0, \(formatMilli(yBottom)) C \(formatMilli(a)) \(formatMilli(b)), \(formatMilli(c)) \(formatMilli(d)), \(formatMilli(e)) \(formatMilli(f))C \(formatMilli(g)) \(formatMilli(h)), \(formatMilli(i)) \(formatMilli(j)), \(formatMilli(k)) \(formatMilli(l))Z"
    }
    let compact = label.hasPrefix("av-body-") || label.hasPrefix("av-brow-")
        || label == "av-pattern-circler"
    let lash = label == "av-lash"
    var output = ""
    for segment in segments {
        switch segment {
        case .moveTo(let x, let y):
            output += compact ? "M\(formatMilli(x)),\(formatMilli(y))" : "M\(formatMilli(x)) \(formatMilli(y))"
        case .lineTo(let x, let y):
            output += compact ? "L\(formatMilli(x)),\(formatMilli(y))" : " L\(formatMilli(x)) \(formatMilli(y))"
        case .cubicTo(let x1, let y1, let x2, let y2, let x, let y):
            if compact {
                output += "C\(formatMilli(x1)) \(formatMilli(y1)), \(formatMilli(x2)) \(formatMilli(y2)), \(formatMilli(x)) \(formatMilli(y))"
            } else if lash {
                output += " C\(formatMilli(x1)) \(formatMilli(y1)), \(formatMilli(x2)) \(formatMilli(y2)), \(formatMilli(x)) \(formatMilli(y))"
            } else {
                output += " C\(formatMilli(x1)) \(formatMilli(y1)),\(formatMilli(x2)) \(formatMilli(y2)),\(formatMilli(x)) \(formatMilli(y))"
            }
        }
    }
    if closed { output += " Z" }
    return output
}

private func bodyLightY(_ shape: Shape) -> Int64 {
    guard let spline = splineFromShape(shape) else { return CENTER }
    let bounds = flatten(spline)
    return bounds.min_y + divI64(bounds.max_y - bounds.min_y, 2)
}

private func splineFromShape(_ shape: Shape) -> Spline? {
    guard case .path(let segments, _, _) = shape,
          case .moveTo(let startX, let startY)? = segments.first
    else { return nil }
    var curves: [Curve] = []
    for segment in segments.dropFirst() {
        if case .cubicTo(let x1, let y1, let x2, let y2, let x, let y) = segment {
            curves.append(Curve(cp1x: x1, cp1y: y1, cp2x: x2, cp2y: y2, x2: x, y2: y))
        }
    }
    return Spline(start_x: startX, start_y: startY, curves: curves)
}

private func maskedPrimitives(_ effects: [Effect]) -> [Primitive]? {
    for effect in effects { if case .masked(let mask) = effect { return mask } }
    return nil
}

private struct TurbulenceValues {
    var seed: UInt8
    var freqH: Q32
    var freqV: Q32
    var octaves: UInt8
    var scale: Q32
}

private func turbulenceEffect(_ effects: [Effect]) -> TurbulenceValues? {
    for effect in effects {
        if case .turbulence(let seed, let freqH, let freqV, let octaves, let scale) = effect {
            return TurbulenceValues(seed: seed, freqH: freqH, freqV: freqV, octaves: octaves, scale: scale)
        }
    }
    return nil
}

private func rootPlacement(_ ops: [TransformOp]) -> (tx: Int64, ty: Int64, sx: Int64, sy: Int64) {
    var tx: Int64 = 0
    var ty: Int64 = 0
    var sx = Q32_ONE
    var sy = Q32_ONE
    for op in ops {
        switch op {
        case .translate(let x, let y): tx = x; ty = y
        case .scale(let x, let y): sx = x.value; sy = y.value
        case .rotate: break
        }
    }
    return (tx, ty, sx, sy)
}

func formatMilli(_ value: Int64) -> String {
    if value == 0 { return "0" }
    let magnitude = value.magnitude
    let whole = magnitude / 100
    let remainder = magnitude % 100
    var text = value < 0 ? "-\(whole)" : "\(whole)"
    if remainder != 0 {
        text += remainder % 10 == 0 ? ".\(remainder / 10)" : String(format: ".%02d", remainder)
    }
    return text
}

func formatQ32Decimal(_ value: Int64) -> String {
    let magnitude = value.magnitude
    let whole = magnitude >> 32
    var fraction = magnitude & ((1 << 32) - 1)
    var digits: [UInt8] = []
    for _ in 0..<6 {
        fraction *= 10
        digits.append(UInt8(fraction >> 32))
        fraction &= (1 << 32) - 1
    }
    while digits.last == 0 { digits.removeLast() }
    var text = value < 0 && (whole != 0 || !digits.isEmpty) ? "-\(whole)" : "\(whole)"
    if !digits.isEmpty {
        text += "." + String(decoding: digits.map { $0 + 48 }, as: UTF8.self)
    }
    return text
}

private func paintValue(_ paint: Paint) -> String {
    switch paint {
    case .solid(let color): return hex(color)
    case .radialGradient: return "url(#av-gradox)"
    case .none: return "none"
    }
}

private func hex(_ color: Rgb) -> String {
    let digits = Array("0123456789abcdef".utf8)
    let bytes = [color.r, color.g, color.b]
    var result = "#"
    for byte in bytes {
        result.append(Character(UnicodeScalar(digits[Int(byte >> 4)])))
        result.append(Character(UnicodeScalar(digits[Int(byte & 15)])))
    }
    return result
}
