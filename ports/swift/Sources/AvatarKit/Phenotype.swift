// Typed phenotype data and its canonical wire representation.
//
// Port of crates/avatar-phenotype/src/lib.rs. Values crossing the phenotype
// boundary are integers in fixed units. Keeping the units in distinct
// newtypes prevents renderers and decoders from accidentally treating a raw
// coefficient as a canvas coordinate.

/// A canvas length or coordinate in milliunits.
public struct Milli: Equatable {
    public var value: Int64

    public init(_ value: Int64) {
        self.value = value
    }
}

/// A raw signed Q32.32 fixed-point coefficient.
public struct Q32: Equatable {
    public var value: Int64

    public init(_ value: Int64) {
        self.value = value
    }

    /// The fixed-point value 1.0 (32 fraction bits).
    public static let one = Q32(1 << 32)
}

/// An angle expressed as a binary angle measurement with 65,536 units per turn.
public struct Bam16: Equatable {
    public var value: UInt16

    public init(_ value: UInt16) {
        self.value = value
    }
}

/// An 8-bit RGB color.
public struct Rgb: Equatable {
    public var r: UInt8
    public var g: UInt8
    public var b: UInt8

    public init(r: UInt8, g: UInt8, b: UInt8) {
        self.r = r
        self.g = g
        self.b = b
    }
}

public struct Phenotype: Equatable {
    public var genome_version: UInt16
    public var body: Body
    public var palette: Palette
    public var face: Face
    public var pattern: Pattern
    public var animation: Animation
    public var mood_response: MoodResponse
    public var asymmetry: Asymmetry
    public var layers: Layers
}

public struct Body: Equatable {
    public var point_count: UInt8
    public var size: Milli
    public var points: [BodyPoint]
}

public struct BodyPoint: Equatable {
    public var x: Milli
    public var y: Milli
    public var tension: Q32
}

public struct Palette: Equatable {
    public var back: Rgb
    public var primary: Rgb
    public var primary_lighter: Rgb
    public var primary_darkish: Rgb
    public var primary_darker: Rgb
    public var primary_darkest: Rgb
    public var harmonic1: Rgb
    public var harmonic1_darkish: Rgb
    public var harmonic1_darkest: Rgb
    public var harmonic2: Rgb
    public var harmonic3: Rgb
    public var harmonic4: Rgb
    public var secondary_darker: Rgb
    public var dark: Rgb
    public var white: Rgb
}

public struct Face: Equatable {
    public var eyes: Eyes
    public var brows: Brows
    public var lashes: Lashes
    public var mouth: Mouth
    public var placement: FacePlacement
}

public struct Eyes: Equatable {
    public var position_mode: UInt8
    // Coefficients, not absolute lengths: eye size and inter-pupil distance
    // scale off body geometry that only exists after render-time spline
    // flattening, so the phenotype cannot carry them as milliunits.
    public var size_h: Q32
    public var ipd: Q32
    public var height_upper: Q32
    public var height_lower: Q32
    public var height_bias_gate: Bool
    public var height_bias: Q32
    public var vertical_pull_gate: Bool
    public var vertical_pull: Q32
    public var contour_inset1: Q32
    public var contour_inset2: Q32
    public var eyeball_radius_scale: Q32
    public var pupil_scale: Q32
    public var iris_shade_opacity: Q32
    public var iris_shade_reduce: Q32
    public var iris_saturation_reduce: Q32
    public var lid_close_upper_neutral: Q32
    public var lid_close_lower_neutral: Q32
    public var lid_extension_gate: UInt8
    public var lid_extension_side: UInt8
    public var noise_freq_h: Q32
    public var noise_freq_v: Q32
    public var noise_seed: UInt8
    public var rotation_seed: UInt8
    public var lid_color_gene: UInt8
    public var iris_color_gene: UInt8
}

public struct Brows: Equatable {
    public var end_height1: Q32
    public var mid_x: Q32
    public var end_height2: Q32
    public var tensions: [Q32]
    public var width_coupling: Q32
    public var clearance: Q32
}

public struct Lashes: Equatable {
    public var count: UInt8
    public var removed: UInt8
    // Scales relative to eye height, resolved at render time (see Eyes:
    // absolute lash geometry depends on flattened body dimensions).
    public var length_scale: Q32
    public var stroke_scale: Q32
    public var direction: UInt8
}

public struct Mouth: Equatable {
    public var family: UInt8
    public var width_scale_gate: Bool
    public var width_scale: Q32
    public var aspect_widen_gate: Bool
    public var params: [Q32]
}

public struct FacePlacement: Equatable {
    public var width_coeff: Q32
    public var height_coeff: Q32
}

public struct Pattern: Equatable {
    public var `class`: UInt8
    public var circler: Circler
    public var voronoi: Voronoi
    public var cut: Cut
}

public struct Circler: Equatable {
    public var point_reduce: UInt8
    public var size: Q32
    public var y_offset_div: Q32
    public var tension: Q32
    public var pull_min: Q32
}

public struct Voronoi: Equatable {
    public var index: UInt8
    public var rotation: Bam16
    public var scale: Q32
}

public struct Cut: Equatable {
    public var spread: Q32
    public var y_adjust_div: Q32
}

public struct Animation: Equatable {
    public var breath_period: UInt16
    public var breath_amplitude: Q32
    public var blink_interval: UInt16
    public var blink_jitter_seed: UInt8
    public var mood_transition_tempo: Q32
}

public struct MoodResponse: Equatable {
    public var lid_reactivity: Q32
    public var brow_reactivity: Q32
    public var eye_rotation_reactivity: Q32
    public var mouth_reactivity: Q32
}

public struct Asymmetry: Equatable {
    public var deltas: [Q32]
}

public struct Layers: Equatable {
    public var count: UInt8
    public var params: [Q32]
}

// MARK: Canonical serialization

/// Serializes a phenotype as avatar canonical JSON v1.
///
/// This writer is deliberately hand-rolled instead of using a JSON library:
/// canonical key ordering and byte shape are part of the wire contract, not
/// an incidental property of a dependency's serializer. Gate booleans are
/// encoded as integer `0` or `1` because canonical JSON v1 permits no boolean
/// token.
public func toCanonicalJson(_ phenotype: Phenotype) -> [UInt8] {
    var writer = CanonicalWriter()
    writePhenotype(&writer, phenotype)
    return writer.bytes
}

