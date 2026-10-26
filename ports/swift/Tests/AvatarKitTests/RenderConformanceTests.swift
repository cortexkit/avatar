import Foundation
import XCTest
#if canImport(CryptoKit)
import CryptoKit
#endif
@testable import AvatarKit

final class RenderConformanceTests: XCTestCase {
    func testAvatarTypeNamesMatchStoredWireNames() {
        XCTAssertEqual(classicAvatarType, "creature.classic")
        XCTAssertEqual(wildAvatarType, "creature.wild")
    }

    func testBoundary3EveryRustTreeDecodesAndReencodesToIdenticalBytes() throws {
        let (root, vectors) = try loadManifest()
        let treeDirectory = root.appendingPathComponent("tree")
        let names = try FileManager.default.contentsOfDirectory(atPath: treeDirectory.path)
            .filter { $0.hasSuffix(".json") }
            .sorted()
        var inventory = TreeInventory()
        for name in names {
            let bytes = try [UInt8](Data(contentsOf: treeDirectory.appendingPathComponent(name)))
            let tree = try decodeRenderTreeJson(bytes)
            inventory.observe(tree)
            assertByteEqual(
                "\(name) lossless tree round trip",
                actual: canonicalTreeJSONBytes(tree),
                expected: bytes)
        }
        let classicCount = vectors.reduce(0) { $0 + $1.moods.count }
        XCTAssertEqual(names.count, classicCount + wildTreeFixtureNames.count)
        XCTAssertTrue(inventory.hasGradient, "second-type fixtures must exercise gradient paint")
        XCTAssertTrue(
            inventory.hasGradientInsideMask,
            "second-type fixtures must exercise gradient-content masks")
        XCTAssertTrue(inventory.hasPathFillRule, "tree fixtures must exercise path fill rules")
    }

    func testTreeDecoderRejectsMissingUnknownAndMalformedValues() throws {
        let minimal = Array("{\"background\":null,\"canvas\":10000,\"primitives\":[]}".utf8)
        XCTAssertNoThrow(try decodeRenderTreeJson(minimal))
        for malformed in [
            "{\"canvas\":10000,\"primitives\":[]}",
            "{\"background\":null,\"canvas\":10000.0,\"primitives\":[]}",
            "{\"background\":null,\"canvas\":4294967296,\"primitives\":[]}",
            "{\"background\":null,\"canvas\":10000,\"extra\":0,\"primitives\":[]}",
        ] {
            XCTAssertThrowsError(try decodeRenderTreeJson(Array(malformed.utf8)))
        }

        let tree = RenderTree(
            canvas: 10_000,
            background: nil,
            primitives: [Primitive(
                shape: .rect(x: 1, y: 2, w: 3, h: 4),
                paint: .none,
                label: "strict")])
        let valid = String(decoding: canonicalTreeJSONBytes(tree), as: UTF8.self)
        XCTAssertThrowsError(try decodeRenderTreeJson(Array(valid.replacingOccurrences(
            of: "\"kind\":\"none\"", with: "\"kind\":\"unknown\"").utf8)))
        XCTAssertThrowsError(try decodeRenderTreeJson(Array(valid.replacingOccurrences(
            of: ",\"transform\":null", with: "").utf8)))
    }

    private var roleDocument: String {
        """
        {"background":null,"canvas":1,"primitives":[{"clip":null,"effects":[],"label":"diagnostic","opacity":null,"paint":{"kind":"none"},"role":"ground_shadow","shape":{"cx":0,"cy":0,"kind":"circle","r":1},"stroke":null,"transform":null}]}
        """
    }

    func testRoleDecoderRejectsUnknownValues() throws {
        let unknown = roleDocument.replacingOccurrences(of: "ground_shadow", with: "unknown_role")
        XCTAssertThrowsError(try decodeRenderTreeJson(Array(unknown.utf8)))
    }

    func testRoleDecoderPreservesKnownAndNullValuesAndRequiresPresence() throws {
        let known = try decodeRenderTreeJson(Array(roleDocument.utf8))
        XCTAssertEqual(known.primitives.first?.role, .groundShadow)
        let null = roleDocument.replacingOccurrences(of: "\"ground_shadow\"", with: "null")
        let absent = roleDocument.replacingOccurrences(of: ",\"role\":\"ground_shadow\"", with: "")
        XCTAssertNil(try decodeRenderTreeJson(Array(null.utf8)).primitives.first?.role)
        XCTAssertThrowsError(try decodeRenderTreeJson(Array(absent.utf8)))
    }

    func testCorpusTreesAreFlatAndEveryAffineMatchesNaiveComposition() throws {
        var transformCount = 0
        for (_, tree) in try decodedCorpusTrees() {
            for primitive in allPrimitives(in: tree.primitives) {
                guard let transform = primitive.transform else { continue }
                assertAffine(transform.affine(), equals: naiveAffine(transform.ops))
                transformCount += 1
            }
        }
        XCTAssertGreaterThan(transformCount, 900)
    }

    func testFixedPointMultiplyKeepsFullWidthAndRoundsHalvesAwayFromZero() {
        XCTAssertEqual(q32Mul(1, 1 << 31), 1)
        XCTAssertEqual(q32Mul(-1, 1 << 31), -1)
        XCTAssertEqual(q32Mul(-1, -(1 << 31)), 1)
        XCTAssertEqual(q32Mul(3, 1 << 31), 2)
        XCTAssertEqual(q32Mul(-3, 1 << 31), -2)
        XCTAssertEqual(q32Mul(1, (1 << 31) - 1), 0)
        XCTAssertEqual(q32Mul(Int64.max, 1 << 32), Int64.max)
        XCTAssertEqual(q32Mul(Int64.min, 1 << 32), Int64.min)
    }

