// Transcribes the pinned numeric tables from the canonical Rust implementation
// into the Swift port's generated sources. Run from ports/swift:
//
//     swift Scripts/import-tables.swift
//
// The output is never reminted: GoldenConformanceTests pins these bytes with
// the same blake3 hashes the Rust tests carry, so any transcription drift
// fails loudly and separately from geometry drift.
//
// The generated basenames are GeneratedTables.swift / GeneratedPalette.swift
// rather than Tables.swift / Palette.swift because SwiftPM names object files
// by source basename within a target: a second Tables.swift would collide
// with the hand-written API file of the same name.
import Foundation

let fileManager = FileManager.default

/// Locates the avatar repository root by walking up from this script until the
/// canonical crate layout appears, so the script works from any working
/// directory and keeps working if the port directory moves.
func repoRoot() -> URL {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    for _ in 0..<8 {
        let marker = directory
            .appendingPathComponent("crates")
            .appendingPathComponent("avatar-tables")
            .appendingPathComponent("src")
            .appendingPathComponent("generated")
            .appendingPathComponent("tables.rs")
        if fileManager.fileExists(atPath: marker.path) {
            return directory
        }
        directory = directory.deletingLastPathComponent()
    }
    fatalError("could not locate the avatar repository root above \(#filePath)")
}

func readSource(_ path: URL) -> String {
    do {
        return try String(contentsOf: path, encoding: .utf8)
    } catch {
        fatalError("cannot read \(path.path): \(error)")
    }
}

/// Extracts one committed Rust table constant's integer values, mirroring the
/// parsing the TypeScript port's import script uses: find the declaration,
/// take everything up to the closing `];`, and read each decimal run. The
/// generated Rust files contain no other digit runs inside a table body, so a
/// count mismatch means the declaration moved and must be inspected by hand.
func valuesFor(source: String, name: String) -> [UInt64] {
    guard let declaration = source.range(of: "pub const \(name):") else {
        fatalError("could not find Rust constant \(name)")
    }
    guard let open = source.range(of: "= [", range: declaration.upperBound ..< source.endIndex) else {
        fatalError("could not find the body of Rust constant \(name)")
    }
    guard let close = source.range(of: "];", range: open.upperBound ..< source.endIndex) else {
        fatalError("could not find the end of Rust constant \(name)")
    }

    var values: [UInt64] = []
    var current = Substring.Index?.none
    let body = source[open.upperBound ..< close.lowerBound]
    for index in body.indices {
        let character = body[index]
        if character.isNumber {
            if current == nil {
                current = index
            }
        } else if let start = current {
            values.append(UInt64(body[start ..< index])!)
            current = nil
        }
    }
    if let start = current {
        values.append(UInt64(body[start ..< body.endIndex])!)
    }
    return values
}

func formatTable(_ values: [UInt64], perLine: Int) -> String {
    var lines: [String] = []
    var index = 0
    while index < values.count {
        let chunkEnd = min(index + perLine, values.count)
        let chunk = (index ..< chunkEnd).map { position in
            var text = String(values[position])
            if position < values.count - 1 {
                text += ","
            }
            return text
        }
        lines.append("        " + chunk.joined(separator: " "))
        index = chunkEnd
    }
    return lines.joined(separator: "\n")
}

let root = repoRoot()
let portRoot = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()
    .deletingLastPathComponent()
let tableSource = readSource(
    root.appendingPathComponent("crates/avatar-tables/src/generated/tables.rs"))
let paletteSource = readSource(
    root.appendingPathComponent("crates/avatar-genome/src/generated/palette.rs"))

let tableDefinitions: [(name: String, count: Int, swiftType: String, perLine: Int)] = [
    ("SIN_QUARTER", 4096, "[UInt32]", 12),
    ("EASE_LINEAR", 256, "[UInt64]", 8),
    ("EASE_EXPO", 256, "[UInt64]", 8),
    ("EASE_IN_CIRC", 256, "[UInt64]", 8),
    ("EASE_IN_QUINT", 256, "[UInt64]", 8),
    ("EASE_IN_OUT_CIRC", 256, "[UInt64]", 8),
]

var tableDeclarations: [String] = []
for definition in tableDefinitions {
    let values = valuesFor(source: tableSource, name: definition.name)
    guard values.count == definition.count else {
        fatalError("\(definition.name) has \(values.count) values, expected \(definition.count)")
    }
    tableDeclarations.append(
        """
            static let \(definition.name): \(definition.swiftType) = [
        \(formatTable(values, perLine: definition.perLine))
            ]
        """)
}

let tablesOutput = """
    // @generated by Scripts/import-tables.swift — do not edit.
    //
    // Transcribed from crates/avatar-tables/src/generated/tables.rs, the committed
    // Rust mint. These values are spec data (design note 001 §2): runtime math
    // must consume this shared table so every port evaluates the same bytes, and
    // GoldenConformanceTests pins them with the same blake3 hashes the Rust crate
    // pins.

    enum GeneratedTables {
    \(tableDeclarations.joined(separator: "\n\n"))
    }

    """

let palette = valuesFor(source: paletteSource, name: "PALETTE_BASE")
let expectedPaletteValues = 2048 * 6 * 3
guard palette.count == expectedPaletteValues else {
    fatalError("PALETTE_BASE has \(palette.count) channel values, expected \(expectedPaletteValues)")
}

let paletteOutput = """
    // @generated by Scripts/import-tables.swift — do not edit.
    //
    // Transcribed from crates/avatar-genome/src/generated/palette.rs, the committed
    // base palette contract shared by every port. Stored flat in slot-major order:
    // the Rust PALETTE_BASE[slot][color][channel] lives at
    // PALETTE_BASE[(slot * 6 + color) * 3 + channel]. GoldenConformanceTests pins
    // these bytes with the same blake3 hash the Rust crate pins.

    enum GeneratedPalette {
        static let PALETTE_BASE: [UInt8] = [
    \(formatTable(palette, perLine: 24))
        ]
    }

    """

let generatedDirectory = portRoot
    .appendingPathComponent("Sources")
    .appendingPathComponent("AvatarKit")
    .appendingPathComponent("Generated")
do {
    try fileManager.createDirectory(at: generatedDirectory, withIntermediateDirectories: true)
    try tablesOutput.write(
        to: generatedDirectory.appendingPathComponent("GeneratedTables.swift"),
        atomically: true, encoding: .utf8)
    try paletteOutput.write(
        to: generatedDirectory.appendingPathComponent("GeneratedPalette.swift"),
        atomically: true, encoding: .utf8)
} catch {
    fatalError("cannot write generated tables: \(error)")
}
print("wrote \(generatedDirectory.appendingPathComponent("GeneratedTables.swift").path)")
print("wrote \(generatedDirectory.appendingPathComponent("GeneratedPalette.swift").path)")
