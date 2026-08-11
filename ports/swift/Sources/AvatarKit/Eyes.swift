// Eye geometry transcribed from crates/avatar-svg/src/eyes.rs.

private let Q32_ONE_POINT_TWO: Int64 = 5_153_960_755
private let Q32_TWO_POINT_EIGHT: Int64 = 12_025_908_429
private let Q32_ZERO_POINT_THREE: Int64 = 1_288_490_189

struct Cubic: Equatable {
    var x0: Int64
    var y0: Int64
    var cp1x: Int64
    var cp1y: Int64
    var cp2x: Int64
    var cp2y: Int64
    var x3: Int64
    var y3: Int64
}

enum EyeSide: String, Equatable {
    case left
    case right
}

struct EyeDimensions: Equatable {
    var width: Int64
    var upper: Int64
    var lower: Int64
}

struct EyeGeometry: Equatable {
    var outline_top: Cubic
    var outline_bottom: Cubic
    var upper_lash_curve: Cubic
    var center_x: Int64
    var center_y: Int64
    var iris_radius: Int64
    var pupil_radius: Int64
    var upper_close: Int64
    var lower_close: Int64
    var rotation: Bam16
    var side: EyeSide
}

struct EyePair: Equatable {
    var left: EyeGeometry
    var right: EyeGeometry
    var dimensions: EyeDimensions
    var lash_length: Int64
    var lower_edge_y: Int64
    var gap: Int64
}

func preliminaryDimensions(_ face: Face, _ bodyWidth: Int64) -> EyeDimensions {
    let available = faceWidth(bodyWidth, face)
    let width = max(max(q32Mul(available, face.eyes.size_h.value) / 3, divI64(available, 5)), 1)
    let upperDivisor = Q32_ONE_POINT_TWO + q32Mul(Q32_TWO_POINT_EIGHT, face.eyes.height_upper.value)
    let lowerDivisor = Q32_ONE_POINT_TWO + q32Mul(Q32_TWO_POINT_EIGHT, face.eyes.height_lower.value)
    var upper = divideByQ32(width, max(upperDivisor, 1))
    var lower = divideByQ32(width, max(lowerDivisor, 1))
    if face.eyes.height_bias_gate {
        let bias = q32Mul(divI64(upper, 3), face.eyes.height_bias.value)
        upper += bias
        lower = max(lower - bias, 1)
    }
    return EyeDimensions(width: width, upper: max(upper, 1), lower: max(lower, 1))
}

