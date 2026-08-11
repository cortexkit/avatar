// Render-tree assembly transcribed from crates/avatar-svg/src/{svg,face}.rs.

func buildTree(_ phenotype: Phenotype, _ params: RenderParams, includeFace: Bool) throws -> RenderTree {
    try validateRenderParams(params)
    let geometry = bodyGeometry(phenotype)
    let placement = bodyPlacement(geometry)
    let rootOps: [TransformOp] = [
        .translate(tx: placement.tx, ty: placement.ty),
        .scale(sx: Q32(placement.scale), sy: Q32(placement.scale)),
    ]
    let rootTransform = Transform(ops: rootOps)
    let bodyStroke = Stroke(color: phenotype.palette.dark, width: bodyStrokeWidth())
    // Resolved here, not in the serializer: light placement derives from the
    // rescaled body geometry, and re-deriving it from the final shape can
    // round one milliunit differently.
    let lightY = geometry.top + divI64(geometry.body_height, 2)
    let bodyShape = geometry.shape
    var primitives: [Primitive] = [
        Primitive(
            shape: bodyShape, transform: rootTransform,
            paint: .solid(color: phenotype.palette.primary), stroke: bodyStroke,
            label: "av-body-base"),
        Primitive(
            shape: bodyShape, transform: rootTransform,
            paint: .solid(color: phenotype.palette.primary), effects: [.lit(lightY: lightY)],
            label: "av-body-lit-primary"),
        Primitive(
            shape: bodyShape, transform: rootTransform,
            paint: .radialGradient(stops: [
                GradientStop(offset_q32: Q32(3_822_520_893), color: phenotype.palette.primary),
                GradientStop(offset_q32: .one, color: phenotype.palette.primary_darkish),
            ]), effects: [.lit(lightY: lightY)], label: "av-body-gradient"),
    ]
    if let mask = patternMaskPrimitives(
        phenotype.pattern, geometry.body_width, geometry.body_height, geometry.top, rootOps)
    {
        primitives.append(Primitive(
            shape: bodyShape, transform: rootTransform,
            paint: .solid(color: phenotype.palette.harmonic3),
            effects: [.lit(lightY: lightY), .masked(mask: mask)], label: "av-body-pattern"))
    }
    if includeFace {
        primitives.append(contentsOf: facePrimitives(phenotype, geometry, params.mood, rootOps))
    }
    return RenderTree(
        canvas: params.canvas, background: phenotype.palette.back, primitives: primitives)
}

/// Builds the geometry-primary avatar tree in painter's order.
public func buildRenderTree(phenotype: Phenotype, params: RenderParams) throws -> RenderTree {
    try buildTree(phenotype, params, includeFace: true)
}

private func facePrimitives(
    _ phenotype: Phenotype, _ body: BodyGeometry, _ selectedMood: Mood,
    _ rootOps: [TransformOp]
) -> [Primitive] {
    let mood = moodDeltas(selectedMood).scaled(phenotype.mood_response)
    let dimensions = preliminaryDimensions(phenotype.face, body.body_width)
    let rect = resolveFaceRect(body, phenotype.face, dimensions.upper + dimensions.lower)
    let eyes = makeEyePair(phenotype.face, rect, dimensions, mood)
    let mouthRect = renderMouthRect(body, rect, eyes, phenotype.face)
    let rootTransform = Transform(ops: rootOps)
    var result: [Primitive] = [
        mouthPrimitive(phenotype.face.mouth, mouthRect, mood, phenotype.palette, rootTransform)
    ]
    result.append(contentsOf: eyePrimitives(eyes.left, phenotype.face, phenotype.palette, rootOps))
    result.append(contentsOf: eyePrimitives(eyes.right, phenotype.face, phenotype.palette, rootOps))
    result.append(contentsOf: browPrimitives(
        phenotype.face, rect, eyes, mood, phenotype.palette, rootOps))
    return result
}

func bodyStrokeWidth() -> Int64 {
    divI64(CANVAS_MILLI, 255)
}

private struct BodyPlacement {
    var tx: Int64
    var ty: Int64
    var scale: Int64
}

private func bodyPlacement(_ geometry: BodyGeometry) -> BodyPlacement {
    let strokeMargin = 8 * bodyStrokeWidth()
    let limit = CANVAS_MILLI - strokeMargin
    if geometry.body_height > limit || geometry.body_width > limit {
        let dimension = max(geometry.body_width, geometry.body_height)
        let scale = divRoundHalfAway(
            DoubleWidth.multiply(limit, Q32_ONE), dimension).toInt64()
        let scaledWidth = q32Mul(geometry.body_width, scale)
        let scaledHeight = q32Mul(geometry.body_height, scale)
        return BodyPlacement(
            tx: divI64(geometry.body_width - scaledWidth, 2),
            ty: divI64(CANVAS_MILLI - scaledHeight, 2) - geometry.top,
            scale: scale)
    }
    return BodyPlacement(
        tx: 0,
        ty: divI64(CANVAS_MILLI - geometry.body_height, 2) - geometry.top,
        scale: Q32_ONE)
}