/// A byte-exact canonical JSON v1 writer: UTF-8, lexicographically sorted
/// keys (the call sites below emit them in sorted order), no insignificant
/// whitespace, values restricted to integers, strings, arrays, and objects.
struct CanonicalWriter {
    private(set) var bytes: [UInt8] = []
    private var containerIsFirst: [Bool] = []
    private var containerIsArray: [Bool] = []

    mutating func beginObject() {
        if containerIsArray.last == true {
            beforeValue()
        }
        bytes.append(UInt8(ascii: "{"))
        containerIsFirst.append(true)
        containerIsArray.append(false)
    }

    mutating func endObject() {
        bytes.append(UInt8(ascii: "}"))
        containerIsFirst.removeLast()
        containerIsArray.removeLast()
    }

    mutating func beginArray() {
        if containerIsArray.last == true {
            beforeValue()
        }
        bytes.append(UInt8(ascii: "["))
        containerIsFirst.append(true)
        containerIsArray.append(true)
    }

    mutating func endArray() {
        bytes.append(UInt8(ascii: "]"))
        containerIsFirst.removeLast()
        containerIsArray.removeLast()
    }

    /// Emits the separator and key that precede an object member's value.
    mutating func fieldKey(_ key: String) {
        beforeValue()
        bytes.append(UInt8(ascii: "\""))
        bytes.append(contentsOf: key.utf8)
        bytes.append(contentsOf: [UInt8(ascii: "\""), UInt8(ascii: ":")])
    }

    mutating func integer<I: FixedWidthInteger>(_ value: I) {
        if containerIsArray.last == true {
            beforeValue()
        }
        bytes.append(contentsOf: String(value).utf8)
    }

    mutating func gate(_ value: Bool) {
        integer(value ? UInt8(1) : UInt8(0))
    }

    private mutating func beforeValue() {
        let index = containerIsFirst.count - 1
        if !containerIsFirst[index] {
            bytes.append(UInt8(ascii: ","))
        }
        containerIsFirst[index] = false
    }
}

func writePhenotype(_ writer: inout CanonicalWriter, _ phenotype: Phenotype) {
    writer.beginObject()
    writer.fieldKey("animation")
    writeAnimation(&writer, phenotype.animation)
    writer.fieldKey("asymmetry")
    writeAsymmetry(&writer, phenotype.asymmetry)
    writer.fieldKey("body")
    writeBody(&writer, phenotype.body)
    writer.fieldKey("face")
    writeFace(&writer, phenotype.face)
    writer.fieldKey("genome_version")
    writer.integer(phenotype.genome_version)
    writer.fieldKey("layers")
    writeLayers(&writer, phenotype.layers)
    writer.fieldKey("mood_response")
    writeMoodResponse(&writer, phenotype.mood_response)
    writer.fieldKey("palette")
    writePalette(&writer, phenotype.palette)
    writer.fieldKey("pattern")
    writePattern(&writer, phenotype.pattern)
    writer.endObject()
}

func writeBody(_ writer: inout CanonicalWriter, _ body: Body) {
    writer.beginObject()
    writer.fieldKey("point_count")
    writer.integer(body.point_count)
    writer.fieldKey("points")
    writer.beginArray()
    for point in body.points {
        writeBodyPoint(&writer, point)
    }
    writer.endArray()
    writer.fieldKey("size")
    writer.integer(body.size.value)
    writer.endObject()
}

func writeBodyPoint(_ writer: inout CanonicalWriter, _ point: BodyPoint) {
    writer.beginObject()
    writer.fieldKey("tension")
    writer.integer(point.tension.value)
    writer.fieldKey("x")
    writer.integer(point.x.value)
    writer.fieldKey("y")
    writer.integer(point.y.value)
    writer.endObject()
}

func writePalette(_ writer: inout CanonicalWriter, _ palette: Palette) {
    writer.beginObject()
    writer.fieldKey("back")
    writeRgb(&writer, palette.back)
    writer.fieldKey("dark")
    writeRgb(&writer, palette.dark)
    writer.fieldKey("harmonic1")
    writeRgb(&writer, palette.harmonic1)
    writer.fieldKey("harmonic1_darkish")
    writeRgb(&writer, palette.harmonic1_darkish)
    writer.fieldKey("harmonic1_darkest")
    writeRgb(&writer, palette.harmonic1_darkest)
    writer.fieldKey("harmonic2")
    writeRgb(&writer, palette.harmonic2)
    writer.fieldKey("harmonic3")
    writeRgb(&writer, palette.harmonic3)
    writer.fieldKey("harmonic4")
    writeRgb(&writer, palette.harmonic4)
    writer.fieldKey("primary")
    writeRgb(&writer, palette.primary)
    writer.fieldKey("primary_darker")
    writeRgb(&writer, palette.primary_darker)
    writer.fieldKey("primary_darkest")
    writeRgb(&writer, palette.primary_darkest)
    writer.fieldKey("primary_darkish")
    writeRgb(&writer, palette.primary_darkish)
    writer.fieldKey("primary_lighter")
    writeRgb(&writer, palette.primary_lighter)
    writer.fieldKey("secondary_darker")
    writeRgb(&writer, palette.secondary_darker)
    writer.fieldKey("white")
    writeRgb(&writer, palette.white)
    writer.endObject()
}

func writeRgb(_ writer: inout CanonicalWriter, _ rgb: Rgb) {
    writer.beginObject()
    writer.fieldKey("b")
    writer.integer(rgb.b)
    writer.fieldKey("g")
    writer.integer(rgb.g)
    writer.fieldKey("r")
    writer.integer(rgb.r)
    writer.endObject()
}

func writeFace(_ writer: inout CanonicalWriter, _ face: Face) {
    writer.beginObject()
    writer.fieldKey("brows")
    writeBrows(&writer, face.brows)
    writer.fieldKey("eyes")
    writeEyes(&writer, face.eyes)
    writer.fieldKey("lashes")
    writeLashes(&writer, face.lashes)
    writer.fieldKey("mouth")
    writeMouth(&writer, face.mouth)
    writer.fieldKey("placement")
    writeFacePlacement(&writer, face.placement)
    writer.endObject()
}

