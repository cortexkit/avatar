#if canImport(AvatarCore)
import AvatarCore

/// Failures from the embedded Rust generator, kept distinct so callers can
/// refuse unsupported stored avatars instead of rendering a default creature.
public enum CoreRenderError: Error, Equatable {
    case unsupportedType
    case unsupportedVersion
    case genomeLength
    case unknownMood
    case invalidArgument
    case renderFailed
    case panic
    case invalidResponse
    case unknownStatus(Int32)
}

/// Renders with the bundled Rust core, not the native Swift classic builder.
/// Each call owns its inputs and result; no shared state or main thread is used.
public func renderTreeViaCore(
    genome: Genome,
    type: String,
    version: UInt32,
    mood: Mood
) throws -> RenderTree {
    try renderTreeViaCoreBytes(
        genomeBytes: genome.bytes, type: type, version: version, moodName: mood.rawValue)
}

/// Renders type-specific genome bytes without imposing the classic Genome size.
/// The bundled Rust core validates the type, version, and required byte count.
public func renderTreeViaCore(
    genomeBytes: [UInt8],
    type: String,
    version: UInt32,
    mood: Mood
) throws -> RenderTree {
    try renderTreeViaCoreBytes(
        genomeBytes: genomeBytes, type: type, version: version, moodName: mood.rawValue)
}

func checkCoreStatus(_ status: Int32) throws {
    switch status {
    case Int32(AVATAR_STATUS_OK): return
    case Int32(AVATAR_STATUS_UNKNOWN_TYPE): throw CoreRenderError.unsupportedType
    case Int32(AVATAR_STATUS_UNKNOWN_VERSION): throw CoreRenderError.unsupportedVersion
    case Int32(AVATAR_STATUS_GENOME_LENGTH): throw CoreRenderError.genomeLength
    case Int32(AVATAR_STATUS_UNKNOWN_MOOD): throw CoreRenderError.unknownMood
    case Int32(AVATAR_STATUS_INVALID_ARGUMENT): throw CoreRenderError.invalidArgument
    case Int32(AVATAR_STATUS_RENDER_ERROR): throw CoreRenderError.renderFailed
    case Int32(AVATAR_STATUS_PANIC): throw CoreRenderError.panic
    default: throw CoreRenderError.unknownStatus(status)
    }
}

func renderTreeViaCoreBytes(
    genomeBytes: [UInt8], type: String, version: UInt32, moodName: String
) throws -> RenderTree {
    // C strings cannot represent embedded NULs. Reject rather than silently
    // truncating a stored type or mood into a supported value.
    guard !type.utf8.contains(0) else { throw CoreRenderError.unsupportedType }
    guard !moodName.utf8.contains(0) else { throw CoreRenderError.unknownMood }
    var json: UnsafeMutablePointer<CChar>?
    defer { avatar_string_free(json) }
    let status = genomeBytes.withUnsafeBufferPointer { bytes in
        type.withCString { typeCString in
            moodName.withCString { moodCString in
                avatar_render_tree_json(
                    bytes.baseAddress, bytes.count, typeCString, version, moodCString, &json)
            }
        }
    }
    try checkCoreStatus(status)
    guard let json else { throw CoreRenderError.invalidResponse }
    return try decodeRenderTreeJson(Array(String(cString: json).utf8))
}
#endif
