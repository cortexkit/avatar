//! Pinned render-time mood deltas and per-creature reactivity scaling.

use avatar_phenotype::{Bam16, MoodResponse, Q32};

use crate::fixed::q32_mul;
use crate::svg::Mood;

/// Geometry deltas applied after the DNA-neutral face has been resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MoodDeltas {
    pub upper_lid_close: Q32,
    pub lower_lid_close: Q32,
    pub eye_rotation: Bam16,
    pub eyeball_shift_x: Q32,
    pub eyeball_shift_y: Q32,
    pub brow_rotation: Bam16,
    pub brow_shift_y: Q32,
    pub mouth_curve: Q32,
    pub lower_contour_raise: Q32,
}

impl MoodDeltas {
    pub(crate) const ZERO: Self = Self {
        upper_lid_close: Q32(0),
        lower_lid_close: Q32(0),
        eye_rotation: Bam16(0),
        eyeball_shift_x: Q32(0),
        eyeball_shift_y: Q32(0),
        brow_rotation: Bam16(0),
        brow_shift_y: Q32(0),
        mouth_curve: Q32(0),
        lower_contour_raise: Q32(0),
    };

    /// Scales every axis exactly once with its matching phenotype response.
    pub(crate) fn scaled(self, response: &MoodResponse) -> Self {
        Self {
            upper_lid_close: Q32(q32_mul(self.upper_lid_close.0, response.lid_reactivity.0)),
            lower_lid_close: Q32(q32_mul(self.lower_lid_close.0, response.lid_reactivity.0)),
            eye_rotation: scale_bam(self.eye_rotation, response.eye_rotation_reactivity),
            eyeball_shift_x: Q32(q32_mul(
                self.eyeball_shift_x.0,
                response.eye_rotation_reactivity.0,
            )),
            eyeball_shift_y: Q32(q32_mul(
                self.eyeball_shift_y.0,
                response.eye_rotation_reactivity.0,
            )),
            brow_rotation: scale_bam(self.brow_rotation, response.brow_reactivity),
            brow_shift_y: Q32(q32_mul(self.brow_shift_y.0, response.brow_reactivity.0)),
            mouth_curve: Q32(q32_mul(self.mouth_curve.0, response.mouth_reactivity.0)),
            lower_contour_raise: Q32(q32_mul(
                self.lower_contour_raise.0,
                response.lid_reactivity.0,
            )),
        }
    }
}

