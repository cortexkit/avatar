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