func makeEyePair(
    _ face: Face, _ rect: FaceRect, _ preliminary: EyeDimensions, _ mood: MoodDeltas
) -> EyePair {
    var dimensions = preliminary
    let totalHeight = dimensions.upper + dimensions.lower
    if totalHeight > rect.height {
        let scale = ratioQ32(rect.height, totalHeight)
        dimensions.width = max(q32Mul(dimensions.width, scale), 1)
        dimensions.upper = max(q32Mul(dimensions.upper, scale), 1)
        dimensions.lower = max(q32Mul(dimensions.lower, scale), 1)
    }
    let eye = face.eyes
    let lashLength = q32Mul(dimensions.upper + dimensions.lower, face.lashes.length_scale.value)
    let remaining = max(rect.width - dimensions.width * 2, 0)
    let gap = max(q32Mul(remaining, eye.ipd.value), 0)
    let faceX = CENTER - divI64(rect.width, 2)
    let leftStartX = faceX + divI64(rect.width - (dimensions.width * 2 + gap), 2)
    let pull = eye.vertical_pull_gate
        ? q32Mul(dimensions.upper, q32Mul(eye.vertical_pull.value, Q32_ZERO_POINT_THREE)) : 0
    let startY = rect.y + dimensions.upper - pull
    let endY = rect.y + dimensions.upper
    let topY = rect.y
    let contourRaise = q32Mul(dimensions.lower, mood.lower_contour_raise.value)
    let bottomY = rect.y + dimensions.upper + dimensions.lower - contourRaise
    let inset1 = q32Mul(divI64(dimensions.width, 2), eye.contour_inset1.value)
    let inset2 = q32Mul(divI64(dimensions.width, 2), eye.contour_inset2.value)
    let leftTop = Cubic(
        x0: leftStartX, y0: startY,
        cp1x: leftStartX + inset1, cp1y: topY,
        cp2x: leftStartX + dimensions.width - inset2, cp2y: topY,
        x3: leftStartX + dimensions.width, y3: endY)
    let leftBottom = Cubic(
        x0: leftTop.x0, y0: leftTop.y0,
        cp1x: leftTop.cp1x, cp1y: bottomY,
        cp2x: leftTop.cp2x, cp2y: bottomY,
        x3: leftTop.x3, y3: leftTop.y3)
    let upperFactor = min(max(eye.lid_close_upper_neutral.value + mood.upper_lid_close.value, 0), Q32_ONE)
    let lowerFactor = min(max(eye.lid_close_lower_neutral.value + mood.lower_lid_close.value, 0), Q32_ONE)
    let upperClose = q32Mul(dimensions.upper, upperFactor)
    let lowerClose = q32Mul(dimensions.lower, lowerFactor)
    let upperLash = Cubic(
        x0: leftTop.x0, y0: leftTop.y0,
        cp1x: leftTop.cp1x, cp1y: leftTop.cp1y + upperClose,
        cp2x: leftTop.cp2x, cp2y: leftTop.cp2y + upperClose,
        x3: leftTop.x3, y3: leftTop.y3)
    let neutralCenterX = leftStartX + divI64(dimensions.width, 2)
    let neutralCenterY = rect.y + divI64(dimensions.upper + dimensions.lower, 2)
    let shiftX = q32Mul(dimensions.width, mood.eyeball_shift_x.value)
    let shiftY = q32Mul(dimensions.upper + dimensions.lower, mood.eyeball_shift_y.value)
    let irisRadius = max(q32Mul(
        divI64(dimensions.upper + dimensions.lower, 4), eye.eyeball_radius_scale.value), 1)
    let pupilRadius = min(max(q32Mul(irisRadius, eye.pupil_scale.value), 1), irisRadius)
    let left = EyeGeometry(
        outline_top: leftTop, outline_bottom: leftBottom, upper_lash_curve: upperLash,
        center_x: neutralCenterX + shiftX, center_y: neutralCenterY + shiftY,
        iris_radius: irisRadius, pupil_radius: pupilRadius,
        upper_close: upperClose, lower_close: lowerClose,
        rotation: mood.eye_rotation, side: .left)
    return EyePair(
        left: left, right: mirrorEye(left), dimensions: dimensions,
        lash_length: lashLength, lower_edge_y: bottomY, gap: gap)
}

