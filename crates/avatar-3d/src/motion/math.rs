//! Vectors, quaternions and matrices for posing, built from the exact
//! arithmetic operations and the portable sine, cosine and arctangent of
//! [`avatar_svg::wild::math`], so a pose is the same on every platform.
//!
//! Quaternions are `[x, y, z, w]`. Matrices are 4×4, column-major, as the
//! robot's bone matrices are. Euler angles are XYZ: the rotation turns about
//! x, then y, then z, each about the body's own turned axes, which is the
//! matrix `Rx · Ry · Rz`.

use avatar_svg::wild::math;

/// A point or direction.
pub type V3 = [f64; 3];
/// A rotation `[x, y, z, w]`, of unit length.
pub type Quat = [f64; 4];
/// A 4×4 matrix, column-major.
pub type M4 = [f64; 16];

/// The identity rotation.
pub const IDENTITY: Quat = [0.0, 0.0, 0.0, 1.0];

/// The identity matrix.
pub const M4_IDENTITY: M4 = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

/// Radians per degree.
pub const DEG: f64 = std::f64::consts::PI / 180.0;

/// `x` held to `[a, b]`.
pub fn clamp(x: f64, a: f64, b: f64) -> f64 {
    if x < a {
        a
    } else if x > b {
        b
    } else {
        x
    }
}

