// Version-one avatar genome parsing, decoding, and identity derivation.
//
// Port of crates/avatar-genome: genome.rs (bit string + hex wire form),
// identity.rs (seat-identity XOF derivation), and decode.rs (the normative
// byte-to-phenotype mapping).

/// An error returned when text cannot represent one complete genome.
public enum GenomeError: Error, Equatable, CustomStringConvertible {
    case invalidLength(actual: Int)
    case invalidHexLength(actual: Int)
    case invalidHexDigit(index: Int, byte: UInt8)

    public var description: String {
        switch self {
        case .invalidLength(let actual):
            return "genome must contain \(Genome.byteCount) bytes, received \(actual)"
        case .invalidHexLength(let actual):
            return "genome hex must contain \(Genome.byteCount * 2) characters, received \(actual)"
        case .invalidHexDigit(let index, let byte):
            return "invalid hexadecimal byte 0x\(hexString([byte])) at index \(index)"
        }
    }
}

/// A version-one avatar genome: exactly 8,192 bits of DNA.
public struct Genome: Equatable, Sendable {
    static let byteCount = 1_024
    static let bitCount = byteCount * 8

    public let bytes: [UInt8]

    init(trustedBytes: [UInt8]) {
        self.bytes = trustedBytes
    }

    /// Constructs a genome from its canonical 1 KiB byte representation.
    public static func fromBytes(_ bytes: [UInt8]) throws -> Genome {
        guard bytes.count == byteCount else {
            throw GenomeError.invalidLength(actual: bytes.count)
        }
        return Genome(trustedBytes: bytes)
    }

    /// Parses the exact lowercase-or-uppercase hexadecimal wire representation.
    public static func fromHex(_ hex: String) throws -> Genome {
        let text = Array(hex.utf8)
        guard text.count == byteCount * 2 else {
            throw GenomeError.invalidHexLength(actual: text.count)
        }
        var bytes = [UInt8](repeating: 0, count: byteCount)
        for index in 0..<byteCount {
            let high = try hexNibble(text[index * 2], at: index * 2)
            let low = try hexNibble(text[index * 2 + 1], at: index * 2 + 1)
            bytes[index] = high << 4 | low
        }
        return Genome(trustedBytes: bytes)
    }

    /// Returns the canonical lowercase hexadecimal representation.
    public func toHex() -> String {
        hexString(bytes)
    }

    /// Reads an unsigned MSB-first field from the genome bit string.
    ///
    /// The fixed bit map only requests fields no wider than 64 bits. Keeping
    /// this reader internal prevents callers from treating versioned genome
    /// offsets as a stable public API instead of decoding through `decode`.
    func readBits(offset: Int, width: Int) -> UInt64 {
        precondition(width <= 64, "genome fields may be at most 64 bits wide")
        precondition(
            offset <= Genome.bitCount - width,
            "genome field must fit within 8,192 bits")

        var value: UInt64 = 0
        for bitOffset in offset ..< offset + width {
            let byte = bytes[bitOffset / 8]
            let bit = (byte >> (7 - bitOffset % 8)) & 1
            value = (value << 1) | UInt64(bit)
        }
        return value
    }
}

func hexNibble(_ byte: UInt8, at index: Int) throws -> UInt8 {
    switch byte {
    case UInt8(ascii: "0") ... UInt8(ascii: "9"):
        return byte - UInt8(ascii: "0")
    case UInt8(ascii: "a") ... UInt8(ascii: "f"):
        return byte - UInt8(ascii: "a") + 10
    case UInt8(ascii: "A") ... UInt8(ascii: "F"):
        return byte - UInt8(ascii: "A") + 10
    default:
        throw GenomeError.invalidHexDigit(index: index, byte: byte)
    }
}

// MARK: Identity derivation

/// The durable source used to derive a seat's initial genome.
///
/// Live-fleet measurement found every candidate name source user-editable,
/// non-unique, or session-derived. Hashing a name would make a rename re-roll
/// a creature and let a collision merge creatures, so names remain presentation
/// only. Identity is the registry's one opaque, stable value; adding a name to
/// this enum or its derivation would reintroduce those failures.
public enum SeatIdentity: Equatable {
    case registered(projectId: String)
    case pathFallback(canonicalPath: String)
}

