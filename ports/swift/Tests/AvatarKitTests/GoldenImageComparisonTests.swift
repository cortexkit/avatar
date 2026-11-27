// Tests of the cross-platform image rule in GoldenImageComparison.swift. They
// use no embedded core, so they run on every platform.
import XCTest

final class GoldenImageComparisonTests: XCTestCase {
    /// Golden-like RGBA bytes whose channels leave room to move up a few
    /// levels.
    private func pixels(sizePx: Int) -> [UInt8] {
        (0..<sizePx * sizePx * 4).map { UInt8($0 * 37 % 200 + 20) }
    }

    /// `pixels` with the green channel of the first `count` pixels raised by
    /// `by` levels.
    private func raisingGreen(_ pixels: [UInt8], count: Int, by: UInt8) -> [UInt8] {
        var changed = pixels
        for pixel in 0..<count {
            changed[pixel * 4 + 1] += by
        }
        return changed
    }

    /// Identical images match. Up to 8/255 on up to max(1, 0.1%) of the
    /// pixels matches. One more level, or one more pixel, does not.
    func testToleranceBoundsTheChannelDifferenceAndThePixelCount() {
        // 65,536 pixels allow 65 to differ; 256 pixels allow 1.
        for (sizePx, allowed) in [(256, 65), (16, 1)] {
            let golden = pixels(sizePx: sizePx)
            let label = "\(sizePx) px"

            let identical = GoldenImageDifference(expected: golden, actual: golden)
            XCTAssertTrue(identical.isIdentical, label)
            XCTAssertTrue(identical.isWithinTolerance, label)

            let within = GoldenImageDifference(
                expected: golden,
                actual: raisingGreen(golden, count: allowed, by: UInt8(goldenImageToleranceChannel)))
            XCTAssertEqual(
                within,
                GoldenImageDifference(
                    maxChannel: goldenImageToleranceChannel, differingPixels: allowed,
                    pixels: sizePx * sizePx),
                label)
            XCTAssertFalse(within.isIdentical, label)
            XCTAssertTrue(within.isWithinTolerance, label)

            let overDifference = GoldenImageDifference(
                expected: golden,
                actual: raisingGreen(golden, count: 1, by: UInt8(goldenImageToleranceChannel + 1)))
            XCTAssertEqual(overDifference.differingPixels, 1, label)
            XCTAssertFalse(overDifference.isWithinTolerance, label)

            let overCount = GoldenImageDifference(
                expected: golden, actual: raisingGreen(golden, count: allowed + 1, by: 1))
            XCTAssertEqual(overCount.maxChannel, 1, label)
            XCTAssertFalse(overCount.isWithinTolerance, label)
        }
    }

    func testAnImageWithinTheToleranceMatchesButIsNotIdentical() {
        let golden = pixels(sizePx: 16)
        XCTAssertTrue(assertImageMatchesGolden(golden, golden: golden, label: "identical"))
        XCTAssertFalse(assertImageMatchesGolden(
            raisingGreen(golden, count: 1, by: UInt8(goldenImageToleranceChannel)),
            golden: golden, label: "within"))
    }

    #if os(macOS)
    // XCTExpectFailure exists only in Apple's XCTest.
    func testAnImageBeyondTheToleranceFails() {
        let golden = pixels(sizePx: 16)
        // A 16 by 16 image may have only 1 differing pixel.
        XCTExpectFailure("2 differing pixels are beyond the tolerance")
        assertImageMatchesGolden(
            raisingGreen(golden, count: 2, by: 1), golden: golden, label: "beyond")
    }
    #endif
}
