// Pattern-mask geometry transcribed from crates/avatar-svg/src/pattern.rs.
import Foundation

private let Q32_POINT_ONE: Int64 = 429_496_730
private let Q32_POINT_EIGHTY_FIVE: Int64 = 3_650_722_202
private let Q32_TWO_POINT_FIVE: Int64 = 10_737_418_240

func patternMaskPrimitives(
    _ pattern: Pattern, _ bodyWidth: Int64, _ bodyHeight: Int64,
    _ top: Int64, _ rootOps: [TransformOp]
) -> [Primitive]? {
    let shape: Shape
    let translate: (Int64, Int64)
    let scale: Int64
    let angle: Bam16
    let center: Int64
    let label: String
    switch pattern.class {
    case 1:
        shape = circlerShape(pattern.circler, true, bodyHeight, top)
        translate = (0, 0); scale = Q32_ONE; angle = Bam16(0); center = CENTER
        label = "av-pattern-circler"
    case 2:
        shape = circlerShape(pattern.circler, false, bodyHeight, top)
        translate = (0, 0); scale = Q32_ONE; angle = Bam16(0); center = CENTER
        label = "av-pattern-circler"
    case 3:
        let placement = voronoiPlacement(pattern.voronoi, bodyWidth, bodyHeight)
        shape = parseVoronoiPath(VORONOI_PATTERNS[placement.index])
        translate = (placement.tx, placement.ty)
        scale = placement.scale
        angle = pattern.voronoi.rotation
        center = placement.center
        label = "av-pattern-voronoi-\(placement.index)"
    case 4:
        shape = cuttingBezierShape(pattern.cut, bodyHeight, top)
        translate = (0, 0); scale = Q32_ONE; angle = Bam16(0); center = CENTER
        label = "av-pattern-cut"
    case 5:
        shape = cuttingBezierShape(pattern.cut, bodyHeight, top)
        translate = (0, 0); scale = Q32_ONE; angle = Bam16(32_768); center = CENTER
        label = "av-pattern-cut-flipped"
    default:
        return nil
    }
    let transform = Transform(ops: rootOps + [
        .translate(tx: translate.0, ty: translate.1),
        .scale(sx: Q32(scale), sy: Q32(scale)),
        .rotate(angle: angle, cx: center, cy: center),
    ])
    return [Primitive(
        shape: shape, transform: transform,
        paint: .solid(color: Rgb(r: 0xef, g: 0xef, b: 0xef)), label: label)]
}

private func circlerShape(
    _ circler: Circler, _ aboveBody: Bool, _ bodyHeight: Int64, _ top: Int64
) -> Shape {
    let easing = Int64(EASE_IN_QUINT[Int(circler.point_reduce)])
    let reduction = divRoundHalfAway(DoubleWidth.multiply(8, easing), Q32_ONE).toInt64()
    let numberOfPoints = 16 - reduction
    let sizeLow = divI64(6 * bodyHeight, 5)
    let size = lerpQ32Milli(sizeLow, 2 * bodyHeight, circler.size.value)
    let divisor = lerpQ32Milli(2 * Q32_ONE, 3 * Q32_ONE, circler.y_offset_div.value)
    let yOffset = divRoundHalfAway(DoubleWidth.multiply(bodyHeight, Q32_ONE), divisor).toInt64()
    let position = aboveBody ? top - size + yOffset : top + size + yOffset
    let tension = Q32_ONE - divI64(circler.tension.value, 2)
    let pull = lerpQ32Milli(Q32_POINT_EIGHTY_FIVE, Q32_ONE, circler.pull_min.value)
    let radius = q32Mul(size, pull)
    let angleStep = UInt16(truncatingIfNeeded: divI64(65_536, 2 * numberOfPoints))
    var ring: [RingPoint] = []
    let count = Int(2 * numberOfPoints)
    ring.reserveCapacity(count)
    for index in 1...count {
        let angle = UInt16(truncatingIfNeeded: UInt32(index) * UInt32(angleStep) + 16_384)
        ring.append(RingPoint(
            x: CENTER + trigOffset(radius, cosQ30(angle)),
            y: position + trigOffset(radius, sinQ30(angle)),
            tension: tension))
    }
    return buildSpline(ring).shape
}

private func trigOffset(_ radius: Int64, _ trigQ30: Int32) -> Int64 {
    divRoundHalfAway(DoubleWidth.multiply(radius, Int64(trigQ30)), 1 << 30).toInt64()
}

private struct VoronoiPlacement {
    var index: Int
    var tx: Int64
    var ty: Int64
    var scale: Int64
    var center: Int64
}

private func voronoiPlacement(
    _ voronoi: Voronoi, _ bodyWidth: Int64, _ bodyHeight: Int64
) -> VoronoiPlacement {
    let index = Int(divI64(Int64(voronoi.index) * Int64(VORONOI_PATTERNS.count - 1), 255))
    let baseScale = lerpQ32Milli(Q32_POINT_ONE, Q32_TWO_POINT_FIVE, voronoi.scale.value)
    let scale = max(max(baseScale, q32Ratio(bodyWidth)), q32Ratio(bodyHeight))
    let scaledCanvas = q32Mul(CANVAS_MILLI, scale)
    let tx = divRoundHalfAway(
        DoubleWidth.multiply(CANVAS_MILLI - scaledCanvas, Q32_ONE), 2 * scale).toInt64()
    let ty = tx - divI64(scaledCanvas, 2)
    let strokeUnit = divI64(CANVAS_MILLI, 255)
    let finalScale = q32Mul(scale, strokeUnit)
    let center = divRoundHalfAway(
        DoubleWidth.multiply(CANVAS_MILLI, Q32_ONE), 2 * finalScale).toInt64()
    return VoronoiPlacement(index: index, tx: tx, ty: ty, scale: finalScale, center: center)
}