let IDENTITY_DOMAIN = "cortexkit-avatar/identity-core/v2"

/// Derives a complete initial genome from a seat identity.
///
/// Domain separation keeps this XOF stream distinct from every other fleet
/// hash. Path fallback adds its own prefix so a mutable path can never
/// produce the same initial DNA as a registered project identifier with
/// equal bytes.
public func deriveGenome(_ identity: SeatIdentity) -> Genome {
    var input = Array(IDENTITY_DOMAIN.utf8)
    input.append(0)
    switch identity {
    case let .registered(projectId):
        input.append(contentsOf: projectId.utf8)
    case let .pathFallback(canonicalPath):
        input.append(contentsOf: Array("path-fallback".utf8))
        input.append(0)
        input.append(contentsOf: canonicalPath.utf8)
    }
    // The XOF fills reserved bits too: they are DNA, dormant today and
    // meaningful under future decoders (design note 001's dormant-region
    // rule).
    return Genome(trustedBytes: Blake3.xof(input, length: Genome.byteCount))
}

// MARK: Decoding

let GENOME_VERSION: UInt16 = 1
let BODY_START = 0
let PALETTE_START = 512
let FACE_START = 576
let PATTERN_START = 1_088
let ASYMMETRY_START = 1_216
let LAYERS_START = 1_280
let ANIMATION_START = 1_536
let Q32_ONE: Int64 = 1 << 32
let Q32_SIZE_MIN: Int64 = 3_006_477_107  // 0.7, rounded to Q32.32.
let Q32_SIZE_MAX: Int64 = 6_012_954_214  // 1.4, rounded to Q32.32.
let Q32_PUPIL_MIN: Int64 = 644_245_094  // 0.15, rounded to Q32.32.
let Q32_PUPIL_MAX: Int64 = 2_576_980_378  // 0.6, rounded to Q32.32.
let Q32_EYEBALL_MIN: Int64 = 2_576_980_378  // 0.6, rounded to Q32.32.
let Q32_LASH_LENGTH_MAX: Int64 = 3_865_470_566  // 0.9, rounded to Q32.32.
let Q32_LASH_STROKE_MIN: Int64 = 85_899_346  // 0.02, rounded to Q32.32.
let Q32_LASH_STROKE_MAX: Int64 = 515_396_076  // 0.12, rounded to Q32.32.
let Q32_REACTIVITY_MIN: Int64 = Q32_ONE / 2
let Q32_REACTIVITY_MAX: Int64 = Q32_ONE + Q32_ONE / 2

enum MappedField {
    case unit
    case bodySize
    case bodyPointCoordinate
    case bodyPointTension
    case eyeSize
    case eyeIpd
    case pupilScale
    case eyeballRadiusScale
    case lashLengthScale
    case lashStrokeScale
    case placementCoefficient
    case breathPeriod
    case blinkInterval
    case moodReactivity
}

struct Mapping {
    let table: [UInt64]
    let lo: Int64
    let hi: Int64
}

