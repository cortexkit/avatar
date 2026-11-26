#if os(macOS) && canImport(AvatarCore)
// Reads the golden images in vectors/image/ for the embedded-core image tests.
import Foundation

enum GoldenImageError: Error, CustomStringConvertible {
    case malformed(String)

    var description: String {
        switch self {
        case .malformed(let message): return message
        }
    }
}

/// The golden image matrix in `vectors/image/manifest.json`.
struct GoldenImageManifest: Decodable {
    let entries: [GoldenImageEntry]
}

/// One cell of the golden image matrix. Field names follow the manifest's
/// snake_case keys once a decoder converts them.
struct GoldenImageEntry: Decodable {
    let type: String
    let genome: String
    let version: UInt32
    let mood: String
    let sizePx: UInt32
    let groundShadow: String
    let alpha: String
    /// The genome file, relative to `vectors/`.
    let genomeHex: String
    /// The PNG file, relative to `vectors/image/`.
    let file: String
}

/// A decoded golden PNG: its size and its RGBA bytes exactly as stored.
struct GoldenPixels {
    let width: Int
    let height: Int
    let pixels: [UInt8]
}

/// Decodes a non-interlaced 8-bit RGBA PNG, the only form the golden images
/// take, into the RGBA bytes stored in it.
///
/// The golden PNGs hold premultiplied images verbatim, so their bytes must
/// come back unchanged. ImageIO is not used because it may premultiply or
/// convert colours while decoding.
func decodeGoldenPNG(_ data: Data, label: String) throws -> GoldenPixels {
    let bytes = [UInt8](data)
    let signature: [UInt8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    guard bytes.count >= signature.count, Array(bytes[0..<signature.count]) == signature else {
        throw GoldenImageError.malformed("\(label): not a PNG file")
    }
    func bigEndian(_ start: Int) -> Int {
        bytes[start..<start + 4].reduce(0) { $0 << 8 | Int($1) }
    }

    // Each chunk is a 4-byte length, a 4-byte type, the data and a 4-byte CRC.
    var offset = signature.count
    var header: [UInt8]?
    var compressed: [UInt8] = []
    var ended = false
    while !ended {
        guard offset + 8 <= bytes.count else {
            throw GoldenImageError.malformed("\(label): truncated chunk header")
        }
        let length = bigEndian(offset)
        let type = String(decoding: bytes[offset + 4..<offset + 8], as: UTF8.self)
        let dataStart = offset + 8
        guard dataStart + length + 4 <= bytes.count else {
            throw GoldenImageError.malformed("\(label): truncated \(type) chunk")
        }
        let chunk = bytes[dataStart..<dataStart + length]
        switch type {
        case "IHDR": header = Array(chunk)
        case "IDAT": compressed += chunk
        case "IEND": ended = true
        default: break
        }
        offset = dataStart + length + 4
    }

    guard let header, header.count == 13 else {
        throw GoldenImageError.malformed("\(label): missing or malformed IHDR")
    }
    let width = header[0..<4].reduce(0) { $0 << 8 | Int($1) }
    let height = header[4..<8].reduce(0) { $0 << 8 | Int($1) }
    // Bit depth 8, colour type 6 (RGBA), standard compression and filtering,
    // no interlacing.
    guard Array(header[8..<13]) == [8, 6, 0, 0, 0] else {
        throw GoldenImageError.malformed("\(label): not a non-interlaced 8-bit RGBA PNG")
    }

    // The image data is a zlib stream: a 2-byte header, raw DEFLATE data and
    // a 4-byte Adler-32 checksum. Foundation's zlib algorithm reads the raw
    // DEFLATE data only.
    guard compressed.count > 6 else {
        throw GoldenImageError.malformed("\(label): image data too short")
    }
    let deflate = Data(compressed[2..<compressed.count - 4])
    let inflated = [UInt8](try (deflate as NSData).decompressed(using: .zlib) as Data)
    let rowBytes = width * 4
    guard inflated.count == height * (rowBytes + 1) else {
        throw GoldenImageError.malformed("\(label): image data has the wrong length")
    }

    // Each row starts with a filter byte; undo the filter against the row
    // above and the pixel to the left, as the PNG specification defines.
    var pixels = [UInt8](repeating: 0, count: height * rowBytes)
    for row in 0..<height {
        let source = row * (rowBytes + 1)
        let filter = inflated[source]
        let target = row * rowBytes
        for column in 0..<rowBytes {
            let left = column >= 4 ? Int(pixels[target + column - 4]) : 0
            let up = row > 0 ? Int(pixels[target - rowBytes + column]) : 0
            let upLeft = row > 0 && column >= 4 ? Int(pixels[target - rowBytes + column - 4]) : 0
            let predictor: Int
            switch filter {
            case 0: predictor = 0
            case 1: predictor = left
            case 2: predictor = up
            case 3: predictor = (left + up) / 2
            case 4:
                let estimate = left + up - upLeft
                let toLeft = abs(estimate - left)
                let toUp = abs(estimate - up)
                let toUpLeft = abs(estimate - upLeft)
                if toLeft <= toUp && toLeft <= toUpLeft {
                    predictor = left
                } else if toUp <= toUpLeft {
                    predictor = up
                } else {
                    predictor = upLeft
                }
            default:
                throw GoldenImageError.malformed("\(label): unknown filter \(filter) in row \(row)")
            }
            pixels[target + column] = UInt8((Int(inflated[source + 1 + column]) + predictor) & 0xFF)
        }
    }
    return GoldenPixels(width: width, height: height, pixels: pixels)
}
#endif
