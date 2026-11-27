//! The genome reader and the genes the wild creature already reads (bytes
//! 0..255, meanings unchanged).

use avatar_svg::wild::genome::Bits;
use std::f64::consts::{PI, TAU};

use crate::geom::Spoke;

/// Reads genes from the stored genome bytes.
pub struct Reader<'a> {
    pub b: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Reader { b }
    }
    pub fn u8(&self, o: usize) -> u8 {
        self.b[o]
    }
    pub fn mod_(&self, o: usize, n: u32) -> u32 {
        u32::from(self.b[o]) % n
    }
    pub fn unit(&self, o: usize) -> f64 {
        f64::from(self.b[o]) / 255.0
    }
    pub fn lin(&self, o: usize, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (f64::from(self.b[o]) / 255.0)
    }
    pub fn sgn(&self, o: usize) -> f64 {
        f64::from(self.b[o]) / 127.5 - 1.0
    }
    /// The byte scrambled by a fixed one-to-one map, so every bit of a gene
    /// that makes a choice can change the choice.
    pub fn mixed(&self, o: usize) -> u32 {
        (u32::from(self.b[o]) * 151 + 101) & 255
    }
    pub fn chance(&self, o: usize, p: f64) -> bool {
        f64::from(self.mixed(o)) < p * 256.0
    }
    /// One of `n` equally likely choices.
    pub fn one(&self, o: usize, n: u32) -> u32 {
        self.mixed(o) * n / 256
    }
    /// An index chosen in proportion to the weights, from one byte.
    pub fn pick(&self, o: usize, weights: &[f64]) -> usize {
        let mut total = 0.0;
        for w in weights {
            total += w;
        }
        let mut t = (f64::from(self.mixed(o)) + 0.5) / 256.0 * total;
        for (i, w) in weights.iter().enumerate() {
            t -= w;
            if t < 0.0 {
                return i;
            }
        }
        weights.len() - 1
    }
}

/// One wild limb slot's genes.
#[derive(Clone, Copy)]
pub struct Ext {
    pub theta: f64,
    pub len: f64,
    pub width: f64,
    pub curl: f64,
    pub taper: f64,
    pub fill: u32,
}

/// One Voronoi site of the wild hide.
#[derive(Clone, Copy)]
pub struct Site {
    pub x: f64,
    pub y: f64,
    pub shade: u8,
}

/// The genes the wild creature reads from bytes 0..255.
pub struct Wild {
    pub spokes: Vec<Spoke>,
    pub exts: Vec<Ext>,
    pub topology: u32,
    pub recipe: u32,
    pub hue: f64,
    pub belly_variant: u32,
    pub eyes: u32,
    pub eye_aspect: f64,
    pub eye_wonk: f64,
    pub mouth: u32,
    pub mouth_w: f64,
    pub mouth_d: f64,
    pub brow: u32,
    pub voronoi: bool,
    pub sites: Vec<Site>,
}

pub fn decode_wild(r: &Reader) -> Wild {
    let spoke_count = 5 + r.mod_(0, 12) as usize;
    let mut spokes = Vec::with_capacity(spoke_count);
    for i in 0..spoke_count {
        let slot = 2 + 3 * i;
        let base = (i as f64 / spoke_count as f64) * TAU;
        let jitter = r.lin(slot, -0.45, 0.45) * TAU / spoke_count as f64;
        spokes.push(Spoke {
            theta: base + jitter,
            r: r.lin(slot + 1, 0.12, 1.35),
            tension: r.lin(slot + 2, 0.4, 2.6),
        });
    }
    let ext_count = r.mod_(64, 5) as usize;
    let topology = r.mod_(65, 3);
    let mut exts = Vec::with_capacity(ext_count);
    for i in 0..ext_count {
        let slot = 64 + 4 + 10 * i;
        exts.push(Ext {
            theta: r.lin(slot, PI * 0.95, TAU * 1.02),
            len: r.lin(slot + 1, 0.35, 1.15),
            width: r.lin(slot + 2, 0.10, 0.30),
            curl: r.lin(slot + 3, -1.3, 1.3),
            taper: r.lin(slot + 4, 0.15, 0.85),
            fill: r.mod_(slot + 5, 3),
        });
    }
    let rsel = r.mod_(160, 8);
    let recipe = if rsel <= 2 {
        0
    } else if rsel <= 4 {
        2
    } else if rsel <= 6 {
        4
    } else {
        3
    };
    let hue = (f64::from(r.u8(161)) / 255.0 + f64::from(r.u8(162)) / 65535.0) * 360.0;
    let belly_variant = r.mod_(163, 4);
    let eyes = r.mod_(176, 5);
    let eye_aspect = r.lin(177, 0.80, 1.28);
    let eye_wonk = r.lin(178, -0.20, 0.20);
    let mouth = r.mod_(179, 6);
    let mouth_w = r.lin(180, 0.75, 1.30);
    let mouth_d = r.lin(181, 0.70, 1.35);
    let brow_roll = r.mod_(182, 8);
    let brow = match brow_roll {
        0 | 1 => 1,
        2 => 2,
        3 => 3,
        _ => 0,
    };
    let voronoi = r.mod_(240, 5) < 2;
    let mut sites = Vec::new();
    if voronoi {
        let pair_count = 8 + r.mod_(241, 6);
        let site_seed =
            ((u64::from(r.u8(242)) << 8) | u64::from(r.u8(243))) ^ 0x5175_7266_6163_6501;
        let mut bits = Bits::new(site_seed);
        for _ in 0..pair_count {
            let x = bits.lin(0.04, 1.05);
            let y = bits.lin(-1.05, 1.05);
            let s3 = bits.byte() % 4;
            let shade = match s3 {
                0 => 0,
                1 => 1,
                _ => 2,
            };
            sites.push(Site { x, y, shade });
            sites.push(Site { x: -x, y, shade });
        }
    }
    Wild {
        spokes,
        exts,
        topology,
        recipe,
        hue,
        belly_variant,
        eyes,
        eye_aspect,
        eye_wonk,
        mouth,
        mouth_w,
        mouth_d,
        brow,
        voronoi,
        sites,
    }
}
