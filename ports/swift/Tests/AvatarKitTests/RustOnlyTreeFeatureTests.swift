// The render-tree features in vectors/tree-features/ (compound paths, focal
// radial gradients, stop opacity and the pixel grid) are drawn only by the
// Rust rasterizer: apps receive those creatures as finished images. This
// decoder deliberately does not learn them, so it must keep failing closed on
// every such tree rather than drawing it without the feature. These tests use
// only the decoder, so they run on every platform.
import Foundation
import XCTest
import AvatarKit

final class RustOnlyTreeFeatureTests: XCTestCase {
    /// Each fixture with the exact refusal the decoder gives today: where
    /// decoding stops, written as the field path, and the error's description.
    /// Pinning both means a fixture that failed for an unrelated reason, such
    /// as a typo in an ordinary field, would not pass.
    private let expectedRefusals: [String: (path: String, reason: String)] = [
        "compound-path.json":
            ("primitives[0].shape", "unknown Shape tag \"compound_path\""),
        "compound-path-clip.json":
            ("masks[0][0].shape", "unknown Shape tag \"compound_path\""),
        "compound-path-mixed.json":
            ("primitives[0].shape", "unknown Shape tag \"compound_path\""),
        "pixel-grid.json":
            ("", "unknown render-tree field(s): pixel_grid"),
        "radial-gradient-at.json":
            ("primitives[0].paint", "unknown Paint tag \"radial_gradient_at\""),
        "radial-gradient-in-box.json":
            ("primitives[0].paint", "unknown Paint tag \"radial_gradient_at\""),
        "stop-opacity.json":
            ("primitives[0].paint.stops[0]", "unknown render-tree field(s): opacity_q32"),
    ]

    private func featureDirectory() throws -> URL {
        try vectorsRoot().appendingPathComponent("tree-features")
    }

    func testEveryTreeFeatureFixtureHasAnExpectedRefusal() throws {
        // A fixture added without an entry above would otherwise never be checked.
        let names = try FileManager.default.contentsOfDirectory(atPath: featureDirectory().path)
            .filter { $0.hasSuffix(".json") }
            .sorted()
        XCTAssertEqual(names, expectedRefusals.keys.sorted())
    }

    func testDecoderRefusesEveryTreeFeatureFixture() throws {
        let directory = try featureDirectory()
        for (name, expected) in expectedRefusals.sorted(by: { $0.key < $1.key }) {
            let bytes = try [UInt8](Data(contentsOf: directory.appendingPathComponent(name)))
            XCTAssertThrowsError(try decodeRenderTreeJson(bytes), name) { error in
                guard case DecodingError.dataCorrupted(let context) = error else {
                    XCTFail("\(name): expected a corrupted-data refusal, got \(error)")
                    return
                }
                XCTAssertEqual(fieldPath(context.codingPath), expected.path, name)
                XCTAssertEqual(context.debugDescription, expected.reason, name)
            }
        }
    }
}

/// Renders a coding path the way the tree's field paths read: names joined by
/// dots, array positions in brackets.
private func fieldPath(_ codingPath: [CodingKey]) -> String {
    var path = ""
    for key in codingPath {
        if let index = key.intValue {
            path += "[\(index)]"
        } else {
            path += path.isEmpty ? key.stringValue : ".\(key.stringValue)"
        }
    }
    return path
}
