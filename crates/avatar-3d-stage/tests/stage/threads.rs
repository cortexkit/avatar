//! Threads: the stage is shared, commands and reads never wait on a held
//! frame or build, and destroy waits only for the running build.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use avatar_3d::motion::{Reaction, StatusLoop};
use avatar_3d_stage::fake::FakeRenderer;
use avatar_3d_stage::{BuildMode, Builder, Stage, StageOptions, StageView, Target};
use avatar_small_robots::Mood;

use crate::support::{harness, spec, TestBuilder, Unblock, WAIT};

#[test]
fn the_stage_is_send_and_sync() {
    fn shared<T: Send + Sync>() {}
    shared::<Stage<FakeRenderer>>();
}

/// A named command or read on a shared stage.
type Call<'a> = (&'static str, Box<dyn Fn() + Sync + 'a>);

/// Each command and read, made while a frame is held at its read-back and
/// a worker is held mid-build. Every call must come back, and quickly.
#[test]
fn commands_and_reads_return_while_a_frame_and_a_build_are_held() {
    let mut h = harness(Target::Image, BuildMode::Workers);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.until_drawn(&["a"]);
    h.builder.hold("b");
    h.stage.set_creatures(&[spec("a", 1), spec("b", 2)]);
    h.frame().unwrap();
    h.builder.wait_for("b's build", |calls| calls.len() == 2);

    h.fake.hold_finish();
    let stage = &h.stage;
    let fake = &h.fake;
    let builder = &h.builder;
    let t = h.t;
    thread::scope(|s| {
        let render = s.spawn(move || stage.render_to_image(t + 1.0).map(|_| ()));
        let _unblock = Unblock(fake);
        assert!(fake.wait_held(WAIT));
        let calls: Vec<Call<'_>> = vec![
            (
                "set_creatures",
                Box::new(|| stage.set_creatures(&[spec("a", 1), spec("b", 2)])),
            ),
            (
                "set_state",
                Box::new(|| stage.set_state("a", Mood::Happy, Some(StatusLoop::Working))),
            ),
            ("play", Box::new(|| stage.play("a", Reaction::Hello))),
            (
                "set_view",
                Box::new(|| stage.set_view(StageView::Stage { key: "a".into() })),
            ),
            ("resize", Box::new(|| stage.resize(32, 32, 2.0).unwrap())),
            ("set_max_fps", Box::new(|| stage.set_max_fps(30))),
            ("pause", Box::new(|| stage.pause())),
            ("resume", Box::new(|| stage.resume())),
            ("events", Box::new(|| drop(stage.events()))),
            (
                "frame_stats",
                Box::new(|| {
                    let _ = stage.frame_stats();
                }),
            ),
        ];
        for (name, call) in &calls {
            // On a thread of its own, so a call that blocked would show as
            // a timeout here rather than hang the test.
            let returned = AtomicBool::new(false);
            let start = Instant::now();
            thread::scope(|inner| {
                inner.spawn(|| {
                    call();
                    returned.store(true, Ordering::Release);
                });
                while !returned.load(Ordering::Acquire) {
                    if start.elapsed() > Duration::from_secs(5) {
                        // Let the call finish, so the scope can end.
                        fake.release_finish();
                        builder.release_all();
                        panic!("{name} waited on the held frame or build");
                    }
                    thread::sleep(Duration::from_micros(100));
                }
            });
        }
        fake.release_finish();
        assert_eq!(render.join().unwrap(), Ok(()));
    });
    h.builder.release("b");
}

#[test]
fn destroy_cancels_queued_builds_and_waits_for_the_running_one() {
    let (renderer, _fake) = FakeRenderer::new(Target::Image, true);
    let builder = Arc::new(TestBuilder::default());
    let options = StageOptions {
        build: BuildMode::Workers,
        build_threads: 1,
    };
    let stage = Stage::with_builder(
        renderer,
        64,
        48,
        1.0,
        options,
        Arc::clone(&builder) as Arc<dyn Builder>,
    )
    .unwrap();
    let specs: Vec<_> = (0..33).map(|i| spec(&format!("k{i}"), i)).collect();
    builder.hold("k0");
    stage.set_creatures(&specs);
    stage.render_to_image(1.0).unwrap();
    builder.wait_for("the first build", |calls| calls.len() == 1);

    let destroyed = AtomicBool::new(false);
    thread::scope(|s| {
        let destroy = s.spawn(|| {
            drop(stage);
            destroyed.store(true, Ordering::Release);
        });
        // Releases the build however the assertion below ends.
        struct Release<'a>(&'a TestBuilder);
        impl Drop for Release<'_> {
            fn drop(&mut self) {
                self.0.release_all();
            }
        }
        let _release = Release(&builder);
        thread::sleep(Duration::from_millis(200));
        assert!(
            !destroyed.load(Ordering::Acquire),
            "destroy waits for the running build"
        );
        builder.release("k0");
        destroy.join().unwrap();
    });
    assert!(destroyed.load(Ordering::Acquire));
    let calls = builder.calls();
    assert_eq!(calls.len(), 1, "no queued build ran");
    assert!(calls[0].finished);
}
