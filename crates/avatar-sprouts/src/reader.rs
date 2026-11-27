//! Genome bytes, read the way the reference reads its genes.

/// Bytes in a stored genome.
pub const GENOME_LEN: usize = 16_384;

/// Where each group of genes starts. Bytes 256 to 1,023 are never read.
pub(crate) mod dna {
    pub(crate) const BODY: usize = 1024;
    pub(crate) const SKIN: usize = 1056;
    pub(crate) const FACE: usize = 1104;
    pub(crate) const COLOUR: usize = 1136;
    pub(crate) const RULES: usize = 1168;
    pub(crate) const HABIT: usize = 1184;
    pub(crate) const LEAF: usize = 1200;
    pub(crate) const ORGANS: usize = 1216;
    pub(crate) const PLAN: usize = 1232;
    pub(crate) const ROOTS: usize = 1264;
    pub(crate) const COAT: usize = 1296;
    pub(crate) const STREAMS: usize = 1344;
    pub(crate) const TEMPER: usize = 1504;
    pub(crate) const EXTRA: usize = 1520;
}

/// Reads genes from a genome. Every choice goes through the same small set
/// of readers, each a fixed function of one byte.
#[derive(Clone, Copy)]
pub(crate) struct Reader<'a> {
    bytes: &'a [u8; GENOME_LEN],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8; GENOME_LEN]) -> Self {
        Self { bytes }
    }

    pub(crate) fn u8(&self, offset: usize) -> u8 {
        self.bytes[offset]
    }

    pub(crate) fn unit(&self, offset: usize) -> f64 {
        f64::from(self.u8(offset)) / 255.0
    }

    pub(crate) fn lin(&self, offset: usize, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (f64::from(self.u8(offset)) / 255.0)
    }

    pub(crate) fn sgn(&self, offset: usize) -> f64 {
        f64::from(self.u8(offset)) / 127.5 - 1.0
    }

    /// The byte through a fixed one-to-one scramble (times 151, plus 101,
    /// modulo 256), so that every bit of a choosing gene can change the choice.
    pub(crate) fn mixed(&self, offset: usize) -> u32 {
        (u32::from(self.u8(offset)) * 151 + 101) & 255
    }

    pub(crate) fn chance(&self, offset: usize, p: f64) -> bool {
        f64::from(self.mixed(offset)) < p * 256.0
    }

    /// One of `n` equally likely choices.
    pub(crate) fn one(&self, offset: usize, n: u32) -> u32 {
        self.mixed(offset) * n / 256
    }

    /// An index chosen in proportion to the weights.
    pub(crate) fn pick(&self, offset: usize, weights: &[u32]) -> usize {
        let total: f64 = weights.iter().map(|&w| f64::from(w)).sum();
        let mut t = (f64::from(self.mixed(offset)) + 0.5) / 256.0 * total;
        for (index, &weight) in weights.iter().enumerate() {
            t -= f64::from(weight);
            if t < 0.0 {
                return index;
            }
        }
        weights.len() - 1
    }

    pub(crate) fn stream(&self, start: usize, length: usize) -> Stream {
        Stream {
            bytes: (0..length).map(|k| self.u8(start + k)).collect(),
            next: 0,
        }
    }
}

/// Bytes handed out one after another, wrapping after the last. Two shoots
/// that follow the same rules read different streams, so they still grow
/// differently. The stream keeps its own copy of its bytes.
#[derive(Clone, Debug)]
pub(crate) struct Stream {
    bytes: Vec<u8>,
    next: usize,
}

impl Stream {
    pub(crate) fn at(&self, k: usize) -> u8 {
        self.bytes[k % self.bytes.len()]
    }

    pub(crate) fn next(&mut self) -> u8 {
        let value = self.at(self.next);
        self.next += 1;
        value
    }

    pub(crate) fn unit(&mut self) -> f64 {
        f64::from(self.next()) / 255.0
    }

    pub(crate) fn reset(&mut self) {
        self.next = 0;
    }

    pub(crate) fn bit(&self, k: usize) -> bool {
        (self.at(k >> 3) >> (k & 7)) & 1 == 1
    }
}