/// The normative v1 byte-to-value mapping table from design note 003.
///
/// These table/range pairs define observable phenotype values without
/// changing genome offsets. Altering any entry changes generated avatars and
/// therefore requires a generator-version bump and renewed compatibility
/// vectors.
func mapping(_ field: MappedField, bodySize: Int64) -> Mapping {
    switch field {
    case .unit:
        return Mapping(table: EASE_LINEAR, lo: 0, hi: Q32_ONE)
    case .bodySize:
        return Mapping(table: EASE_LINEAR, lo: 714_285, hi: 833_333)
    case .bodyPointCoordinate:
        return Mapping(table: EASE_LINEAR, lo: 0, hi: bodySize)
    case .bodyPointTension:
        return Mapping(table: EASE_LINEAR, lo: 0, hi: 3 * Q32_ONE / 2)
    case .eyeSize, .placementCoefficient:
        return Mapping(table: EASE_LINEAR, lo: Q32_SIZE_MIN, hi: Q32_SIZE_MAX)
    case .eyeIpd:
        return Mapping(table: EASE_LINEAR, lo: Q32_ONE / 4, hi: 3 * Q32_ONE / 4)
    case .pupilScale:
        return Mapping(table: EASE_IN_CIRC, lo: Q32_PUPIL_MIN, hi: Q32_PUPIL_MAX)
    case .eyeballRadiusScale:
        return Mapping(table: EASE_LINEAR, lo: Q32_EYEBALL_MIN, hi: Q32_ONE)
    case .lashLengthScale:
        return Mapping(table: EASE_EXPO, lo: 0, hi: Q32_LASH_LENGTH_MAX)
    case .lashStrokeScale:
        return Mapping(
            table: EASE_LINEAR, lo: Q32_LASH_STROKE_MIN, hi: Q32_LASH_STROKE_MAX)
    case .breathPeriod:
        return Mapping(table: EASE_LINEAR, lo: 90, hi: 240)
    case .blinkInterval:
        return Mapping(table: EASE_LINEAR, lo: 80, hi: 400)
    case .moodReactivity:
        return Mapping(table: EASE_LINEAR, lo: Q32_REACTIVITY_MIN, hi: Q32_REACTIVITY_MAX)
    }
}

/// Decodes every genome into one complete phenotype.
///
/// Selector reduction and byte mappings make this a total function:
/// unassigned or future-reserved bits remain DNA rather than invalid input,
/// so no genome needs an error path during rendering.
public func decode(_ genome: Genome) -> Phenotype {
    // Floor of 4 (generator_version 2): flatter bodies cannot carry a face.
    // Transcribed from crates/avatar-genome/src/decode.rs.
    let pointCount = UInt8(4 + genome.readBits(offset: BODY_START, width: 4) % 13)
    let bodySize = map(.bodySize, byteAtBits(genome, BODY_START + 4))
    let points: [BodyPoint] = (0..<16).map { index in
        guard index < Int(pointCount) else {
            return BodyPoint(x: Milli(0), y: Milli(0), tension: Q32(0))
        }
        let offset = BODY_START + 12 + index * 24
        return BodyPoint(
            x: Milli(map(.bodyPointCoordinate, byteAtBits(genome, offset), bodySize)),
            y: Milli(map(.bodyPointCoordinate, byteAtBits(genome, offset + 8), bodySize)),
            tension: Q32(map(.bodyPointTension, byteAtBits(genome, offset + 16), bodySize)))
    }

    return Phenotype(
        genome_version: GENOME_VERSION,
        body: Body(point_count: pointCount, size: Milli(bodySize), points: points),
        palette: decodePalette(genome),
        face: decodeFace(genome),
        pattern: decodePattern(genome),
        animation: decodeAnimation(genome),
        mood_response: decodeMoodResponse(genome),
        asymmetry: Asymmetry(
            deltas: (0..<8).map { index in
                Q32(map(.unit, byteAt(genome, ASYMMETRY_START, index)))
            }),
        layers: Layers(
            count: byteAt(genome, LAYERS_START, 0),
            params: (0..<16).map { index in
                Q32(map(.unit, byteAt(genome, LAYERS_START, index + 1)))
            }))
}

func decodePalette(_ genome: Genome) -> Palette {
    let slot = Int(genome.readBits(offset: PALETTE_START, width: 11))
    // Background variants are dormant in v1: preserving their DNA today means
    // a later background treatment can activate them without reshaping
    // genomes.
    _ = genome.readBits(offset: PALETTE_START + 11, width: 3)

    func baseColor(_ index: Int) -> Rgb {
        let offset = (slot * 6 + index) * 3
        return Rgb(
            r: PALETTE_BASE[offset],
            g: PALETTE_BASE[offset + 1],
            b: PALETTE_BASE[offset + 2])
    }

    let primary = baseColor(1)
    let harmonic1 = baseColor(0)
    let back = baseColor(5)

    return Palette(
        back: back,
        primary: primary,
        primary_lighter: shade(primary, 30),
        primary_darkish: shade(primary, -20),
        primary_darker: shade(primary, -40),
        primary_darkest: shade(primary, -80),
        harmonic1: harmonic1,
        harmonic1_darkish: shade(harmonic1, -15),
        harmonic1_darkest: shade(harmonic1, -30),
        harmonic2: baseColor(3),
        harmonic3: baseColor(4),
        harmonic4: back,
        secondary_darker: shade(back, -80),
        dark: shade(harmonic1, -250),
        white: Rgb(r: 255, g: 255, b: 255))
}

