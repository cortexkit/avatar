//! The stage clock: stage time, which only committed frames advance, and
//! the fps throttle, both measured from the last committed frame.

/// The largest step one frame takes when uncapped, in seconds, so a long
/// gap between host calls does not jump the robots ahead.
pub const MAX_STEP: f64 = 0.25;

/// How early a capped call may come and still be due, in seconds, so a
/// display link's small jitter does not skip a frame.
pub const DUE_SLACK: f64 = 0.002;

/// The period dropped frames are counted in when uncapped.
pub const UNCAPPED_PERIOD: f64 = 1.0 / 60.0;

/// How a committed frame moved the clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tick {
    /// Seconds of stage time it added.
    pub step: f64,
    /// Frames its gap from the reference skipped, counted once the frame
    /// succeeds.
    pub dropped: u64,
}

/// Stage time, the reference frame's `target_time` and the fps cap.
#[derive(Clone, Debug, Default)]
pub(crate) struct Clock {
    time: f64,
    /// The latest `target_time` a committed frame carried; `None` before
    /// the first committed frame after creation or resume.
    reference: Option<f64>,
    max_fps: u32,
}

impl Clock {
    /// Stage time in seconds.
    pub fn time(&self) -> f64 {
        self.time
    }

    pub fn set_max_fps(&mut self, max_fps: u32) {
        self.max_fps = max_fps;
    }

    /// Makes the next committed frame the origin again, as after resume.
    pub fn restart(&mut self) {
        self.reference = None;
    }

    /// Whether a call at `target_time` may draw: always when uncapped or
    /// before the first frame, otherwise once a period less
    /// [`DUE_SLACK`] has passed since the reference.
    pub fn due(&self, target_time: f64) -> bool {
        match (self.reference, self.max_fps) {
            (None, _) | (_, 0) => true,
            (Some(r), fps) => target_time >= r + 1.0 / f64::from(fps) - DUE_SLACK,
        }
    }

    /// Commits a frame at `target_time`: stage time advances by the gap
    /// from the reference, clamped to 0..max(0.25, 1.5 periods) when
    /// capped or 0..0.25 uncapped, and the frame becomes the reference. A
    /// time before the reference advances nothing and leaves the reference
    /// where it is, so a host clock that steps back never makes the next
    /// frame jump.
    pub fn commit(&mut self, target_time: f64) -> Tick {
        let Some(reference) = self.reference else {
            self.reference = Some(target_time);
            return Tick {
                step: 0.0,
                dropped: 0,
            };
        };
        let gap = target_time - reference;
        let (period, max_step) = match self.max_fps {
            0 => (UNCAPPED_PERIOD, MAX_STEP),
            fps => {
                let period = 1.0 / f64::from(fps);
                (period, MAX_STEP.max(1.5 * period))
            }
        };
        let step = gap.clamp(0.0, max_step);
        let dropped = if gap > 1.5 * period {
            // The gap is finite and over 1.5 periods, so this is at least 1.
            (gap / period).round() as u64 - 1
        } else {
            0
        };
        self.time += step;
        self.reference = Some(reference.max(target_time));
        Tick { step, dropped }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_frame_sets_the_origin() {
        let mut c = Clock::default();
        assert!(c.due(100.0));
        assert_eq!(c.commit(100.0).step, 0.0);
        assert_eq!(c.commit(100.5).step, 0.25);
        assert_eq!(c.time(), 0.25);
        c.restart();
        assert_eq!(c.commit(500.0).step, 0.0);
        assert_eq!(c.time(), 0.25);
    }

    #[test]
    fn steps_clamp_to_the_cap() {
        let mut c = Clock::default();
        c.set_max_fps(2);
        c.commit(0.0);
        // 1.5 periods of 0.5 s.
        assert_eq!(c.commit(10.0).step, 0.75);
        c.set_max_fps(30);
        // max(0.25, 0.05).
        assert_eq!(c.commit(20.0).step, 0.25);
    }

    #[test]
    fn dropped_frames_count_whole_periods_past_one_and_a_half() {
        let mut c = Clock::default();
        c.set_max_fps(30);
        c.commit(0.0);
        // 1.4 periods: none.
        assert_eq!(c.commit(1.4 / 30.0).dropped, 0);
        // 2 periods: one skipped.
        let r = 1.4 / 30.0;
        assert_eq!(c.commit(r + 2.0 / 30.0).dropped, 1);
        // Uncapped, a 10 s gap: 600 periods of 1/60 s, so 599 skipped.
        let mut c = Clock::default();
        c.commit(0.0);
        assert_eq!(c.commit(10.0).dropped, 599);
    }
}
