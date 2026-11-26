/// A finished square creature image drawn by the embedded Rust core.
///
/// It is not called `CoreImage`, which would shadow Apple's module of that
/// name in an app that imports both.
public struct AvatarImage: Equatable, Sendable {
    /// How the colour channels of `pixels` relate to the alpha channel.
    public enum Alpha: Equatable, Sendable {
        /// Each colour channel is already multiplied by alpha, as the core's
        /// rasterizer produces it.
        case premultiplied
        /// Colour channels do not depend on alpha. A fully transparent pixel
        /// is `[0, 0, 0, 0]`; any other pixel's colour channel is
        /// `(c × 255 + a / 2) / a` in integers, where `c` is the
        /// premultiplied channel and `a` the alpha.
        case straight
    }

    /// The edge length in pixels, on both axes.
    public let sizePx: UInt32
    /// Whether `pixels` is premultiplied or straight.
    public let alpha: Alpha
    /// Exactly `sizePx × sizePx × 4` bytes of sRGB RGBA8: R, G, B, A for
    /// each pixel, rows top first, each row left to right, with no padding.
    public let pixels: [UInt8]
}

/// How the embedded core draws an `AvatarImage`. The default keeps the
/// ground shadow and returns premultiplied pixels.
public struct AvatarImageOptions: Equatable, Sendable {
    /// Whether the primitives that draw the creature's ground shadow are kept.
    public enum GroundShadow: Equatable, Sendable {
        /// Draw every primitive, the ground shadow included.
        case keep
        /// Leave the creature's ground shadow out, for a host that draws its
        /// own. Only primitives the generator marks as the ground shadow are
        /// left out.
        case drop
    }

    public var groundShadow: GroundShadow
    public var alpha: AvatarImage.Alpha

    public init(groundShadow: GroundShadow = .keep, alpha: AvatarImage.Alpha = .premultiplied) {
        self.groundShadow = groundShadow
        self.alpha = alpha
    }
}