func decodeFace(_ genome: Genome) -> Face {
    let flags = byteAt(genome, FACE_START, 5)
    let eyes = Eyes(
        position_mode: byteAt(genome, FACE_START, 0) % 5,
        size_h: Q32(map(.eyeSize, byteAt(genome, FACE_START, 1))),
        ipd: Q32(map(.eyeIpd, byteAt(genome, FACE_START, 2))),
        height_upper: unitQ32(genome, FACE_START, 3),
        height_lower: unitQ32(genome, FACE_START, 4),
        height_bias_gate: namedFlag(flags, 0),
        height_bias: unitQ32(genome, FACE_START, 6),
        vertical_pull_gate: namedFlag(flags, 1),
        vertical_pull: unitQ32(genome, FACE_START, 7),
        contour_inset1: unitQ32(genome, FACE_START, 8),
        contour_inset2: unitQ32(genome, FACE_START, 9),
        eyeball_radius_scale: Q32(map(.eyeballRadiusScale, byteAt(genome, FACE_START, 10))),
        pupil_scale: Q32(map(.pupilScale, byteAt(genome, FACE_START, 11))),
        iris_shade_opacity: unitQ32(genome, FACE_START, 12),
        iris_shade_reduce: unitQ32(genome, FACE_START, 13),
        iris_saturation_reduce: unitQ32(genome, FACE_START, 14),
        lid_close_upper_neutral: unitQ32(genome, FACE_START, 15),
        lid_close_lower_neutral: unitQ32(genome, FACE_START, 16),
        lid_extension_gate: byteAt(genome, FACE_START, 17),
        lid_extension_side: byteAt(genome, FACE_START, 18),
        noise_freq_h: unitQ32(genome, FACE_START, 19),
        noise_freq_v: unitQ32(genome, FACE_START, 20),
        noise_seed: byteAt(genome, FACE_START, 21),
        rotation_seed: byteAt(genome, FACE_START, 22),
        lid_color_gene: byteAt(genome, FACE_START, 23),
        iris_color_gene: byteAt(genome, FACE_START, 24))
    let brows = Brows(
        end_height1: unitQ32(genome, FACE_START, 25),
        mid_x: unitQ32(genome, FACE_START, 26),
        end_height2: unitQ32(genome, FACE_START, 27),
        tensions: (0..<3).map { index in unitQ32(genome, FACE_START, index + 28) },
        width_coupling: unitQ32(genome, FACE_START, 31),
        clearance: unitQ32(genome, FACE_START, 32))
    let lashes = Lashes(
        count: byteAt(genome, FACE_START, 33) % 12,
        removed: byteAt(genome, FACE_START, 34) % 4,
        length_scale: Q32(map(.lashLengthScale, byteAt(genome, FACE_START, 35))),
        stroke_scale: Q32(map(.lashStrokeScale, byteAt(genome, FACE_START, 36))),
        direction: byteAt(genome, FACE_START, 37))
    let mouth = Mouth(
        family: byteAt(genome, FACE_START, 38) % 8,
        width_scale_gate: namedFlag(flags, 2),
        width_scale: unitQ32(genome, FACE_START, 39),
        aspect_widen_gate: namedFlag(flags, 3),
        params: (0..<8).map { index in unitQ32(genome, FACE_START, index + 40) })
    let placement = FacePlacement(
        width_coeff: Q32(map(.placementCoefficient, byteAt(genome, FACE_START, 48))),
        height_coeff: Q32(map(.placementCoefficient, byteAt(genome, FACE_START, 49)))
    )
    return Face(eyes: eyes, brows: brows, lashes: lashes, mouth: mouth, placement: placement)
}