func eyePrimitives(
    _ eye: EyeGeometry, _ genes: Face, _ palette: Palette, _ rootOps: [TransformOp]
) -> [Primitive] {
    let transform = Transform(ops: rootOps + [
        .rotate(angle: eye.rotation, cx: eye.center_x, cy: eye.center_y)
    ])
    let clip = outlineShape(eye)
    let colors = eyeColors(eye.side, genes.eyes, palette)
    let shadeRadius = max(
        eye.iris_radius - q32Mul(divI64(eye.iris_radius * 2, 5), genes.eyes.iris_shade_reduce.value),
        1)
    let freqH = Q32(42_949_673 + q32Mul(386_547_057, genes.eyes.noise_freq_h.value))
    let freqV = Q32(42_949_673 + q32Mul(386_547_057, genes.eyes.noise_freq_v.value))
    let displacementMilli = 3_000 + q32Mul(47_000, genes.eyes.iris_shade_reduce.value)
    let displacement = Q32(divRoundHalfAway(
        DoubleWidth.multiply(displacementMilli, Q32_ONE), 100).toInt64())
    var result: [Primitive] = [
        Primitive(
            shape: clip, transform: transform, paint: .solid(color: palette.white),
            stroke: Stroke(color: palette.dark, width: 196), label: "av-eyeball"),
        Primitive(
            shape: .circle(cx: eye.center_x, cy: eye.center_y, r: eye.iris_radius),
            transform: transform, paint: .solid(color: colors.iris),
            stroke: Stroke(color: palette.dark, width: 80), clip: clip,
            label: "av-iris"),
        Primitive(
            shape: .ellipse(
                cx: eye.center_x, cy: eye.center_y,
                rx: shadeRadius, ry: divI64(shadeRadius * 5, 6)),
            transform: transform, paint: .solid(color: palette.primary_darkest),
            opacity: genes.eyes.iris_shade_opacity, clip: clip,
            effects: [.turbulence(
                seed: genes.eyes.noise_seed, freq_h: freqH, freq_v: freqV,
                octaves: 5, scale: displacement)],
            label: "av-iris-shade"),
        Primitive(
            shape: .circle(cx: eye.center_x, cy: eye.center_y, r: eye.pupil_radius),
            transform: transform, paint: .solid(color: palette.dark), clip: clip,
            label: "av-pupil"),
    ]
    result.append(highlightPrimitive(eye, -5, -5, 12, palette.white, 3_865_470_566, transform, clip))
    result.append(highlightPrimitive(eye, 6, 6, 19, palette.harmonic1, 1_288_490_189, transform, clip))
    result.append(highlightPrimitive(eye, 5, 5, 28, palette.white, 3_435_973_837, transform, clip))
    result.append(Primitive(
        shape: upperLidShape(eye, genes.eyes.lid_extension_gate, genes.eyes.lid_extension_side),
        transform: transform, paint: .solid(color: colors.lid),
        stroke: Stroke(color: palette.dark, width: 196), label: "av-upper-lid"))
    result.append(Primitive(
        shape: lowerLidShape(eye, genes.eyes.lid_extension_gate, genes.eyes.lid_extension_side),
        transform: transform, paint: .solid(color: colors.lid),
        stroke: Stroke(color: palette.dark, width: 196), label: "av-lower-lid"))
    result.append(contentsOf: lashPrimitives(
        genes.lashes, eye.upper_lash_curve,
        eye.outline_bottom.cp1y - eye.outline_top.cp1y,
        eye.side, palette.dark, transform))
    return result
}

private func mirrorEye(_ left: EyeGeometry) -> EyeGeometry {
    EyeGeometry(
        outline_top: mirrorCurve(left.outline_top),
        outline_bottom: mirrorCurve(left.outline_bottom),
        upper_lash_curve: mirrorCurve(left.upper_lash_curve),
        center_x: CANVAS_MILLI - left.center_x,
        center_y: left.center_y,
        iris_radius: left.iris_radius,
        pupil_radius: left.pupil_radius,
        upper_close: left.upper_close,
        lower_close: left.lower_close,
        rotation: oppositeBam(left.rotation),
        side: .right)
}

private func mirrorCurve(_ curve: Cubic) -> Cubic {
    Cubic(
        x0: CANVAS_MILLI - curve.x0, y0: curve.y0,
        cp1x: CANVAS_MILLI - curve.cp1x, cp1y: curve.cp1y,
        cp2x: CANVAS_MILLI - curve.cp2x, cp2y: curve.cp2y,
        x3: CANVAS_MILLI - curve.x3, y3: curve.y3)
}

func outlineShape(_ eye: EyeGeometry) -> Shape {
    let top = eye.outline_top
    let bottom = eye.outline_bottom
    return .path(segments: [
        .moveTo(x: top.x0, y: top.y0),
        .cubicTo(cp1x: top.cp1x, cp1y: top.cp1y, cp2x: top.cp2x, cp2y: top.cp2y, x: top.x3, y: top.y3),
        .cubicTo(cp1x: bottom.cp2x, cp1y: bottom.cp2y, cp2x: bottom.cp1x, cp2y: bottom.cp1y, x: bottom.x0, y: bottom.y0),
    ], closed: true, fill_rule: .nonZero)
}

private func upperLidShape(_ eye: EyeGeometry, _ gate: UInt8, _ sideGene: UInt8) -> Shape {
    let curve = eye.outline_top
    let extensionAmount = lidExtensions(curve, gate, sideGene).0
    let signed = eye.side == .left ? -extensionAmount : extensionAmount
    return .path(segments: [
        .moveTo(x: curve.x0 + signed, y: curve.y0),
        .cubicTo(cp1x: curve.cp1x + signed, cp1y: curve.cp1y, cp2x: curve.cp2x, cp2y: curve.cp2y, x: curve.x3, y: curve.y3),
        .cubicTo(cp1x: curve.cp2x, cp1y: curve.cp2y + eye.upper_close, cp2x: curve.cp1x, cp2y: curve.cp1y + eye.upper_close, x: curve.x0, y: curve.y0),
    ], closed: true, fill_rule: .nonZero)
}

