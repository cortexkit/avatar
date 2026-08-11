// Body geometry transcribed from crates/avatar-svg/src/{fixed,geometry}.rs.

let CANVAS_MILLI: Int64 = 1_000_000
let CENTER: Int64 = CANVAS_MILLI / 2
private let FLATTEN_STEPS: Int64 = 20

func q32Mul(_ value: Int64, _ factor: Int64) -> Int64 {
    divRoundHalfAway(DoubleWidth.multiply(value, factor), Q32_ONE).toInt64()
}

func lerpQ32Milli(_ lo: Int64, _ hi: Int64, _ factor: Int64) -> Int64 {
    lo + q32Mul(hi - lo, factor)
}

func q32Ratio(_ dimension: Int64) -> Int64 {
    divRoundHalfAway(DoubleWidth.multiply(dimension, Q32_ONE), CANVAS_MILLI).toInt64()
}

func divI64(_ value: Int64, _ divisor: Int64) -> Int64 {
    precondition(divisor != 0, "divI64 requires a nonzero denominator")
    let sign: Int64 = divisor < 0 ? -1 : 1
    return divRoundHalfAway(DoubleWidth.multiply(value, sign), divisor.magnitude > UInt64(Int64.max) ? Int64.max : Swift.abs(divisor)).toInt64()
}

func ratioQ32(_ numerator: Int64, _ denominator: Int64) -> Int64 {
    if denominator == 0 { return 0 }
    let sign: Int64 = denominator < 0 ? -1 : 1
    return divRoundHalfAway(
        DoubleWidth.multiply(numerator * sign, Q32_ONE), Swift.abs(denominator)
    ).toInt64()
}

func bamDegreesQ32(_ angle: Bam16) -> Int64 {
    let signed = Int64(Int16(bitPattern: angle.value))
    return divRoundHalfAway(DoubleWidth.multiply(signed * 360, Q32_ONE), 65_536).toInt64()
}

func oppositeBam(_ angle: Bam16) -> Bam16 {
    Bam16(UInt16(bitPattern: -Int16(bitPattern: angle.value)))
}

struct RingPoint: Equatable {
    var x: Int64
    var y: Int64
    var tension: Int64
}

struct Curve: Equatable {
    var cp1x: Int64
    var cp1y: Int64
    var cp2x: Int64
    var cp2y: Int64
    var x2: Int64
    var y2: Int64
}

struct Spline: Equatable {
    var start_x: Int64
    var start_y: Int64
    var curves: [Curve]

    var shape: Shape {
        var segments: [Segment] = [.moveTo(x: start_x, y: start_y)]
        segments.reserveCapacity(curves.count + 1)
        for curve in curves {
            segments.append(.cubicTo(
                cp1x: curve.cp1x, cp1y: curve.cp1y,
                cp2x: curve.cp2x, cp2y: curve.cp2y,
                x: curve.x2, y: curve.y2))
        }
        return .path(segments: segments, closed: false, fill_rule: .nonZero)
    }
}

struct Bounds: Equatable {
    var min_x: Int64
    var min_x_y: Int64
    var max_x: Int64
    var min_y: Int64
    var max_y: Int64
    var max_width_y: Int64
    var samples: [(Int64, Int64)]

    static func == (lhs: Bounds, rhs: Bounds) -> Bool {
        lhs.min_x == rhs.min_x && lhs.min_x_y == rhs.min_x_y
            && lhs.max_x == rhs.max_x && lhs.min_y == rhs.min_y
            && lhs.max_y == rhs.max_y && lhs.max_width_y == rhs.max_width_y
            && lhs.samples.elementsEqual(rhs.samples, by: ==)
    }
}

/// Flattened body outline and the landmarks consumed by face placement.
public struct BodyGeometry: Equatable {
    public var shape: Shape
    public var body_width: Int64
    public var body_height: Int64
    public var top: Int64
    public var bottom: Int64
    public var max_width_y: Int64
    public var min_width_y: Int64
    public var all_points: [Point]

    public init(
        shape: Shape, body_width: Int64, body_height: Int64, top: Int64,
        bottom: Int64, max_width_y: Int64, min_width_y: Int64, all_points: [Point]
    ) {
        self.shape = shape
        self.body_width = body_width
        self.body_height = body_height
        self.top = top
        self.bottom = bottom
        self.max_width_y = max_width_y
        self.min_width_y = min_width_y
        self.all_points = all_points
    }
}

