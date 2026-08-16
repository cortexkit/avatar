//! The gacha test: note-009 mappings (float rendition) over RANDOM bytes.
//!
//! Design law 3 says every bit pattern lands on a presentable creature.
//! The other grids show hand-picked casts; this module decodes random
//! genomes exactly the way the v2 decoder will, so the claim is tested
//! against the mapping bands, not against taste.

use crate::body::BodyParams;
use crate::face::{EyeKind, MouthKind};
use crate::parts::{BottomPart, TopPart};

/// splitmix64: tiny, seedable, good-enough bit source for the playground.
/// Counts bytes served so decoders can report their real DNA appetite.
pub struct Bits(u64, usize);

impl Bits {
    pub fn new(seed: u64) -> Self {
        Bits(seed, 0)
    }
    /// Bytes served so far — the decoder's actual consumption, as opposed to
    /// the region map's allocation.
    pub fn consumed(&self) -> usize {
        self.1
    }
    pub fn byte(&mut self) -> u8 {
        self.1 += 1;
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((z ^ (z >> 31)) >> 24) as u8
    }
    /// Byte eased linearly into [lo, hi].
    pub fn lin(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (self.byte() as f64 / 255.0)
    }
}

pub struct RandomCreature {
    pub bp: BodyParams,
    pub top: TopPart,
    pub bottom: BottomPart,
    pub eyes: EyeKind,
    pub mouth: MouthKind,
    pub recipe: usize,
    pub hue: f64,
    pub eye_scale: f64,
}

/// Decode per note 009's BODY/PALETTE/PARTS/FACE bands.
pub fn decode(seed: u64) -> RandomCreature {
    let mut b = Bits::new(seed);
    // BODY: byte 0 = plan (mod 1, discard), bytes 1..6 the bands.
    let _plan = b.byte();
    let bp = BodyParams {
        height: b.lin(140.0, 205.0),
        w_top: b.lin(50.0, 88.0),
        w_bot: b.lin(56.0, 96.0),
        n_top: b.lin(2.0, 4.6),
        n_bot: b.lin(2.0, 5.0),
        waist: b.lin(0.40, 0.70),
    };
    // PALETTE: weighted recipe via mod-8 alias table (note 009 weights:
    // pastel x3, toy x2, earthy x2, dark x1).
    let recipe = match b.byte() % 8 {
        0 | 1 | 2 => 0, // pastel analogous
        3 | 4 => 2,     // toy saturated
        5 | 6 => 4,     // muted earthy
        _ => 3,         // dark gravitas
    };
    let hue = (b.byte() as f64 / 255.0 + b.byte() as f64 / 65535.0) * 360.0;
    // PARTS: top mod 6, side skipped in v2.0 rendering, bottom mod 4.
    let top = match b.byte() % 6 {
        0 => TopPart::None,
        1 => TopPart::Ears,
        2 => TopPart::Horns,
        3 => TopPart::Antennae,
        4 => TopPart::Tuft,
        _ => TopPart::Nub,
    };
    let _side = b.byte();
    let bottom = match b.byte() % 4 {
        0 => BottomPart::None,
        1 => BottomPart::FeetNubs,
        2 => BottomPart::DuckFeet,
        _ => BottomPart::Tail,
    };
    // FACE: eye kind mod 5, scale band, mouth mod 6.
    let eyes = match b.byte() % 5 {
        0 => EyeKind::DotShine,
        1 => EyeKind::Lidded,
        2 => EyeKind::HappyArc,
        3 => EyeKind::Uneven,
        _ => EyeKind::HalfLid,
    };
    let eye_scale = b.lin(0.85, 1.18);
    let mouth = match b.byte() % 6 {
        0 => MouthKind::Arc,
        1 => MouthKind::CatW,
        2 => MouthKind::OpenHappy,
        3 => MouthKind::Snaggle,
        4 => MouthKind::Flat,
        _ => MouthKind::Fangs,
    };
    RandomCreature {
        bp,
        top,
        bottom,
        eyes,
        mouth,
        recipe,
        hue,
        eye_scale,
    }
}
