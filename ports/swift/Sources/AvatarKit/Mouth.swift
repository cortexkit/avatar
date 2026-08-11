// Eight mouth families transcribed from crates/avatar-svg/src/mouth.rs.

private struct MouthShape {
    var shape: Shape
    var fill: Paint
    var stroke: Rgb
}

private struct MouthMetrics {
    var rect: MouthRect
    var width: Int64
    var height: Int64
    var center_x: Int64
    var center_y: Int64

    init(_ rect: MouthRect) {
        self.rect = rect
        width = max(rect.width, 1)
        height = max(rect.height, 1)
        center_x = divI64(rect.x1 + rect.x2, 2)
        center_y = divI64(rect.y1 + rect.y2, 2)
    }

    func x(_ factor: Int64) -> Int64 {
        rect.x1 + q32Mul(width, min(max(factor, 0), Q32_ONE))
    }

    func y(_ factor: Int64) -> Int64 {
        rect.y1 + q32Mul(height, min(max(factor, 0), Q32_ONE))
    }

    func clamp(_ point: (Int64, Int64)) -> (Int64, Int64) {
        (min(max(point.0, rect.x1), rect.x2), min(max(point.1, rect.y1), rect.y2))
    }
}

func mouthPrimitive(
    _ mouth: Mouth, _ rect: MouthRect, _ mood: MoodDeltas,
    _ palette: Palette, _ transform: Transform
) -> Primitive {
    let shape = mouthShape(mouth, rect, mood, palette)
    return Primitive(
        shape: shape.shape, transform: transform, paint: shape.fill,
        stroke: Stroke(color: shape.stroke, width: 196, join: .round),
        label: "av-mouth-\(mouth.family % 8)")
}

private func mouthShape(
    _ mouth: Mouth, _ rect: MouthRect, _ mood: MoodDeltas, _ palette: Palette
) -> MouthShape {
    let metrics = MouthMetrics(rect)
    switch mouth.family % 8 {
    case 0: return mouth1(mouth, metrics, mood, palette)
    case 1: return mouth3(mouth, metrics, mood, palette)
    case 2: return mouth4(mouth, metrics, mood, palette)
    case 3: return mouth5(mouth, metrics, mood, palette)
    case 4: return mouth6(mouth, metrics, mood, palette)
    case 5: return mouth7(mouth, metrics, mood, palette)
    case 6: return mouth23(mouth, metrics, mood, palette)
    default: return mouth233(mouth, metrics, mood, palette)
    }
}

private func mouth1(_ mouth: Mouth, _ m: MouthMetrics, _ mood: MoodDeltas, _ palette: Palette) -> MouthShape {
    let baseline = m.y(1_431_655_765 + q32Mul(1_431_655_765, parameter(mouth, 0)))
    let neutral = q32Mul(divI64(m.height, 3), parameter(mouth, 1) - divI64(Q32_ONE, 2))
    let moodShift = q32Mul(divI64(m.height, 4), mood.mouth_curve.value)
    let controlY = min(max(baseline + neutral + moodShift, m.rect.y1), m.rect.y2)
    return openShape([
        (m.rect.x1, baseline),
        (m.x(858_993_459 + q32Mul(1_431_655_765, parameter(mouth, 2))), controlY),
        (m.x(2_435_496_789 + q32Mul(1_001_390_507, parameter(mouth, 3))), controlY),
        (m.rect.x2, baseline),
    ], .none, palette.dark)
}

private func mouth3(_ mouth: Mouth, _ m: MouthMetrics, _ mood: MoodDeltas, _ palette: Palette) -> MouthShape {
    let baseline = m.y(1_288_490_189 + q32Mul(1_717_986_918, parameter(mouth, 0)))
    let neutral = q32Mul(divI64(m.height, 3), parameter(mouth, 1) - divI64(Q32_ONE, 2))
    let moodShift = q32Mul(divI64(m.height, 3), mood.mouth_curve.value)
    let skew = q32Mul(divI64(m.height, 6), parameter(mouth, 2) - divI64(Q32_ONE, 2))
    let spread = 1_073_741_824 + q32Mul(1_073_741_824, parameter(mouth, 3))
    return openShape([
        (m.rect.x1, baseline),
        (m.x(spread), min(max(baseline + neutral + moodShift + skew, m.rect.y1), m.rect.y2)),
        (m.x(Q32_ONE - spread), min(max(baseline + neutral + moodShift - skew, m.rect.y1), m.rect.y2)),
        (m.rect.x2, baseline),
    ], .none, palette.primary_darker)
}

