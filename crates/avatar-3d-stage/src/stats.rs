//! Frame statistics: the timing windows of the last frames and the
//! counters kept since the stage was created.
//!
//! Times here are wall-clock time and are used for nothing else; what a
//! frame draws takes its time only from `target_time`.

use std::collections::VecDeque;

/// How many of the latest successful frames the timing series cover.
pub const STATS_WINDOW: usize = 120;

/// The summary of a series of times, in seconds. Ranks are nearest-rank:
/// the p-th percentile of n samples is the ⌈p·n⌉-th smallest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Times {
    /// The median.
    pub p50: f64,
    /// The 99th percentile.
    pub p99: f64,
    /// The largest sample.
    pub max: f64,
}

/// A timing series over the window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Series {
    /// How many samples the window holds, at most [`STATS_WINDOW`].
    pub count: usize,
    /// Their summary; `None` when there are none.
    pub times: Option<Times>,
}

/// What [`crate::Stage::frame_stats`] returns. The timing series cover the
/// last [`STATS_WINDOW`] frames that were presented or read back; the
/// counters run from the stage's creation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct FrameStats {
    /// CPU time of each frame, from the render call's entry to its submit.
    pub cpu: Series,
    /// GPU time of each frame from timestamp queries, whose results arrive
    /// one frame late. It has no samples, and so no times, when the device
    /// has no timestamp queries ([`FrameStats::gpu_timestamps`]) or before
    /// the first result.
    pub gpu: Series,
    /// Whether the device has timestamp queries, so GPU times can come.
    pub gpu_timestamps: bool,
    /// The time each read-back frame waited for its pixels, apart from its
    /// CPU time. Frames presented to a surface add no sample.
    pub readback: Series,
    /// Frames the host's calls skipped: each successful frame whose
    /// `target_time` is more than 1.5 periods after the previous committed
    /// frame's adds round(gap / period) − 1. The period is 1 / max fps, or
    /// 1/60 s uncapped.
    pub frames_dropped: u64,
    /// Face commands held until their face set could be drawn. Always 0
    /// until face sets load on demand.
    pub held_face_commands: u64,
    /// Face switches made 1 s late, off a blink. Always 0 until drawn moods
    /// are tracked.
    pub late_face_switches: u64,
    /// The renderer's estimate of the GPU memory it holds, in bytes.
    pub gpu_memory_bytes: u64,
}

/// The last [`STATS_WINDOW`] samples of one series.
#[derive(Clone, Debug, Default)]
pub(crate) struct Window {
    samples: VecDeque<f64>,
}

impl Window {
    pub fn push(&mut self, seconds: f64) {
        if self.samples.len() == STATS_WINDOW {
            self.samples.pop_front();
        }
        self.samples.push_back(seconds);
    }

    pub fn series(&self) -> Series {
        let mut sorted: Vec<f64> = self.samples.iter().copied().collect();
        sorted.sort_by(f64::total_cmp);
        summarise(&sorted)
    }
}

/// The nearest-rank summary of samples sorted in ascending order.
pub(crate) fn summarise(sorted: &[f64]) -> Series {
    let n = sorted.len();
    if n == 0 {
        return Series::default();
    }
    // ⌈p·n⌉ in integers, so a rank never rounds the wrong way.
    let rank = |percent: usize| (percent * n).div_ceil(100);
    Series {
        count: n,
        times: Some(Times {
            p50: sorted[rank(50) - 1],
            p99: sorted[rank(99) - 1],
            max: sorted[n - 1],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_are_nearest_rank() {
        let one = summarise(&[0.5]);
        assert_eq!(one.count, 1);
        assert_eq!(
            one.times,
            Some(Times {
                p50: 0.5,
                p99: 0.5,
                max: 0.5
            })
        );
        // 1..=100: the 50th and the 99th smallest.
        let hundred: Vec<f64> = (1..=100).map(f64::from).collect();
        let s = summarise(&hundred);
        assert_eq!(
            s.times.map(|t| (t.p50, t.p99, t.max)),
            Some((50.0, 99.0, 100.0))
        );
        // 1..=120: ⌈60⌉ = 60th and ⌈118.8⌉ = 119th.
        let window: Vec<f64> = (1..=120).map(f64::from).collect();
        let s = summarise(&window);
        assert_eq!(
            s.times.map(|t| (t.p50, t.p99, t.max)),
            Some((60.0, 119.0, 120.0))
        );
        // Two samples: ⌈1⌉ = 1st and ⌈1.98⌉ = 2nd.
        let s = summarise(&[1.0, 2.0]);
        assert_eq!(s.times.map(|t| (t.p50, t.p99)), Some((1.0, 2.0)));
        assert_eq!(summarise(&[]), Series::default());
    }

    #[test]
    fn the_window_keeps_the_latest_samples() {
        let mut w = Window::default();
        assert_eq!(w.series(), Series::default());
        for i in 1..=130 {
            w.push(f64::from(i));
        }
        let s = w.series();
        assert_eq!(s.count, STATS_WINDOW);
        // 11..=130 remain: the 60th is 70, the 119th is 129.
        assert_eq!(
            s.times.map(|t| (t.p50, t.p99, t.max)),
            Some((70.0, 129.0, 130.0))
        );
        // Samples arrive in any order.
        let mut w = Window::default();
        for x in [3.0, 1.0, 2.0] {
            w.push(x);
        }
        assert_eq!(w.series().times.map(|t| (t.p50, t.max)), Some((2.0, 3.0)));
    }
}
