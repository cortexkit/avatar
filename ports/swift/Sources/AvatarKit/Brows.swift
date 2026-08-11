// Brow splines transcribed from crates/avatar-svg/src/brows.rs.

func browPrimitives(
    _ face: Face, _ rect: FaceRect, _ eyes: EyePair, _ mood: MoodDeltas,
    _ palette: Palette, _ rootOps: [TransformOp]
) -> [Primitive] {
    let leftPoints = neutralBrowPoints(face, rect, eyes)
    let rightPoints = leftPoints.map { RingPoint(
        x: CANVAS_MILLI - $0.x, y: $0.y, tension: $0.tension) }
    let shiftY = q32Mul(rect.height, mood.brow_shift_y.value)
    let leftCenter = browCenter(leftPoints)
    let rightCenter = browCenter(rightPoints)
    return [
        Primitive(
            shape: buildSpline(leftPoints).shape,
            transform: Transform(ops: rootOps + [
                .translate(tx: 0, ty: shiftY),
                .rotate(angle: mood.brow_rotation, cx: leftCenter.0, cy: leftCenter.1),
            ]),
            paint: .solid(color: palette.secondary_darker), label: "av-brow-left"),
        Primitive(
            shape: buildSpline(rightPoints).shape,
            transform: Transform(ops: rootOps + [
                .translate(tx: 0, ty: shiftY),
                .rotate(angle: oppositeBam(mood.brow_rotation), cx: rightCenter.0, cy: rightCenter.1),
            ]),
            paint: .solid(color: palette.secondary_darker), label: "av-brow-right"),
    ]
}

private func neutralBrowPoints(_ face: Face, _ rect: FaceRect, _ eyes: EyePair) -> [RingPoint] {
    let eyeWidth = eyes.dimensions.width
    let widthFactor = 1_717_986_918 + q32Mul(3_435_973_837, face.brows.width_coupling.value)
    let total = q32Mul(eyeWidth, widthFactor)
    let centerX = eyes.left.center_x - divI64(eyeWidth, 10)
    let peak = divI64(rect.height, 12)
    let eyeHeight = eyes.dimensions.upper + eyes.dimensions.lower
    let lowClearance = divI64(eyeHeight, 16)
    let highClearance = divI64(eyeHeight, 10)
    let clearance = lowClearance
        + q32Mul(highClearance - lowClearance, face.brows.clearance.value)
        + eyes.lash_length
    let top = eyes.left.outline_top.cp1y - peak - clearance
    let firstY = top + q32Mul(peak * 2, face.brows.end_height1.value) - peak
    let lastY = top + q32Mul(peak * 2, face.brows.end_height2.value) - peak
    let midMinimum = -divI64(total, 5)
    let midMaximum = divI64(total, 4)
    let midOffset = midMinimum + q32Mul(midMaximum - midMinimum, face.brows.mid_x.value)
    return [
        RingPoint(
            x: centerX - divI64(total, 2), y: firstY,
            tension: min(max(face.brows.tensions[0].value, 0), Q32_ONE)),
        RingPoint(
            x: centerX + midOffset, y: top - peak,
            tension: min(max(face.brows.tensions[1].value, 0), Q32_ONE)),
        RingPoint(
            x: centerX + divI64(total, 2), y: lastY,
            tension: min(max(face.brows.tensions[2].value, 0), Q32_ONE)),
    ]
}

private func browCenter(_ points: [RingPoint]) -> (Int64, Int64) {
    let minimumX = points.map(\.x).min() ?? 0
    let maximumX = points.map(\.x).max() ?? 0
    let minimumY = points.map(\.y).min() ?? 0
    let maximumY = points.map(\.y).max() ?? 0
    return (divI64(minimumX + maximumX, 2), divI64(minimumY + maximumY, 2))
}
