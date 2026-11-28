//! The clock: stage time, the fps throttle and dropped frames, and what a
//! failed frame advances.

use avatar_3d_stage::fake::Step;
use avatar_3d_stage::{BuildMode, Failure, RenderStatus, Target};

use crate::support::{harness, Harness, TICK};

fn stage_time(h: &Harness) -> f64 {
    h.last().expect("a frame was drawn").stage_time
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn backward_and_repeated_times_advance_nothing() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.render_at(10.0).unwrap();
    assert_eq!(stage_time(&h), 0.0);
    h.render_at(10.1).unwrap();
    let after = stage_time(&h);
    assert!(close(after, 0.1));
    // Uncapped, a repeated or earlier time draws but advances nothing.
    h.render_at(10.1).unwrap();
    assert_eq!(stage_time(&h), after);
    h.render_at(9.0).unwrap();
    assert_eq!(stage_time(&h), after);
    // The next step is measured from the latest time, not the earlier one.
    h.render_at(10.2).unwrap();
    assert!(close(stage_time(&h), 0.2));
    // Each 0.1 s step skips five 60 Hz frames; measured from 9.0 the last
    // step would have skipped 71.
    assert_eq!(h.stage.frame_stats().frames_dropped, 10);

    // Capped, they are not due.
    h.stage.set_max_fps(30);
    let frames = h.fake.frame_count();
    assert_eq!(h.render_at(10.2), Err(RenderStatus::NotDue));
    assert_eq!(h.render_at(5.0), Err(RenderStatus::NotDue));
    assert_eq!(h.fake.frame_count(), frames);
}

#[test]
fn a_cap_of_30_draws_every_fourth_call_at_120_hz() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.set_max_fps(30);
    for i in 0..=120 {
        let result = h.render_at(100.0 + f64::from(i) / 120.0);
        if i % 4 == 0 {
            assert_eq!(result, Ok(()), "call {i}");
        } else {
            assert_eq!(result, Err(RenderStatus::NotDue), "call {i}");
        }
    }
    assert_eq!(h.fake.frame_count(), 31);
    // One second of calls advances one second, give or take a period.
    assert!((stage_time(&h) - 1.0).abs() <= 1.0 / 30.0);
    assert_eq!(h.stage.frame_stats().frames_dropped, 0);
}

#[test]
fn a_cap_of_2_keeps_real_time() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.set_max_fps(2);
    // Calls at 60 Hz for 10 s: one in thirty draws, each a half-second
    // step, longer than the uncapped quarter-second clamp.
    for i in 0..=600 {
        let _ = h.render_at(f64::from(i) / 60.0);
    }
    assert!((stage_time(&h) - 10.0).abs() <= 0.5);
    assert!(h.fake.frames().windows(2).all(|w| {
        let step = w[1].stage_time - w[0].stage_time;
        step > 0.25 && step <= 0.75
    }));
}

#[test]
fn uncapped_calls_each_draw_and_a_long_gap_steps_a_quarter_second() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    for _ in 0..10 {
        h.frame().unwrap();
    }
    assert_eq!(h.fake.frame_count(), 10);
    assert!(close(stage_time(&h), 9.0 * TICK));
    let before = stage_time(&h);
    let t = h.t;
    h.render_at(t + 10.0).unwrap();
    assert!(close(stage_time(&h) - before, 0.25));
    // 10 s of 1/60 s periods, less the frame that came.
    assert_eq!(h.stage.frame_stats().frames_dropped, 599);
}

#[test]
fn a_failed_acquisition_commits_nothing_and_a_retry_advances_once() {
    let mut h = harness(Target::Surface, BuildMode::Inline);
    h.render_at(0.0).unwrap();
    h.render_at(TICK).unwrap();
    let before = stage_time(&h);
    let frames = h.fake.frame_count();
    h.stage.play("zz", avatar_3d::motion::Reaction::Poke);

    h.fake.fail(Step::Begin, Failure::SurfaceLost);
    let t1 = 2.0 * TICK;
    assert_eq!(h.render_at(t1), Err(RenderStatus::SurfaceLost));
    assert_eq!(h.fake.frame_count(), frames);
    assert!(h.events.is_empty(), "the queued command waits");

    h.render_at(t1).unwrap();
    assert!(close(stage_time(&h) - before, TICK));
    assert_eq!(h.events.len(), 1, "the command applies once");

    // The next step and the dropped count run from the retry.
    h.render_at(t1 + 3.0 * TICK).unwrap();
    assert!(close(stage_time(&h) - before, 4.0 * TICK));
    assert_eq!(h.stage.frame_stats().frames_dropped, 2);
}

#[test]
fn a_device_lost_after_submit_has_advanced_once_and_a_retry_advances_nothing() {
    for capped in [false, true] {
        let mut h = harness(Target::Image, BuildMode::Inline);
        if capped {
            h.stage.set_max_fps(60);
        }
        h.render_at(0.0).unwrap();
        h.render_at(TICK).unwrap();
        let before = stage_time(&h);

        h.fake.fail(Step::Finish, Failure::DeviceLost);
        let t1 = 2.0 * TICK;
        assert_eq!(h.render_at(t1), Err(RenderStatus::DeviceLost));
        let lost = h.last().unwrap();
        assert!(!lost.finished);
        assert!(close(lost.stage_time - before, TICK));

        let next = if capped {
            // A retry at the same time is not due, and does not recover.
            assert_eq!(h.render_at(t1), Err(RenderStatus::NotDue));
            assert_eq!(h.fake.recoveries(), 0);
            t1 + 3.0 * TICK
        } else {
            // Uncapped, it recovers and draws, advancing nothing.
            h.render_at(t1).unwrap();
            assert_eq!(h.fake.recoveries(), 1);
            assert_eq!(stage_time(&h), lost.stage_time);
            t1 + 3.0 * TICK
        };
        h.render_at(next).unwrap();
        assert_eq!(h.fake.recoveries(), 1);
        assert!(
            close(stage_time(&h) - lost.stage_time, 3.0 * TICK),
            "capped {capped}"
        );
        assert_eq!(h.stage.frame_stats().frames_dropped, 2, "capped {capped}");
    }
}
