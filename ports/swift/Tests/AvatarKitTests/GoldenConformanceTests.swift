// Golden-vector conformance for the Swift port (design note 001 §6).
//
// The corpus is minted by the canonical Rust implementation and committed at
// the avatar repository root; these tests assert byte equality against it.
// Weaker comparisons would hide wire drift: canonical JSON is compared byte
// for byte, table transcriptions by blake3 hash, and identity derivation by
// pinned hex prefixes.
import XCTest
@testable import AvatarKit

enum ConformanceTestError: Error {
    case vectorsNotFound
    case malformedManifest(String)
}

/// Walks up from this file's location until the corpus appears, the SUBC
/// ProtocolConstantsTests pattern: ports/swift lives inside the avatar
/// repository, so relative traversal finds vectors/ on any checkout without
/// hardcoding a machine path.
func vectorsRoot() throws -> URL {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    for _ in 0..<8 {
        let candidate = directory.appendingPathComponent("vectors")
        var isDirectory: ObjCBool = false
        let manifest = candidate.appendingPathComponent("manifest.json")
        if FileManager.default.fileExists(atPath: manifest.path, isDirectory: &isDirectory) {
            return candidate
        }
        directory = directory.deletingLastPathComponent()
    }
    throw ConformanceTestError.vectorsNotFound
}

struct ManifestVector {
    let name: String
    let moods: [String]
}

/// Fixtures for the creature type this package cannot build natively.
/// They gate the decoder against primitive variants the shipped type never
/// produces. Their names must match `WILD_TREE_FIXTURES` in
/// `crates/avatar-vectors/src/lib.rs`, which mints these files; the
/// inventory assertion fails on a mismatch rather than silently skipping
/// a fixture.
let wildTreeFixtureNames: Set<String> = [
    "wild-port-0005__neutral.json",
    "wild-port-0005__working.json",
    "wild-port-0005__happy.json",
    "wild-port-0005__waiting.json",
    "wild-port-0005__blocked.json",
    "wild-port-0005__sad.json",
    "wild-port-0005__sleepy.json",
    "wild-port-0031__neutral.json",
    "wild-port-0009__neutral.json",
    "wild-port-0022__neutral.json",
    "wild-port-0077__neutral.json",
    "wild-port-0043__neutral.json",
    "wild-port-0016__neutral.json",
    "wild-port-0021__neutral.json",
    "wild-port-0000__neutral.json",
    "wild-port-0001__neutral.json",
    "wild-port-0002__neutral.json",
    "wild-port-0007__neutral.json",
]

func loadManifest() throws -> (root: URL, vectors: [ManifestVector]) {
    let root = try vectorsRoot()
    let data = try Data(contentsOf: root.appendingPathComponent("manifest.json"))
    var parser = JsonParser(bytes: [UInt8](data))
    let document = try parser.parseDocument()
    guard case .object(let manifest) = document else {
        throw ConformanceTestError.malformedManifest("manifest.json is not an object")
    }
    guard case .array(let entries)? = manifest["vectors"] else {
        throw ConformanceTestError.malformedManifest("manifest.json has no vectors array")
    }
    var vectors: [ManifestVector] = []
    for entry in entries {
        guard case .object(let fields) = entry,
            case .string(let name)? = fields["name"],
            case .array(let moodValues)? = fields["moods"]
        else {
            throw ConformanceTestError.malformedManifest("vector entry is missing name or moods")
        }
        var moods: [String] = []
        for moodValue in moodValues {
            guard case .string(let mood) = moodValue else {
                throw ConformanceTestError.malformedManifest("mood entry is not a string")
            }
            moods.append(mood)
        }
        vectors.append(ManifestVector(name: name, moods: moods))
    }
    return (root, vectors)
}

/// 40-byte window around the first mismatch, hex-encoded, so a failure reads
/// like a wire diff instead of a multi-kilobyte JSON dump.
func hexWindow(_ bytes: [UInt8], center: Int) -> String {
    let start = max(0, center - 20)
    let end = min(bytes.count, center + 20)
    let hexDigits = Array("0123456789abcdef".utf8)
    var hex: [String] = []
    for byte in bytes[start ..< end] {
        hex.append(String(
            decoding: [hexDigits[Int(byte >> 4)], hexDigits[Int(byte & 0x0F)]],
            as: UTF8.self))
    }
    return "\(start)..<\(end): " + hex.joined(separator: " ")
}

func assertByteEqual(
    _ label: String, actual: [UInt8], expected: [UInt8],
    file: StaticString = #filePath, line: UInt = #line
) {
    let commonLength = min(actual.count, expected.count)
    var offset = 0
    while offset < commonLength && actual[offset] == expected[offset] {
        offset += 1
    }
    if offset == actual.count && offset == expected.count {
        return
    }
    XCTFail(
        "\(label): canonical JSON differs at byte \(offset) "
            + "(actual \(actual.count) bytes, expected \(expected.count))\n"
            + "actual   \(hexWindow(actual, center: offset))\n"
            + "expected \(hexWindow(expected, center: offset))",
        file: file, line: line)
}