func writeEyes(_ writer: inout CanonicalWriter, _ eyes: Eyes) {
    writer.beginObject()
    writer.fieldKey("contour_inset1")
    writer.integer(eyes.contour_inset1.value)
    writer.fieldKey("contour_inset2")
    writer.integer(eyes.contour_inset2.value)
    writer.fieldKey("eyeball_radius_scale")
    writer.integer(eyes.eyeball_radius_scale.value)
    writer.fieldKey("height_bias")
    writer.integer(eyes.height_bias.value)
    writer.fieldKey("height_bias_gate")
    writer.gate(eyes.height_bias_gate)
    writer.fieldKey("height_lower")
    writer.integer(eyes.height_lower.value)
    writer.fieldKey("height_upper")
    writer.integer(eyes.height_upper.value)
    writer.fieldKey("ipd")
    writer.integer(eyes.ipd.value)
    writer.fieldKey("iris_color_gene")
    writer.integer(eyes.iris_color_gene)
    writer.fieldKey("iris_saturation_reduce")
    writer.integer(eyes.iris_saturation_reduce.value)
    writer.fieldKey("iris_shade_opacity")
    writer.integer(eyes.iris_shade_opacity.value)
    writer.fieldKey("iris_shade_reduce")
    writer.integer(eyes.iris_shade_reduce.value)
    writer.fieldKey("lid_close_lower_neutral")
    writer.integer(eyes.lid_close_lower_neutral.value)
    writer.fieldKey("lid_close_upper_neutral")
    writer.integer(eyes.lid_close_upper_neutral.value)
    writer.fieldKey("lid_color_gene")
    writer.integer(eyes.lid_color_gene)
    writer.fieldKey("lid_extension_gate")
    writer.integer(eyes.lid_extension_gate)
    writer.fieldKey("lid_extension_side")
    writer.integer(eyes.lid_extension_side)
    writer.fieldKey("noise_freq_h")
    writer.integer(eyes.noise_freq_h.value)
    writer.fieldKey("noise_freq_v")
    writer.integer(eyes.noise_freq_v.value)
    writer.fieldKey("noise_seed")
    writer.integer(eyes.noise_seed)
    writer.fieldKey("position_mode")
    writer.integer(eyes.position_mode)
    writer.fieldKey("pupil_scale")
    writer.integer(eyes.pupil_scale.value)
    writer.fieldKey("rotation_seed")
    writer.integer(eyes.rotation_seed)
    writer.fieldKey("size_h")
    writer.integer(eyes.size_h.value)
    writer.fieldKey("vertical_pull")
    writer.integer(eyes.vertical_pull.value)
    writer.fieldKey("vertical_pull_gate")
    writer.gate(eyes.vertical_pull_gate)
    writer.endObject()
}

func writeBrows(_ writer: inout CanonicalWriter, _ brows: Brows) {
    writer.beginObject()
    writer.fieldKey("clearance")
    writer.integer(brows.clearance.value)
    writer.fieldKey("end_height1")
    writer.integer(brows.end_height1.value)
    writer.fieldKey("end_height2")
    writer.integer(brows.end_height2.value)
    writer.fieldKey("mid_x")
    writer.integer(brows.mid_x.value)
    writer.fieldKey("tensions")
    writeQ32Array(&writer, brows.tensions)
    writer.fieldKey("width_coupling")
    writer.integer(brows.width_coupling.value)
    writer.endObject()
}

func writeLashes(_ writer: inout CanonicalWriter, _ lashes: Lashes) {
    writer.beginObject()
    writer.fieldKey("count")
    writer.integer(lashes.count)
    writer.fieldKey("direction")
    writer.integer(lashes.direction)
    writer.fieldKey("length_scale")
    writer.integer(lashes.length_scale.value)
    writer.fieldKey("removed")
    writer.integer(lashes.removed)
    writer.fieldKey("stroke_scale")
    writer.integer(lashes.stroke_scale.value)
    writer.endObject()
}

func writeMouth(_ writer: inout CanonicalWriter, _ mouth: Mouth) {
    writer.beginObject()
    writer.fieldKey("aspect_widen_gate")
    writer.gate(mouth.aspect_widen_gate)
    writer.fieldKey("family")
    writer.integer(mouth.family)
    writer.fieldKey("params")
    writeQ32Array(&writer, mouth.params)
    writer.fieldKey("width_scale")
    writer.integer(mouth.width_scale.value)
    writer.fieldKey("width_scale_gate")
    writer.gate(mouth.width_scale_gate)
    writer.endObject()
}

func writeFacePlacement(_ writer: inout CanonicalWriter, _ placement: FacePlacement) {
    writer.beginObject()
    writer.fieldKey("height_coeff")
    writer.integer(placement.height_coeff.value)
    writer.fieldKey("width_coeff")
    writer.integer(placement.width_coeff.value)
    writer.endObject()
}

func writePattern(_ writer: inout CanonicalWriter, _ pattern: Pattern) {
    writer.beginObject()
    writer.fieldKey("circler")
    writeCircler(&writer, pattern.circler)
    writer.fieldKey("class")
    writer.integer(pattern.`class`)
    writer.fieldKey("cut")
    writeCut(&writer, pattern.cut)
    writer.fieldKey("voronoi")
    writeVoronoi(&writer, pattern.voronoi)
    writer.endObject()
}

func writeCircler(_ writer: inout CanonicalWriter, _ circler: Circler) {
    writer.beginObject()
    writer.fieldKey("point_reduce")
    writer.integer(circler.point_reduce)
    writer.fieldKey("pull_min")
    writer.integer(circler.pull_min.value)
    writer.fieldKey("size")
    writer.integer(circler.size.value)
    writer.fieldKey("tension")
    writer.integer(circler.tension.value)
    writer.fieldKey("y_offset_div")
    writer.integer(circler.y_offset_div.value)
    writer.endObject()
}

func writeVoronoi(_ writer: inout CanonicalWriter, _ voronoi: Voronoi) {
    writer.beginObject()
    writer.fieldKey("index")
    writer.integer(voronoi.index)
    writer.fieldKey("rotation")
    writer.integer(voronoi.rotation.value)
    writer.fieldKey("scale")
    writer.integer(voronoi.scale.value)
    writer.endObject()
}

func writeCut(_ writer: inout CanonicalWriter, _ cut: Cut) {
    writer.beginObject()
    writer.fieldKey("spread")
    writer.integer(cut.spread.value)
    writer.fieldKey("y_adjust_div")
    writer.integer(cut.y_adjust_div.value)
    writer.endObject()
}

