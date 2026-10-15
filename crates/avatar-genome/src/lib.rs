//! Version-one avatar genome parsing, decoding, and identity derivation.

mod decode;
mod genome;
mod identity;

pub use decode::decode;
pub use genome::{Genome, GenomeError};
pub use identity::{derive_genome, derive_genome_bytes, SeatIdentity};
