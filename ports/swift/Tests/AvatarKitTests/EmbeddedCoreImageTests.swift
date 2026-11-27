#if os(macOS) && canImport(AvatarCore)
import Foundation
import XCTest
@testable import AvatarKit

final class EmbeddedCoreImageTests: XCTestCase {
    func testEveryGoldenImageMatchesTheCoreImage() throws {
        let root = try vectorsRoot()
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let manifest = try decoder.decode(
            GoldenImageManifest.self,
            from: Data(contentsOf: root.appendingPathComponent("image/manifest.json")))
        XCTAssertFalse(manifest.entries.isEmpty)

        var optionsSeen: Set<String> = []
        var typesSeen: Set<String> = []
        var identical = 0
        for entry in manifest.entries {
            let mood = try XCTUnwrap(Mood(rawValue: entry.mood), "\(entry.file): unknown mood")
            let options = try imageOptions(entry)
            // Every type but classic reads the 16,384-byte genome wild reads.
            let genome = try readGenomeBytes(
                root.appendingPathComponent(entry.genomeHex),
                expectedCount: entry.type == classicAvatarType ? 1_024 : 16_384)
            let image = try renderImageViaCore(
                genomeBytes: genome, type: entry.type, version: entry.version, mood: mood,
                sizePx: entry.sizePx, options: options)
            let golden = try decodeGoldenPNG(
                Data(contentsOf: root.appendingPathComponent("image/\(entry.file)")),
                label: entry.file)

            XCTAssertEqual(golden.width, Int(entry.sizePx), entry.file)
            XCTAssertEqual(golden.height, Int(entry.sizePx), entry.file)
            XCTAssertEqual(image.sizePx, entry.sizePx, entry.file)
            XCTAssertEqual(image.alpha, options.alpha, entry.file)
            // Images may differ slightly between platforms; see
            // GoldenImageComparison.swift for the rule and its cause.
            if assertImageMatchesGolden(image.pixels, golden: golden.pixels, label: entry.file) {
                identical += 1
            }
            optionsSeen.insert("\(entry.groundShadow)/\(entry.alpha)")
            typesSeen.insert(entry.type)
        }
        // A swapped or ignored option passes unless the matrix sets each
        // option both ways.
        XCTAssertEqual(optionsSeen.count, 4)
        XCTAssertTrue(typesSeen.isSuperset(of: [classicAvatarType, wildAvatarType]))
        print(
            "Embedded core matched \(manifest.entries.count) golden images on "
                + "\(goldenImagePlatform), \(identical) of them exactly")
    }

    /// Knitted toys draw with compound paths, focal radial gradients and stop
    /// opacity, tree features this package's decoder deliberately refuses. An
    /// app still gets the creature, as a finished image from the core.
    func testANewTypeRendersAsAnImageThoughItsTreeIsRefused() throws {
        let root = try vectorsRoot()
        let type = "creature.knitted-toys"
        let genome = try readGenomeBytes(
            root.appendingPathComponent("wild/wild-port-0005.hex"), expectedCount: 16_384)
        let rustOnlyFeatures = ["compound_path", "radial_gradient_at", "opacity_q32"]

        // The core draws the tree, and the decoder refuses it at one of the
        // features it does not know.
        XCTAssertThrowsError(try renderTreeViaCore(
            genomeBytes: genome, type: type, version: 1, mood: .neutral
        )) { error in
            guard case DecodingError.dataCorrupted(let context) = error else {
                return XCTFail("expected the decoder to refuse the tree, got \(error)")
            }
            XCTAssertTrue(
                rustOnlyFeatures.contains { context.debugDescription.contains($0) },
                context.debugDescription)
        }
        // It refuses the committed golden tree for the same genome too.
        XCTAssertThrowsError(try decodeRenderTreeJson([UInt8](Data(contentsOf:
            root.appendingPathComponent("knitted-toys/tree/wild-port-0005__neutral.json")))))

        let image = try renderImageViaCore(
            genomeBytes: genome, type: type, version: 1, mood: .neutral, sizePx: 256)
        let file = "knitted-toys__wild-port-0005__v1__neutral__256px__keep__premultiplied.png"
        let golden = try decodeGoldenPNG(
            Data(contentsOf: root.appendingPathComponent("image/\(file)")), label: file)
        XCTAssertEqual(image.sizePx, 256)
        XCTAssertEqual(image.alpha, .premultiplied)
        assertImageMatchesGolden(image.pixels, golden: golden.pixels, label: file)
    }

