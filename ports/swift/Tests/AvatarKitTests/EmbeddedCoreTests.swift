#if os(macOS) && canImport(AvatarCore)
import AvatarCore
import Foundation
import XCTest
@testable import AvatarKit

final class EmbeddedCoreTests: XCTestCase {
    func testEveryCommittedClassicTreeMatchesFixtureThroughPublicBytesAPI() throws {
        let (root, vectors) = try loadManifest()
        var names: Set<String> = []
        var count = 0
        for vector in vectors {
            let bytes = try readGenomeBytes(
                root.appendingPathComponent("dna/\(vector.name).hex"), expectedCount: 1_024)
            for moodName in vector.moods {
                let mood = try XCTUnwrap(Mood(rawValue: moodName), "unknown mood: \(moodName)")
                let name = "\(vector.name)__\(moodName).json"
                let fixture = try decodeRenderTreeJson(
                    [UInt8](Data(contentsOf: root.appendingPathComponent("tree/\(name)"))))
                let core = try renderTreeViaCore(
                    genomeBytes: bytes, type: "creature.classic", version: 2, mood: mood)
                // RenderTree equality compares every field and ordered primitive,
                // not a raster or an SVG that could hide transport differences.
                XCTAssertEqual(core, fixture, name)
                names.insert(name)
                count += 1
            }
        }
        let allTreeNames = Set(try FileManager.default.contentsOfDirectory(
            atPath: root.appendingPathComponent("tree").path).filter { $0.hasSuffix(".json") })
        // Together with the wild inventory test, this covers every committed tree.
        let classicTreeNames = allTreeNames.subtracting(wildTreeFixtureNames)
        XCTAssertFalse(classicTreeNames.isEmpty)
        XCTAssertEqual(names, classicTreeNames)
        XCTAssertEqual(count, classicTreeNames.count)
        print("Embedded core compared \(count) classic fixture pairs across \(vectors.count) genomes")
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

        var names: Set<String> = []
        var versions: Set<UInt32> = []
        var count = 0
        for vector in manifest.vectors {
            let bytes = try readGenomeBytes(
                root.appendingPathComponent("wild/\(vector.projectId).hex"),
                expectedCount: manifest.genomeBytes)
            for moodName in vector.moods {
                let mood = try XCTUnwrap(Mood(rawValue: moodName), "unknown mood: \(moodName)")
                // Version 1 trees keep their shipped names; later generations
                // carry their version in the name.
                let name = vector.version == 1
                    ? "\(vector.projectId)__\(moodName).json"
                    : "\(vector.projectId)__v\(vector.version)__\(moodName).json"
                let fixture = try decodeRenderTreeJson(
                    [UInt8](Data(contentsOf: root.appendingPathComponent("tree/\(name)"))))
                // Each tree renders at the version its row names, as a stored
                // avatar does.
                let core = try renderTreeViaCore(
                    genomeBytes: bytes, type: manifest.type, version: vector.version, mood: mood)
                XCTAssertEqual(core, fixture, name)
                names.insert(name)
                count += 1
            }
            versions.insert(vector.version)
        }
        // The independent fixture inventory catches omitted or duplicated manifest pairs.
        XCTAssertEqual(names, wildTreeFixtureNames)
        XCTAssertEqual(count, wildTreeFixtureNames.count)
        XCTAssertEqual(versions, [1, 2])
        print("Embedded core compared \(count) wild fixture pairs across \(manifest.vectors.count) corpus rows")
    }

    func testRefusedInputsSurfaceDistinctCoreErrors() throws {
        let bytes = [UInt8](repeating: 0, count: 1_024)
        let cases: [([UInt8], String, UInt32, String, CoreRenderError)] = [
            (bytes, "creature.unknown", 2, "neutral", .unsupportedType),
            (bytes, "creature.classic", 999, "neutral", .unsupportedVersion),
            (Array(bytes.dropLast()), "creature.classic", 2, "neutral", .genomeLength),
            (bytes, "creature.classic", 2, "unknown", .unknownMood),
            (bytes, "creature.classic\0ignored", 2, "neutral", .unsupportedType),
            (bytes, "creature.classic", 2, "neutral\0ignored", .unknownMood),
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
        let genome = [UInt8](repeating: 0x5a, count: 1_024)
        let expected = try renderTreeViaCore(
            genomeBytes: genome, type: "creature.classic", version: 2, mood: .happy)
        DispatchQueue.concurrentPerform(iterations: 16) { _ in
            do {
                let tree = try renderTreeViaCore(
                    genomeBytes: genome, type: "creature.classic", version: 2, mood: .happy)
                XCTAssertEqual(tree, expected)
            } catch {
                XCTFail("Concurrent core call failed: \(error)")
            }
        }
    }
}

private func readGenomeBytes(_ url: URL, expectedCount: Int) throws -> [UInt8] {
    let hex = Array(try String(contentsOf: url, encoding: .utf8)
        .trimmingCharacters(in: .whitespacesAndNewlines).utf8)
    guard hex.count == expectedCount * 2 else {
        throw ConformanceTestError.malformedManifest("\(url.lastPathComponent): wrong genome length")
    }
    return try stride(from: 0, to: hex.count, by: 2).map { offset in
        try XCTUnwrap(UInt8(
            String(decoding: hex[offset..<offset + 2], as: UTF8.self), radix: 16),
            "\(url.lastPathComponent): invalid hex at byte \(offset / 2)")
    }
}

private struct WildManifest: Decodable {
    let genomeBytes: Int
    let type: String
    let vectors: [WildVector]
}

private struct WildVector: Decodable {
    let projectId: String
    /// The `creature.wild` generation this row's trees are rendered at.
    let version: UInt32
    let moods: [String]
}
#endif