public struct Point: Equatable {
    public var x: Int64
    public var y: Int64

    public init(x: Int64, y: Int64) {
        self.x = x
        self.y = y
    }
}

/// Computes the flattened outline, dimensions, and landmarks used for body
/// painting and face placement; mirrors `body_geometry` in
/// crates/avatar-svg/src/geometry.rs.
public func bodyGeometry(_ phenotype: Phenotype) -> BodyGeometry {
    let ring = mirrorRing(phenotype.body)
    var spline = buildSpline(ring)
    var bounds = flatten(spline)
    let bodyWidth = rescale(&spline, &bounds)
    let top = bounds.min_y
    let bottom = bounds.max_y
    let bodyHeight = bottom - top
    return BodyGeometry(
        shape: spline.shape,
        body_width: bodyWidth,
        body_height: bodyHeight,
        top: top,
        bottom: bottom,
        max_width_y: bounds.max_width_y,
        min_width_y: minWidthY(bounds.samples, top, bodyHeight),
        all_points: bounds.samples.map { Point(x: $0.0, y: $0.1) })
}

func mirrorRing(_ body: Body) -> [RingPoint] {
    let count = Int(body.point_count.clamped(to: 2...16))
    let points = Array(body.points.prefix(count))
    let maxY = points.map { $0.y.value }.max() ?? 0
    let verticalShift = CENTER - divI64(maxY, 2)
    var ring: [RingPoint] = []
    ring.reserveCapacity(2 * count)
    for point in points {
        ring.append(RingPoint(
            x: CENTER - divI64(point.x.value, 2),
            y: point.y.value + verticalShift,
            tension: point.tension.value))
    }
    for index in 1...count {
        let point = points[count - index]
        ring.append(RingPoint(
            x: CENTER + divI64(point.x.value, 2),
            y: point.y.value + verticalShift,
            tension: point.tension.value))
    }
    return ring
}

func buildSpline(_ ring: [RingPoint]) -> Spline {
    let count = ring.count
    var tensionIndex = 0
    let start = ring[count - 1]
    var curves: [Curve] = []
    curves.reserveCapacity(count)
    for segment in 0..<count {
        let cursor = count - 2 + segment
        let p0 = ring[cursor % count]
        let p1 = ring[(cursor + 1) % count]
        let p2 = ring[(cursor + 2) % count]
        let p3 = ring[(cursor + 3) % count]
        let tension = ring[tensionIndex].tension
        curves.append(Curve(
            cp1x: p1.x + tensionOffset(p2.x - p0.x, tension),
            cp1y: p1.y + tensionOffset(p2.y - p0.y, tension),
            cp2x: p2.x - tensionOffset(p3.x - p1.x, tension),
            cp2y: p2.y - tensionOffset(p3.y - p1.y, tension),
            x2: p2.x, y2: p2.y))
        if segment != count / 2 { tensionIndex += 1 }
    }
    return Spline(start_x: start.x, start_y: start.y, curves: curves)
}

private func tensionOffset(_ delta: Int64, _ tension: Int64) -> Int64 {
    divRoundHalfAway(DoubleWidth.multiply(delta, tension), 6 * Q32_ONE).toInt64()
}

func flatten(_ spline: Spline) -> Bounds {
    var bounds = Bounds(
        min_x: .max, min_x_y: 0, max_x: 0, min_y: .max, max_y: 0,
        max_width_y: 0, samples: [])
    bounds.samples.reserveCapacity(spline.curves.count * (Int(FLATTEN_STEPS) + 1))
    var anchorX = spline.start_x
    var anchorY = spline.start_y
    for curve in spline.curves {
        for step in 0...FLATTEN_STEPS {
            let x = bezierSample(anchorX, curve.cp1x, curve.cp2x, curve.x2, step)
            let y = bezierSample(anchorY, curve.cp1y, curve.cp2y, curve.y2, step)
            if x < bounds.min_x {
                bounds.min_x = x
                bounds.min_x_y = y
            } else if x > bounds.max_x {
                bounds.max_x = x
                bounds.max_width_y = y
            }
            if y < bounds.min_y {
                bounds.min_y = y
            } else if y > bounds.max_y {
                bounds.max_y = y
            }
            bounds.samples.append((x, y))
        }
        anchorX = curve.x2
        anchorY = curve.y2
    }
    return bounds
}