private func mouth4(_ mouth: Mouth, _ m: MouthMetrics, _ mood: MoodDeltas, _ palette: Palette) -> MouthShape {
    let top = m.center_y - q32Mul(divI64(m.height, 3), 1_073_741_824 + q32Mul(1_073_741_824, parameter(mouth, 0)))
    let bottom = m.center_y + q32Mul(divI64(m.height, 3), 1_073_741_824 + q32Mul(1_073_741_824, parameter(mouth, 1)))
    let slant = q32Mul(divI64(m.height, 5), parameter(mouth, 2) - divI64(Q32_ONE, 2))
    let curve = q32Mul(divI64(m.height, 5), mood.mouth_curve.value)
    let inset = 644_245_094 + q32Mul(644_245_094, parameter(mouth, 3))
    return closedShape([
        m.clamp((m.rect.x1, m.center_y + slant)),
        m.clamp((m.x(inset), top + curve)),
        m.clamp((m.x(Q32_ONE - inset), top - curve)),
        m.clamp((m.rect.x2, m.center_y - slant)),
        m.clamp((m.x(Q32_ONE - inset), bottom - curve)),
        m.clamp((m.x(inset), bottom + curve)),
    ], .solid(color: palette.primary_darker), palette.dark)
}

private func mouth5(_ mouth: Mouth, _ m: MouthMetrics, _ mood: MoodDeltas, _ palette: Palette) -> MouthShape {
    let opening = divI64(m.height, 8) + q32Mul(divI64(m.height, 4), parameter(mouth, 0))
    let upperCrown = q32Mul(divI64(m.height, 6), parameter(mouth, 1))
    let lowerCrown = q32Mul(divI64(m.height, 6), parameter(mouth, 2))
    let curve = q32Mul(divI64(m.height, 6), mood.mouth_curve.value)
    let inset = 429_496_730 + q32Mul(429_496_730, parameter(mouth, 3))
    return closedShape([
        (m.x(inset), m.center_y),
        m.clamp((m.x(1_431_655_765), m.center_y - opening - upperCrown - curve)),
        m.clamp((m.x(2_863_311_531), m.center_y - opening - upperCrown - curve)),
        (m.x(Q32_ONE - inset), m.center_y),
        m.clamp((m.x(2_863_311_531), m.center_y + opening + lowerCrown + curve)),
        m.clamp((m.x(1_431_655_765), m.center_y + opening + lowerCrown + curve)),
    ], .solid(color: palette.white), palette.dark)
}

private func mouth6(_ mouth: Mouth, _ m: MouthMetrics, _ mood: MoodDeltas, _ palette: Palette) -> MouthShape {
    let top = m.y(1_073_741_824)
    let bottom = m.y(3_221_225_472)
    let topBulge = q32Mul(divI64(m.height, 4), parameter(mouth, 0))
    let bottomBulge = q32Mul(divI64(m.height, 4), parameter(mouth, 1))
    let asymmetry = q32Mul(divI64(m.height, 5), parameter(mouth, 2) - divI64(Q32_ONE, 2))
    let curve = q32Mul(divI64(m.height, 5), mood.mouth_curve.value)
    let inset = 858_993_459 + q32Mul(429_496_730, parameter(mouth, 3))
    return closedShape([
        (m.rect.x1, m.center_y),
        m.clamp((m.x(inset), top - topBulge + asymmetry + curve)),
        m.clamp((m.x(Q32_ONE - inset), top - topBulge - asymmetry - curve)),
        (m.rect.x2, m.center_y),
        m.clamp((m.x(Q32_ONE - inset), bottom + bottomBulge - asymmetry)),
        m.clamp((m.x(inset), bottom + bottomBulge + asymmetry)),
    ], .solid(color: palette.harmonic1), palette.dark)
}

private func mouth7(_ mouth: Mouth, _ m: MouthMetrics, _ mood: MoodDeltas, _ palette: Palette) -> MouthShape {
    let top = m.center_y - divI64(m.height, 5) - q32Mul(divI64(m.height, 4), parameter(mouth, 0))
    let bottom = m.center_y + divI64(m.height, 5) + q32Mul(divI64(m.height, 4), parameter(mouth, 1))
    let sideShift = q32Mul(divI64(m.height, 8), parameter(mouth, 2) - divI64(Q32_ONE, 2))
    let curve = q32Mul(divI64(m.height, 6), mood.mouth_curve.value)
    let inset = 429_496_730 + q32Mul(644_245_094, parameter(mouth, 3))
    return closedShape([
        (m.x(inset), m.center_y + sideShift),
        m.clamp((m.x(1_431_655_765), top)),
        m.clamp((m.x(2_863_311_531), top)),
        (m.x(Q32_ONE - inset), m.center_y - sideShift),
        m.clamp((m.x(2_863_311_531), bottom + curve)),
        m.clamp((m.x(1_431_655_765), bottom + curve)),
    ], .solid(color: palette.primary_darkest), palette.dark)
}

