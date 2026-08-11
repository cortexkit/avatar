import Foundation
import XCTest
@testable import AvatarKit

final class RenderConformanceTests: XCTestCase {
    func testBoundary2EveryManifestMoodSerializesToCanonicalSvgBytes() throws {
        let (root, vectors) = try loadManifest()
        var count = 0
        for vector in vectors {
            let phenotype = try loadPhenotype(vector.name, root: root)
            for moodName in vector.moods {
                guard let mood = Mood(rawValue: moodName) else {
                    throw ConformanceTestError.malformedManifest("unknown mood \(moodName)")
                }
                let expected = try [UInt8](Data(contentsOf:
                    root.appendingPathComponent("svg/\(vector.name)__\(moodName).svg")))
                let tree = try buildRenderTree(
                    phenotype: phenotype, params: RenderParams(mood: mood))
                let actual = serializeSvg(tree)
                assertByteEqual(
                    "\(vector.name)__\(moodName)", actual: actual, expected: expected)
                count += 1
            }
        }
        XCTAssertEqual(count, 30)
    }

    func testCorpusTreesAreFlatAndEveryAffineMatchesNaiveComposition() throws {
        let (root, vectors) = try loadManifest()
        var transformCount = 0
        for vector in vectors {
            let phenotype = try loadPhenotype(vector.name, root: root)
            for moodName in vector.moods {
                let tree = try buildRenderTree(
                    phenotype: phenotype,
                    params: RenderParams(mood: try XCTUnwrap(Mood(rawValue: moodName))))
                var primitives = tree.primitives
                for primitive in tree.primitives {
                    for effect in primitive.effects {
                        if case .masked(let mask) = effect {
                            primitives.append(contentsOf: mask)
                        }
                    }
                }
                for primitive in primitives {
                    guard let transform = primitive.transform else { continue }
                    assertAffine(transform.affine(), equals: naiveAffine(transform.ops))
                    transformCount += 1
                }
            }
        }
        XCTAssertGreaterThan(transformCount, 900)
    }

    func testNeutralMoodIsIdenticalToAZeroedMoodResponse() throws {
        let (root, vectors) = try loadManifest()
        for vector in vectors {
            let phenotype = try loadPhenotype(vector.name, root: root)
            var zeroed = phenotype
            zeroed.mood_response = MoodResponse(
                lid_reactivity: Q32(0), brow_reactivity: Q32(0),
                eye_rotation_reactivity: Q32(0), mouth_reactivity: Q32(0))
            let neutral = try buildRenderTree(
                phenotype: phenotype, params: RenderParams(mood: .neutral))
            for mood in Mood.allCases {
                XCTAssertEqual(
                    neutral,
                    try buildRenderTree(
                        phenotype: zeroed, params: RenderParams(mood: mood)),
                    "\(vector.name) \(mood.rawValue)")
            }
        }
    }

    func testStrokeCapsAndJoinsMatchTheCorpusInventory() throws {
        let (root, vectors) = try loadManifest()
        var roundCaps = 0
        var roundJoins = 0
        for vector in vectors {
            let phenotype = try loadPhenotype(vector.name, root: root)
            for moodName in vector.moods {
                let svg = String(decoding: try renderSvg(
                    phenotype: phenotype,
                    params: RenderParams(mood: try XCTUnwrap(Mood(rawValue: moodName)))), as: UTF8.self)
                roundCaps += svg.nonOverlappingCount(of: "stroke-linecap=\"round\"")
                roundJoins += svg.nonOverlappingCount(of: "stroke-linejoin=\"round\"")
            }
        }
        XCTAssertEqual(roundCaps, 212)
        XCTAssertEqual(roundJoins, 30)

        let pattern = try loadPhenotype("pattern_0", root: root)
        let tree = try buildRenderTree(
            phenotype: pattern, params: RenderParams(mood: .neutral))
        let caps = tree.primitives.compactMap(\.stroke?.cap)
        XCTAssertTrue(caps.contains(.round))
        XCTAssertTrue(caps.contains(.butt))
    }

    func testMoodTableRowsKeepTheRustIntegerLiterals() {
        let happy = moodDeltas(.happy)
        XCTAssertEqual(happy.upper_lid_close.value, 858_993_459)
        XCTAssertEqual(happy.lower_lid_close.value, 1_431_655_765)
        XCTAssertEqual(happy.eye_rotation.value, 1_183)
        XCTAssertEqual(happy.brow_rotation.value, 64_444)
        XCTAssertEqual(happy.mouth_curve.value, 4_294_967_296)

        let blocked = moodDeltas(.blocked)
        XCTAssertEqual(blocked.upper_lid_close.value, 2_386_092_942)
        XCTAssertEqual(blocked.eyeball_shift_y.value, 515_396_076)
        XCTAssertEqual(blocked.brow_rotation.value, 6_372)
        XCTAssertEqual(blocked.mouth_curve.value, -3_006_477_107)

        let sleepy = moodDeltas(.sleepy)
        XCTAssertEqual(sleepy.upper_lid_close.value, 3_650_722_202)
        XCTAssertEqual(sleepy.lower_lid_close.value, 2_147_483_648)
        XCTAssertEqual(sleepy.eyeball_shift_y.value, -515_396_076)
    }

