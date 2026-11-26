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

/// Renders every creature type with the bundled Rust core.
/// The bundled Rust core validates the type, version, and required byte count.
/// Each call owns its inputs and result; no shared state or main thread is used.
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

/// Draws a stored avatar as a finished square image with the bundled Rust
/// core, `sizePx` pixels on each edge.
/// The core checks the type, version and byte count as `renderTreeViaCore`
/// does, and only then the size: a size outside 1...2,048 throws
/// `CoreRenderError.invalidArgument`. The pixels are copied into the returned
/// image, which Swift owns, and the core's allocation is freed before this
/// returns. Each call owns its inputs and result; no shared state or main
/// thread is used.
public func renderImageViaCore(
    genomeBytes: [UInt8],
    type: String,
    version: UInt32,
    mood: Mood,
    sizePx: UInt32,
    options: AvatarImageOptions = AvatarImageOptions()
) throws -> AvatarImage {
    try renderImageViaCoreBytes(
        genomeBytes: genomeBytes, type: type, version: version, moodName: mood.rawValue,
        sizePx: sizePx, options: options)
}

func renderImageViaCoreBytes(
    genomeBytes: [UInt8], type: String, version: UInt32, moodName: String,
    sizePx: UInt32, options: AvatarImageOptions
) throws -> AvatarImage {
    // C strings cannot represent embedded NULs. Reject rather than silently
    // truncating a stored type or mood into a supported value.
    guard !type.utf8.contains(0) else { throw CoreRenderError.unsupportedType }
    guard !moodName.utf8.contains(0) else { throw CoreRenderError.unknownMood }
    let dropGroundShadow: UInt32
    switch options.groundShadow {
    case .keep: dropGroundShadow = 0
    case .drop: dropGroundShadow = 1
    }
    let straightAlpha: UInt32
    switch options.alpha {
    case .premultiplied: straightAlpha = 0
    case .straight: straightAlpha = 1
    }
    var pixels: UnsafeMutablePointer<UInt8>?
    var length = 0
    // Free with exactly the length the core returned, also when the result
    // is refused below. The core leaves both null and 0 on failure.
    defer { avatar_image_free(pixels, length) }
    let status = genomeBytes.withUnsafeBufferPointer { bytes in
        type.withCString { typeCString in
            moodName.withCString { moodCString in
                avatar_render_image(
                    bytes.baseAddress, bytes.count, typeCString, version, moodCString,
                    sizePx, dropGroundShadow, straightAlpha, &pixels, &length)
            }
        }
    }
    try checkCoreStatus(status)
    // A successful image is exactly sizePx × sizePx RGBA pixels. The
    // arithmetic reports overflow rather than trapping, so a core that
    // claims success for an impossible size fails closed.
    let (area, areaOverflow) = Int(sizePx).multipliedReportingOverflow(by: Int(sizePx))
    let (expectedLength, lengthOverflow) = area.multipliedReportingOverflow(by: 4)
    guard let pixels, !areaOverflow, !lengthOverflow, length == expectedLength else {
        throw CoreRenderError.invalidResponse
    }
    return AvatarImage(
        sizePx: sizePx, alpha: options.alpha,
        pixels: Array(UnsafeBufferPointer(start: pixels, count: length)))
}
#endif
