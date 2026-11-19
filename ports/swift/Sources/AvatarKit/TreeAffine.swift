// Drawing helpers for the transform operations carried by every render tree.

extension Transform {
    /// Composes SVG transform operations in their serialized order. Matrix
    /// products round half away from zero at each Q32 multiply, matching Rust.
    /// - Precondition: All intermediate products and sums must fit in `Int64`
    ///   after fixed-point rounding. The render-tree decoder checks this.
    public func affine() -> Affine {
        guard let result = try? checkedAffine() else {
            preconditionFailure("transform composition is outside the signed 64-bit range")
        }
        return result
    }

    func checkedAffine() throws -> Affine {
        var result = Affine(
            a: .one, b: Q32(0), c: Q32(0), d: .one,
            tx: 0, ty: 0)
        for op in ops {
            let next: Affine
            switch op {
            case .translate(let tx, let ty):
                next = Affine(a: .one, b: Q32(0), c: Q32(0), d: .one, tx: tx, ty: ty)
            case .scale(let sx, let sy):
                next = Affine(a: sx, b: Q32(0), c: Q32(0), d: sy, tx: 0, ty: 0)
            case .rotate(let angle, let cx, let cy):
                // The sine table is scaled by 2^30, so shifting by two bits cannot overflow.
                let cosine = Int64(cosQ30(angle.value)) << 2
                let sine = Int64(sinQ30(angle.value)) << 2
                next = try Affine(
                    a: Q32(cosine), b: Q32(sine), c: Q32(-sine), d: Q32(cosine),
                    tx: add(subtract(cx, q32Mul(cx, cosine)), q32Mul(cy, sine)),
                    ty: subtract(subtract(cy, q32Mul(cx, sine)), q32Mul(cy, cosine)))
            }
            result = try composeAffine(result, next)
        }
        return result
    }
}

private func composeAffine(_ lhs: Affine, _ rhs: Affine) throws -> Affine {
    try Affine(
        a: Q32(add(q32Mul(lhs.a.value, rhs.a.value), q32Mul(lhs.c.value, rhs.b.value))),
        b: Q32(add(q32Mul(lhs.b.value, rhs.a.value), q32Mul(lhs.d.value, rhs.b.value))),
        c: Q32(add(q32Mul(lhs.a.value, rhs.c.value), q32Mul(lhs.c.value, rhs.d.value))),
        d: Q32(add(q32Mul(lhs.b.value, rhs.c.value), q32Mul(lhs.d.value, rhs.d.value))),
        tx: add(add(lhs.tx, q32Mul(rhs.tx, lhs.a.value)), q32Mul(rhs.ty, lhs.c.value)),
        ty: add(add(lhs.ty, q32Mul(rhs.tx, lhs.b.value)), q32Mul(rhs.ty, lhs.d.value)))
}

private func transformOverflow() -> ParseError {
    ParseError("render-tree transform composition is outside the signed 64-bit range")
}

private func add(_ lhs: Int64, _ rhs: Int64) throws -> Int64 {
    let (result, overflow) = lhs.addingReportingOverflow(rhs)
    guard !overflow else { throw transformOverflow() }
    return result
}

private func subtract(_ lhs: Int64, _ rhs: Int64) throws -> Int64 {
    let (result, overflow) = lhs.subtractingReportingOverflow(rhs)
    guard !overflow else { throw transformOverflow() }
    return result
}

/// Multiplies signed integers with 32 fractional bits and rounds halves away
/// from zero. The full-width product preserves low bits needed for rounding.
func q32Mul(_ value: Int64, _ factor: Int64) throws -> Int64 {
    let product = value.magnitude.multipliedFullWidth(by: factor.magnitude)
    guard product.high <= UInt64.max >> 32 else { throw transformOverflow() }
    var magnitude = (product.high << 32) | (product.low >> 32)
    if (product.low & 0xFFFF_FFFF) >= (1 << 31) {
        let (rounded, overflow) = magnitude.addingReportingOverflow(1)
        guard !overflow else { throw transformOverflow() }
        magnitude = rounded
    }
    let negative = (value < 0) != (factor < 0)
    if negative && magnitude == Int64.min.magnitude { return Int64.min }
    guard let signed = Int64(exactly: magnitude) else { throw transformOverflow() }
    return negative ? -signed : signed
}

private let quarterBam16: UInt16 = 1 << 14
private let quarterSamples = 4_096

/// Returns the Q1.30 sine of a BAM16 angle from the committed quarter wave.
/// The low two angle bits share a sample; reverse quadrants supply the omitted
/// peak exactly before applying the sign of the half-turn.
public func sinQ30(_ angle: UInt16) -> Int32 {
    let quadrant = Int(angle / quarterBam16)
    let index = Int((angle % quarterBam16) >> 2)
    let magnitude: Int32
    switch quadrant {
    case 0, 2:
        magnitude = Int32(SIN_QUARTER[index])
    default:
        magnitude = index == 0 ? 1 << 30 : Int32(SIN_QUARTER[quarterSamples - index])
    }
    return quadrant >= 2 ? -magnitude : magnitude
}

/// Returns the Q1.30 cosine of a BAM16 angle.
public func cosQ30(_ angle: UInt16) -> Int32 {
    sinQ30(angle &+ quarterBam16)
}