func writeAnimation(_ writer: inout CanonicalWriter, _ animation: Animation) {
    writer.beginObject()
    writer.fieldKey("blink_interval")
    writer.integer(animation.blink_interval)
    writer.fieldKey("blink_jitter_seed")
    writer.integer(animation.blink_jitter_seed)
    writer.fieldKey("breath_amplitude")
    writer.integer(animation.breath_amplitude.value)
    writer.fieldKey("breath_period")
    writer.integer(animation.breath_period)
    writer.fieldKey("mood_transition_tempo")
    writer.integer(animation.mood_transition_tempo.value)
    writer.endObject()
}

func writeMoodResponse(_ writer: inout CanonicalWriter, _ moodResponse: MoodResponse) {
    writer.beginObject()
    writer.fieldKey("brow_reactivity")
    writer.integer(moodResponse.brow_reactivity.value)
    writer.fieldKey("eye_rotation_reactivity")
    writer.integer(moodResponse.eye_rotation_reactivity.value)
    writer.fieldKey("lid_reactivity")
    writer.integer(moodResponse.lid_reactivity.value)
    writer.fieldKey("mouth_reactivity")
    writer.integer(moodResponse.mouth_reactivity.value)
    writer.endObject()
}

func writeAsymmetry(_ writer: inout CanonicalWriter, _ asymmetry: Asymmetry) {
    writer.beginObject()
    writer.fieldKey("deltas")
    writeQ32Array(&writer, asymmetry.deltas)
    writer.endObject()
}

func writeLayers(_ writer: inout CanonicalWriter, _ layers: Layers) {
    writer.beginObject()
    writer.fieldKey("count")
    writer.integer(layers.count)
    writer.fieldKey("params")
    writeQ32Array(&writer, layers.params)
    writer.endObject()
}

func writeQ32Array(_ writer: inout CanonicalWriter, _ values: [Q32]) {
    writer.beginArray()
    for value in values {
        writer.integer(value.value)
    }
    writer.endArray()
}

// MARK: Strict parsing

/// An error returned when input cannot represent the complete phenotype schema.
public struct ParseError: Error, Equatable, CustomStringConvertible {
    public let message: String

    init(_ message: String) {
        self.message = message
    }

    public var description: String { message }
}

/// Parses a semantically valid phenotype JSON document.
///
/// Parsing accepts non-canonical whitespace and key order so consumers can
/// receive ordinary JSON, but rejects unknown and missing fields. Strict
/// shape validation prevents a silently ignored field from changing an
/// avatar's identity when it is canonicalized again.
public func fromCanonicalJson(_ bytes: [UInt8]) throws -> Phenotype {
    var parser = JsonParser(bytes: bytes)
    let document = try parser.parseDocument()
    return try parsePhenotype(document)
}

/// One JSON value, as parsed by the strict reader below. Numbers with a
/// fraction or exponent part are kept as a distinct case because canonical
/// JSON v1 admits integers only: they must fail field validation, not
/// silently coerce.
indirect enum JsonValue {
    case object([String: JsonValue])
    case array([JsonValue])
    case integer(Int64)
    case floatNumber
    case string(String)
    case boolean(Bool)
    case null
}

/// A minimal strict JSON reader. The port has no dependency budget for a JSON
/// library, and the standard Foundation parser cannot distinguish `1` from
/// `1.0` — a distinction the strict schema requires (Rust's `serde_json`
/// rejects `1.0` where an integer is required).
struct JsonParser {
    let bytes: [UInt8]
    var index = 0

    init(bytes: [UInt8]) {
        self.bytes = bytes
    }

    mutating func parseDocument() throws -> JsonValue {
        skipWhitespace()
        let value = try parseValue()
        skipWhitespace()
        guard index == bytes.count else {
            throw ParseError("invalid JSON: trailing characters")
        }
        return value
    }

    private var peek: UInt8? {
        index < bytes.count ? bytes[index] : nil
    }

    private mutating func skipWhitespace() {
        while let byte = peek, byte == 0x20 || byte == 0x0A || byte == 0x0D || byte == 0x09 {
            index += 1
        }
    }

    private mutating func parseValue() throws -> JsonValue {
        guard let byte = peek else {
            throw ParseError("invalid JSON: unexpected end of input")
        }
        switch byte {
        case UInt8(ascii: "{"):
            return try parseObject()
        case UInt8(ascii: "["):
            return try parseArray()
        case UInt8(ascii: "\""):
            return .string(try parseString())
        case UInt8(ascii: "t"):
            try expectLiteral("true")
            return .boolean(true)
        case UInt8(ascii: "f"):
            try expectLiteral("false")
            return .boolean(false)
        case UInt8(ascii: "n"):
            try expectLiteral("null")
            return .null
        default:
            return try parseNumber()
        }
    }

    private mutating func expectLiteral(_ literal: String) throws {
        let literalBytes = Array(literal.utf8)
        guard index + literalBytes.count <= bytes.count,
            bytes[index ..< index + literalBytes.count].elementsEqual(literalBytes)
        else {
            throw ParseError("invalid JSON: malformed literal")
        }
        index += literalBytes.count
    }

    private mutating func parseObject() throws -> JsonValue {
        index += 1
        var members: [String: JsonValue] = [:]
        skipWhitespace()
        if peek == UInt8(ascii: "}") {
            index += 1
            return .object(members)
        }
        while true {
            skipWhitespace()
            guard peek == UInt8(ascii: "\"") else {
                throw ParseError("invalid JSON: expected object key")
            }
            let key = try parseString()
            skipWhitespace()
            guard peek == UInt8(ascii: ":") else {
                throw ParseError("invalid JSON: expected ':'")
            }
            index += 1
            skipWhitespace()
            // Duplicate keys keep the last value, matching serde_json's Map.
            members[key] = try parseValue()
            skipWhitespace()
            if peek == UInt8(ascii: ",") {
                index += 1
                continue
            }
            guard peek == UInt8(ascii: "}") else {
                throw ParseError("invalid JSON: expected ',' or '}'")
            }
            index += 1
            return .object(members)
        }
    }

    private mutating func parseArray() throws -> JsonValue {
        index += 1
        var items: [JsonValue] = []
        skipWhitespace()
        if peek == UInt8(ascii: "]") {
            index += 1
            return .array(items)
        }
        while true {
            skipWhitespace()
            items.append(try parseValue())
            skipWhitespace()
            if peek == UInt8(ascii: ",") {
                index += 1
                continue
            }
            guard peek == UInt8(ascii: "]") else {
                throw ParseError("invalid JSON: expected ',' or ']'")
            }
            index += 1
            return .array(items)
        }
    }

