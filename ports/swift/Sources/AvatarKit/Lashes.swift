// Lash sampling transcribed from crates/avatar-svg/src/lashes.rs.

func lashSampleCount(_ count: UInt8, _ removed: UInt8) -> Int {
    count == 0 || removed > count ? 0 : Int(min(count - removed, 10)) + 1
}

func lashPrimitives(
    _ genes: Lashes, _ curve: Cubic, _ eyeHeight: Int64, _ side: EyeSide,
    _ color: Rgb, _ transform: Transform
) -> [Primitive] {
    let samples = lashSampleCount(genes.count, genes.removed)
    if samples == 0 { return [] }
    let length = max(q32Mul(eyeHeight, genes.length_scale.value), 0)
    let stroke = max(q32Mul(eyeHeight, genes.stroke_scale.value), 1)
    let angularOffset = lashAngleOffset(genes.direction)
    var result: [Primitive] = []
    result.reserveCapacity(samples)
    for index in 0..<samples {
        let point = cubicPoint(curve, Int64(index), Int64(genes.count))
        let tangent = cubicTangent(curve, Int64(index), Int64(genes.count))
        let tangentAngle = atan2Bam(tangent.1, tangent.0)
        let angle = side == .left
            ? tangentAngle &- angularOffset
            : tangentAngle &+ angularOffset
        let endX = point.0 + trigLength(length, cosQ30(angle))
        let endY = point.1 + trigLength(length, sinQ30(angle))
        let sign: Int64 = side == .left ? -1 : 1
        let shape = Shape.path(segments: [
            .moveTo(x: point.0, y: point.1),
            .cubicTo(
                cp1x: point.0 + sign * stroke * 3, cp1y: point.1 - stroke,
                cp2x: endX + sign * stroke, cp2y: endY - stroke,
                x: endX + sign * stroke * 2, y: endY - stroke * 2),
        ], closed: false, fill_rule: .nonZero)
        result.append(Primitive(
            shape: shape, transform: transform, paint: .solid(color: color),
            stroke: Stroke(color: color, width: stroke, cap: .round),
            label: "av-lash"))
    }
    return result
}

private func lashAngleOffset(_ direction: UInt8) -> UInt16 {
    let fraction = divRoundHalfAway(
        DoubleWidth.multiply(Int64(direction), Q32_ONE), 255).toInt64()
    let divisor = Q32_ONE + fraction
    return UInt16(truncatingIfNeeded: divRoundHalfAway(
        DoubleWidth.multiply(32_768, Q32_ONE), divisor).toInt64())
}

private func cubicPoint(
    _ curve: Cubic, _ numerator: Int64, _ denominator: Int64
) -> (Int64, Int64) {
    let p01 = lerpPoint((curve.x0, curve.y0), (curve.cp1x, curve.cp1y), numerator, denominator)
    let p12 = lerpPoint((curve.cp1x, curve.cp1y), (curve.cp2x, curve.cp2y), numerator, denominator)
    let p23 = lerpPoint((curve.cp2x, curve.cp2y), (curve.x3, curve.y3), numerator, denominator)
    let p012 = lerpPoint(p01, p12, numerator, denominator)
    let p123 = lerpPoint(p12, p23, numerator, denominator)
    return lerpPoint(p012, p123, numerator, denominator)
}

private func cubicTangent(
    _ curve: Cubic, _ numerator: Int64, _ denominator: Int64
) -> (Int64, Int64) {
    let d0 = (3 * (curve.cp1x - curve.x0), 3 * (curve.cp1y - curve.y0))
    let d1 = (3 * (curve.cp2x - curve.cp1x), 3 * (curve.cp2y - curve.cp1y))
    let d2 = (3 * (curve.x3 - curve.cp2x), 3 * (curve.y3 - curve.cp2y))
    let d01 = lerpPoint(d0, d1, numerator, denominator)
    let d12 = lerpPoint(d1, d2, numerator, denominator)
    return lerpPoint(d01, d12, numerator, denominator)
}

private func lerpPoint(
    _ a: (Int64, Int64), _ b: (Int64, Int64), _ numerator: Int64, _ denominator: Int64
) -> (Int64, Int64) {
    (a.0 + ratioMul(b.0 - a.0, numerator, denominator),
     a.1 + ratioMul(b.1 - a.1, numerator, denominator))
}

private func ratioMul(_ value: Int64, _ numerator: Int64, _ denominator: Int64) -> Int64 {
    denominator == 0 ? 0 : divRoundHalfAway(DoubleWidth.multiply(value, numerator), denominator).toInt64()
}

private func atan2Bam(_ y: Int64, _ x: Int64) -> UInt16 {
    if x == 0 && y == 0 { return 0 }
    var bestAngle: UInt16 = 0
    var bestDot = Int64.min
    var raw = 0
    while raw < 65_536 {
        let angle = UInt16(raw)
        let dot = x * Int64(cosQ30(angle)) + y * Int64(sinQ30(angle))
        if dot > bestDot {
            bestDot = dot
            bestAngle = angle
        }
        raw += 4
    }
    return bestAngle
}

private func trigLength(_ length: Int64, _ trigQ30: Int32) -> Int64 {
    divRoundHalfAway(DoubleWidth.multiply(length, Int64(trigQ30)), 1 << 30).toInt64()
}
