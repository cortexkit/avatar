//! Events the stage reports to the host, and the bounded queue they wait
//! in until a host thread drains them.

use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_queue::ArrayQueue;

use crate::spec::SkipReason;

/// How many events wait for [`crate::Stage::events`] at most. When the
/// queue is full the oldest event is dropped to make room, and counted.
pub const EVENT_CAPACITY: usize = 256;

/// A command that was dropped rather than applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandKind {
    /// [`crate::Stage::set_state`].
    SetState,
    /// [`crate::Stage::play`].
    Play,
}

/// Something the host should know, delivered once by
/// [`crate::Stage::events`].
///
/// `time` is the `target_time` of the frame the event belongs to, on the
/// host's clock: the frame that first showed the robot for [`Event::Ready`],
/// and the frame whose commit applied the command for the others. Commands
/// and events therefore share the one clock the host renders with.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Event {
    /// A newly built robot for `key` was shown for the first time, in a
    /// frame that was presented or read back: after its first build, after
    /// its spec changed, after [`crate::Stage::resume`] and after the stage
    /// recovered from a lost device.
    Ready {
        /// The creature's key.
        key: String,
        /// The `target_time` of the frame that first showed it.
        time: f64,
    },
    /// A row of [`crate::Stage::set_creatures`] was skipped and nothing is
    /// drawn for it.
    Skipped {
        /// The row's key.
        key: String,
        /// Why.
        reason: SkipReason,
        /// The `target_time` of the frame that applied the batch.
        time: f64,
    },
    /// A command was dropped: its key is not in the cast, or, for
    /// [`crate::Stage::play`], its robot is not built yet.
    CommandDropped {
        /// The command's key.
        key: String,
        /// Which command.
        command: CommandKind,
        /// The `target_time` of the frame that would have applied it.
        time: f64,
    },
    /// This many events were dropped because the queue was full, oldest
    /// first. It comes first in the batch that reports them.
    Overflow {
        /// How many.
        dropped: u64,
    },
}

/// A bounded lock-free queue that many threads may drain at once: each
/// event goes to exactly one caller, or, when it is pushed out by a newer
/// one, is counted in an [`Event::Overflow`].
pub(crate) struct EventQueue {
    queue: ArrayQueue<Event>,
    dropped: AtomicU64,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            queue: ArrayQueue::new(EVENT_CAPACITY),
            dropped: AtomicU64::new(0),
        }
    }

    /// Adds an event, dropping and counting the oldest when full.
    pub fn push(&self, event: Event) {
        if self.queue.force_push(event).is_some() {
            self.dropped.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// Takes every event waiting, after an [`Event::Overflow`] when some
    /// were dropped since the last caller took the count.
    pub fn drain(&self) -> Vec<Event> {
        let mut out = Vec::new();
        let dropped = self.dropped.swap(0, Ordering::AcqRel);
        if dropped > 0 {
            out.push(Event::Overflow { dropped });
        }
        while let Some(event) = self.queue.pop() {
            out.push(event);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(i: usize) -> Event {
        Event::Ready {
            key: i.to_string(),
            time: 0.0,
        }
    }

    #[test]
    fn a_full_queue_drops_the_oldest_and_reports_the_count_first() {
        let q = EventQueue::new();
        for i in 0..EVENT_CAPACITY + 3 {
            q.push(ready(i));
        }
        let events = q.drain();
        assert_eq!(events[0], Event::Overflow { dropped: 3 });
        assert_eq!(events[1], ready(3));
        assert_eq!(events.len(), 1 + EVENT_CAPACITY);
        assert_eq!(events.last(), Some(&ready(EVENT_CAPACITY + 2)));
        assert!(q.drain().is_empty());
    }
}