    func testSizesOutsideOneTo2048ThrowInvalidArgument() throws {
        let genome = [UInt8](repeating: 7, count: 16_384)
        for sizePx: UInt32 in [0, 2_049, .max] {
            XCTAssertThrowsError(try renderImageViaCore(
                genomeBytes: genome, type: wildAvatarType, version: 2, mood: .neutral,
                sizePx: sizePx
            )) { XCTAssertEqual($0 as? CoreRenderError, .invalidArgument, "size \(sizePx)") }
        }
        for sizePx: UInt32 in [1, 2_048] {
            let image = try renderImageViaCore(
                genomeBytes: genome, type: wildAvatarType, version: 2, mood: .neutral,
                sizePx: sizePx)
            XCTAssertEqual(image.sizePx, sizePx)
            XCTAssertEqual(image.pixels.count, Int(sizePx) * Int(sizePx) * 4)
        }
    }

    func testRefusedImageInputsSurfaceTheTreeCallsErrors() throws {
        let wild = [UInt8](repeating: 7, count: 16_384)
        let classic = [UInt8](repeating: 0, count: 1_024)
        // Type, version and genome length are checked before the size, so
        // the first three ask for a size the core would also refuse and must
        // still report the type, version or length.
        let cases: [([UInt8], String, UInt32, String, UInt32, CoreRenderError)] = [
            (wild, "creature.unknown", 1, "neutral", 0, .unsupportedType),
            (classic, classicAvatarType, 999, "neutral", 2_049, .unsupportedVersion),
            (Array(classic.dropLast()), classicAvatarType, 2, "neutral", 0, .genomeLength),
            (classic, classicAvatarType, 2, "unknown", 256, .unknownMood),
            (wild, "creature.wild\0suffix", 2, "neutral", 256, .unsupportedType),
            (wild, wildAvatarType, 2, "neutral\0suffix", 256, .unknownMood),
        ]
        for (genome, type, version, mood, sizePx, expected) in cases {
            XCTAssertThrowsError(try renderImageViaCoreBytes(
                genomeBytes: genome, type: type, version: version, moodName: mood,
                sizePx: sizePx, options: AvatarImageOptions()
            )) { XCTAssertEqual($0 as? CoreRenderError, expected, "\(type) \(mood)") }
        }
        // The public call refuses a type with an embedded NUL as the tree
        // call does, rather than drawing the type before the NUL.
        XCTAssertThrowsError(try renderImageViaCore(
            genomeBytes: wild, type: "creature.wild\0suffix", version: 2, mood: .neutral,
            sizePx: 64
        )) { XCTAssertEqual($0 as? CoreRenderError, .unsupportedType) }
        XCTAssertThrowsError(try renderTreeViaCore(
            genomeBytes: wild, type: "creature.wild\0suffix", version: 2, mood: .neutral
        )) { XCTAssertEqual($0 as? CoreRenderError, .unsupportedType) }
    }

    func testImageCallsCanRunConcurrently() throws {
        let genome = [UInt8](repeating: 0x5a, count: 16_384)
        let options = AvatarImageOptions(groundShadow: .drop, alpha: .straight)
        let expected = try renderImageViaCore(
            genomeBytes: genome, type: wildAvatarType, version: 2, mood: .happy, sizePx: 96,
            options: options)
        DispatchQueue.concurrentPerform(iterations: 16) { _ in
            do {
                let image = try renderImageViaCore(
                    genomeBytes: genome, type: wildAvatarType, version: 2, mood: .happy,
                    sizePx: 96, options: options)
                XCTAssertEqual(image, expected)
            } catch {
                XCTFail("Concurrent image call failed: \(error)")
            }
        }
    }
}

private func imageOptions(_ entry: GoldenImageEntry) throws -> AvatarImageOptions {
    let groundShadow: AvatarImageOptions.GroundShadow
    switch entry.groundShadow {
    case "keep": groundShadow = .keep
    case "drop": groundShadow = .drop
    default:
        throw GoldenImageError.malformed("\(entry.file): unknown ground_shadow \(entry.groundShadow)")
    }
    let alpha: AvatarImage.Alpha
    switch entry.alpha {
    case "premultiplied": alpha = .premultiplied
    case "straight": alpha = .straight
    default:
        throw GoldenImageError.malformed("\(entry.file): unknown alpha \(entry.alpha)")
    }
    return AvatarImageOptions(groundShadow: groundShadow, alpha: alpha)
}
#endif