func decodePattern(_ genome: Genome) -> Pattern {
    Pattern(
        class: byteAt(genome, PATTERN_START, 0) % 6,
        circler: Circler(
            point_reduce: byteAt(genome, PATTERN_START, 1),
            size: unitQ32(genome, PATTERN_START, 2),
            y_offset_div: unitQ32(genome, PATTERN_START, 3),
            tension: unitQ32(genome, PATTERN_START, 4),
            pull_min: unitQ32(genome, PATTERN_START, 5)),
        voronoi: Voronoi(
            index: byteAt(genome, PATTERN_START, 6),
            rotation: Bam16(UInt16(genome.readBits(offset: PATTERN_START + 7 * 8, width: 16))),
            scale: unitQ32(genome, PATTERN_START, 9)),
        cut: Cut(
            spread: unitQ32(genome, PATTERN_START, 10),
            y_adjust_div: unitQ32(genome, PATTERN_START, 11)))
}

func decodeAnimation(_ genome: Genome) -> Animation {
    Animation(
        breath_period: UInt16(map(.breathPeriod, byteAt(genome, ANIMATION_START, 0))),
        breath_amplitude: unitQ32(genome, ANIMATION_START, 1),
        blink_interval: UInt16(map(.blinkInterval, byteAt(genome, ANIMATION_START, 2))),
        blink_jitter_seed: byteAt(genome, ANIMATION_START, 3),
        mood_transition_tempo: unitQ32(genome, ANIMATION_START, 4))
}

func decodeMoodResponse(_ genome: Genome) -> MoodResponse {
    MoodResponse(
        lid_reactivity: Q32(map(.moodReactivity, byteAt(genome, ANIMATION_START, 5))),
        brow_reactivity: Q32(map(.moodReactivity, byteAt(genome, ANIMATION_START, 6))),
        eye_rotation_reactivity: Q32(map(.moodReactivity, byteAt(genome, ANIMATION_START, 7))),
        mouth_reactivity: Q32(map(.moodReactivity, byteAt(genome, ANIMATION_START, 8))))
}

func byteAt(_ genome: Genome, _ regionStart: Int, _ byteIndex: Int) -> UInt8 {
    byteAtBits(genome, regionStart + byteIndex * 8)
}

func byteAtBits(_ genome: Genome, _ offset: Int) -> UInt8 {
    UInt8(genome.readBits(offset: offset, width: 8))
}

func unitQ32(_ genome: Genome, _ regionStart: Int, _ byteIndex: Int) -> Q32 {
    Q32(map(.unit, byteAt(genome, regionStart, byteIndex)))
}

func namedFlag(_ flags: UInt8, _ bit: UInt8) -> Bool {
    flags & (1 << (7 - bit)) != 0
}

func map(_ field: MappedField, _ byte: UInt8, _ bodySize: Int64 = 0) -> Int64 {
    let fieldMapping = mapping(field, bodySize: bodySize)
    // The product widens before the single pinned narrowing, exactly like
    // the Rust decoder's i128 arithmetic: delta fits i64, the table entry is
    // at most Q32_ONE, and their product can exceed i64.
    let delta = fieldMapping.hi - fieldMapping.lo
    let scaled = divRoundHalfAway(
        DoubleWidth.multiply(delta, Int64(fieldMapping.table[Int(byte)])),
        Q32_ONE)
    return fieldMapping.lo + scaled.toInt64()
}

func shade(_ color: Rgb, _ amount: Int16) -> Rgb {
    Rgb(
        r: shadeChannel(color.r, amount),
        g: shadeChannel(color.g, amount),
        b: shadeChannel(color.b, amount))
}

func shadeChannel(_ channel: UInt8, _ amount: Int16) -> UInt8 {
    UInt8(clamping: Int16(channel) + amount)
}