/// The smoothstep of `x` held to `[0, 1]`: `x² (3 − 2x)`.
pub fn smooth(x: f64) -> f64 {
    let x = clamp(x, 0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// `a` moved toward `b` by `t`.
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// An integer hash: lowbias32 of `a` mixed with `b`. Only 32-bit wrapping
/// integer arithmetic, so it gives the same bits in every language. `b` is
/// taken modulo 2³², so a negative slot number hashes as its two's
/// complement.
pub fn hash32(a: u32, b: i64) -> u32 {
    let mixed = (b.wrapping_add(0x9e37_79b9) as u32).wrapping_mul(0x85eb_ca6b);
    let mut x = a ^ mixed;
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// [`hash32`] as a fraction of 2³², in `[0, 1)`.
pub fn hash_unit(a: u32, b: i64) -> f64 {
    f64::from(hash32(a, b)) / 4_294_967_296.0
}

/// The sine of an angle in radians, portable.
pub fn sin(a: f64) -> f64 {
    math::sin(a)
}

/// The cosine of an angle in radians, portable.
pub fn cos(a: f64) -> f64 {
    math::cos(a)
}

fn asin(v: f64) -> f64 {
    let v = clamp(v, -1.0, 1.0);
    math::atan2(v, (1.0 - v * v).sqrt())
}

/// `a + b`.
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `a − b`.
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `a · k`.
pub fn scale(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// The dot product.
pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The cross product `a × b`.
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The length of `a`.
pub fn length(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// `a` scaled to unit length; the zero vector stays zero.
pub fn normalize(a: V3) -> V3 {
    let l = length(a);
    if l == 0.0 {
        a
    } else {
        scale(a, 1.0 / l)
    }
}

/// The distance between two points.
pub fn distance(a: V3, b: V3) -> f64 {
    length(sub(a, b))
}

/// `a` moved toward `b` by `t`, per component.
pub fn lerp3(a: V3, b: V3, t: f64) -> V3 {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

/// The Hamilton product `a · b`: the rotation `b`, then `a`.
pub fn q_mul(a: Quat, b: Quat) -> Quat {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        ax * bw + aw * bx + ay * bz - az * by,
        ay * bw + aw * by + az * bx - ax * bz,
        az * bw + aw * bz + ax * by - ay * bx,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

/// The inverse of a unit quaternion.
pub fn q_invert(q: Quat) -> Quat {
    [-q[0], -q[1], -q[2], q[3]]
}

/// `q` scaled to unit length.
pub fn q_normalize(q: Quat) -> Quat {
    let l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if l == 0.0 {
        IDENTITY
    } else {
        [q[0] / l, q[1] / l, q[2] / l, q[3] / l]
    }
}

/// The rotation of XYZ Euler angles in radians.
pub fn q_from_euler(e: V3) -> Quat {
    let (c1, c2, c3) = (cos(e[0] / 2.0), cos(e[1] / 2.0), cos(e[2] / 2.0));
    let (s1, s2, s3) = (sin(e[0] / 2.0), sin(e[1] / 2.0), sin(e[2] / 2.0));
    [
        s1 * c2 * c3 + c1 * s2 * s3,
        c1 * s2 * c3 - s1 * c2 * s3,
        c1 * c2 * s3 + s1 * s2 * c3,
        c1 * c2 * c3 - s1 * s2 * s3,
    ]
}

/// The XYZ Euler angles in radians of a unit quaternion, with the middle
/// angle in `[−π/2, π/2]`.
pub fn q_to_euler(q: Quat) -> V3 {
    let m = compose([0.0; 3], q, [1.0; 3]);
    let (m11, m12, m13) = (m[0], m[4], m[8]);
    let (m22, m23) = (m[5], m[9]);
    let (m32, m33) = (m[6], m[10]);
    let y = asin(m13);
    if m13.abs() < 0.999_999_9 {
        [math::atan2(-m23, m33), y, math::atan2(-m12, m11)]
    } else {
        [math::atan2(m32, m22), y, 0.0]
    }
}

/// The shortest rotation taking unit direction `a` to unit direction `b`.
pub fn q_from_unit_vectors(a: V3, b: V3) -> Quat {
    let r = dot(a, b) + 1.0;
    let q = if r < f64::EPSILON {
        // Opposite directions: half a turn about any axis across `a`.
        if a[0].abs() > a[2].abs() {
            [-a[1], a[0], 0.0, 0.0]
        } else {
            [0.0, -a[2], a[1], 0.0]
        }
    } else {
        let c = cross(a, b);
        [c[0], c[1], c[2], r]
    };
    q_normalize(q)
}

/// The rotation by `angle` radians about the unit `axis`.
pub fn q_from_axis_angle(axis: V3, angle: f64) -> Quat {
    let s = sin(angle / 2.0);
    [axis[0] * s, axis[1] * s, axis[2] * s, cos(angle / 2.0)]
}

/// `v` turned by the unit quaternion `q`.
pub fn q_rotate(q: Quat, v: V3) -> V3 {
    let [qx, qy, qz, qw] = q;
    let ix = qw * v[0] + qy * v[2] - qz * v[1];
    let iy = qw * v[1] + qz * v[0] - qx * v[2];
    let iz = qw * v[2] + qx * v[1] - qy * v[0];
    let iw = -qx * v[0] - qy * v[1] - qz * v[2];
    [
        ix * qw + iw * -qx + iy * -qz - iz * -qy,
        iy * qw + iw * -qy + iz * -qx - ix * -qz,
        iz * qw + iw * -qz + ix * -qy - iy * -qx,
    ]
}

/// The spherical blend from `a` toward `b` by `t` in `[0, 1]`, along the
/// shorter arc.
pub fn q_slerp(a: Quat, b: Quat, t: f64) -> Quat {
    if t == 0.0 {
        return a;
    }
    if t == 1.0 {
        return b;
    }
    let mut cos_half = a[3] * b[3] + a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let b = if cos_half < 0.0 {
        cos_half = -cos_half;
        [-b[0], -b[1], -b[2], -b[3]]
    } else {
        b
    };
    if cos_half >= 1.0 {
        return a;
    }
    let sqr_sin_half = 1.0 - cos_half * cos_half;
    if sqr_sin_half <= f64::EPSILON {
        let s = 1.0 - t;
        return q_normalize([0, 1, 2, 3].map(|k| s * a[k] + t * b[k]));
    }
    let sin_half = sqr_sin_half.sqrt();
    let half = math::atan2(sin_half, cos_half);
    let ra = sin((1.0 - t) * half) / sin_half;
    let rb = sin(t * half) / sin_half;
    [0, 1, 2, 3].map(|k| a[k] * ra + b[k] * rb)
}

/// The matrix of a translation `t`, a rotation `q` and a scale `s`, applied
/// to a point as scale, then rotation, then translation.
pub fn compose(t: V3, q: Quat, s: V3) -> M4 {
    let [x, y, z, w] = q;
    let (x2, y2, z2) = (x + x, y + y, z + z);
    let (xx, xy, xz) = (x * x2, x * y2, x * z2);
    let (yy, yz, zz) = (y * y2, y * z2, z * z2);
    let (wx, wy, wz) = (w * x2, w * y2, w * z2);
    [
        (1.0 - (yy + zz)) * s[0],
        (xy + wz) * s[0],
        (xz - wy) * s[0],
        0.0,
        (xy - wz) * s[1],
        (1.0 - (xx + zz)) * s[1],
        (yz + wx) * s[1],
        0.0,
        (xz + wy) * s[2],
        (yz - wx) * s[2],
        (1.0 - (xx + yy)) * s[2],
        0.0,
        t[0],
        t[1],
        t[2],
        1.0,
    ]
}

/// The product `a · b` of column-major matrices.
pub fn m_mul(a: &M4, b: &M4) -> M4 {
    let mut out = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            out[c * 4 + r] = a[r] * b[c * 4]
                + a[4 + r] * b[c * 4 + 1]
                + a[8 + r] * b[c * 4 + 2]
                + a[12 + r] * b[c * 4 + 3];
        }
    }
    out
}

/// The point `p` carried by `m`.
pub fn m_apply(m: &M4, p: V3) -> V3 {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

/// The height (y) of the point `p` carried by `m`.
pub fn m_height(m: &M4, p: V3) -> f64 {
    m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13]
}
