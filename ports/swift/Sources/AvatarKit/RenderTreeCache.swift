import Foundation

/// A host-owned bounded cache of resolution-independent render trees.
///
/// Keep one instance where the host manages image memory. There is no shared
/// cache because only the host knows when a memory warning requires eviction.
///
/// Safe to call from any thread. A list view prefetching avatars off the main
/// queue is the expected usage, so the alternative — documenting single-thread
/// confinement — would be guidance the caller has to remember instead of a
/// property the type guarantees.
///
/// The lock covers map access only, never a render: holding it across a build
/// would serialise concurrent renders of different avatars for no benefit. Two
/// threads racing on the same cold key therefore both build the tree, which is
/// wasted work but never wrong, because rendering is deterministic and both
/// produce identical trees.
public final class RenderTreeCache: @unchecked Sendable {
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
    private let lock = NSLock()
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
        lock.lock()
        if var entry = entries[key] {
            entry.lastAccess = nextAccess()
            entries[key] = entry
            lock.unlock()
            return entry.tree
        }
        lock.unlock()

        let tree = try AvatarKit.renderTree(
            genome: genome, type: type, version: version, mood: mood)
        guard capacity > 0 else { return tree }

        lock.lock()
        defer { lock.unlock() }
        // Re-check: another thread may have inserted this key while the tree
        // was building. Overwriting is harmless — the trees are identical —
        // but skipping the eviction pass keeps the count honest.
        if entries[key] == nil, entries.count >= capacity,
            let leastRecent = entries.min(by: {
                $0.value.lastAccess < $1.value.lastAccess
            })?.key
        {
            entries.removeValue(forKey: leastRecent)
        }
        entries[key] = Entry(tree: tree, lastAccess: nextAccess())
        return tree
    }

    /// Drops every cached tree so the host can respond to memory pressure.
    public func removeAll() {
        lock.lock()
        defer { lock.unlock() }
        entries.removeAll(keepingCapacity: false)
    }

    var count: Int {
        lock.lock()
        defer { lock.unlock() }
        return entries.count
    }

    /// Caller must hold `lock`.
    private func nextAccess() -> UInt64 {
        accessClock &+= 1
        return accessClock
    }
}
