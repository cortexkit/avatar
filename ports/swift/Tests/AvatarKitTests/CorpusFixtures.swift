// Shared golden-corpus fixtures and byte comparisons for tree consumer tests.
import XCTest
@testable import AvatarKit

enum ConformanceTestError: Error {
    case vectorsNotFound
    case malformedManifest(String)
}

/// Finds vectors/ relative to the test source so the corpus works in any checkout.
func vectorsRoot() throws -> URL {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    for _ in 0..<8 {
        let candidate = directory.appendingPathComponent("vectors")
        var isDirectory: ObjCBool = false
        let manifest = candidate.appendingPathComponent("manifest.json")
        if FileManager.default.fileExists(atPath: manifest.path, isDirectory: &isDirectory) {
            return candidate
        }
        directory = directory.deletingLastPathComponent()
    }
    throw ConformanceTestError.vectorsNotFound
}

struct ManifestVector {
    let name: String
    let moods: [String]
}

/// Wild tree fixtures exercise primitive variants absent from classic trees.
/// Their names must match `WILD_TREE_FIXTURES` in
/// `crates/avatar-vectors/src/lib.rs`, which mints these files; the
/// inventory assertion fails on a mismatch rather than silently skipping
/// a fixture.
let wildTreeFixtureNames: Set<String> = [
    "wild-port-0005__neutral.json",
    "wild-port-0005__working.json",
    "wild-port-0005__happy.json",
    "wild-port-0005__waiting.json",
    "wild-port-0005__blocked.json",
    "wild-port-0005__sad.json",
    "wild-port-0005__sleepy.json",
    "wild-port-0031__neutral.json",
    "wild-port-0009__neutral.json",
    "wild-port-0022__neutral.json",
    "wild-port-0077__neutral.json",
    "wild-port-0043__neutral.json",
    "wild-port-0016__neutral.json",
    "wild-port-0021__neutral.json",
    "wild-port-0000__neutral.json",
    "wild-port-0001__neutral.json",
    "wild-port-0002__neutral.json",
    "wild-port-0007__neutral.json",
]

func loadManifest() throws -> (root: URL, vectors: [ManifestVector]) {
    let root = try vectorsRoot()
    let data = try Data(contentsOf: root.appendingPathComponent("manifest.json"))
    var parser = JsonParser(bytes: [UInt8](data))
    let document = try parser.parseDocument()
    guard case .object(let manifest) = document else {
        throw ConformanceTestError.malformedManifest("manifest.json is not an object")
    }
    guard case .array(let entries)? = manifest["vectors"] else {
        throw ConformanceTestError.malformedManifest("manifest.json has no vectors array")
    }
    var vectors: [ManifestVector] = []
    for entry in entries {
        guard case .object(let fields) = entry,
            case .string(let name)? = fields["name"],
            case .array(let moodValues)? = fields["moods"]
        else {
            throw ConformanceTestError.malformedManifest("vector entry is missing name or moods")
        }
        var moods: [String] = []
        for moodValue in moodValues {
            guard case .string(let mood) = moodValue else {
                throw ConformanceTestError.malformedManifest("mood entry is not a string")
            }
            moods.append(mood)
        }
        vectors.append(ManifestVector(name: name, moods: moods))
    }
    return (root, vectors)
}

/// 40-byte window around the first mismatch, hex-encoded, so a failure reads
/// like a wire diff instead of a multi-kilobyte JSON dump.
func hexWindow(_ bytes: [UInt8], center: Int) -> String {
    let start = max(0, center - 20)
    let end = min(bytes.count, center + 20)
    let hexDigits = Array("0123456789abcdef".utf8)
    var hex: [String] = []
    for byte in bytes[start ..< end] {
        hex.append(String(
            decoding: [hexDigits[Int(byte >> 4)], hexDigits[Int(byte & 0x0F)]],
            as: UTF8.self))
    }
    return "\(start)..<\(end): " + hex.joined(separator: " ")
}

func assertByteEqual(
    _ label: String, actual: [UInt8], expected: [UInt8],
    file: StaticString = #filePath, line: UInt = #line
) {
    let commonLength = min(actual.count, expected.count)
    var offset = 0
    while offset < commonLength && actual[offset] == expected[offset] {
        offset += 1
    }
    if offset == actual.count && offset == expected.count {
        return
    }
    XCTFail(
        "\(label): canonical JSON differs at byte \(offset) "
            + "(actual \(actual.count) bytes, expected \(expected.count))\n"
            + "actual   \(hexWindow(actual, center: offset))\n"
            + "expected \(hexWindow(expected, center: offset))",
        file: file, line: line)
}
