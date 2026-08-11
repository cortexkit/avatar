// Shared fixed-point arithmetic for the port.
//
// All deterministic math widens i64 products to i128 before narrowing through
// one pinned rounding rule (design note 001 §2): half away from zero. Swift
// 5.9 has no Int128, so the two i128-shaped operations the port needs —
// widening i64×i64 multiplication and half-away division of the wide product
// — run on `DoubleWidth`, a signed 128-bit two's-complement (hi, lo) pair.
//
// The division primitive is `divRoundHalfAway`, a transcription of
// `div_round_half_away` in crates/avatar-tables/src/lib.rs. Native Swift `/`
// truncates toward zero exactly like Rust, which is precisely why signed
// halves must never reach it: the explicit remainder test below is what gives
// negative halves their required direction.

/// A signed 128-bit integer in two's complement: value == hi × 2^64 + lo.
public struct DoubleWidth: Equatable, CustomStringConvertible {
    public var hi: Int64
    public var lo: UInt64

    public init(hi: Int64, lo: UInt64) {
        self.hi = hi
        self.lo = lo
    }

    /// Sign-extends a 64-bit value into the wide representation.
    public init(_ value: Int64) {
        self.hi = value < 0 ? -1 : 0
        self.lo = UInt64(bitPattern: value)
    }

    public var isNegative: Bool { hi < 0 }

    public var isZero: Bool { hi == 0 && lo == 0 }

    /// Two's-complement negation. Undefined for the 128-bit minimum value,
    /// which the avatar value domain never approaches: wide products here are
    /// bounded by roughly 2^63 × 2^32.
    public func negated() -> DoubleWidth {
        let lo = ~self.lo &+ 1
        let hi = ~self.hi &+ (lo == 0 ? 1 : 0)
        return DoubleWidth(hi: hi, lo: lo)
    }

    /// Adds a 64-bit value, sign-extended. Covers the ±1 rounding step in
    /// `divRoundHalfAway` and small offsets such as `lo + scaled`.
    public func adding(_ value: Int64) -> DoubleWidth {
        let (lo, overflow) = self.lo.addingReportingOverflow(UInt64(bitPattern: value))
        var hi = self.hi &+ (value < 0 ? -1 : 0)
        if overflow {
            hi &+= 1
        }
        return DoubleWidth(hi: hi, lo: lo)
    }

    /// Bit `index` of the 128-bit representation (0 = least significant).
    func bit(_ index: Int) -> UInt64 {
        if index < 64 {
            return (lo >> UInt64(index)) & 1
        }
        return (UInt64(bitPattern: hi) >> UInt64(index - 64)) & 1
    }

    /// Widening multiplication of two 64-bit factors — the i128 product shape
    /// every Q32 narrowing passes through.
    public static func multiply(_ lhs: Int64, _ rhs: Int64) -> DoubleWidth {
        let lhsMagnitude = magnitude(of: lhs)
        let rhsMagnitude = magnitude(of: rhs)
        // Unsigned 64×64→128 product via 32-bit limbs. Every intermediate
        // fits in UInt64: limb products stay below 2^64 and the largest
        // summed term (three limb products plus a carry) is bounded by the
        // true high word, which cannot exceed 2^64 - 2.
        let aLo = lhsMagnitude & 0xFFFF_FFFF
        let aHi = lhsMagnitude >> 32
        let bLo = rhsMagnitude & 0xFFFF_FFFF
        let bHi = rhsMagnitude >> 32
        let ll = aLo * bLo
        let lh = aLo * bHi
        let hl = aHi * bLo
        let hh = aHi * bHi
        let mid = (ll >> 32) + (lh & 0xFFFF_FFFF) + (hl & 0xFFFF_FFFF)
        let lo = (mid << 32) | (ll & 0xFFFF_FFFF)
        let hi = hh &+ (lh >> 32) &+ (hl >> 32) &+ (mid >> 32)
        let product = DoubleWidth(hi: Int64(bitPattern: hi), lo: lo)
        return (lhs < 0) != (rhs < 0) ? product.negated() : product
    }

    /// Divides by a positive 64-bit denominator with Rust's i128 `/` and `%`
    /// semantics: quotient truncated toward zero, remainder carrying the
    /// dividend's sign.
    ///
    /// Long division, MSB first. The running remainder always stays below the
    /// divisor, and the divisor is at most `Int64.max`, so shifting the
    /// remainder left by one cannot overflow UInt64.
    public static func divMod(
        _ numerator: DoubleWidth, _ denominator: Int64
    ) -> (quotient: DoubleWidth, remainder: Int64) {
        precondition(denominator > 0, "denominator must be positive")
        let divisor = UInt64(denominator)
        let negative = numerator.isNegative
        let magnitude = negative ? numerator.negated() : numerator

        var remainder: UInt64 = 0
        var quotientHi: UInt64 = 0
        var quotientLo: UInt64 = 0
        for index in stride(from: 127, through: 0, by: -1) {
            remainder = (remainder << 1) | magnitude.bit(index)
            quotientHi = (quotientHi << 1) | (quotientLo >> 63)
            quotientLo <<= 1
            if remainder >= divisor {
                remainder -= divisor
                quotientLo |= 1
            }
        }

        let quotient = DoubleWidth(hi: Int64(bitPattern: quotientHi), lo: quotientLo)
        let rest = Int64(bitPattern: remainder)
        return (negative ? quotient.negated() : quotient, negative ? -rest : rest)
    }

    /// Narrows to Int64 like Rust's `as i64`: keeps the low 64 bits. Every
    /// call site in the port bounds-checks by construction (table ranges are
    /// far inside i64), so no trap is needed.
    public func toInt64() -> Int64 {
        Int64(bitPattern: lo)
    }

    public var description: String {
        if isZero {
            return "0"
        }
        let magnitude = isNegative ? negated() : self
        var digits: [Character] = []
        var rest = magnitude
        while !rest.isZero {
            let (quotient, remainder) = DoubleWidth.divMod(rest, 10)
            digits.append(Character(String(remainder)))
            rest = quotient
        }
        if isNegative {
            digits.append("-")
        }
        return String(digits.reversed())
    }

    private static func magnitude(of value: Int64) -> UInt64 {
        value < 0 ? ~UInt64(bitPattern: value) &+ 1 : UInt64(bitPattern: value)
    }
}

/// Divides `numerator` by positive `denominator`, rounding halves away from
/// zero — the single pinned rounding rule shared by every port.
///
/// Transcribed from `div_round_half_away` in crates/avatar-tables/src/lib.rs,
/// including the signed test table it carries there. The half threshold is
/// `den / 2 + den % 2` so odd denominators round at the true midpoint rather
/// than the floored one.
public func divRoundHalfAway(_ numerator: DoubleWidth, _ denominator: Int64) -> DoubleWidth {
    precondition(denominator > 0, "divRoundHalfAway requires a positive denominator")
    let (quotient, remainder) = DoubleWidth.divMod(numerator, denominator)
    let halfThreshold = denominator / 2 + denominator % 2
    if remainder >= halfThreshold {
        return quotient.adding(1)
    }
    if remainder <= -halfThreshold {
        return quotient.adding(-1)
    }
    return quotient
}