final class GoldenConformanceTests: XCTestCase {
    // MARK: Boundary 1 — DNA to canonical phenotype JSON

    func testEveryManifestDnaDecodesToTheCanonicalPhenotypeBytes() throws {
        let (root, vectors) = try loadManifest()
        for vector in vectors {
            let hex = try String(
                contentsOf: root.appendingPathComponent("dna/\(vector.name).hex"),
                encoding: .utf8
            ).trimmingCharacters(in: .whitespacesAndNewlines)
            let expected = try [UInt8](Data(
                contentsOf: root.appendingPathComponent("phenotype/\(vector.name).json")))
            let actual = toCanonicalJson(decode(try Genome.fromHex(hex)))
            // Byte equality is deliberate: semantic JSON equality would hide
            // wire drift.
            assertByteEqual(vector.name, actual: actual, expected: expected)

            // Parsing the producer bytes and writing them again proves the
            // strict schema accepts the canonical corpus without using JSON
            // number arithmetic.
            assertByteEqual(
                "\(vector.name) parser round trip",
                actual: toCanonicalJson(try fromCanonicalJson(expected)),
                expected: expected)
        }
    }

    // MARK: Identity derivation pins

    func testIdentityPrefixesAreCompatibilityPins() {
        // Fixed inputs and expected genome prefixes, copied verbatim from
        // crates/avatar-genome/src/identity.rs. They pin the domain-separated
        // blake3 XOF derivation shared by every port: any drift in the prefix
        // strings, the zero separators, or the XOF fill fails here.
        let registered = deriveGenome(.registered(projectId: "project-42"))
        let fallback = deriveGenome(.pathFallback(canonicalPath: "project-42"))
        let empty = deriveGenome(.registered(projectId: ""))

        XCTAssertEqual(
            String(registered.toHex().prefix(64)),
            "497c376676efcfbabd57a41123143dbb72df26ad5f2b09c6d7331410ea13c289")
        XCTAssertEqual(
            String(fallback.toHex().prefix(64)),
            "70e579fc8f565c4563aeccb739a43eb9867e2264e6cb2a658adc8621edb63cd3")
        XCTAssertEqual(
            String(empty.toHex().prefix(64)),
            "ac844c9c9582832925ef89a4b581d9f31d23d5b69702fd408f5e1a2167d5bfcb")
    }

    // MARK: Table hash pins

    func testImportedTableHashesEqualTheRustPins() {
        // The expected hashes are the BLAKE3 digests of the canonical table
        // bytes committed in the Rust mint; matching them detects any drift
        // in the transcribed tables separately from geometry drift. Pin
        // source: crates/avatar-tables/src/lib.rs, table_hashes_are_pinned.
        let hashes = Dictionary(
            tableHashes().map { ($0.name, $0.hash) },
            uniquingKeysWith: { first, _ in first })
        XCTAssertEqual(hashes, [
            "SIN_QUARTER": "4bd5cfa26052a9ddc2daffeffcd4ddd35e27cb9b3ce6da53c1d6364b48c30a4a",
            "EASE_LINEAR": "d8e9e650133191b2ced42e6ff826e793dc382e3a6677a7cef8ad920dd09af19c",
            "EASE_EXPO": "428838f70e47b6c28fbb15fbcd94ec6af0ad04ce315eaa8c3876261d6f5784f1",
            "EASE_IN_CIRC": "9a5f6ff31360612ccc3b76fd903e62ac520e8dbf2627f8392e4ea3bf9cb28528",
            "EASE_IN_QUINT": "1d0af9d20330ac7bd15ffc75c18c853af8ef97b832ec05d45285ecefee71aeb5",
            "EASE_IN_OUT_CIRC": "af3d7c927cf3a59ecc867e4512e8aeb2a568c2ad70aabe96337c076656fa317d",
        ])
        // The expected hash covers the palette bytes in slot/color/channel
        // order, keeping the imported palette byte-identical to the committed
        // Rust table. Pin source: crates/avatar-genome/src/decode.rs,
        // palette_table_hash_is_pinned.
        XCTAssertEqual(
            paletteHash(),
            "c5f14dae266fc9d875d3d4b6727f2cff228ac6c01eb012fe5813b1e9005198f8")
    }

    // MARK: divRoundHalfAway — the Rust test table verbatim

    func testDivRoundHalfAwayMatchesTheRustTestTable() {
        // Falsifying vectors from crates/avatar-tables/src/lib.rs; the
        // negative halves are exactly the cases native signed division gets
        // wrong, so they must stay in every port's table.
        let table: [(numerator: Int64, denominator: Int64, expected: Int64)] = [
            (-3, 2, -2),
            (-1, 2, -1),
            (3, 2, 2),
            (1, 2, 1),
            (-4, 2, -2),
            (4, 2, 2),
            (0, 7, 0),
        ]
        for row in table {
            XCTAssertEqual(
                divRoundHalfAway(DoubleWidth(row.numerator), row.denominator),
                DoubleWidth(row.expected),
                "\(row.numerator) / \(row.denominator)")
        }
    }

