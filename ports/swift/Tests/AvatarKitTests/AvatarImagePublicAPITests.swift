#if os(macOS) && canImport(AvatarCore)
import Foundation
import XCTest
// Deliberately not @testable: this file sees only AvatarKit's public API, as
// an app does, so it fails to compile if anything an app needs to request
// and read a finished image is not public.
import AvatarKit

final class AvatarImagePublicAPITests: XCTestCase {
    func testAnAppCanRequestNonDefaultOptionsAndReadTheImage() throws {
        let root = try vectorsRoot()
        let genome = try readGenomeBytes(
            root.appendingPathComponent("wild/wild-port-0022.hex"), expectedCount: 16_384)
        let options = AvatarImageOptions(groundShadow: .drop, alpha: .straight)
        XCTAssertNotEqual(options, AvatarImageOptions())

        let image = try renderImageViaCore(
            genomeBytes: genome, type: wildAvatarType, version: 2, mood: .neutral, sizePx: 256,
            options: options)
        XCTAssertEqual(image.sizePx, 256)
        XCTAssertEqual(image.alpha, .straight)
        XCTAssertEqual(image.pixels.count, 256 * 256 * 4)

        // The same drawing in premultiplied alpha lets this test recompute
        // every straight pixel with the documented formula, so it reads
        // every pixel rather than only the count.
        var premultipliedOptions = options
        premultipliedOptions.alpha = .premultiplied
        let premultiplied = try renderImageViaCore(
            genomeBytes: genome, type: wildAvatarType, version: 2, mood: .neutral, sizePx: 256,
            options: premultipliedOptions)
        XCTAssertEqual(premultiplied.alpha, .premultiplied)
        XCTAssertEqual(premultiplied.pixels.count, image.pixels.count)
        var transparent = 0
        var partial = 0
        var mismatches = 0
        for offset in stride(from: 0, to: image.pixels.count, by: 4) {
            let alpha = Int(premultiplied.pixels[offset + 3])
            if alpha == 0 { transparent += 1 } else if alpha < 255 { partial += 1 }
            if image.pixels[offset + 3] != premultiplied.pixels[offset + 3] { mismatches += 1 }
            for channel in 0..<3 {
                let colour = Int(premultiplied.pixels[offset + channel])
                let expected = alpha == 0 ? 0 : (colour * 255 + alpha / 2) / alpha
                if Int(image.pixels[offset + channel]) != expected { mismatches += 1 }
            }
        }
        XCTAssertEqual(mismatches, 0)
        // Both alpha branches of the formula must actually occur.
        XCTAssertGreaterThan(transparent, 0)
        XCTAssertGreaterThan(partial, 0)

        // Dropping the ground shadow changes the image of this creature.
        let withShadow = try renderImageViaCore(
            genomeBytes: genome, type: wildAvatarType, version: 2, mood: .neutral, sizePx: 256,
            options: AvatarImageOptions(groundShadow: .keep, alpha: .straight))
        XCTAssertNotEqual(withShadow.pixels, image.pixels)
    }
}
#endif
