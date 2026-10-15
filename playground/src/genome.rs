//! The playground's bit source.
//!
//! The blob-era decoder that lived here (RandomCreature, decode) is gone
//! with the generator it fed: the wild pipeline reads its own bands in
//! main.rs::decode_full and never called it. It survived a clippy run
//! with -D warnings only because this crate is a lib, where `pub` means
//! "someone outside might call this" and dead-code analysis stops asking.

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

pub const GENOME_LEN: usize = 16_384;

/// Expand exploration seeds into a complete genome, so changing which fields
/// the decoder reads cannot shift the bytes assigned to other fields.
pub fn probe_genome(seed: u64) -> [u8; GENOME_LEN] {
    let mut bits = Bits::new(seed);
    std::array::from_fn(|_| bits.byte())
}

/// Byte eased linearly into [lo, hi], shared by the region decoders.
pub fn lin(byte: u8, lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * (byte as f64 / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_expands_every_byte_of_the_splitmix_stream() {
        let bytes = probe_genome(0);
        assert_eq!(
            &bytes[..16],
            &[123, 161, 128, 114, 81, 116, 31, 201, 65, 54, 60, 5, 224, 152, 0, 151]
        );
        let mut source = Bits::new(0);
        for byte in bytes {
            assert_eq!(byte, source.byte());
        }
        assert_eq!(source.consumed(), GENOME_LEN);
    }
}