private func lowerLidShape(_ eye: EyeGeometry, _ gate: UInt8, _ sideGene: UInt8) -> Shape {
    let curve = eye.outline_bottom
    let extensionAmount = lidExtensions(curve, gate, sideGene).1
    let signed = eye.side == .left ? -extensionAmount : extensionAmount
    return .path(segments: [
        .moveTo(x: curve.x0 + signed, y: curve.y0),
        .cubicTo(cp1x: curve.cp1x + signed, cp1y: curve.cp1y, cp2x: curve.cp2x, cp2y: curve.cp2y, x: curve.x3, y: curve.y3),
        .cubicTo(cp1x: curve.cp2x, cp1y: curve.cp2y - eye.lower_close, cp2x: curve.cp1x, cp2y: curve.cp1y - eye.lower_close, x: curve.x0, y: curve.y0),
    ], closed: true, fill_rule: .nonZero)
}

private func lidExtensions(_ curve: Cubic, _ gate: UInt8, _ side: UInt8) -> (Int64, Int64) {
    if gate <= 220 { return (0, 0) }
    let base = divI64(Swift.abs(curve.x3 - curve.x0), 30)
    let extensionAmount = base + divI64(base * 3 * Int64(gate - 220), 35)
    if side > 127 {
        return (divI64(extensionAmount, 3), extensionAmount)
    }
    return (extensionAmount, divI64(extensionAmount, 3))
}

private func highlightPrimitive(
    _ eye: EyeGeometry, _ xDivisor: Int64, _ yDivisor: Int64, _ radiusDivisor: Int64,
    _ color: Rgb, _ opacity: Int64, _ transform: Transform, _ clip: Shape
) -> Primitive {
    Primitive(
        shape: .circle(
            cx: eye.center_x + divI64(eye.iris_radius, xDivisor),
            cy: eye.center_y + divI64(eye.iris_radius, yDivisor),
            r: max(divI64(eye.iris_radius, radiusDivisor), 1)),
        transform: transform, paint: .solid(color: color), opacity: Q32(opacity), clip: clip,
        label: "av-highlight")
}

private func eyeColors(_ side: EyeSide, _ eyes: Eyes, _ palette: Palette) -> (iris: Rgb, lid: Rgb) {
    let iris: Rgb
    switch eyes.iris_color_gene {
    case 254...255: iris = side == .left ? palette.harmonic3 : palette.harmonic4
    case 241...253: iris = palette.harmonic4
    case 236...240: iris = palette.harmonic3
    case 211...235: iris = palette.harmonic1
    case 0...1: iris = side == .left ? palette.harmonic2 : palette.harmonic3
    default: iris = palette.harmonic2
    }
    let lid: Rgb
    switch eyes.lid_color_gene {
    case 254...255: lid = side == .left ? palette.harmonic1 : palette.harmonic4
    case 241...253: lid = side == .left ? palette.primary : palette.harmonic1
    case 231...240: lid = side == .left ? palette.harmonic1 : palette.primary
    case 211...230: lid = side == .left ? palette.primary_darkest : palette.primary
    case 0...39: lid = palette.primary_darker
    default: lid = palette.primary
    }
    return (reduceSaturation(iris, eyes.iris_saturation_reduce.value), lid)
}

private func reduceSaturation(_ color: Rgb, _ amount: Int64) -> Rgb {
    let gray = divI64(Int64(color.r) + Int64(color.g) + Int64(color.b), 3)
    func channel(_ value: UInt8) -> UInt8 {
        UInt8(min(max(Int64(value) + q32Mul(gray - Int64(value), min(max(amount, 0), Q32_ONE)), 0), 255))
    }
    return Rgb(r: channel(color.r), g: channel(color.g), b: channel(color.b))
}

private func divideByQ32(_ value: Int64, _ divisor: Int64) -> Int64 {
    divRoundHalfAway(DoubleWidth.multiply(value, Q32_ONE), divisor).toInt64()
}