private func cuttingBezierShape(_ cut: Cut, _ bodyHeight: Int64, _ top: Int64) -> Shape {
    let spread = lerpQ32Milli(Q32_ONE / 2, 10 * Q32_ONE, cut.spread.value)
    let leftRight = q32Mul(CANVAS_MILLI, spread)
    let divisor = lerpQ32Milli(2 * Q32_ONE, 100 * Q32_ONE, cut.y_adjust_div.value)
    let yAdjust = divRoundHalfAway(DoubleWidth.multiply(bodyHeight, Q32_ONE), divisor).toInt64()
    let bottom = top + bodyHeight + yAdjust
    let upper = top + yAdjust
    let middle = top + leftRight + yAdjust
    return .path(segments: [
        .moveTo(x: 0, y: bottom),
        .cubicTo(
            cp1x: CENTER - leftRight, cp1y: bottom,
            cp2x: CENTER, cp2y: middle, x: CENTER, y: upper),
        .cubicTo(
            cp1x: CENTER, cp1y: middle,
            cp2x: CENTER + leftRight, cp2y: bottom, x: CANVAS_MILLI, y: bottom),
    ], closed: true, fill_rule: .nonZero)
}

/// The vendored paths use only relative m/c/s commands. Parsing them gives
/// geometry consumers absolute MoveTo/CubicTo segments; the serializer uses
/// the pinned source spelling for the matching pattern label.
private func parseVoronoiPath(_ source: String) -> Shape {
    let tokens = SvgPathTokens(source)
    var index = 0
    var command: Character = "m"
    var current = (x: Int64(0), y: Int64(0))
    var previousControl = current
    var segments: [Segment] = []
    var firstMove = true
    while index < tokens.values.count {
        if case .command(let next) = tokens.values[index] {
            command = next
            index += 1
            firstMove = command == "m"
            continue
        }
        switch command {
        case "m":
            guard let x = tokens.number(at: index), let y = tokens.number(at: index + 1) else { break }
            current = (current.x + x, current.y + y)
            segments.append(firstMove ? .moveTo(x: current.x, y: current.y) : .lineTo(x: current.x, y: current.y))
            previousControl = current
            firstMove = false
            index += 2
        case "c":
            guard let x1 = tokens.number(at: index), let y1 = tokens.number(at: index + 1),
                  let x2 = tokens.number(at: index + 2), let y2 = tokens.number(at: index + 3),
                  let x = tokens.number(at: index + 4), let y = tokens.number(at: index + 5)
            else { index = tokens.values.count; continue }
            let control1 = (current.x + x1, current.y + y1)
            let control2 = (current.x + x2, current.y + y2)
            current = (current.x + x, current.y + y)
            segments.append(.cubicTo(
                cp1x: control1.0, cp1y: control1.1,
                cp2x: control2.0, cp2y: control2.1, x: current.x, y: current.y))
            previousControl = control2
            index += 6
        case "s":
            guard let x2 = tokens.number(at: index), let y2 = tokens.number(at: index + 1),
                  let x = tokens.number(at: index + 2), let y = tokens.number(at: index + 3)
            else { index = tokens.values.count; continue }
            let control1 = (2 * current.x - previousControl.x, 2 * current.y - previousControl.y)
            let control2 = (current.x + x2, current.y + y2)
            current = (current.x + x, current.y + y)
            segments.append(.cubicTo(
                cp1x: control1.0, cp1y: control1.1,
                cp2x: control2.0, cp2y: control2.1, x: current.x, y: current.y))
            previousControl = control2
            index += 4
        default:
            index += 1
        }
    }
    return .path(segments: segments, closed: false, fill_rule: .nonZero)
}

private struct SvgPathTokens {
    enum Value { case command(Character), number(Int64) }
    var values: [Value] = []

    init(_ source: String) {
        let bytes = Array(source.utf8)
        var index = 0
        while index < bytes.count {
            let byte = bytes[index]
            if byte == 109 || byte == 99 || byte == 115 {
                values.append(.command(Character(UnicodeScalar(byte))))
                index += 1
            } else if byte == 32 || byte == 44 {
                index += 1
            } else {
                let start = index
                if byte == 45 || byte == 43 { index += 1 }
                while index < bytes.count && bytes[index] >= 48 && bytes[index] <= 57 { index += 1 }
                if index < bytes.count && bytes[index] == 46 {
                    index += 1
                    while index < bytes.count && bytes[index] >= 48 && bytes[index] <= 57 { index += 1 }
                }
                let text = String(decoding: bytes[start..<index], as: UTF8.self)
                values.append(.number(decimalMilli(text)))
            }
        }
    }

    func number(at index: Int) -> Int64? {
        guard index < values.count, case .number(let value) = values[index] else { return nil }
        return value
    }
}

private func decimalMilli(_ text: String) -> Int64 {
    let negative = text.first == "-"
    let unsigned = text.trimmingCharacters(in: CharacterSet(charactersIn: "+-"))
    let pieces = unsigned.split(separator: ".", omittingEmptySubsequences: false)
    let whole = Int64(pieces[0]) ?? 0
    let fractionText = pieces.count > 1 ? String(pieces[1]).padding(toLength: 2, withPad: "0", startingAt: 0) : "00"
    let fraction = Int64(fractionText.prefix(2)) ?? 0
    let value = whole * 100 + fraction
    return negative ? -value : value
}
