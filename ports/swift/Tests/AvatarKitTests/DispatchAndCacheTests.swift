import XCTest
@testable import AvatarKit

final class DispatchAndCacheTests: XCTestCase {
    private func genome() throws -> Genome {
        try Genome.fromBytes([UInt8](repeating: 0, count: Genome.byteCount))
    }

    func testSupportedPairUsesTheExistingTreeBuilder() throws {
        let genome = try genome()
        let expected = try renderTree(
            phenotype: decode(genome), params: RenderParams(mood: .happy))

        XCTAssertEqual(
            try renderTree(
                genome: genome, type: classicAvatarType, version: 2, mood: .happy),
            expected)
    }

    func testUnknownTypeIsRefusedWithoutRendering() throws {
        XCTAssertThrowsError(try renderTree(
            genome: genome(), type: "creature.wild", version: 2, mood: .neutral)) { error in
            XCTAssertEqual(
                error as? DispatchError,
                .unsupportedType(requested: "creature.wild"))
        }
    }

    func testUnshippedClassicVersionIsRefusedWithoutRendering() throws {
        XCTAssertThrowsError(try renderTree(
            genome: genome(), type: classicAvatarType, version: 1, mood: .neutral)) { error in
            XCTAssertEqual(
                error as? DispatchError,
                .unsupportedVersion(avatarType: classicAvatarType, requested: 1))
        }
    }

    func testWrongGenomeLengthIsRefusedBeforeRendering() {
        let malformed = Genome(trustedBytes: [UInt8](repeating: 0, count: Genome.byteCount - 1))

        XCTAssertThrowsError(try renderTree(
            genome: malformed, type: classicAvatarType, version: 2, mood: .neutral)) { error in
            XCTAssertEqual(
                error as? DispatchError,
                .genomeLength(expected: Genome.byteCount, actual: Genome.byteCount - 1))
        }
    }

    func testCacheReusesIdenticalRequestsAndRemoveAllEmptiesIt() throws {
        let cache = RenderTreeCache(capacity: 3)
        let genome = try genome()
        let first = try cache.renderTree(
            genome: genome, type: classicAvatarType, version: 2, mood: .neutral)
        let second = try cache.renderTree(
            genome: genome, type: classicAvatarType, version: 2, mood: .neutral)

        XCTAssertEqual(first, second)
        XCTAssertEqual(cache.count, 1)
        cache.removeAll()
        XCTAssertEqual(cache.count, 0)
    }

    func testCacheSurvivesConcurrentUse() throws {
        // The cache is documented as callable from any thread because a list
        // view prefetches avatars off the main queue. An untested concurrency
        // claim is worth nothing, so this hammers the same instance from many
        // threads across several keys: without the lock, the dictionary
        // mutations race and this crashes or corrupts rather than failing an
        // assertion politely.
        let cache = RenderTreeCache(capacity: 4)
        let genome = try genome()
        let moods: [Mood] = [.neutral, .happy, .working, .blocked]
        let expected = try moods.map {
            try renderTree(genome: genome, type: classicAvatarType, version: 2, mood: $0)
        }

        DispatchQueue.concurrentPerform(iterations: 200) { iteration in
            let index = iteration % moods.count
            let tree = try? cache.renderTree(
                genome: genome, type: classicAvatarType, version: 2, mood: moods[index])
            XCTAssertEqual(tree, expected[index])
        }

        XCTAssertEqual(cache.count, moods.count)
    }

    func testCacheIncludesMoodInItsKey() throws {
        let cache = RenderTreeCache(capacity: 3)
        let genome = try genome()
        let neutral = try cache.renderTree(
            genome: genome, type: classicAvatarType, version: 2, mood: .neutral)
        let happy = try cache.renderTree(
            genome: genome, type: classicAvatarType, version: 2, mood: .happy)
        let expectedHappy = try renderTree(
            genome: genome, type: classicAvatarType, version: 2, mood: .happy)

        XCTAssertNotEqual(neutral, happy)
        XCTAssertEqual(happy, expectedHappy)
        XCTAssertEqual(cache.count, 2)
    }
}