private func mouth23(_ mouth: Mouth, _ m: MouthMetrics, _ mood: MoodDeltas, _ palette: Palette) -> MouthShape {
    let inset = q32Mul(divI64(Q32_ONE, 10), parameter(mouth, 0))
    let center = m.y(1_288_490_189 + q32Mul(1_717_986_918, parameter(mouth, 3)))
    let opening = divI64(m.height, 8) + q32Mul(divI64(m.height, 5), parameter(mouth, 1))
    let pull = q32Mul(divI64(m.height, 6), parameter(mouth, 2))
    let curve = q32Mul(divI64(m.height, 5), mood.mouth_curve.value)
    return closedShape([
        (m.x(inset), center),
        m.clamp((m.x(1_073_741_824), center - opening - pull - curve)),
        m.clamp((m.x(3_221_225_472), center - opening - pull - curve)),
        (m.x(Q32_ONE - inset), center),
        m.clamp((m.x(3_221_225_472), center + opening + pull + curve)),
        m.clamp((m.x(1_073_741_824), center + opening + pull + curve)),
    ], .solid(color: palette.primary_darkest), palette.primary_darker)
}

private func mouth233(_ mouth: Mouth, _ m: MouthMetrics, _ mood: MoodDeltas, _ palette: Palette) -> MouthShape {
    let upper = divI64(m.height, 8) + q32Mul(divI64(m.height, 6), parameter(mouth, 0))
    let lower = divI64(m.height, 8) + q32Mul(divI64(m.height, 5), parameter(mouth, 1))
    let bow = q32Mul(divI64(m.height, 8), parameter(mouth, 2))
    let corner = q32Mul(divI64(m.height, 8), parameter(mouth, 3) - divI64(Q32_ONE, 2))
    let curve = q32Mul(divI64(m.height, 6), mood.mouth_curve.value)
    return multiLipShape([
        m.clamp((m.rect.x1, m.center_y + corner)),
        m.clamp((m.x(1_073_741_824), m.center_y - upper - curve)),
        m.clamp((m.center_x, m.center_y - upper + bow - curve)),
        m.clamp((m.x(3_221_225_472), m.center_y - upper - curve)),
        m.clamp((m.rect.x2, m.center_y - corner)),
        m.clamp((m.x(3_221_225_472), m.center_y + lower + curve)),
        m.clamp((m.center_x, m.center_y + lower + bow + curve)),
        m.clamp((m.x(1_073_741_824), m.center_y + lower + curve)),
    ], .solid(color: palette.harmonic3), palette.secondary_darker)
}

private func openShape(_ points: [(Int64, Int64)], _ fill: Paint, _ stroke: Rgb) -> MouthShape {
    MouthShape(shape: .path(segments: [
        .moveTo(x: points[0].0, y: points[0].1),
        .cubicTo(cp1x: points[1].0, cp1y: points[1].1, cp2x: points[2].0, cp2y: points[2].1, x: points[3].0, y: points[3].1),
    ], closed: false, fill_rule: .nonZero), fill: fill, stroke: stroke)
}

private func closedShape(_ points: [(Int64, Int64)], _ fill: Paint, _ stroke: Rgb) -> MouthShape {
    MouthShape(shape: .path(segments: [
        .moveTo(x: points[0].0, y: points[0].1),
        .cubicTo(cp1x: points[1].0, cp1y: points[1].1, cp2x: points[2].0, cp2y: points[2].1, x: points[3].0, y: points[3].1),
        .cubicTo(cp1x: points[4].0, cp1y: points[4].1, cp2x: points[5].0, cp2y: points[5].1, x: points[0].0, y: points[0].1),
    ], closed: true, fill_rule: .nonZero), fill: fill, stroke: stroke)
}

private func multiLipShape(_ points: [(Int64, Int64)], _ fill: Paint, _ stroke: Rgb) -> MouthShape {
    MouthShape(shape: .path(segments: [
        .moveTo(x: points[0].0, y: points[0].1),
        .cubicTo(cp1x: points[1].0, cp1y: points[1].1, cp2x: points[1].0, cp2y: points[1].1, x: points[2].0, y: points[2].1),
        .cubicTo(cp1x: points[3].0, cp1y: points[3].1, cp2x: points[3].0, cp2y: points[3].1, x: points[4].0, y: points[4].1),
        .cubicTo(cp1x: points[5].0, cp1y: points[5].1, cp2x: points[5].0, cp2y: points[5].1, x: points[6].0, y: points[6].1),
        .cubicTo(cp1x: points[7].0, cp1y: points[7].1, cp2x: points[7].0, cp2y: points[7].1, x: points[0].0, y: points[0].1),
    ], closed: true, fill_rule: .nonZero), fill: fill, stroke: stroke)
}

private func parameter(_ mouth: Mouth, _ index: Int) -> Int64 {
    min(max(mouth.params[index].value, 0), Q32_ONE)
}
