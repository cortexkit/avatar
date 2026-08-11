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
