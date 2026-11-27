//! Reading the genome, and the fixed hashes behind the handwork's small
//! irregularities.

/// The genome bytes this type reads its own traits from: 1024 to 1279, plus
/// creature.wild's colour bytes 160 to 163.
pub const OWN_FIRST: usize = 1024;
pub const OWN_END: usize = 1280;

/// Reads the genome a few bits at a time. A field is the top n bits of one
/// byte, so low bits a trait could not show stay unread.
pub struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    fn byte(&self, addr: usize) -> u32 {
        assert!(
            (OWN_FIRST..OWN_END).contains(&addr) || (160..=163).contains(&addr),
            "read outside this type's bytes: {addr}"
        );
        u32::from(self.bytes[addr])
    }

    /// The top n bits of a byte.
    pub fn u(&self, addr: usize, n: u32) -> u32 {
        self.byte(addr) >> (8 - n)
    }

    /// The low n bits of a byte. Only wild's colour bytes are read this way.
    pub fn low(&self, addr: usize, n: u32) -> u32 {
        self.byte(addr) & ((1 << n) - 1)
    }

    pub fn f(&self, addr: usize, n: u32) -> f64 {
        f64::from(self.u(addr, n)) / f64::from((1_u32 << n) - 1)
    }

    pub fn lin(&self, addr: usize, n: u32, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f(addr, n)
    }

    pub fn s(&self, addr: usize, n: u32) -> f64 {
        self.f(addr, n) * 2.0 - 1.0
    }

    pub fn bit(&self, addr: usize) -> bool {
        self.u(addr, 1) == 1
    }

    /// A weighted choice: integer weights summing to 2^n. `turn` is added to
    /// the field's value first, modulo 2^n, which changes which values give
    /// which variant but never the shares.
    pub fn pick<T: Copy>(&self, addr: usize, n: u32, pairs: &[(T, u32)], turn: u32) -> T {
        let total: u32 = pairs.iter().map(|p| p.1).sum();
        assert_eq!(total, 1 << n, "pick weights must sum to 2^n");
        let v = (self.u(addr, n) + turn) & (total - 1);
        let mut acc = 0;
        for p in pairs {
            acc += p.1;
            if v < acc {
                return p.0;
            }
        }
        pairs[pairs.len() - 1].0
    }
}

/// A 32-bit finaliser (the "lowbias32" mix).
pub fn mix32(x: u32) -> u32 {
    let x = (x ^ (x >> 16)).wrapping_mul(0x7feb_352d);
    let x = (x ^ (x >> 15)).wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// FNV-1a over the string's UTF-16 code units, then mixed. Every name hashed
/// here is ASCII, so code units and bytes agree.
pub fn hash_str(s: &str, seed: u32) -> u32 {
    let mut h = seed ^ 0x811c_9dc5;
    for c in s.encode_utf16() {
        h = (h ^ u32::from(c)).wrapping_mul(0x0100_0193);
    }
    mix32(h)
}

/// A deterministic stream of numbers in [0, 1) from a key.
pub struct Stream(u32);

impl Stream {
    pub fn new(key: u32) -> Self {
        Self(key)
    }

    pub fn draw(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9);
        f64::from(mix32(self.0)) / 4_294_967_296.0
    }
}
