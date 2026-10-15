#if os(macOS) && canImport(AvatarCore)
import AvatarCore
import Foundation
import XCTest
@testable import AvatarKit

final class EmbeddedCoreTests: XCTestCase {
    func testEveryCorpusVectorAndMoodMatchesNativeClassic() throws {
        let (root, vectors) = try loadManifest()
        var count = 0
        for vector in vectors {
            let hex = try String(
                contentsOf: root.appendingPathComponent("dna/\(vector.name).hex"),
                encoding: .utf8
            ).trimmingCharacters(in: .whitespacesAndNewlines)
            let genome = try Genome.fromHex(hex)
            for mood in Mood.allCases {
                let native = try renderTree(
                    genome: genome, type: classicAvatarType, version: 2, mood: mood)
                let core = try renderTreeViaCore(
                    genome: genome, type: classicAvatarType, version: 2, mood: mood)
                // RenderTree equality compares every field and ordered primitive,
                // not a raster or an SVG that could hide transport differences.
                XCTAssertEqual(core, native, "\(vector.name)__\(mood.rawValue)")
                count += 1
            }
        }
        XCTAssertGreaterThanOrEqual(vectors.count, 18, "manifest looks truncated")
        XCTAssertEqual(count, vectors.count * Mood.allCases.count)
        print("Embedded core compared \(count) trees across \(vectors.count) genomes and all moods")
    }

    func testEveryCommittedWildVectorAndMoodMatchesFixtureThroughPublicBytesOverload() throws {
        let root = try vectorsRoot()
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let manifest = try decoder.decode(
            WildManifest.self,
            from: Data(contentsOf: root.appendingPathComponent("wild/manifest.json")))
        XCTAssertEqual(manifest.genomeBytes, 16_384)
        XCTAssertEqual(manifest.type, "creature.wild")
        XCTAssertEqual(manifest.version, 1)

        var names: Set<String> = []
        var count = 0
        for vector in manifest.vectors {
            let hex = Array(try String(
                contentsOf: root.appendingPathComponent("wild/\(vector.projectId).hex"),
                encoding: .utf8
            ).trimmingCharacters(in: .whitespacesAndNewlines).utf8)
            XCTAssertEqual(hex.count, manifest.genomeBytes * 2, vector.projectId)
            let bytes = try stride(from: 0, to: hex.count, by: 2).map { offset in
                try XCTUnwrap(UInt8(
                    String(decoding: hex[offset..<min(offset + 2, hex.count)], as: UTF8.self),
                    radix: 16), "\(vector.projectId): invalid hex at byte \(offset / 2)")
            }
            for moodName in vector.moods {
                let mood = try XCTUnwrap(Mood(rawValue: moodName), "unknown mood: \(moodName)")
                let name = "\(vector.projectId)__\(moodName).json"
                let fixture = try decodeRenderTreeJson(
                    [UInt8](Data(contentsOf: root.appendingPathComponent("tree/\(name)"))))
                let core = try renderTreeViaCore(
                    genomeBytes: bytes, type: manifest.type, version: manifest.version, mood: mood)
                XCTAssertEqual(core, fixture, name)
                names.insert(name)
                count += 1
            }
        }
        // The independent fixture inventory catches omitted or duplicated manifest pairs.
        XCTAssertEqual(names, wildTreeFixtureNames)
        XCTAssertEqual(count, wildTreeFixtureNames.count)
        print("Embedded core compared \(count) wild fixture pairs across \(manifest.vectors.count) genomes")
    }

    func testRefusedInputsSurfaceDistinctCoreErrors() throws {
        let bytes = [UInt8](repeating: 0, count: 1_024)
        let cases: [([UInt8], String, UInt32, String, CoreRenderError)] = [
            (bytes, "creature.unknown", 2, "neutral", .unsupportedType),
            (bytes, classicAvatarType, 999, "neutral", .unsupportedVersion),
            (Array(bytes.dropLast()), classicAvatarType, 2, "neutral", .genomeLength),
            (bytes, classicAvatarType, 2, "unknown", .unknownMood),
            (bytes, classicAvatarType + "\0ignored", 2, "neutral", .unsupportedType),
            (bytes, classicAvatarType, 2, "neutral\0ignored", .unknownMood),
        ]
        for (genome, type, version, mood, expected) in cases {
            XCTAssertThrowsError(try renderTreeViaCoreBytes(
                genomeBytes: genome, type: type, version: version, moodName: mood
            )) { XCTAssertEqual($0 as? CoreRenderError, expected) }
        }
    }

    func testEveryCErrorStatusHasItsOwnSwiftError() throws {
        let cases: [(Int32, CoreRenderError)] = [
            (Int32(AVATAR_STATUS_UNKNOWN_TYPE), .unsupportedType),
            (Int32(AVATAR_STATUS_UNKNOWN_VERSION), .unsupportedVersion),
            (Int32(AVATAR_STATUS_GENOME_LENGTH), .genomeLength),
            (Int32(AVATAR_STATUS_UNKNOWN_MOOD), .unknownMood),
            (Int32(AVATAR_STATUS_INVALID_ARGUMENT), .invalidArgument),
            (Int32(AVATAR_STATUS_RENDER_ERROR), .renderFailed),
            (Int32(AVATAR_STATUS_PANIC), .panic),
            (999, .unknownStatus(999)),
        ]
        XCTAssertNoThrow(try checkCoreStatus(Int32(AVATAR_STATUS_OK)))
        for (status, expected) in cases {
            XCTAssertThrowsError(try checkCoreStatus(status)) {
                XCTAssertEqual($0 as? CoreRenderError, expected)
            }
        }
        XCTAssertEqual(Set(cases.map { $0.0 }).count, cases.count)
    }

    func testCoreCallsCanRunConcurrently() throws {
        let genome = try Genome.fromBytes([UInt8](repeating: 0x5a, count: 1_024))
        let expected = try renderTree(
            genome: genome, type: classicAvatarType, version: 2, mood: .happy)
        DispatchQueue.concurrentPerform(iterations: 16) { _ in
            do {
                let tree = try renderTreeViaCore(
                    genome: genome, type: classicAvatarType, version: 2, mood: .happy)
                XCTAssertEqual(tree, expected)
            } catch {
                XCTFail("Concurrent core call failed: \(error)")
            }
        }
    }
}

private struct WildManifest: Decodable {
    let genomeBytes: Int
    let type: String
    let version: UInt32
    let vectors: [WildVector]
}

private struct WildVector: Decodable {
    let projectId: String
    let moods: [String]
}
#endif