/// Returns the fixed mood-delta row for each supported mood.
///
/// Wrapped BAM16 constants store negative angles in two's-complement form;
/// conversion to SVG degrees interprets them through `i16` first.
pub(crate) const fn deltas(mood: Mood) -> MoodDeltas {
    match mood {
        // face.tsx 620–622 and 797–799 make no changes in neutral. Keeping this
        // row literally zero is what makes mood application an identity.
        Mood::Neutral => MoodDeltas::ZERO,
        // New focused-work row: 0.15 upper-lid close and a 0.04 inward pupil
        // shift use the same axes centralized from face.tsx 620–817.
        Mood::Working => MoodDeltas {
            upper_lid_close: Q32(644_245_094), // 0.15
            eyeball_shift_x: Q32(171_798_692), // 0.04, inward after mirroring
            ..MoodDeltas::ZERO
        },
        // face.tsx 661–681: midpoint 6.5° eye rotation = 1_183 BAM16,
        // −6° brow rotation = wrapped 64_444, lid fractions are 1/5 and 1/3,
        // the brow rises, and the lower contour rises by 1/4 of its height.
        Mood::Happy => MoodDeltas {
            upper_lid_close: Q32(858_993_459),   // 0.20
            lower_lid_close: Q32(1_431_655_765), // 0.333333333
            eye_rotation: Bam16(1_183),
            brow_rotation: Bam16(64_444),
            brow_shift_y: Q32(-171_798_692),         // -0.04
            mouth_curve: Q32(4_294_967_296),         // 1.0
            lower_contour_raise: Q32(1_073_741_824), // 0.25
            ..MoodDeltas::ZERO
        },
        // New waiting row: −0.10 opens both lids, −0.06 of face height raises
        // the brows, and −0.04 of eye height gives the slight upward look.
        Mood::Waiting => MoodDeltas {
            upper_lid_close: Q32(-429_496_730), // -0.10
            lower_lid_close: Q32(-429_496_730), // -0.10
            eyeball_shift_y: Q32(-171_798_692), // -0.04
            brow_shift_y: Q32(-257_698_038),    // -0.06
            ..MoodDeltas::ZERO
        },
        // face.tsx 623–633 and 800–803: midpoint 17.5° = 3_186 BAM16,
        // doubled brow rotation 35° = 6_372 BAM16, upper closure 1/1.8 =
        // 0.555555556, and lower closure 1/2.5 = 0.4. The downward eyeball
        // placement is normalized to 0.12 of eye height.
        Mood::Blocked => MoodDeltas {
            upper_lid_close: Q32(2_386_092_942), // 0.555555556
            lower_lid_close: Q32(1_717_986_918), // 0.40
            eye_rotation: Bam16(3_186),
            eyeball_shift_y: Q32(515_396_076), // 0.12
            brow_rotation: Bam16(6_372),
            mouth_curve: Q32(-3_006_477_107), // -0.70
            ..MoodDeltas::ZERO
        },
        // face.tsx 635–654 and 804–809: 1/2 upper and 1/8 lower closure,
        // −9° = wrapped 63_898 BAM16, −10° brow = wrapped 63_716, and the
        // radius/8 shifts are normalized to 0.04 inward and 0.06 downward.
        Mood::Sad => MoodDeltas {
            upper_lid_close: Q32(2_147_483_648), // 0.50
            lower_lid_close: Q32(536_870_912),   // 0.125
            eye_rotation: Bam16(63_898),
            eyeball_shift_x: Q32(171_798_692), // 0.04, inward after mirroring
            eyeball_shift_y: Q32(257_698_038), // 0.06
            brow_rotation: Bam16(63_716),
            mouth_curve: Q32(-3_435_973_837), // -0.80
            ..MoodDeltas::ZERO
        },
        // face.tsx 655–660 and 810–813 close the lower lid by 1/2 and put the
        // upper lid beyond the eye top. The ratio-dependent expression is
        // pinned as 0.85 here; the high eyeball placement becomes −0.12.
        Mood::Sleepy => MoodDeltas {
            upper_lid_close: Q32(3_650_722_202), // 0.85
            lower_lid_close: Q32(2_147_483_648), // 0.50
            eyeball_shift_y: Q32(-515_396_076),  // -0.12
            ..MoodDeltas::ZERO
        },
    }
}

fn scale_bam(angle: Bam16, reactivity: Q32) -> Bam16 {
    let signed = i64::from(angle.0 as i16);
    let scaled = q32_mul(signed, reactivity.0);
    Bam16((scaled as i16) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_is_the_literal_zero_row() {
        assert_eq!(deltas(Mood::Neutral), MoodDeltas::ZERO);
    }

    #[test]
    fn pinned_rows_keep_the_intended_lid_ordering() {
        let blocked = deltas(Mood::Blocked);
        let sad = deltas(Mood::Sad);
        let sleepy = deltas(Mood::Sleepy);
        let waiting = deltas(Mood::Waiting);
        assert!(blocked.upper_lid_close.0 > sad.upper_lid_close.0);
        assert!(sleepy.upper_lid_close.0 > blocked.upper_lid_close.0);
        assert!(waiting.upper_lid_close.0 < 0);
        assert!(waiting.lower_lid_close.0 < 0);
    }

    #[test]
    fn scaling_handles_wrapped_negative_angles() {
        let response = MoodResponse {
            lid_reactivity: Q32::ONE,
            brow_reactivity: Q32(Q32::ONE.0 / 2),
            eye_rotation_reactivity: Q32::ONE,
            mouth_reactivity: Q32::ONE,
        };
        let scaled = deltas(Mood::Sad).scaled(&response);
        assert_eq!(scaled.eye_rotation, Bam16(63_898));
        assert_eq!(scaled.brow_rotation.0 as i16, -910);
    }
}
