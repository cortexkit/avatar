// Public rendering surface corresponding to crates/avatar-svg/src/svg.rs.

public struct RenderParams: Equatable {
    public var mood: Mood
    public var growth: Q32
    public var canvas: UInt32

    public init(mood: Mood, growth: Q32 = .one, canvas: UInt32 = 10_000) {
        self.mood = mood
        self.growth = growth
        self.canvas = canvas
    }
}

public enum RenderError: Error, Equatable, CustomStringConvertible {
    case unsupportedGrowth(Q32)
    case unsupportedCanvas(UInt32)

    public var description: String {
        switch self {
        case .unsupportedGrowth(let growth):
            return "growth morph is unimplemented; v1 renders growth = 1 only (got \(growth.value))"
        case .unsupportedCanvas(let canvas):
            return "v1 renders the pinned 10000-unit canvas only (got \(canvas))"
        }
    }
}

func validateRenderParams(_ params: RenderParams) throws {
    if params.growth != .one { throw RenderError.unsupportedGrowth(params.growth) }
    if params.canvas != 10_000 { throw RenderError.unsupportedCanvas(params.canvas) }
}

/// The stable registry type for the classic creature representation.
public let classicAvatarType = "creature.classic"

private let classicAvatarVersion: UInt32 = 2
private let classicGenomeBytes = 1_024

/// Why a stored avatar could not be rendered.
public enum DispatchError: Error, Equatable, CustomStringConvertible {
    case unsupportedType(requested: String)
    case unsupportedVersion(avatarType: String, requested: UInt32)
    case genomeLength(expected: Int, actual: Int)
    case render(RenderError)

    public var description: String {
        switch self {
        case .unsupportedType(let requested):
            return "unsupported avatar type \(requested.debugDescription)"
        case .unsupportedVersion(let avatarType, let requested):
            return "unsupported avatar version \(requested) for type \(avatarType.debugDescription)"
        case .genomeLength(let expected, let actual):
            return "genome must contain \(expected) bytes, received \(actual)"
        case .render(let error):
            return error.description
        }
    }
}

/// Builds the geometry tree for the type and generation version stored by a
/// registry row.
public func renderTree(
    genome: Genome,
    type: String,
    version: UInt32,
    mood: Mood
) throws -> RenderTree {
    try dispatchRenderTree(genomeBytes: genome.bytes, type: type, version: version, mood: mood)
}

private func dispatchRenderTree(
    genomeBytes: [UInt8],
    type: String,
    version: UInt32,
    mood: Mood
) throws -> RenderTree {
    switch type {
    case classicAvatarType:
        switch version {
        // Keep this arm when adding a newer version. Stored rows render at
        // their named generation, so a newer version is an additional arm.
        case classicAvatarVersion:
            return try renderClassicV2(genomeBytes: genomeBytes, mood: mood)
        default:
            throw DispatchError.unsupportedVersion(avatarType: type, requested: version)
        }
    default:
        throw DispatchError.unsupportedType(requested: type)
    }
}

private func renderClassicV2(genomeBytes: [UInt8], mood: Mood) throws -> RenderTree {
    guard genomeBytes.count == classicGenomeBytes else {
        throw DispatchError.genomeLength(expected: classicGenomeBytes, actual: genomeBytes.count)
    }

    let genome = Genome(trustedBytes: genomeBytes)
    do {
        return try buildRenderTree(phenotype: decode(genome), params: RenderParams(mood: mood))
    } catch let error as RenderError {
        throw DispatchError.render(error)
    }
}

/// Returns the geometry tree used by every renderer.
public func renderTree(phenotype: Phenotype, params: RenderParams) throws -> RenderTree {
    try buildRenderTree(phenotype: phenotype, params: params)
}

/// Builds and serializes the complete avatar.
public func renderSvg(phenotype: Phenotype, params: RenderParams) throws -> [UInt8] {
    serializeSvg(try buildRenderTree(phenotype: phenotype, params: params))
}

/// Builds and serializes the background and body without face primitives.
public func renderBodySvg(phenotype: Phenotype, params: RenderParams) throws -> [UInt8] {
    serializeSvg(try buildTree(phenotype, params, includeFace: false))
}
