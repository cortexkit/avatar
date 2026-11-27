//! Reading genes from the stored genome, as the reference reads them.

use avatar_svg::wild::GENOME_LEN;

/// Where each group of genes starts. Bytes 256 to 1023 are never read.
pub mod dna {
    /// 1024 outline and size, 1025 to 1026 its proportions, 1027 its line.
    pub const HEAD: usize = 1024;
    pub const NECK: usize = 1030;
    pub const EARS: usize = 1032;
    /// Where the features sit.
    pub const FACE: usize = 1036;
    pub const EYES: usize = 1040;
    pub const BROWS: usize = 1046;
    pub const NOSE: usize = 1050;
    pub const MOUTH: usize = 1054;
    /// 1060 to 1063 style, hairline and volume; 1064 to 1079 one byte per lock.
    pub const HAIR: usize = 1060;
    /// 1084 hair colour, 1085 iris and lips.
    pub const COLOUR: usize = 1084;
    /// 1088 to 1090 facial hair and its shade; 1091 to 1098 one byte per tuft.
    pub const BEARD: usize = 1088;
    /// Age and its lines, cheeks, a mole, the chin.
    pub const MARKS: usize = 1102;
    /// How the head carries its moods.
    pub const TEMPER: usize = 1108;
}

/// The genome, read a few bits at a time.
#[derive(Clone, Copy)]
pub struct Genes<'a> {
    bytes: &'a [u8; GENOME_LEN],
}

/// The fixed one-to-one scramble a choice gene's bits go through, by width,
/// so that every bit of a choice gene can change the choice.
const SCRAMBLE: [u32; 7] = [1, 1, 3, 3, 5, 11, 19];

impl<'a> Genes<'a> {
    pub fn new(bytes: &'a [u8; GENOME_LEN]) -> Self {
        Self { bytes }
    }

    /// `n` bits of the byte at `byte`, starting `shift` bits above its lowest bit.
    pub fn raw(&self, byte: usize, shift: u32, n: u32) -> u32 {
        (u32::from(self.bytes[byte]) >> shift) & ((1 << n) - 1)
    }

    /// The bits as a number from 0 to 1.
    pub fn unit(&self, byte: usize, shift: u32, n: u32) -> f64 {
        f64::from(self.raw(byte, shift, n)) / f64::from((1u32 << n) - 1)
    }

    pub fn lin(&self, byte: usize, shift: u32, n: u32, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit(byte, shift, n)
    }

    /// The bits as a number from -1 to 1.
    pub fn signed(&self, byte: usize, shift: u32, n: u32) -> f64 {
        self.unit(byte, shift, n) * 2.0 - 1.0
    }

    pub fn flag(&self, byte: usize, shift: u32) -> bool {
        self.raw(byte, shift, 1) == 1
    }

    /// 1 or -1.
    pub fn side(&self, byte: usize, shift: u32) -> f64 {
        if self.flag(byte, shift) {
            1.0
        } else {
            -1.0
        }
    }

    /// An index chosen in proportion to whole-number weights that add up to
    /// a power of two, so that every bit pattern is a choice.
    pub fn pick(&self, byte: usize, shift: u32, weights: &[u32]) -> usize {
        let total: u32 = weights.iter().sum();
        let n = total.trailing_zeros();
        assert_eq!(
            1 << n,
            total,
            "choice weights must add up to a power of two"
        );
        let mut t = (self.raw(byte, shift, n) * SCRAMBLE[n as usize]) as i64 & i64::from(total - 1);
        for (i, &w) in weights.iter().enumerate() {
            t -= i64::from(w);
            if t < 0 {
                return i;
            }
        }
        weights.len() - 1
    }
}