private func bezierSample(_ p0: Int64, _ p1: Int64, _ p2: Int64, _ p3: Int64, _ step: Int64) -> Int64 {
    let q0 = lerpStep(p0, p1, step)
    let q1 = lerpStep(p1, p2, step)
    let q2 = lerpStep(p2, p3, step)
    let r0 = lerpStep(q0, q1, step)
    let r1 = lerpStep(q1, q2, step)
    return lerpStep(r0, r1, step)
}

private func lerpStep(_ a: Int64, _ b: Int64, _ step: Int64) -> Int64 {
    a + divRoundHalfAway(DoubleWidth.multiply(b - a, step), FLATTEN_STEPS).toInt64()
}

@discardableResult
func rescale(_ spline: inout Spline, _ bounds: inout Bounds) -> Int64 {
    let margin = divI64(CANVAS_MILLI, 6)
    var maxWidth = bounds.max_x - bounds.min_x
    if bounds.min_x - margin < 0 {
        maxWidth = bounds.min_x < 0
            ? -bounds.min_x + bounds.max_x + margin
            : bounds.max_x - bounds.min_x + margin
    }
    var maxHeight1 = bounds.max_y - bounds.min_y
    var maxHeight2 = maxHeight1
    if bounds.min_y - margin < 0 {
        maxHeight1 = bounds.min_y < 0
            ? 2 * (-bounds.min_y + CENTER) + margin
            : 2 * (CENTER - bounds.min_y) + margin
    }
    if bounds.max_y + margin > CANVAS_MILLI {
        maxHeight2 = 2 * (bounds.max_y - CENTER) + margin
    }
    let finalRatio = max(q32Ratio(maxWidth), q32Ratio(max(maxHeight1, maxHeight2)))
    if finalRatio > Q32_ONE {
        spline.start_x = adjustPoint(spline.start_x, finalRatio)
        spline.start_y = adjustPoint(spline.start_y, finalRatio)
        for index in spline.curves.indices {
            spline.curves[index].cp1x = adjustPoint(spline.curves[index].cp1x, finalRatio)
            spline.curves[index].cp1y = adjustPoint(spline.curves[index].cp1y, finalRatio)
            spline.curves[index].cp2x = adjustPoint(spline.curves[index].cp2x, finalRatio)
            spline.curves[index].cp2y = adjustPoint(spline.curves[index].cp2y, finalRatio)
            spline.curves[index].x2 = adjustPoint(spline.curves[index].x2, finalRatio)
            spline.curves[index].y2 = adjustPoint(spline.curves[index].y2, finalRatio)
        }
        bounds.max_width_y = adjustPoint(bounds.max_width_y, finalRatio)
        bounds.min_x = adjustPoint(bounds.min_x, finalRatio)
        bounds.min_x_y = adjustPoint(bounds.min_x_y, finalRatio)
        bounds.min_y = adjustPoint(bounds.min_y, finalRatio)
        bounds.max_y = adjustPoint(bounds.max_y, finalRatio)
        maxWidth = divRoundHalfAway(DoubleWidth.multiply(maxWidth, Q32_ONE), finalRatio).toInt64()
        for index in bounds.samples.indices {
            bounds.samples[index].0 = adjustPoint(bounds.samples[index].0, finalRatio)
            bounds.samples[index].1 = adjustPoint(bounds.samples[index].1, finalRatio)
        }
    }
    return maxWidth
}

private func adjustPoint(_ point: Int64, _ ratio: Int64) -> Int64 {
    func scaled(_ delta: Int64) -> Int64 {
        divRoundHalfAway(DoubleWidth.multiply(delta, Q32_ONE), ratio).toInt64()
    }
    if point > CENTER { return CENTER + scaled(point - CENTER) }
    if point > 0 { return CENTER - scaled(CENTER - point) }
    if point < 0 { return CENTER - scaled(-point + CENTER) }
    return point
}

private func minWidthY(_ samples: [(Int64, Int64)], _ top: Int64, _ height: Int64) -> Int64 {
    var bestInsetX: Int64 = 0
    var bestY: Int64 = 0
    for band in 1..<9 {
        let bandTop = top + divI64(height * Int64(band), 10)
        let bandBottom = top + divI64(height * Int64(band + 1), 10)
        var minimum: Int64 = .max
        for (x, y) in samples where x < CENTER && y > bandTop && y <= bandBottom && x < minimum {
            minimum = x
        }
        if minimum > bestInsetX {
            bestInsetX = minimum
            bestY = bandTop + divI64(height, 20)
        }
    }
    return bestY
}

extension Comparable {
    fileprivate func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}
