//! Frame statistics through the stage: the windows' counts and ranks, GPU
//! times a frame late, and what failed frames leave out.

use avatar_3d_stage::fake::Step;
use avatar_3d_stage::{BuildMode, Failure, RenderError, Series, Target, Times, STATS_WINDOW};

use crate::support::{harness, harness_with, spec, TICK};

#[test]
fn gpu_times_arrive_a_frame_late_and_the_window_rolls_over() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    let scripted: Vec<f64> = (1..=130).map(|ms| f64::from(ms) / 1000.0).collect();
    h.fake.script_gpu_times(&scripted);

    h.frame().unwrap();
    let stats = h.stage.frame_stats();
    assert_eq!(stats.cpu.count, 1);
    assert!(stats.cpu.times.is_some());
    assert_eq!(stats.readback.count, 1);
    assert!(stats.gpu_timestamps);
    assert_eq!(stats.gpu, Series::default(), "no GPU result yet");

    h.frame().unwrap();
    let stats = h.stage.frame_stats();
    assert_eq!(stats.gpu.count, 1);
    assert_eq!(
        stats.gpu.times,
        Some(Times {
            p50: 0.001,
            p99: 0.001,
            max: 0.001
        })
    );

    for _ in 2..130 {
        h.frame().unwrap();
    }
    let stats = h.stage.frame_stats();
    assert_eq!(stats.cpu.count, STATS_WINDOW);
    assert_eq!(stats.readback.count, STATS_WINDOW);
    // 129 results came, 1 ms to 129 ms; the window holds the last 120,
    // 10 ms to 129 ms: the 60th and the 119th of them, and the largest.
    assert_eq!(stats.gpu.count, STATS_WINDOW);
    assert_eq!(
        stats.gpu.times,
        Some(Times {
            p50: 0.069,
            p99: 0.128,
            max: 0.129
        })
    );
    assert_eq!((stats.held_face_commands, stats.late_face_switches), (0, 0));
}

#[test]
fn without_timestamps_gpu_times_never_come() {
    let mut h = harness_with(Target::Surface, BuildMode::Inline, 1, false);
    for _ in 0..5 {
        h.frame().unwrap();
    }
    let stats = h.stage.frame_stats();
    assert!(!stats.gpu_timestamps);
    assert_eq!(stats.gpu, Series::default());
    assert_eq!(stats.cpu.count, 5);
    // Presented frames are not read back.
    assert_eq!(stats.readback, Series::default());
}

#[test]
fn failed_frames_add_no_samples_and_counters_accumulate() {
    let mut h = harness(Target::Image, BuildMode::Inline);
    h.stage.set_creatures(&[spec("a", 1)]);
    h.frame().unwrap();
    h.fake
        .fail(Step::Finish, Failure::Render(RenderError::new("lost")));
    assert!(h.frame().is_err());
    assert_eq!(h.stage.frame_stats().cpu.count, 1);

    // Two gaps of four periods: three dropped frames each.
    let t = h.t;
    h.render_at(t + 4.0 * TICK).unwrap();
    h.render_at(t + 8.0 * TICK).unwrap();
    let stats = h.stage.frame_stats();
    assert_eq!(stats.frames_dropped, 6);
    assert_eq!(stats.cpu.count, 3);
    assert_eq!(stats.gpu_memory_bytes, h.fake.memory_bytes());
    assert!(stats.gpu_memory_bytes > 0);
}