    private mutating func parseString() throws -> String {
        index += 1
        var utf8: [UInt8] = []
        while true {
            guard index < bytes.count else {
                throw ParseError("invalid JSON: unterminated string")
            }
            let byte = bytes[index]
            index += 1
            if byte == UInt8(ascii: "\"") {
                break
            }
            if byte != UInt8(ascii: "\\") {
                utf8.append(byte)
                continue
            }
            guard index < bytes.count else {
                throw ParseError("invalid JSON: unterminated escape")
            }
            let escape = bytes[index]
            index += 1
            switch escape {
            case UInt8(ascii: "\""): utf8.append(UInt8(ascii: "\""))
            case UInt8(ascii: "\\"): utf8.append(UInt8(ascii: "\\"))
            case UInt8(ascii: "/"): utf8.append(UInt8(ascii: "/"))
            case UInt8(ascii: "b"): utf8.append(0x08)
            case UInt8(ascii: "f"): utf8.append(0x0C)
            case UInt8(ascii: "n"): utf8.append(0x0A)
            case UInt8(ascii: "r"): utf8.append(0x0D)
            case UInt8(ascii: "t"): utf8.append(0x09)
            case UInt8(ascii: "u"):
                var code = try parseHex4()
                // Combine UTF-16 surrogate pairs; a lone surrogate becomes
                // the replacement character, matching serde_json's lossy
                // string conversion.
                if code >= 0xD800 && code <= 0xDBFF,
                    peek == UInt8(ascii: "\\"),
                    bytes[index + 1] == UInt8(ascii: "u")
                {
                    index += 2
                    let low = try parseHex4()
                    if low >= 0xDC00 && low <= 0xDFFF {
                        code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00)
                    } else {
                        code = 0xFFFD
                    }
                } else if code >= 0xD800 && code <= 0xDFFF {
                    code = 0xFFFD
                }
                utf8.append(contentsOf: String(Unicode.Scalar(code)!).utf8)
            default:
                throw ParseError("invalid JSON: unknown escape")
            }
        }
        return String(decoding: utf8, as: UTF8.self)
    }

    private mutating func parseHex4() throws -> UInt32 {
        guard index + 4 <= bytes.count else {
            throw ParseError("invalid JSON: truncated \\u escape")
        }
        var value: UInt32 = 0
        for offset in 0..<4 {
            let byte = bytes[index + offset]
            let nibble: UInt32
            switch byte {
            case UInt8(ascii: "0") ... UInt8(ascii: "9"):
                nibble = UInt32(byte - UInt8(ascii: "0"))
            case UInt8(ascii: "a") ... UInt8(ascii: "f"):
                nibble = UInt32(byte - UInt8(ascii: "a")) + 10
            case UInt8(ascii: "A") ... UInt8(ascii: "F"):
                nibble = UInt32(byte - UInt8(ascii: "A")) + 10
            default:
                throw ParseError("invalid JSON: malformed \\u escape")
            }
            value = value << 4 | nibble
        }
        index += 4
        return value
    }

    private mutating func parseNumber() throws -> JsonValue {
        let start = index
        if peek == UInt8(ascii: "-") {
            index += 1
        }
        guard let firstDigit = peek, firstDigit >= UInt8(ascii: "0") && firstDigit <= UInt8(ascii: "9") else {
            throw ParseError("invalid JSON: malformed number")
        }
        if firstDigit == UInt8(ascii: "0") {
            index += 1
            if let next = peek, next >= UInt8(ascii: "0") && next <= UInt8(ascii: "9") {
                throw ParseError("invalid JSON: leading zeros are not allowed")
            }
        } else {
            while let digit = peek, digit >= UInt8(ascii: "0") && digit <= UInt8(ascii: "9") {
                index += 1
            }
        }
        var isFloat = false
        if peek == UInt8(ascii: ".") {
            isFloat = true
            index += 1
            guard peek.map({ $0 >= UInt8(ascii: "0") && $0 <= UInt8(ascii: "9") }) == true else {
                throw ParseError("invalid JSON: malformed number")
            }
            while let digit = peek, digit >= UInt8(ascii: "0") && digit <= UInt8(ascii: "9") {
                index += 1
            }
        }
        if peek == UInt8(ascii: "e") || peek == UInt8(ascii: "E") {
            isFloat = true
            index += 1
            if peek == UInt8(ascii: "+") || peek == UInt8(ascii: "-") {
                index += 1
            }
            guard peek.map({ $0 >= UInt8(ascii: "0") && $0 <= UInt8(ascii: "9") }) == true else {
                throw ParseError("invalid JSON: malformed number")
            }
            while let digit = peek, digit >= UInt8(ascii: "0") && digit <= UInt8(ascii: "9") {
                index += 1
            }
        }
        if isFloat {
            return .floatNumber
        }
        let token = String(decoding: bytes[start ..< index], as: UTF8.self)
        // An integer token outside the i64 range behaves like serde_json's
        // Number here: it is not usable as an i64/u64 field value.
        guard let value = Int64(token) else {
            return .floatNumber
        }
        return .integer(value)
    }
}

// MARK: Schema validation

func parsePhenotype(_ value: JsonValue) throws -> Phenotype {
    let object = try jsonObject(value, "phenotype")
    try checkKeys(
        object,
        allowed: [
            "genome_version", "body", "palette", "face", "pattern",
            "animation", "mood_response", "asymmetry", "layers",
        ],
        context: "phenotype")
    return Phenotype(
        genome_version: try u16Value(
            try required(object, "genome_version", "phenotype"), "genome_version"),
        body: try parseBody(try required(object, "body", "phenotype")),
        palette: try parsePalette(try required(object, "palette", "phenotype")),
        face: try parseFace(try required(object, "face", "phenotype")),
        pattern: try parsePattern(try required(object, "pattern", "phenotype")),
        animation: try parseAnimation(try required(object, "animation", "phenotype")),
        mood_response: try parseMoodResponse(
            try required(object, "mood_response", "phenotype")),
        asymmetry: try parseAsymmetry(try required(object, "asymmetry", "phenotype")),
        layers: try parseLayers(try required(object, "layers", "phenotype")))
}

