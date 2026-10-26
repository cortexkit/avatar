#if os(macOS) && canImport(AvatarCore)
import Foundation
import XCTest
@testable import AvatarKit

final class RenderTreeCacheTests: XCTestCase {
    private let genome = [UInt8](repeating: 0, count: 1_024)

    func testCacheReusesIdenticalRequestsAndRemoveAllEmptiesIt() throws {
        let cache = RenderTreeCache(capacity: 3)
        let first = try cache.renderTree(
            genomeBytes: genome, type: "creature.classic", version: 2, mood: .neutral)
        let second = try cache.renderTree(
            genomeBytes: genome, type: "creature.classic", version: 2, mood: .neutral)

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
        let genome = self.genome
        let moods: [Mood] = [.neutral, .happy, .working, .blocked]
        let expected = try moods.map {
            try renderTreeViaCore(genomeBytes: genome, type: "creature.classic", version: 2, mood: $0)
        }

        DispatchQueue.concurrentPerform(iterations: 200) { iteration in
            let index = iteration % moods.count
            let tree = try? cache.renderTree(
                genomeBytes: genome, type: "creature.classic", version: 2, mood: moods[index])
            XCTAssertEqual(tree, expected[index])
        }

        XCTAssertEqual(cache.count, moods.count)
    }

    func testCacheIncludesMoodInItsKey() throws {
        let cache = RenderTreeCache(capacity: 3)
        let neutral = try cache.renderTree(
            genomeBytes: genome, type: "creature.classic", version: 2, mood: .neutral)
        let happy = try cache.renderTree(
            genomeBytes: genome, type: "creature.classic", version: 2, mood: .happy)
        let expectedHappy = try renderTreeViaCore(
            genomeBytes: genome, type: "creature.classic", version: 2, mood: .happy)

        XCTAssertNotEqual(neutral, happy)
        XCTAssertEqual(happy, expectedHappy)
        XCTAssertEqual(cache.count, 2)
    }

    func testCacheAcceptsWildGenomesAndDoesNotCacheRefusedRequests() throws {
        let cache = RenderTreeCache(capacity: 3)
        let wildGenome = [UInt8](repeating: 0, count: 16_384)
        let expected = try renderTreeViaCore(
            genomeBytes: wildGenome, type: "creature.wild", version: 1, mood: .neutral)
        XCTAssertEqual(try cache.renderTree(
            genomeBytes: wildGenome, type: "creature.wild", version: 1, mood: .neutral), expected)
        XCTAssertThrowsError(try cache.renderTree(
            genomeBytes: wildGenome, type: "creature.wild", version: 999, mood: .neutral)) {
                XCTAssertEqual($0 as? CoreRenderError, .unsupportedVersion)
        }
        XCTAssertEqual(cache.count, 1)
    }
}
#endif