    func testVoronoiTableHashMatchesTheRustPin() {
        var bytes: [UInt8] = []
        for pattern in VORONOI_PATTERNS {
            bytes.append(contentsOf: pattern.utf8)
            bytes.append(0x1f)
        }
        XCTAssertEqual(
            hexString(Blake3.hash(bytes)),
            "0f080cce4b31019700389825292df78b9fe921e8d410acb2ed763952e899a150")
    }

    func testRendererFailsClosedForUnsupportedGrowthAndCanvas() throws {
        let (root, _) = try loadManifest()
        let phenotype = try loadPhenotype("all_zero", root: root)
        XCTAssertThrowsError(try renderTree(
            phenotype: phenotype,
            params: RenderParams(mood: .neutral, growth: Q32(Q32.one.value - 1)))) {
            XCTAssertEqual($0 as? RenderError, .unsupportedGrowth(Q32(Q32.one.value - 1)))
        }
        XCTAssertThrowsError(try renderSvg(
            phenotype: phenotype,
            params: RenderParams(mood: .neutral, canvas: 9_999))) {
            XCTAssertEqual($0 as? RenderError, .unsupportedCanvas(9_999))
        }
    }

    func testBodyOnlyApiOmitsTheFace() throws {
        let (root, _) = try loadManifest()
        let phenotype = try loadPhenotype("pattern_0", root: root)
        let svg = String(decoding: try renderBodySvg(
            phenotype: phenotype, params: RenderParams(mood: .happy)), as: UTF8.self)
        XCTAssertFalse(svg.contains("id=\"av-face\""))
        XCTAssertTrue(svg.contains("id=\"av-bp\""))
    }
}

private func loadPhenotype(_ name: String, root: URL) throws -> Phenotype {
    let hex = try String(
        contentsOf: root.appendingPathComponent("dna/\(name).hex"),
        encoding: .utf8
    ).trimmingCharacters(in: .whitespacesAndNewlines)
    return decode(try Genome.fromHex(hex))
}

private struct TestAffine {
    var a: Double = 1
    var b: Double = 0
    var c: Double = 0
    var d: Double = 1
    var tx: Double = 0
    var ty: Double = 0
}

private func naiveAffine(_ ops: [TransformOp]) -> TestAffine {
    var result = TestAffine()
    for op in ops {
        let next: TestAffine
        switch op {
        case .translate(let tx, let ty):
            next = TestAffine(tx: Double(tx), ty: Double(ty))
        case .scale(let sx, let sy):
            next = TestAffine(
                a: Double(sx.value) / 4_294_967_296,
                d: Double(sy.value) / 4_294_967_296)
        case .rotate(let angle, let cx, let cy):
            // Binary-angle rotations use the pinned quarter-wave lookup table.
            // Composing its resolved sine and cosine values directly keeps this
            // check independent of Transform.affine().
            let cosine = Double(cosQ30(angle.value)) / Double(1 << 30)
            let sine = Double(sinQ30(angle.value)) / Double(1 << 30)
            let x = Double(cx)
            let y = Double(cy)
            next = TestAffine(
                a: cosine, b: sine, c: -sine, d: cosine,
                tx: x - cosine * x + sine * y,
                ty: y - sine * x - cosine * y)
        }
        result = TestAffine(
            a: result.a * next.a + result.c * next.b,
            b: result.b * next.a + result.d * next.b,
            c: result.a * next.c + result.c * next.d,
            d: result.b * next.c + result.d * next.d,
            tx: result.tx + result.a * next.tx + result.c * next.ty,
            ty: result.ty + result.b * next.tx + result.d * next.ty)
    }
    return result
}

private func assertAffine(
    _ actual: Affine, equals expected: TestAffine,
    file: StaticString = #filePath, line: UInt = #line
) {
    let divisor = 4_294_967_296.0
    XCTAssertEqual(Double(actual.a.value) / divisor, expected.a, accuracy: 0.000_001, file: file, line: line)
    XCTAssertEqual(Double(actual.b.value) / divisor, expected.b, accuracy: 0.000_001, file: file, line: line)
    XCTAssertEqual(Double(actual.c.value) / divisor, expected.c, accuracy: 0.000_001, file: file, line: line)
    XCTAssertEqual(Double(actual.d.value) / divisor, expected.d, accuracy: 0.000_001, file: file, line: line)
    XCTAssertEqual(Double(actual.tx), expected.tx, accuracy: 2, file: file, line: line)
    XCTAssertEqual(Double(actual.ty), expected.ty, accuracy: 2, file: file, line: line)
}

private extension String {
    func nonOverlappingCount(of needle: String) -> Int {
        var count = 0
        var search = startIndex..<endIndex
        while let range = range(of: needle, range: search) {
            count += 1
            search = range.upperBound..<endIndex
        }
        return count
    }
}
