//! The event queue under concurrent drains, and its overflow count.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;

use avatar_3d::motion::Reaction;
use avatar_3d_stage::{BuildMode, CommandKind, Event, Target, EVENT_CAPACITY};

use crate::support::harness;

#[test]
fn four_threads_get_every_event_exactly_once_or_count_it() {
    let h = harness(Target::Image, BuildMode::Inline);
    const COMMANDS: usize = 20_000;
    let delivered = Mutex::new(Vec::new());
    let overflowed = Mutex::new(0_u64);
    let done = AtomicBool::new(false);
    let stage = &h.stage;
    thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                let mut keys = Vec::new();
                let mut dropped = 0;
                loop {
                    let finished = done.load(Ordering::Acquire);
                    let events = stage.events();
                    if events.is_empty() {
                        thread::yield_now();
                    }
                    for event in events {
                        match event {
                            Event::CommandDropped {
                                key,
                                command: CommandKind::Play,
                                ..
                            } => keys.push(key),
                            Event::Overflow { dropped: n } => dropped += n,
                            other => panic!("unexpected {other:?}"),
                        }
                    }
                    if finished {
                        break;
                    }
                }
                delivered.lock().unwrap().extend(keys);
                *overflowed.lock().unwrap() += dropped;
            });
        }
        // Stops the drains however the sending below ends.
        struct Done<'a>(&'a AtomicBool);
        impl Drop for Done<'_> {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let _done = Done(&done);
        // Batches of every size up to past the queue's capacity, one frame
        // each, so some overflow while the drains race.
        let mut sent = 0;
        let mut t = 0.0;
        let mut batch = 1;
        while sent < COMMANDS {
            for _ in 0..batch.min(COMMANDS - sent) {
                stage.play(&format!("k{sent}"), Reaction::Poke);
                sent += 1;
            }
            t += 1.0 / 60.0;
            stage.render_to_image(t).unwrap();
            batch = batch % (EVENT_CAPACITY + 64) + 37;
        }
    });
    let delivered = delivered.into_inner().unwrap();
    let overflowed = overflowed.into_inner().unwrap();
    let unique: HashSet<&String> = delivered.iter().collect();
    assert_eq!(unique.len(), delivered.len(), "an event came twice");
    assert_eq!(delivered.len() as u64 + overflowed, COMMANDS as u64);
    assert!(h.stage.events().is_empty());
}

#[test]
fn a_full_queue_reports_how_many_it_dropped_first() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    for i in 0..EVENT_CAPACITY + 44 {
        h.stage.play(&format!("k{i}"), Reaction::Poke);
    }
    h.stage.render_to_image(1.0).unwrap();
    let events = h.stage.events();
    assert_eq!(events.len(), 1 + EVENT_CAPACITY);
    assert_eq!(events[0], Event::Overflow { dropped: 44 });
    assert!(matches!(&events[1], Event::CommandDropped { key, .. } if key == "k44"));
    h.frame().unwrap();
    assert!(h.events.is_empty());
}
