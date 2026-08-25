/// A host-owned bounded cache of resolution-independent render trees.
///
/// Keep one instance where the host manages image memory. There is no shared
/// cache because only the host knows when a memory warning requires eviction.
public final class RenderTreeCache {
    private struct Key: Hashable {
        let genomeBytes: [UInt8]
        let type: String
        let version: UInt32
        let mood: Mood
    }

    private struct Entry {
        let tree: RenderTree
        var lastAccess: UInt64
    }

    private let capacity: Int
    private var entries: [Key: Entry] = [:]
    private var accessClock: UInt64 = 0

    /// Creates a tree cache sized for about thirty agents across seven moods.
    public init(capacity: Int = 256) {
        precondition(capacity >= 0, "render tree cache capacity must not be negative")
        self.capacity = capacity
    }

    /// Builds or returns the tree for one complete rendering identity.
    public func renderTree(
        genome: Genome,
        type: String,
        version: UInt32,
        mood: Mood
    ) throws -> RenderTree {
        let key = Key(genomeBytes: genome.bytes, type: type, version: version, mood: mood)
        if var entry = entries[key] {
            entry.lastAccess = nextAccess()
            entries[key] = entry
            return entry.tree
        }

        let tree = try AvatarKit.renderTree(
            genome: genome, type: type, version: version, mood: mood)
        guard capacity > 0 else { return tree }

        if entries.count == capacity, let leastRecent = entries.min(by: {
            $0.value.lastAccess < $1.value.lastAccess
        })?.key {
            entries.removeValue(forKey: leastRecent)
        }
        entries[key] = Entry(tree: tree, lastAccess: nextAccess())
        return tree
    }

    /// Drops every cached tree so the host can respond to memory pressure.
    public func removeAll() {
        entries.removeAll(keepingCapacity: false)
    }

    var count: Int { entries.count }

    private func nextAccess() -> UInt64 {
        accessClock &+= 1
        return accessClock
    }
}
