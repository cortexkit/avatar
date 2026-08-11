// Face placement and mouth fitting transcribed from crates/avatar-svg/src/face.rs.

private let MOUTH_SEARCH_MAX: UInt32 = 3_000

struct MouthRect: Equatable {
    var x1: Int64 = 0
    var y1: Int64 = 0
    var x2: Int64 = 0
    var y2: Int64 = 0

    var width: Int64 { x2 - x1 }
    var height: Int64 { y2 - y1 }
}

struct FaceRect: Equatable {
    var width: Int64
    var height: Int64
    var y: Int64
    var mouth: MouthRect
}

func eyeLineAnchor(_ positionMode: UInt8, _ body: BodyGeometry) -> Int64 {
    switch positionMode {
    case 0: return body.min_width_y
    case 1: return body.max_width_y
    case 2: return body.top + divI64(body.body_height, 3)
    case 3: return body.top + divI64(body.body_height, 2)
    default: return body.max_width_y
    }
}

func faceWidth(_ bodyWidth: Int64, _ face: Face) -> Int64 {
    var width = q32Mul(divI64(bodyWidth, 2), face.placement.width_coeff.value)
    let minimum = divI64(divI64(CANVAS_MILLI * 2, 5), 2)
    width = max(width, minimum)
    return min(width, divI64(CANVAS_MILLI * 10, 11))
}

func resolveFaceRect(_ body: BodyGeometry, _ face: Face, _ eyesHeight: Int64) -> FaceRect {
    var centerY = eyeLineAnchor(face.eyes.position_mode, body)
    let width = faceWidth(body.body_width, face)
    var height = q32Mul(divI64(body.body_height, 6), face.placement.height_coeff.value)
    height = max(height, divI64(divI64(CANVAS_MILLI, 5), 2))
    if width * 10 < height * 17 {
        height = divI64(width * 10, 17)
    }
    height = min(height, max(eyesHeight, 1))
    if centerY + height > CANVAS_MILLI {
        centerY -= CANVAS_MILLI - centerY + height
    }

    let leftSpace = body.top + body.body_height - centerY
    let minimumMouthHeight = max(divI64(body.body_height, 15), divI64(CANVAS_MILLI, 20))
    var counter: UInt32 = 0
    var mouth = searchMouthRows(
        body, centerY, height, leftSpace, minimumMouthHeight, &counter)
    if mouth.y1 >= body.top + body.body_height {
        centerY = body.top
        mouth = searchMouthRows(
            body, centerY, height, leftSpace, minimumMouthHeight, &counter)
    }
    if mouth.y1 != 0 {
        centerY = mouth.y1 - divI64(height, 2)
    }
    return FaceRect(
        width: width, height: height, y: centerY - divI64(height, 2), mouth: mouth)
}

func renderMouthRect(_ body: BodyGeometry, _ faceRect: FaceRect, _ eyes: EyePair, _ face: Face) -> MouthRect {
    let eyeDistance = Swift.abs(eyes.right.center_x - eyes.left.center_x)
    var halfWidth = max(
        max(divI64(min(divI64(eyeDistance, 2), divI64(body.body_width, 7)), 2), divI64(faceRect.width, 12)),
        1)
    if face.mouth.width_scale_gate {
        let factor = 3_435_973_837 + q32Mul(5_153_960_755, face.mouth.width_scale.value)
        halfWidth = max(q32Mul(halfWidth, factor), 1)
    }
    var rect = faceRect.mouth
    if rect.x1 == 0 || rect.width <= 0 || rect.height <= 0 {
        let height = max(divI64(body.body_height, 12), divI64(CANVAS_MILLI, 25))
        rect = MouthRect(
            x1: CENTER - halfWidth,
            y1: eyes.lower_edge_y + divI64(faceRect.height, 4),
            x2: CENTER + halfWidth,
            y2: eyes.lower_edge_y + divI64(faceRect.height, 4) + height)
    } else {
        rect.x1 = CENTER - halfWidth
        rect.x2 = CENTER + halfWidth
    }
    if face.mouth.aspect_widen_gate && rect.width * 20 < rect.height * 23 {
        let requiredWidth = divI64(rect.height * 23, 20)
        rect.x1 = CENTER - divI64(requiredWidth, 2)
        rect.x2 = CENTER + divI64(requiredWidth, 2)
    }
    let eyeSpan = eyes.right.outline_top.x3 - eyes.left.outline_top.x3
    if eyeSpan * 10 > rect.width * 11 && body.body_height < divI64(CANVAS_MILLI, 20) {
        rect.y1 -= divI64(faceRect.height, 2)
        rect.y2 -= divI64(faceRect.height, 2)
    }
    return rect
}

private func searchMouthRows(
    _ body: BodyGeometry, _ centerY: Int64, _ faceHeight: Int64,
    _ leftSpace: Int64, _ minimumMouthHeight: Int64, _ counter: inout UInt32
) -> MouthRect {
    var widest = MouthRect()
    let rows = ceilPositive(leftSpace, minimumMouthHeight)
    guard rows > 0 else { return widest }
    for row in 0..<rows {
        let top = centerY + divI64(faceHeight, 2) + row * minimumMouthHeight
        let candidate = findRect(
            body.all_points, body.body_width, top, minimumMouthHeight, &counter)
        if counter > MOUTH_SEARCH_MAX { return MouthRect() }
        let area = candidate.width * candidate.height
        let threshold = body.body_width * body.body_height / 80
        if area > threshold { return candidate }
        if candidate.width > widest.width { widest = candidate }
    }
    return widest
}

func findRect(
    _ points: [Point], _ bodyWidth: Int64, _ top: Int64,
    _ minimumMouthHeight: Int64, _ counter: inout UInt32
) -> MouthRect {
    var active = points
    var reductionPercent: Int64 = 5
    while true {
        let inset = divI64(bodyWidth * reductionPercent, 100)
        let rect = MouthRect(
            x1: CENTER - divI64(bodyWidth, 2) + inset,
            y1: top,
            x2: CENTER + divI64(bodyWidth, 2) - inset,
            y2: top + minimumMouthHeight)
        if rect.x1 > rect.x2 || counter > MOUTH_SEARCH_MAX { return rect }
        var inside: [Point] = []
        for point in active {
            counter = counter == .max ? .max : counter + 1
            if point.x >= rect.x1 && point.x <= rect.x2
                && point.y >= rect.y1 && point.y <= rect.y2
            {
                inside.append(point)
            }
        }
        if inside.isEmpty { return rect }
        active = inside
        reductionPercent += 2
    }
}

private func ceilPositive(_ value: Int64, _ divisor: Int64) -> Int64 {
    value <= 0 || divisor <= 0 ? 0 : (value + divisor - 1) / divisor
}