    // MARK: Manifest consistency

    func testManifestListsEveryVectorFileWithoutStragglers() throws {
        let (root, vectors) = try loadManifest()
        let names = vectors.map(\.name)
        XCTAssertEqual(Set(names).count, names.count, "manifest names must be unique")
        let listed = Set(names)

        XCTAssertEqual(try fileStems(in: root.appendingPathComponent("dna"), "hex"), listed)
        XCTAssertEqual(
            try fileStems(in: root.appendingPathComponent("phenotype"), "json"), listed)

        // SVG bytes belong to boundary 2 (the renderer leg), but the
        // manifest's mood index must already match the corpus on disk so a
        // stray or missing file fails this consistency gate immediately.
        var expectedSvgs = Set<String>()
        for vector in vectors {
            for mood in vector.moods {
                expectedSvgs.insert("\(vector.name)__\(mood).svg")
            }
        }
        let svgDirectory = root.appendingPathComponent("svg").path
        let svgs = Set(
            try FileManager.default.contentsOfDirectory(atPath: svgDirectory)
                .filter { $0.hasSuffix(".svg") })
        XCTAssertEqual(svgs, expectedSvgs)

        let classicTrees = Set(expectedSvgs.map {
            String($0.dropLast(".svg".count)) + ".json"
        })
        let treeDirectory = root.appendingPathComponent("tree").path
        let trees = Set(
            try FileManager.default.contentsOfDirectory(atPath: treeDirectory)
                .filter { $0.hasSuffix(".json") })
        XCTAssertEqual(trees, classicTrees.union(wildTreeFixtureNames))
    }

    // MARK: Genome bit reader and hex wire form (Rust unit-test vectors)

    func testBitReaderReadsMsbFirstFieldsAcrossBytes() throws {
        // These vectors verify that the reader extracts MSB-first fields
        // correctly, including fields that cross byte boundaries. Source:
        // reads_msb_first_fields_across_bytes in
        // crates/avatar-genome/src/genome.rs.
        var bytes = [UInt8](repeating: 0, count: Genome.byteCount)
        bytes[0] = 0b1010_0000
        bytes[1] = 0b0101_1100
        let genome = try Genome.fromBytes(bytes)

        XCTAssertEqual(genome.readBits(offset: 0, width: 4), 0b1010)
        XCTAssertEqual(genome.readBits(offset: 3, width: 7), 0b000_0001)
        XCTAssertEqual(genome.readBits(offset: 4, width: 12), 0b0000_0101_1100)
    }

    func testHexadecimalRoundTripIsCanonical() throws {
        // These vectors verify that converting bytes to hexadecimal and back
        // yields the canonical lowercase spelling without changing the genome
        // bytes. Source: hexadecimal_round_trip_is_canonical in
        // crates/avatar-genome/src/genome.rs.
        var bytes = [UInt8](repeating: 0, count: Genome.byteCount)
        bytes[0] = 0xAB
        bytes[Genome.byteCount - 1] = 0xCD
        let genome = try Genome.fromBytes(bytes)

        let hex = genome.toHex()
        XCTAssertTrue(hex.hasPrefix("ab"))
        XCTAssertTrue(hex.hasSuffix("cd"))
        XCTAssertEqual(try Genome.fromHex(hex), genome)
    }

    func testHexadecimalParserRejectsIncompleteAndInvalidValues() {
        // These cases verify that incomplete hex and non-hex digits are
        // rejected instead of being parsed as partial or silently altered
        // DNA. Source:
        // hexadecimal_parser_rejects_incomplete_and_invalid_values in
        // crates/avatar-genome/src/genome.rs.
        XCTAssertThrowsError(try Genome.fromHex("00")) { error in
            XCTAssertEqual(
                error as? GenomeError, GenomeError.invalidHexLength(actual: 2))
        }

        var hex = String(repeating: "00", count: Genome.byteCount)
        let replacement = hex.index(hex.startIndex, offsetBy: 42)
        hex.replaceSubrange(
            replacement ..< hex.index(after: replacement), with: "g")
        XCTAssertThrowsError(try Genome.fromHex(hex)) { error in
            XCTAssertEqual(
                error as? GenomeError,
                GenomeError.invalidHexDigit(index: 42, byte: UInt8(ascii: "g")))
        }
    }
}

func fileStems(in directory: URL, _ fileExtension: String) throws -> Set<String> {
    let suffix = ".\(fileExtension)"
    return Set(
        try FileManager.default.contentsOfDirectory(atPath: directory.path)
            .filter { $0.hasSuffix(suffix) }
            .map { String($0.dropLast(suffix.count)) })
}