func parseBody(_ value: JsonValue) throws -> Body {
    let object = try jsonObject(value, "body")
    try checkKeys(object, allowed: ["point_count", "size", "points"], context: "body")
    return Body(
        point_count: try u8Value(
            try required(object, "point_count", "body"), "body.point_count"),
        size: Milli(try i64Value(try required(object, "size", "body"), "body.size")),
        points: try arrayValue(
            try required(object, "points", "body"), "body.points", count: 16,
            parseItem: { item, _ in try parseBodyPoint(item) }))
}

func parseBodyPoint(_ value: JsonValue) throws -> BodyPoint {
    let object = try jsonObject(value, "body point")
    try checkKeys(object, allowed: ["x", "y", "tension"], context: "body point")
    return BodyPoint(
        x: Milli(try i64Value(
            try required(object, "x", "body point"), "body.points[].x")),
        y: Milli(try i64Value(
            try required(object, "y", "body point"), "body.points[].y")),
        tension: Q32(try i64Value(
            try required(object, "tension", "body point"), "body.points[].tension")))
}

func parsePalette(_ value: JsonValue) throws -> Palette {
    let object = try jsonObject(value, "palette")
    try checkKeys(
        object,
        allowed: [
            "back", "primary", "primary_lighter", "primary_darkish",
            "primary_darker", "primary_darkest", "harmonic1",
            "harmonic1_darkish", "harmonic1_darkest", "harmonic2",
            "harmonic3", "harmonic4", "secondary_darker", "dark", "white",
        ],
        context: "palette")
    return Palette(
        back: try parseRgb(try required(object, "back", "palette")),
        primary: try parseRgb(try required(object, "primary", "palette")),
        primary_lighter: try parseRgb(
            try required(object, "primary_lighter", "palette")),
        primary_darkish: try parseRgb(
            try required(object, "primary_darkish", "palette")),
        primary_darker: try parseRgb(
            try required(object, "primary_darker", "palette")),
        primary_darkest: try parseRgb(
            try required(object, "primary_darkest", "palette")),
        harmonic1: try parseRgb(try required(object, "harmonic1", "palette")),
        harmonic1_darkish: try parseRgb(
            try required(object, "harmonic1_darkish", "palette")),
        harmonic1_darkest: try parseRgb(
            try required(object, "harmonic1_darkest", "palette")),
        harmonic2: try parseRgb(try required(object, "harmonic2", "palette")),
        harmonic3: try parseRgb(try required(object, "harmonic3", "palette")),
        harmonic4: try parseRgb(try required(object, "harmonic4", "palette")),
        secondary_darker: try parseRgb(
            try required(object, "secondary_darker", "palette")),
        dark: try parseRgb(try required(object, "dark", "palette")),
        white: try parseRgb(try required(object, "white", "palette")))
}

func parseRgb(_ value: JsonValue) throws -> Rgb {
    let object = try jsonObject(value, "RGB color")
    try checkKeys(object, allowed: ["r", "g", "b"], context: "RGB color")
    return Rgb(
        r: try u8Value(try required(object, "r", "RGB color"), "color.r"),
        g: try u8Value(try required(object, "g", "RGB color"), "color.g"),
        b: try u8Value(try required(object, "b", "RGB color"), "color.b"))
}

func parseFace(_ value: JsonValue) throws -> Face {
    let object = try jsonObject(value, "face")
    try checkKeys(
        object, allowed: ["eyes", "brows", "lashes", "mouth", "placement"],
        context: "face")
    return Face(
        eyes: try parseEyes(try required(object, "eyes", "face")),
        brows: try parseBrows(try required(object, "brows", "face")),
        lashes: try parseLashes(try required(object, "lashes", "face")),
        mouth: try parseMouth(try required(object, "mouth", "face")),
        placement: try parseFacePlacement(
            try required(object, "placement", "face")))
}

func parseEyes(_ value: JsonValue) throws -> Eyes {
    let object = try jsonObject(value, "eyes")
    try checkKeys(
        object,
        allowed: [
            "position_mode", "size_h", "ipd", "height_upper", "height_lower",
            "height_bias_gate", "height_bias", "vertical_pull_gate",
            "vertical_pull", "contour_inset1", "contour_inset2",
            "eyeball_radius_scale", "pupil_scale", "iris_shade_opacity",
            "iris_shade_reduce", "iris_saturation_reduce",
            "lid_close_upper_neutral", "lid_close_lower_neutral",
            "lid_extension_gate", "lid_extension_side", "noise_freq_h",
            "noise_freq_v", "noise_seed", "rotation_seed", "lid_color_gene",
            "iris_color_gene",
        ],
        context: "eyes")
    return Eyes(
        position_mode: try u8Value(
            try required(object, "position_mode", "eyes"), "eyes.position_mode"),
        size_h: Q32(try i64Value(
            try required(object, "size_h", "eyes"), "eyes.size_h")),
        ipd: Q32(try i64Value(try required(object, "ipd", "eyes"), "eyes.ipd")),
        height_upper: Q32(try i64Value(
            try required(object, "height_upper", "eyes"), "eyes.height_upper")),
        height_lower: Q32(try i64Value(
            try required(object, "height_lower", "eyes"), "eyes.height_lower")),
        height_bias_gate: try gateValue(
            try required(object, "height_bias_gate", "eyes"),
            "eyes.height_bias_gate"),
        height_bias: Q32(try i64Value(
            try required(object, "height_bias", "eyes"), "eyes.height_bias")),
        vertical_pull_gate: try gateValue(
            try required(object, "vertical_pull_gate", "eyes"),
            "eyes.vertical_pull_gate"),
        vertical_pull: Q32(try i64Value(
            try required(object, "vertical_pull", "eyes"), "eyes.vertical_pull")),
        contour_inset1: Q32(try i64Value(
            try required(object, "contour_inset1", "eyes"),
            "eyes.contour_inset1")),
        contour_inset2: Q32(try i64Value(
            try required(object, "contour_inset2", "eyes"),
            "eyes.contour_inset2")),
        eyeball_radius_scale: Q32(try i64Value(
            try required(object, "eyeball_radius_scale", "eyes"),
            "eyes.eyeball_radius_scale")),
        pupil_scale: Q32(try i64Value(
            try required(object, "pupil_scale", "eyes"), "eyes.pupil_scale")),
        iris_shade_opacity: Q32(try i64Value(
            try required(object, "iris_shade_opacity", "eyes"),
            "eyes.iris_shade_opacity")),
        iris_shade_reduce: Q32(try i64Value(
            try required(object, "iris_shade_reduce", "eyes"),
            "eyes.iris_shade_reduce")),
        iris_saturation_reduce: Q32(try i64Value(
            try required(object, "iris_saturation_reduce", "eyes"),
            "eyes.iris_saturation_reduce")),
        lid_close_upper_neutral: Q32(try i64Value(
            try required(object, "lid_close_upper_neutral", "eyes"),
            "eyes.lid_close_upper_neutral")),
        lid_close_lower_neutral: Q32(try i64Value(
            try required(object, "lid_close_lower_neutral", "eyes"),
            "eyes.lid_close_lower_neutral")),
        lid_extension_gate: try u8Value(
            try required(object, "lid_extension_gate", "eyes"),
            "eyes.lid_extension_gate"),
        lid_extension_side: try u8Value(
            try required(object, "lid_extension_side", "eyes"),
            "eyes.lid_extension_side"),
        noise_freq_h: Q32(try i64Value(
            try required(object, "noise_freq_h", "eyes"), "eyes.noise_freq_h")),
        noise_freq_v: Q32(try i64Value(
            try required(object, "noise_freq_v", "eyes"), "eyes.noise_freq_v")),
        noise_seed: try u8Value(
            try required(object, "noise_seed", "eyes"), "eyes.noise_seed"),
        rotation_seed: try u8Value(
            try required(object, "rotation_seed", "eyes"), "eyes.rotation_seed"),
        lid_color_gene: try u8Value(
            try required(object, "lid_color_gene", "eyes"), "eyes.lid_color_gene"),
        iris_color_gene: try u8Value(
            try required(object, "iris_color_gene", "eyes"),
            "eyes.iris_color_gene"))
}