    func testStrokeCapsAndJoinsMatchTheCorpusInventory() throws {
        let root = try vectorsRoot()
        var roundCaps = 0
        var roundJoins = 0
        for (name, tree) in try decodedCorpusTrees() {
            let strokes = allPrimitives(in: tree.primitives).compactMap(\.stroke)
            let svg = try String(contentsOf: root.appendingPathComponent(
                "tree-svg/\(name.replacingOccurrences(of: ".json", with: ".svg"))"),
                encoding: .utf8)
            // The generic Rust SVG fixtures independently pin the decoded enum values,
            // including strokes inside masks, without rebuilding native geometry.
            for cap in [StrokeCap.butt, .round, .square] {
                XCTAssertEqual(strokes.filter { $0.cap == cap }.count,
                    svg.components(separatedBy: "stroke-linecap=\"\(cap.rawValue)\"").count - 1,
                    "\(name): \(cap.rawValue) caps")
            }
            for join in [StrokeJoin.miter, .round, .bevel] {
                XCTAssertEqual(strokes.filter { $0.join == join }.count,
                    svg.components(separatedBy: "stroke-linejoin=\"\(join.rawValue)\"").count - 1,
                    "\(name): \(join.rawValue) joins")
            }
            roundCaps += strokes.filter { $0.cap == .round }.count
            roundJoins += strokes.filter { $0.join == .round }.count
        }
        XCTAssertGreaterThan(roundCaps, 0)
        XCTAssertGreaterThan(roundJoins, 0)
        let pattern = try decodeRenderTreeJson([UInt8](Data(contentsOf:
            root.appendingPathComponent("tree/pattern_0__neutral.json"))))
        let caps = pattern.primitives.compactMap(\.stroke?.cap)
        XCTAssertTrue(caps.contains(.round))
        XCTAssertTrue(caps.contains(.butt))
    }

    func testSinQuarterMatchesTheRustTable() throws {
        let root = try vectorsRoot().deletingLastPathComponent()
        let source = try String(contentsOf: root.appendingPathComponent(
            "crates/avatar-tables/src/generated/tables.rs"), encoding: .utf8)
        let declaration = try XCTUnwrap(source.range(of:
            "pub const SIN_QUARTER: [u32; 4096] = ["))
        let end = try XCTUnwrap(source.range(of: "];",
            range: declaration.upperBound..<source.endIndex))
        let rust = try source[declaration.upperBound..<end.lowerBound]
            .split { $0 == "," || $0.isWhitespace }
            .map { try XCTUnwrap(UInt32($0)) }
        XCTAssertEqual(SIN_QUARTER.count, 4_096)
        XCTAssertEqual(SIN_QUARTER, rust)
    }

    #if canImport(CryptoKit)
    func testSinQuarterCanonicalBytesHavePinnedHash() {
        var bytes: [UInt8] = []
        for value in SIN_QUARTER {
            withUnsafeBytes(of: value.littleEndian) { bytes.append(contentsOf: $0) }
        }
        // SHA-256 over the Rust mint's little-endian u32 samples pins the data
        // without retaining CBlake3 solely for a test. Every platform also checks
        // sample-for-sample equality with the committed Rust table above.
        let hash = SHA256.hash(data: Data(bytes)).map { String(format: "%02x", $0) }.joined()
        XCTAssertEqual(hash, "62759494c1cbb3f6a3f6f93a66338b017072c9c278f79d85823281a29b2ae89f")
    }
    #endif
}

private func decodedCorpusTrees() throws -> [(String, RenderTree)] {
    let directory = try vectorsRoot().appendingPathComponent("tree")
    let names = try FileManager.default.contentsOfDirectory(atPath: directory.path)
        .filter { $0.hasSuffix(".json") }.sorted()
    XCTAssertFalse(names.isEmpty)
    return try names.map { name in
        let bytes = try [UInt8](Data(contentsOf: directory.appendingPathComponent(name)))
        return (name, try decodeRenderTreeJson(bytes))
    }
}

private func allPrimitives(in primitives: [Primitive]) -> [Primitive] {
    primitives.flatMap { primitive in
        [primitive] + primitive.effects.flatMap { effect -> [Primitive] in
            if case .masked(let mask) = effect {
                return allPrimitives(in: mask)
            }
            return []
        }
    }
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
            // Resolve the pinned trig values, then compose with Double arithmetic
            // independently of the drawing helper's fixed-point multiplication.
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

private struct TreeInventory {
    var hasGradient = false
    var hasGradientInsideMask = false
    var hasPathFillRule = false

    mutating func observe(_ tree: RenderTree) {
        observe(tree.primitives, insideMask: false)
    }

    private mutating func observe(_ primitives: [Primitive], insideMask: Bool) {
        for primitive in primitives {
            if case .path = primitive.shape {
                hasPathFillRule = true
            }
            if case .radialGradient = primitive.paint {
                hasGradient = true
                hasGradientInsideMask = hasGradientInsideMask || insideMask
            }
            for effect in primitive.effects {
                if case .masked(let mask) = effect {
                    observe(mask, insideMask: true)
                }
            }
        }
    }
}
