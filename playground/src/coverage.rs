//! Branch coverage for corpus design.
//!
//! Note 011 names the weakness this closes: with three implementations of
//! ~800 lines of branch-heavy geometry, a port that gets one tiebreak wrong
//! fails only on the genomes that reach that branch — so corpus coverage
//! "has to be argued about rather than assumed". Arguing is what this
//! module replaces.
//!
//! The counters deliberately track the RARE paths, not the obvious ones.
//! Every seed exercises the spline and the palette; almost none exercise a
//! marching-squares saddle or the eyes-only placement fallback, and those
//! are precisely where two implementations can disagree while both look
//! correct on a typical sheet.

use std::sync::atomic::{AtomicUsize, Ordering};

/// One counter per divergence-prone branch. Process-global because the
/// branches live deep inside geometry code whose signatures should not grow
/// a context parameter for the sake of a diagnostic.
pub struct Counters {
    /// Marching squares hit an ambiguous 4-crossing cell (saddle). Its
    /// pairing is arbitrary-but-pinned, so ports must pair identically.
    pub saddle: AtomicUsize,
    /// Extremity half-width was clamped by the curvature cap — the branch
    /// that stops a capsule folding over itself.
    pub curvature_clamp: AtomicUsize,
    /// Face placement found no candidate that also fits a mouth and fell
    /// back to eyes-only.
    pub placement_fallback: AtomicUsize,
    /// Belly patch suppressed for being too small to read.
    pub belly_suppressed: AtomicUsize,
    /// Belly walk chose a span other than the widest, i.e. it stayed on one
    /// mass across a split body.
    pub belly_span_switch: AtomicUsize,
    /// Segment chaining had two candidates within eps and used the
    /// nearest-then-lowest-index tiebreak.
    pub chain_tiebreak: AtomicUsize,
}

pub static COUNTERS: Counters = Counters {
    saddle: AtomicUsize::new(0),
    curvature_clamp: AtomicUsize::new(0),
    placement_fallback: AtomicUsize::new(0),
    belly_suppressed: AtomicUsize::new(0),
    belly_span_switch: AtomicUsize::new(0),
    chain_tiebreak: AtomicUsize::new(0),
};

pub fn hit(c: &AtomicUsize) {
    c.fetch_add(1, Ordering::Relaxed);
}

pub fn snapshot() -> [(&'static str, usize); 6] {
    [
        (
            "marching-squares saddle",
            COUNTERS.saddle.load(Ordering::Relaxed),
        ),
        (
            "extremity curvature clamp",
            COUNTERS.curvature_clamp.load(Ordering::Relaxed),
        ),
        (
            "placement eyes-only fallback",
            COUNTERS.placement_fallback.load(Ordering::Relaxed),
        ),
        (
            "belly suppressed",
            COUNTERS.belly_suppressed.load(Ordering::Relaxed),
        ),
        (
            "belly span switch",
            COUNTERS.belly_span_switch.load(Ordering::Relaxed),
        ),
        (
            "chain tiebreak",
            COUNTERS.chain_tiebreak.load(Ordering::Relaxed),
        ),
    ]
}

pub fn reset() {
    COUNTERS.saddle.store(0, Ordering::Relaxed);
    COUNTERS.curvature_clamp.store(0, Ordering::Relaxed);
    COUNTERS.placement_fallback.store(0, Ordering::Relaxed);
    COUNTERS.belly_suppressed.store(0, Ordering::Relaxed);
    COUNTERS.belly_span_switch.store(0, Ordering::Relaxed);
    COUNTERS.chain_tiebreak.store(0, Ordering::Relaxed);
}
