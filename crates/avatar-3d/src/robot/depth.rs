//! The depth traits: bytes 6144 to 6153 of the genome, which the 2D type
//! never reads. They give the 3D robot its depth and nothing else: seen from
//! the front, the 3D robot keeps the 2D geometry.

use avatar_small_robots::GENOME_LEN;

/// The first depth byte.
pub const FIRST_BYTE: usize = 6144;
/// The last depth byte.
pub const LAST_BYTE: usize = 6153;

/// What the back of the robot carries.
pub const BACK_KINDS: [&str; 4] = ["hatch", "vents", "plate", "grille"];

/// The decoded depth traits. Each is read from a bit field of one depth
/// byte and spread evenly over its range, as the experimental browser
/// renderer the 3D robots were first drawn in reads them, so a genome keeps
/// its depth between the two.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Depth {
    /// Extruded chassis depth over its width, 0.72 to 1 (byte 6144, bits
    /// 0–3). A lathed chassis maps the same bits to 0.86 to 1.
    pub chassis_depth: f64,
    /// Chassis edge rounding as a share of the largest bevel, 0.3 to 1
    /// (6144, bits 4–7).
    pub chassis_round: f64,
    /// Head depth over its width, 0.76 to 1 (6145, bits 0–3). A lathed
    /// head maps the same bits to 0.86 to 1.
    pub head_depth: f64,
    /// Head edge rounding, 0.3 to 1 (6145, bits 4–7).
    pub head_round: f64,
    /// Wheel width over chassis depth, 0.3 to 0.6 (6146, bits 0–3).
    pub wheel_width: f64,
    /// Track and tread length over chassis depth, 0.7 to 1.05 (6146, bits
    /// 4–7).
    pub tread_width: f64,
    /// Degrees the arms lean toward the viewer, −10 to 26 (6147, bits 0–4).
    pub arm_tilt: f64,
    /// Where the shoulders sit in front of the body's middle, over its
    /// depth, −0.08 to 0.14 (6147, bits 5–7).
    pub shoulder_z: f64,
    /// Degrees the antennae lean forward (negative: back), −24 to 10 (6148,
    /// bits 0–4).
    pub antenna_tilt: f64,
    /// Glass bulge over glass width, 0 to 0.1 (6149, bits 0–3). Read but
    /// unused: the face is a texture on the head's front.
    pub screen_bulge: f64,
    /// Lens dome height over the lens radius, 0.35 to 0.7 (6149, bits 4–7).
    /// Read but unused: a stalk eye's lens is drawn in its
    /// texture, on the front of its pod.
    pub lens_dome: f64,
    /// What the back carries, from [`BACK_KINDS`] (6150, bits 0–1). Read but
    /// unused, like the next two: the 2D type has no rear
    /// fitting, and the 3D parts are exactly the 2D ones.
    pub back_kind: &'static str,
    /// Size of the back fitting, 0.8 to 1.2 (6150, bits 2–4).
    pub back_size: f64,
    /// Variant of the back fitting, 0 to 7 (6150, bits 5–7).
    pub back_style: u8,
    /// Shine within each finish's range, 0 to 1 (6151, bits 0–3).
    pub gloss: f64,
    /// Screen bezel standing proud, 0.6 to 2.4 per 100 units of width
    /// (6151, bits 4–7). Read but unused: the bezel is drawn in
    /// the face texture.
    pub bezel_depth: f64,
    /// Foot depth over foot width, 1 to 1.7 (6152, bits 0–3).
    pub foot_length: f64,
    /// How far the chest panel stands proud in robot units, 0.3 to 1.6
    /// (6152, bits 4–7).
    pub panel_raise: f64,
    /// How deep a stalk eye's pod is: its depth is its larger radius times
    /// one plus twice this, 0.25 to 0.6 (6153, bits 0–3).
    pub housing_depth: f64,
    /// Share of a rivet's sphere proud of the paint, 0.4 to 0.9 (6153, bits
    /// 4–7). Read but unused: rivets are drawn in the marks
    /// textures.
    pub rivet_dome: f64,
}

impl Depth {
    /// Reads the depth traits from a genome.
    pub fn new(genome: &[u8; GENOME_LEN]) -> Self {
        let bits = |byte: usize, lo: u32, n: u32| {
            debug_assert!((FIRST_BYTE..=LAST_BYTE).contains(&byte));
            (u32::from(genome[byte]) >> lo) & ((1 << n) - 1)
        };
        let lin = |byte: usize, lo: u32, n: u32, a: f64, b: f64| {
            a + (b - a) * f64::from(bits(byte, lo, n)) / f64::from((1_u32 << n) - 1)
        };
        Self {
            chassis_depth: lin(6144, 0, 4, 0.72, 1.0),
            chassis_round: lin(6144, 4, 4, 0.3, 1.0),
            head_depth: lin(6145, 0, 4, 0.76, 1.0),
            head_round: lin(6145, 4, 4, 0.3, 1.0),
            wheel_width: lin(6146, 0, 4, 0.3, 0.6),
            tread_width: lin(6146, 4, 4, 0.7, 1.05),
            arm_tilt: lin(6147, 0, 5, -10.0, 26.0),
            shoulder_z: lin(6147, 5, 3, -0.08, 0.14),
            antenna_tilt: lin(6148, 0, 5, -24.0, 10.0),
            screen_bulge: lin(6149, 0, 4, 0.0, 0.1),
            lens_dome: lin(6149, 4, 4, 0.35, 0.7),
            back_kind: BACK_KINDS[bits(6150, 0, 2) as usize],
            back_size: lin(6150, 2, 3, 0.8, 1.2),
            back_style: bits(6150, 5, 3) as u8,
            gloss: lin(6151, 0, 4, 0.0, 1.0),
            bezel_depth: lin(6151, 4, 4, 0.6, 2.4),
            foot_length: lin(6152, 0, 4, 1.0, 1.7),
            panel_raise: lin(6152, 4, 4, 0.3, 1.6),
            housing_depth: lin(6153, 0, 4, 0.25, 0.6),
            rivet_dome: lin(6153, 4, 4, 0.4, 0.9),
        }
    }

    /// A lathed hull's depth over width: the depth trait's position within
    /// its range, mapped to 0.86 to 1.
    pub(super) fn lathe_ratio(value: f64, low: f64) -> f64 {
        0.86 + 0.14 * (value - low) / (1.0 - low)
    }
}