func parseBrows(_ value: JsonValue) throws -> Brows {
    let object = try jsonObject(value, "brows")
    try checkKeys(
        object,
        allowed: [
            "end_height1", "mid_x", "end_height2", "tensions",
            "width_coupling", "clearance",
        ],
        context: "brows")
    return Brows(
        end_height1: Q32(try i64Value(
            try required(object, "end_height1", "brows"), "brows.end_height1")),
        mid_x: Q32(try i64Value(
            try required(object, "mid_x", "brows"), "brows.mid_x")),
        end_height2: Q32(try i64Value(
            try required(object, "end_height2", "brows"), "brows.end_height2")),
        tensions: try arrayValue(
            try required(object, "tensions", "brows"), "brows.tensions",
            count: 3, parseItem: q32Item),
        width_coupling: Q32(try i64Value(
            try required(object, "width_coupling", "brows"),
            "brows.width_coupling")),
        clearance: Q32(try i64Value(
            try required(object, "clearance", "brows"), "brows.clearance")))
}

func parseLashes(_ value: JsonValue) throws -> Lashes {
    let object = try jsonObject(value, "lashes")
    try checkKeys(
        object,
        allowed: ["count", "removed", "length_scale", "stroke_scale", "direction"],
        context: "lashes")
    return Lashes(
        count: try u8Value(
            try required(object, "count", "lashes"), "lashes.count"),
        removed: try u8Value(
            try required(object, "removed", "lashes"), "lashes.removed"),
        length_scale: Q32(try i64Value(
            try required(object, "length_scale", "lashes"),
            "lashes.length_scale")),
        stroke_scale: Q32(try i64Value(
            try required(object, "stroke_scale", "lashes"),
            "lashes.stroke_scale")),
        direction: try u8Value(
            try required(object, "direction", "lashes"), "lashes.direction"))
}

func parseMouth(_ value: JsonValue) throws -> Mouth {
    let object = try jsonObject(value, "mouth")
    try checkKeys(
        object,
        allowed: [
            "family", "width_scale_gate", "width_scale", "aspect_widen_gate",
            "params",
        ],
        context: "mouth")
    return Mouth(
        family: try u8Value(
            try required(object, "family", "mouth"), "mouth.family"),
        width_scale_gate: try gateValue(
            try required(object, "width_scale_gate", "mouth"),
            "mouth.width_scale_gate"),
        width_scale: Q32(try i64Value(
            try required(object, "width_scale", "mouth"), "mouth.width_scale")),
        aspect_widen_gate: try gateValue(
            try required(object, "aspect_widen_gate", "mouth"),
            "mouth.aspect_widen_gate"),
        params: try arrayValue(
            try required(object, "params", "mouth"), "mouth.params",
            count: 8, parseItem: q32Item))
}

func parseFacePlacement(_ value: JsonValue) throws -> FacePlacement {
    let object = try jsonObject(value, "face placement")
    try checkKeys(
        object, allowed: ["width_coeff", "height_coeff"], context: "face placement")
    return FacePlacement(
        width_coeff: Q32(try i64Value(
            try required(object, "width_coeff", "face placement"),
            "placement.width_coeff")),
        height_coeff: Q32(try i64Value(
            try required(object, "height_coeff", "face placement"),
            "placement.height_coeff")))
}

func parsePattern(_ value: JsonValue) throws -> Pattern {
    let object = try jsonObject(value, "pattern")
    try checkKeys(
        object, allowed: ["class", "circler", "voronoi", "cut"], context: "pattern")
    return Pattern(
        class: try u8Value(
            try required(object, "class", "pattern"), "pattern.class"),
        circler: try parseCircler(try required(object, "circler", "pattern")),
        voronoi: try parseVoronoi(try required(object, "voronoi", "pattern")),
        cut: try parseCut(try required(object, "cut", "pattern")))
}

func parseCircler(_ value: JsonValue) throws -> Circler {
    let object = try jsonObject(value, "circler pattern")
    try checkKeys(
        object,
        allowed: ["point_reduce", "size", "y_offset_div", "tension", "pull_min"],
        context: "circler pattern")
    return Circler(
        point_reduce: try u8Value(
            try required(object, "point_reduce", "circler pattern"),
            "circler.point_reduce"),
        size: Q32(try i64Value(
            try required(object, "size", "circler pattern"), "circler.size")),
        y_offset_div: Q32(try i64Value(
            try required(object, "y_offset_div", "circler pattern"),
            "circler.y_offset_div")),
        tension: Q32(try i64Value(
            try required(object, "tension", "circler pattern"),
            "circler.tension")),
        pull_min: Q32(try i64Value(
            try required(object, "pull_min", "circler pattern"),
            "circler.pull_min")))
}

