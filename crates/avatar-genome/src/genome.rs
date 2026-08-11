use std::{error::Error, fmt};

const GENOME_BYTES: usize = 1_024;
const GENOME_BITS: usize = GENOME_BYTES * 8;

/// A version-one avatar genome: exactly 8,192 bits of DNA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Genome([u8; GENOME_BYTES]);

impl Genome {
    /// Constructs a genome from its canonical 1 KiB byte representation.
    pub fn from_bytes(bytes: [u8; GENOME_BYTES]) -> Self {
        Self(bytes)
    }

    /// Parses the exact lowercase-or-uppercase hexadecimal wire representation.
    pub fn from_hex(hex: &str) -> Result<Self, GenomeError> {
        if hex.len() != GENOME_BYTES * 2 {
            return Err(GenomeError::InvalidHexLength { actual: hex.len() });
        }

        let mut bytes = [0; GENOME_BYTES];
        for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
            let high = hex_nibble(pair[0]).ok_or(GenomeError::InvalidHexDigit {
                index: index * 2,
                byte: pair[0],
            })?;
            let low = hex_nibble(pair[1]).ok_or(GenomeError::InvalidHexDigit {
                index: index * 2 + 1,
                byte: pair[1],
            })?;
            bytes[index] = high << 4 | low;
        }
        Ok(Self(bytes))
    }

    /// Returns the canonical fixed-size genome bytes.
    pub fn as_bytes(&self) -> &[u8; GENOME_BYTES] {
        &self.0
    }

    /// Returns the canonical lowercase hexadecimal representation.
    pub fn to_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut hex = String::with_capacity(GENOME_BYTES * 2);
        for byte in self.0 {
            hex.push(HEX[(byte >> 4) as usize] as char);
            hex.push(HEX[(byte & 0x0f) as usize] as char);
        }
        hex
    }

    /// Reads an unsigned MSB-first field from the genome bit string.
    ///
    /// The fixed bit map only requests fields no wider than 64 bits. Keeping
    /// this reader private prevents callers from treating versioned genome
    /// offsets as a stable public API instead of decoding through `decode`.
    pub(crate) fn read_bits(&self, offset: usize, width: usize) -> u64 {
        assert!(width <= 64, "genome fields may be at most 64 bits wide");
        assert!(
            offset <= GENOME_BITS.saturating_sub(width),
            "genome field must fit within 8,192 bits"
        );

        let mut value = 0;
        for bit_offset in offset..offset + width {
            let byte = self.0[bit_offset / 8];
            let bit = (byte >> (7 - bit_offset % 8)) & 1;
            value = (value << 1) | u64::from(bit);
        }
        value
    }
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// An error returned when text cannot represent one complete genome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenomeError {
    InvalidHexLength { actual: usize },
    InvalidHexDigit { index: usize, byte: u8 },
}

impl fmt::Display for GenomeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHexLength { actual } => write!(
                formatter,
                "genome hex must contain {} characters, received {actual}",
                GENOME_BYTES * 2
            ),
            Self::InvalidHexDigit { index, byte } => {
                write!(
                    formatter,
                    "invalid hexadecimal byte 0x{byte:02x} at index {index}"
                )
            }
        }
    }
}

impl Error for GenomeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_msb_first_fields_across_bytes() {
        let mut bytes = [0; GENOME_BYTES];
        bytes[0] = 0b1010_0000;
        bytes[1] = 0b0101_1100;
        let genome = Genome::from_bytes(bytes);

        assert_eq!(genome.read_bits(0, 4), 0b1010);
        assert_eq!(genome.read_bits(3, 7), 0b000_0001);
        assert_eq!(genome.read_bits(4, 12), 0b0000_0101_1100);
    }

    #[test]
    fn hexadecimal_round_trip_is_canonical() {
        let mut bytes = [0; GENOME_BYTES];
        bytes[0] = 0xab;
        bytes[GENOME_BYTES - 1] = 0xcd;
        let genome = Genome::from_bytes(bytes);

        let hex = genome.to_hex();
        assert!(hex.starts_with("ab"));
        assert!(hex.ends_with("cd"));
        assert_eq!(Genome::from_hex(&hex), Ok(genome));
    }

    #[test]
    fn hexadecimal_parser_rejects_incomplete_and_invalid_values() {
        assert_eq!(
            Genome::from_hex("00"),
            Err(GenomeError::InvalidHexLength { actual: 2 })
        );

        let mut hex = "00".repeat(GENOME_BYTES);
        hex.replace_range(42..43, "g");
        assert_eq!(
            Genome::from_hex(&hex),
            Err(GenomeError::InvalidHexDigit {
                index: 42,
                byte: b'g'
            })
        );
    }
}
