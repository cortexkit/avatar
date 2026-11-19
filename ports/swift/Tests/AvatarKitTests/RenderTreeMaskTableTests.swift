import Foundation
import XCTest
@testable import AvatarKit

// Masks travel in the tree's top-level "masks" table, and a masked effect
// names its entry with "mask_ref". These documents are hand-written in that
// canonical form so each rejection below isolates one rule of the table.
final class RenderTreeMaskTableTests: XCTestCase {
    private struct NotMasked: Error {}

    private func primitive(_ label: String, _ effects: [String] = []) -> String {
        #"{"clip":null,"effects":[\#(effects.joined(separator: ","))],"label":"\#(label)","opacity":null,"paint":{"kind":"none"},"role":null,"shape":{"cx":0,"cy":0,"kind":"circle","r":1},"stroke":null,"transform":null}"#
    }

    private func masked(_ maskRef: String) -> String {
        #"{"kind":"masked","mask_ref":\#(maskRef)}"#
    }

    private func document(_ masks: [[String]], _ primitives: [String]) -> [UInt8] {
        let table = masks.map { "[" + $0.joined(separator: ",") + "]" }.joined(separator: ",")
        let list = primitives.joined(separator: ",")
        return Array(#"{"background":null,"canvas":240,"masks":[\#(table)],"primitives":[\#(list)]}"#.utf8)
    }

    private func maskOf(_ primitive: Primitive) throws -> [Primitive] {
        guard case .masked(let mask)? = primitive.effects.first else {
            throw NotMasked()
        }
        return mask
    }

    private func assertRejects(
        _ bytes: [UInt8], _ phrase: String,
        file: StaticString = #filePath, line: UInt = #line
    ) {
        XCTAssertThrowsError(try decodeRenderTreeJson(bytes), file: file, line: line) { error in
            XCTAssertTrue(
                "\(error)".contains(phrase), "expected \"\(phrase)\" in \(error)",
                file: file, line: line)
        }
    }

    func testSharedReferencesResolveToOneStoredMaskAndReencodeIdentically() throws {
        let source = document(
            [[primitive("inner")], [primitive("outer", [masked("0")])]],
            [
                primitive("a", [masked("1")]),
                primitive("b", [masked("1")]),
                primitive("c", [masked("0")]),
            ])
        let tree = try decodeRenderTreeJson(source)
        XCTAssertEqual(tree.primitives.count, 3)
        guard tree.primitives.count == 3 else { return }
        let a = try maskOf(tree.primitives[0])
        let b = try maskOf(tree.primitives[1])
        let c = try maskOf(tree.primitives[2])
        XCTAssertEqual(a.map(\.label), ["outer"])
        XCTAssertEqual(c.map(\.label), ["inner"])
        XCTAssertEqual(a, b)
        XCTAssertEqual(try maskOf(XCTUnwrap(a.first)), c)
        // Both references hold one array's storage, not two copies of it.
        XCTAssertEqual(
            a.withUnsafeBufferPointer { $0.baseAddress },
            b.withUnsafeBufferPointer { $0.baseAddress })
        // The source is canonical, so re-encoding the decoded tree reproduces it.
        XCTAssertEqual(canonicalTreeJSONBytes(tree), source)
    }

    func testRejectsMaskRefOutsideTheTable() {
        for outside in ["1", "-1"] {
            assertRejects(
                document(
                    [[primitive("m")]],
                    [primitive("a", [masked("0")]), primitive("b", [masked(outside)])]),
                "is outside the mask table of 1 entries")
        }
        assertRejects(
            document([], [primitive("a", [masked("0")])]),
            "is outside the mask table of 0 entries")
    }

    func testRejectsMaskEntryThatRefersToItselfOrToALaterEntry() {
        assertRejects(
            document([[primitive("m", [masked("0")])]], [primitive("a", [masked("0")])]),
            "must refer to an entry before mask entry 0")
        assertRejects(
            document(
                [[primitive("m", [masked("1")])], [primitive("n")]],
                [primitive("a", [masked("0")]), primitive("b", [masked("1")])]),
            "must refer to an entry before mask entry 0")
    }

    func testRejectsInlineMaskMember() {
        assertRejects(
            document(
                [[primitive("m")]],
                [primitive("a", [#"{"kind":"masked","mask":[],"mask_ref":0}"#])]),
            "unknown render-tree field(s): mask")
        assertRejects(
            document([], [primitive("a", [#"{"kind":"masked","mask":["# + primitive("m") + "]}"])]),
            "unknown render-tree field(s): mask")
    }

    func testRejectsMaskEntryThatNothingReferences() {
        assertRejects(
            document([[primitive("m")]], [primitive("a")]),
            "mask entry 0 is never referenced")
        // When an unused entry is the only one referring to another, the
        // unused one is reported.
        assertRejects(
            document([[primitive("m")], [primitive("n", [masked("0")])]], [primitive("a")]),
            "mask entry 1 is never referenced")
    }

    func testRejectsTreeWithoutMaskTable() {
        let missing = Array(#"{"background":null,"canvas":240,"primitives":[]}"#.utf8)
        XCTAssertThrowsError(try decodeRenderTreeJson(missing)) { error in
            guard case DecodingError.keyNotFound(let key, _) = error else {
                return XCTFail("expected a missing masks key, got \(error)")
            }
            XCTAssertEqual(key.stringValue, "masks")
        }
    }

    func testRejectsNonIntegerMaskRef() {
        for maskRef in [#""0""#, "null", "true", "[0]"] {
            XCTAssertThrowsError(try decodeRenderTreeJson(
                document([[primitive("m")]], [primitive("a", [masked(maskRef)])])), maskRef)
        }
        // Fraction and exponent spellings never get past the integer-only grammar.
        for maskRef in ["0.0", "0.5", "1e0"] {
            assertRejects(
                document([[primitive("m")]], [primitive("a", [masked(maskRef)])]),
                "render-tree numbers must use integer JSON tokens")
        }
    }

    func testDecodingWithoutTheTreeDecoderFailsClosed() {
        // Mask references resolve only inside decodeRenderTreeJson, so a bare
        // JSONDecoder refuses the tree instead of decoding it without masks.
        let minimal = Data(#"{"background":null,"canvas":1,"masks":[],"primitives":[]}"#.utf8)
        XCTAssertNoThrow(try decodeRenderTreeJson([UInt8](minimal)))
        XCTAssertThrowsError(try JSONDecoder().decode(RenderTree.self, from: minimal))
    }
}
