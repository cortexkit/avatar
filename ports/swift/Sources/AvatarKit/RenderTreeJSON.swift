// Numeric value types and strict JSON token parsing for the render-tree wire format.

/// A raw signed Q32.32 fixed-point coefficient.
public struct Q32: Equatable, Sendable {
    public var value: Int64

    public init(_ value: Int64) {
        self.value = value
    }

    /// The fixed-point value 1.0 (32 fraction bits).
    public static let one = Q32(1 << 32)
}

/// An angle expressed as a binary angle measurement with 65,536 units per turn.
public struct Bam16: Equatable, Sendable {
    public var value: UInt16

    public init(_ value: UInt16) {
        self.value = value
    }
}

/// An 8-bit RGB color.
public struct Rgb: Equatable, Sendable {
    public var r: UInt8
    public var g: UInt8
    public var b: UInt8

    public init(r: UInt8, g: UInt8, b: UInt8) {
        self.r = r
        self.g = g
        self.b = b
    }
}

/// An error returned for malformed render-tree JSON or unrepresentable transforms.
public struct ParseError: Error, Equatable, CustomStringConvertible {
    public let message: String

    init(_ message: String) {
        self.message = message
    }

    public var description: String { message }
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
                    peek == UInt8(ascii: "\\")
                {
                    guard index + 1 < bytes.count else {
                        throw ParseError("invalid JSON: truncated surrogate pair")
                    }
                    if bytes[index + 1] == UInt8(ascii: "u") {
                        index += 2
                        let low = try parseHex4()
                        if low >= 0xDC00 && low <= 0xDFFF {
                            code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00)
                        }
                    }
                }
                if code >= 0xD800 && code <= 0xDFFF {
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
