import Foundation
import XCTest
@testable import AvatarKit

final class RenderConformanceTests: XCTestCase {
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
