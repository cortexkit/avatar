/// Stable registry type names passed to the embedded Rust core.
public let classicAvatarType = "creature.classic"
public let wildAvatarType = "creature.wild"

/// Mood names accepted by the embedded Rust core for every creature type.
public enum Mood: String, CaseIterable, Equatable, Sendable {
    case neutral
    case working
    case happy
    case waiting
    case blocked
    case sad
    case sleepy
}
