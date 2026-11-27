// The rule for comparing a finished image with its golden in vectors/image/.
//
// Render trees are byte-identical on every platform. Finished images are
// identical on one platform, but may differ slightly between platforms,
// because the rasterizer uses the platform's maths library. When tiny-skia
// 0.11.4 clips a cubic curve at the edge of the area being drawn
// (edge_clipper.rs, through path_geometry::chop_mono_cubic_at_y), it solves
// for the crossing with path64::cubic64::roots_real. That function calls
// f64::acos and f64::cos, which come from the system's maths library and can
// differ in the last bit between, for example, macOS and Linux. A different
// last bit can move a clipped edge by one sample of the anti-aliasing grid,
// which changes a pixel's coverage by 1/16.
//
// So an image matches its golden if its pixels are identical, or if no
// channel differs by more than 8/255 and at most max(1, 0.1%) of its pixels
// differ. The Rust golden-image test (crates/avatar-vectors/src/tests/image.rs)
// and scripts/wild-port/compare_images.py apply the same rule. This file uses
// only Foundation and XCTest, so it compiles on every platform.
import Foundation
import XCTest

/// The largest channel difference the cross-platform image rule allows.
let goldenImageToleranceChannel = 8

/// How a finished image differs from its golden, pixel by pixel.
struct GoldenImageDifference: Equatable {
    /// The largest difference in any channel of any pixel.
    let maxChannel: Int
    /// How many pixels differ in at least one channel.
    let differingPixels: Int
    /// How many pixels the image has.
    let pixels: Int

    var isIdentical: Bool { differingPixels == 0 }

    /// How many pixels may differ: 0.1% of the image, but at least one.
    var allowedDifferingPixels: Int { max(1, pixels / 1_000) }

    var isWithinTolerance: Bool {
        maxChannel <= goldenImageToleranceChannel && differingPixels <= allowedDifferingPixels
    }
}

extension GoldenImageDifference {
    /// Compares two RGBA8 pixel buffers of the same length.
    init(expected: [UInt8], actual: [UInt8]) {
        precondition(
            expected.count == actual.count && expected.count % 4 == 0,
            "compared images must be RGBA8 buffers of the same length")
        var maxChannel = 0
        var differingPixels = 0
        for pixel in stride(from: 0, to: expected.count, by: 4) {
            var differs = false
            for channel in pixel..<pixel + 4 where expected[channel] != actual[channel] {
                differs = true
                maxChannel = max(maxChannel, abs(Int(expected[channel]) - Int(actual[channel])))
            }
            if differs { differingPixels += 1 }
        }
        self.init(
            maxChannel: maxChannel, differingPixels: differingPixels, pixels: expected.count / 4)
    }
}

/// The platform the tests run on, named as the Rust image tests name it.
let goldenImagePlatform: String = {
    #if os(macOS)
    let system = "macos"
    #elseif os(Linux)
    let system = "linux"
    #else
    let system = "other"
    #endif
    #if arch(arm64)
    return "\(system)-aarch64"
    #elseif arch(x86_64)
    return "\(system)-x86_64"
    #else
    return "\(system)-other"
    #endif
}()

/// Checks an image's pixels against its golden's under the cross-platform
/// rule. Every image that is not identical is printed with the platform, the
/// case, the largest channel difference and the number of differing pixels,
/// so drift stays visible even when it passes. Returns whether the pixels
/// were identical.
@discardableResult
func assertImageMatchesGolden(
    _ actual: [UInt8], golden: [UInt8], label: String,
    file: StaticString = #filePath, line: UInt = #line
) -> Bool {
    // Compare the arrays first: equal pixels need no measuring, and an
    // XCTAssertEqual on a mismatch would print megabytes of pixels.
    if actual == golden { return true }
    guard actual.count == golden.count else {
        XCTFail(
            "\(label): \(actual.count) bytes of pixels, the golden has \(golden.count)",
            file: file, line: line)
        return false
    }
    let difference = GoldenImageDifference(expected: golden, actual: actual)
    let measured = "\(goldenImagePlatform) \(label): max channel difference "
        + "\(difference.maxChannel)/255, \(difference.differingPixels) of "
        + "\(difference.pixels) pixels differ"
    if difference.isWithinTolerance {
        print("\(measured); within the image tolerance")
    } else {
        XCTFail(
            "\(measured), beyond the image tolerance of \(goldenImageToleranceChannel)/255 "
                + "on at most \(difference.allowedDifferingPixels) pixels",
            file: file, line: line)
    }
    return false
}