func parseVoronoi(_ value: JsonValue) throws -> Voronoi {
    let object = try jsonObject(value, "voronoi pattern")
    try checkKeys(
        object, allowed: ["index", "rotation", "scale"], context: "voronoi pattern")
    return Voronoi(
        index: try u8Value(
            try required(object, "index", "voronoi pattern"), "voronoi.index"),
        rotation: Bam16(try u16Value(
            try required(object, "rotation", "voronoi pattern"),
            "voronoi.rotation")),
        scale: Q32(try i64Value(
            try required(object, "scale", "voronoi pattern"), "voronoi.scale")))
}

func parseCut(_ value: JsonValue) throws -> Cut {
    let object = try jsonObject(value, "cut pattern")
    try checkKeys(
        object, allowed: ["spread", "y_adjust_div"], context: "cut pattern")
    return Cut(
        spread: Q32(try i64Value(
            try required(object, "spread", "cut pattern"), "cut.spread")),
        y_adjust_div: Q32(try i64Value(
            try required(object, "y_adjust_div", "cut pattern"),
            "cut.y_adjust_div")))
}

func parseAnimation(_ value: JsonValue) throws -> Animation {
    let object = try jsonObject(value, "animation")
    try checkKeys(
        object,
        allowed: [
            "breath_period", "breath_amplitude", "blink_interval",
            "blink_jitter_seed", "mood_transition_tempo",
        ],
        context: "animation")
    return Animation(
        breath_period: try u16Value(
            try required(object, "breath_period", "animation"),
            "animation.breath_period"),
        breath_amplitude: Q32(try i64Value(
            try required(object, "breath_amplitude", "animation"),
            "animation.breath_amplitude")),
        blink_interval: try u16Value(
            try required(object, "blink_interval", "animation"),
            "animation.blink_interval"),
        blink_jitter_seed: try u8Value(
            try required(object, "blink_jitter_seed", "animation"),
            "animation.blink_jitter_seed"),
        mood_transition_tempo: Q32(try i64Value(
            try required(object, "mood_transition_tempo", "animation"),
            "animation.mood_transition_tempo")))
}

func parseMoodResponse(_ value: JsonValue) throws -> MoodResponse {
    let object = try jsonObject(value, "mood response")
    try checkKeys(
        object,
        allowed: [
            "lid_reactivity", "brow_reactivity", "eye_rotation_reactivity",
            "mouth_reactivity",
        ],
        context: "mood response")
    return MoodResponse(
        lid_reactivity: Q32(try i64Value(
            try required(object, "lid_reactivity", "mood response"),
            "mood_response.lid_reactivity")),
        brow_reactivity: Q32(try i64Value(
            try required(object, "brow_reactivity", "mood response"),
            "mood_response.brow_reactivity")),
        eye_rotation_reactivity: Q32(try i64Value(
            try required(object, "eye_rotation_reactivity", "mood response"),
            "mood_response.eye_rotation_reactivity")),
        mouth_reactivity: Q32(try i64Value(
            try required(object, "mouth_reactivity", "mood response"),
            "mood_response.mouth_reactivity")))
}

func parseAsymmetry(_ value: JsonValue) throws -> Asymmetry {
    let object = try jsonObject(value, "asymmetry")
    try checkKeys(object, allowed: ["deltas"], context: "asymmetry")
    return Asymmetry(
        deltas: try arrayValue(
            try required(object, "deltas", "asymmetry"), "asymmetry.deltas",
            count: 8, parseItem: q32Item))
}

func parseLayers(_ value: JsonValue) throws -> Layers {
    let object = try jsonObject(value, "layers")
    try checkKeys(object, allowed: ["count", "params"], context: "layers")
    return Layers(
        count: try u8Value(
            try required(object, "count", "layers"), "layers.count"),
        params: try arrayValue(
            try required(object, "params", "layers"), "layers.params",
            count: 16, parseItem: q32Item))
}

func q32Item(_ value: JsonValue, _ context: String) throws -> Q32 {
    Q32(try i64Value(value, context))
}

func jsonObject(_ value: JsonValue, _ context: String) throws -> [String: JsonValue] {
    guard case .object(let members) = value else {
        throw ParseError("\(context) must be an object")
    }
    return members
}

func checkKeys(
    _ object: [String: JsonValue], allowed: [String], context: String
) throws {
    for key in object.keys where !allowed.contains(key) {
        throw ParseError("unknown field `\(key)` in \(context)")
    }
}

func required(
    _ object: [String: JsonValue], _ key: String, _ context: String
) throws -> JsonValue {
    guard let value = object[key] else {
        throw ParseError("missing field `\(key)` in \(context)")
    }
    return value
}

func arrayValue<T>(
    _ value: JsonValue, _ context: String, count: Int,
    parseItem: (JsonValue, String) throws -> T
) throws -> [T] {
    guard case .array(let items) = value else {
        throw ParseError("\(context) must be an array")
    }
    guard items.count == count else {
        throw ParseError(
            "\(context) must contain exactly \(count) values, found \(items.count)")
    }
    return try items.enumerated().map { index, item in
        try parseItem(item, "\(context)[\(index)]")
    }
}

func i64Value(_ value: JsonValue, _ context: String) throws -> Int64 {
    guard case .integer(let number) = value else {
        throw ParseError("\(context) must be an i64 integer")
    }
    return number
}

func u8Value(_ value: JsonValue, _ context: String) throws -> UInt8 {
    guard case .integer(let number) = value, number >= 0 else {
        throw ParseError("\(context) must be a u8 integer")
    }
    guard number <= Int64(UInt8.max) else {
        throw ParseError("\(context) is outside the u8 range")
    }
    return UInt8(number)
}

func u16Value(_ value: JsonValue, _ context: String) throws -> UInt16 {
    guard case .integer(let number) = value, number >= 0 else {
        throw ParseError("\(context) must be a u16 integer")
    }
    guard number <= Int64(UInt16.max) else {
        throw ParseError("\(context) is outside the u16 range")
    }
    return UInt16(number)
}

func gateValue(_ value: JsonValue, _ context: String) throws -> Bool {
    switch try u8Value(value, context) {
    case 0:
        return false
    case 1:
        return true
    default:
        throw ParseError(
            "\(context) must be encoded as canonical gate integer 0 or 1")
    }
}
