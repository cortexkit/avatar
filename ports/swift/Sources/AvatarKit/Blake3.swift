// BLAKE3 access for the port, over the vendored portable-only C reference
// implementation (Sources/CBlake3). The Rust crates hash tables and derive
// genomes with blake3; the Swift port must produce the identical bytes, so
// both operations go through the same upstream code rather than a re-derivation.
import CBlake3

enum Blake3 {
    /// The standard 32-byte BLAKE3 digest of `input`.
    static func hash(_ input: [UInt8]) -> [UInt8] {
        var hasher = blake3_hasher()
        blake3_hasher_init(&hasher)
        update(&hasher, with: input)
        var output = [UInt8](repeating: 0, count: 32)
        output.withUnsafeMutableBufferPointer { buffer in
            blake3_hasher_finalize(&hasher, buffer.baseAddress, buffer.count)
        }
        return output
    }

    /// Fills `length` bytes from the root XOF stream, matching the Rust
    /// crate's `Hasher::finalize_xof().fill(...)`: upstream 1.8.6 names this
    /// operation `blake3_hasher_finalize_seek` with seek 0.
    static func xof(_ input: [UInt8], length: Int) -> [UInt8] {
        var hasher = blake3_hasher()
        blake3_hasher_init(&hasher)
        update(&hasher, with: input)
        var output = [UInt8](repeating: 0, count: length)
        output.withUnsafeMutableBufferPointer { buffer in
            blake3_hasher_finalize_seek(&hasher, 0, buffer.baseAddress, buffer.count)
        }
        return output
    }

    private static func update(_ hasher: inout blake3_hasher, with input: [UInt8]) {
        input.withUnsafeBufferPointer { buffer in
            if let base = buffer.baseAddress {
                blake3_hasher_update(&hasher, base, buffer.count)
            }
        }
    }
}

/// Lowercase hexadecimal, the genome and hash-pin wire spelling.
func hexString(_ bytes: [UInt8]) -> String {
    let hexDigits = Array("0123456789abcdef".utf8)
    var utf8 = [UInt8]()
    utf8.reserveCapacity(bytes.count * 2)
    for byte in bytes {
        utf8.append(hexDigits[Int(byte >> 4)])
        utf8.append(hexDigits[Int(byte & 0x0F)])
    }
    return String(decoding: utf8, as: UTF8.self)
}
