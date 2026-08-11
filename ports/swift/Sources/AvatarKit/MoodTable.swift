// Mood rows transcribed literally from crates/avatar-svg/src/mood.rs.

public enum Mood: String, CaseIterable, Equatable {
    case neutral
    case working
    case happy
    case waiting
    case blocked
    case sad
    case sleepy
}

public struct MoodDeltas: Equatable {
    public var upper_lid_close: Q32
    public var lower_lid_close: Q32
    public var eye_rotation: Bam16
    public var eyeball_shift_x: Q32
    public var eyeball_shift_y: Q32
    public var brow_rotation: Bam16
    public var brow_shift_y: Q32
    public var mouth_curve: Q32
    public var lower_contour_raise: Q32

    public init(
        upper_lid_close: Q32 = Q32(0), lower_lid_close: Q32 = Q32(0),
        eye_rotation: Bam16 = Bam16(0), eyeball_shift_x: Q32 = Q32(0),
        eyeball_shift_y: Q32 = Q32(0), brow_rotation: Bam16 = Bam16(0),
        brow_shift_y: Q32 = Q32(0), mouth_curve: Q32 = Q32(0),
        lower_contour_raise: Q32 = Q32(0)
    ) {
        self.upper_lid_close = upper_lid_close
        self.lower_lid_close = lower_lid_close
        self.eye_rotation = eye_rotation
        self.eyeball_shift_x = eyeball_shift_x
        self.eyeball_shift_y = eyeball_shift_y
        self.brow_rotation = brow_rotation
        self.brow_shift_y = brow_shift_y
        self.mouth_curve = mouth_curve
        self.lower_contour_raise = lower_contour_raise
    }

    func scaled(_ response: MoodResponse) -> MoodDeltas {
        MoodDeltas(
            upper_lid_close: Q32(q32Mul(upper_lid_close.value, response.lid_reactivity.value)),
            lower_lid_close: Q32(q32Mul(lower_lid_close.value, response.lid_reactivity.value)),
            eye_rotation: scaleBam(eye_rotation, response.eye_rotation_reactivity),
            eyeball_shift_x: Q32(q32Mul(eyeball_shift_x.value, response.eye_rotation_reactivity.value)),
            eyeball_shift_y: Q32(q32Mul(eyeball_shift_y.value, response.eye_rotation_reactivity.value)),
            brow_rotation: scaleBam(brow_rotation, response.brow_reactivity),
            brow_shift_y: Q32(q32Mul(brow_shift_y.value, response.brow_reactivity.value)),
            mouth_curve: Q32(q32Mul(mouth_curve.value, response.mouth_reactivity.value)),
            lower_contour_raise: Q32(q32Mul(lower_contour_raise.value, response.lid_reactivity.value)))
    }
}

public func moodDeltas(_ mood: Mood) -> MoodDeltas {
    switch mood {
    case .neutral:
        return MoodDeltas()
    case .working:
        return MoodDeltas(
            upper_lid_close: Q32(644_245_094),
            eyeball_shift_x: Q32(171_798_692))
    case .happy:
        return MoodDeltas(
            upper_lid_close: Q32(858_993_459),
            lower_lid_close: Q32(1_431_655_765),
            eye_rotation: Bam16(1_183),
            brow_rotation: Bam16(64_444),
            brow_shift_y: Q32(-171_798_692),
            mouth_curve: Q32(4_294_967_296),
            lower_contour_raise: Q32(1_073_741_824))
    case .waiting:
        return MoodDeltas(
            upper_lid_close: Q32(-429_496_730),
            lower_lid_close: Q32(-429_496_730),
            eyeball_shift_y: Q32(-171_798_692),
            brow_shift_y: Q32(-257_698_038))
    case .blocked:
        return MoodDeltas(
            upper_lid_close: Q32(2_386_092_942),
            lower_lid_close: Q32(1_717_986_918),
            eye_rotation: Bam16(3_186),
            eyeball_shift_y: Q32(515_396_076),
            brow_rotation: Bam16(6_372),
            mouth_curve: Q32(-3_006_477_107))
    case .sad:
        return MoodDeltas(
            upper_lid_close: Q32(2_147_483_648),
            lower_lid_close: Q32(536_870_912),
            eye_rotation: Bam16(63_898),
            eyeball_shift_x: Q32(171_798_692),
            eyeball_shift_y: Q32(257_698_038),
            brow_rotation: Bam16(63_716),
            mouth_curve: Q32(-3_435_973_837))
    case .sleepy:
        return MoodDeltas(
            upper_lid_close: Q32(3_650_722_202),
            lower_lid_close: Q32(2_147_483_648),
            eyeball_shift_y: Q32(-515_396_076))
    }
}

private func scaleBam(_ angle: Bam16, _ reactivity: Q32) -> Bam16 {
    let signed = Int64(Int16(bitPattern: angle.value))
    return Bam16(UInt16(bitPattern: Int16(truncatingIfNeeded: q32Mul(signed, reactivity.value))))
}
