// Pinned numeric lookup tables for deterministic avatar generation.
//
// Port of crates/avatar-tables/src/lib.rs. The committed table bytes, rather
// than a platform's math library, define the trigonometric and easing results
// shared by every implementation. This keeps generated creatures reproducible
// across languages and operating systems.

public let SIN_QUARTER: [UInt32] = GeneratedTables.SIN_QUARTER
public let EASE_LINEAR: [UInt64] = GeneratedTables.EASE_LINEAR
public let EASE_EXPO: [UInt64] = GeneratedTables.EASE_EXPO
public let EASE_IN_CIRC: [UInt64] = GeneratedTables.EASE_IN_CIRC
public let EASE_IN_QUINT: [UInt64] = GeneratedTables.EASE_IN_QUINT
public let EASE_IN_OUT_CIRC: [UInt64] = GeneratedTables.EASE_IN_OUT_CIRC

public let PALETTE_BASE: [UInt8] = GeneratedPalette.PALETTE_BASE

let QUARTER_BAM16: UInt16 = 1 << 14
let QUARTER_SAMPLES = 4096

/// Returns the Q1.30 sine of a BAM16 angle from the committed quarter wave.
///
/// A BAM16 turn contains four 16,384-unit quadrants, while the committed
/// quarter wave has 4,096 samples. The low two angle bits select the same
/// sample, so `j = (angle within quadrant) >> 2`. Quadrants one and three
/// read `SIN_QUARTER[j]`; quadrants two and four conceptually read
/// `SIN_QUARTER[4096 - j]`. Because index 4096 is the omitted peak, the fold
/// returns `1 << 30` when `j == 0` before applying the quadrant's sign.
public func sinQ30(_ angle: UInt16) -> Int32 {
    let quadrant = Int(angle / QUARTER_BAM16)
    let index = Int((angle % QUARTER_BAM16) >> 2)

    let magnitude: Int32
    switch quadrant {
    case 0, 2:
        magnitude = Int32(SIN_QUARTER[index])
    default:
        // Quadrants 1 and 3 fold the quarter wave backwards. Index 0 would
        // read the omitted sample 4096 (the peak), so the fold returns the
        // peak exactly instead.
        magnitude = index == 0 ? 1 << 30 : Int32(SIN_QUARTER[QUARTER_SAMPLES - index])
    }

    return quadrant >= 2 ? -magnitude : magnitude
}

/// Returns the Q1.30 cosine of a BAM16 angle.
public func cosQ30(_ angle: UInt16) -> Int32 {
    sinQ30(angle &+ QUARTER_BAM16)
}

/// BLAKE3 hashes of the canonical little-endian bytes of every table.
///
/// Explicit byte order makes these pins comparable by implementations on any
/// architecture; the names identify the corresponding committed spec data.
/// Hashing the imported constants catches an accidental re-mint or a manual
/// transcription edit separately from any geometry drift.
public func tableHashes() -> [(name: String, hash: String)] {
    [
        ("SIN_QUARTER", hashLittleEndianUInt32(SIN_QUARTER)),
        ("EASE_LINEAR", hashLittleEndianUInt64(EASE_LINEAR)),
        ("EASE_EXPO", hashLittleEndianUInt64(EASE_EXPO)),
        ("EASE_IN_CIRC", hashLittleEndianUInt64(EASE_IN_CIRC)),
        ("EASE_IN_QUINT", hashLittleEndianUInt64(EASE_IN_QUINT)),
        ("EASE_IN_OUT_CIRC", hashLittleEndianUInt64(EASE_IN_OUT_CIRC)),
    ]
}

/// BLAKE3 hash of the palette base table bytes in slot/color/channel order.
///
/// Pin source: `palette_table_hash_is_pinned` in
/// crates/avatar-genome/src/decode.rs, which feeds each slot's six RGB
/// triples to the hasher in order — byte-identical to the flat layout here.
public func paletteHash() -> String {
    hexString(Blake3.hash(PALETTE_BASE))
}

func hashLittleEndianUInt32(_ table: [UInt32]) -> String {
    var bytes = [UInt8]()
    bytes.reserveCapacity(table.count * 4)
    for value in table {
        withUnsafeBytes(of: value.littleEndian) { bytes.append(contentsOf: $0) }
    }
    return hexString(Blake3.hash(bytes))
}

func hashLittleEndianUInt64(_ table: [UInt64]) -> String {
    var bytes = [UInt8]()
    bytes.reserveCapacity(table.count * 8)
    for value in table {
        withUnsafeBytes(of: value.littleEndian) { bytes.append(contentsOf: $0) }
    }
    return hexString(Blake3.hash(bytes))
}
