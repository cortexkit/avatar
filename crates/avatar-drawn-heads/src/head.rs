//! The head's outline, its neck, its ears and where the features sit.

use crate::genes::{dna, Genes};
use crate::geom::{
    clamp, curve, half_width_at, lerp, max, min, pt, ray_hit, sample, sharp, up_dir, Contour, Pt,
};
use crate::hair::HairStyle;

/// One head shape. Widths are shares of the head's greatest half-width,
/// heights shares of its height from the top; `long` is the range of height
/// over width, and `tx`, `ty` place the shoulder of the cranium.
struct HeadKind {
    name: &'static str,
    w: u32,
    long: [f64; 2],
    cw: f64,
    yc: f64,
    kw: f64,
    yk: f64,
    jw: f64,
    yj: f64,
    nw: f64,
    yn: f64,
    tx: f64,
    ty: f64,
}

macro_rules! head_kind {
    ($name:literal, $w:literal, [$l0:literal, $l1:literal], $cw:literal, $yc:literal, $kw:literal, $yk:literal, $jw:literal, $yj:literal, $nw:literal, $yn:literal, $tx:literal, $ty:literal) => {
        HeadKind {
            name: $name,
            w: $w,
            long: [$l0, $l1],
            cw: $cw,
            yc: $yc,
            kw: $kw,
            yk: $yk,
            jw: $jw,
            yj: $yj,
            nw: $nw,
            yn: $yn,
            tx: $tx,
            ty: $ty,
        }
    };
}

#[rustfmt::skip]
const HEAD_KINDS: [HeadKind; 16] = [
    head_kind!("oval", 3, [1.2, 1.44], 0.95, 0.34, 1.0, 0.54, 0.8, 0.8, 0.4, 0.955, 0.62, 0.21),
    head_kind!("round", 3, [0.98, 1.14], 0.97, 0.38, 1.0, 0.56, 0.84, 0.8, 0.46, 0.96, 0.62, 0.21),
    head_kind!("long", 2, [1.42, 1.66], 0.96, 0.3, 1.0, 0.52, 0.86, 0.82, 0.46, 0.96, 0.62, 0.21),
    head_kind!("square", 2, [1.08, 1.24], 0.98, 0.26, 1.0, 0.55, 0.96, 0.84, 0.62, 0.975, 0.8, 0.12),
    head_kind!("heart", 2, [1.12, 1.28], 1.0, 0.3, 0.93, 0.54, 0.62, 0.8, 0.22, 0.95, 0.64, 0.2),
    head_kind!("pear", 2, [1.12, 1.3], 0.72, 0.28, 0.9, 0.58, 1.0, 0.8, 0.58, 0.97, 0.62, 0.21),
    head_kind!("diamond", 2, [1.18, 1.34], 0.72, 0.24, 1.0, 0.5, 0.66, 0.78, 0.28, 0.95, 0.6, 0.22),
    head_kind!("egg", 2, [1.16, 1.32], 1.0, 0.34, 0.94, 0.58, 0.7, 0.82, 0.36, 0.96, 0.62, 0.21),
    head_kind!("bulb", 2, [1.14, 1.3], 1.0, 0.3, 0.74, 0.6, 0.52, 0.82, 0.3, 0.965, 0.62, 0.21),
    head_kind!("block", 2, [1.24, 1.52], 0.98, 0.2, 1.0, 0.5, 0.98, 0.86, 0.72, 0.98, 0.86, 0.1),
    head_kind!("trapezoid", 2, [1.1, 1.26], 0.74, 0.2, 0.88, 0.52, 1.0, 0.84, 0.7, 0.98, 0.8, 0.12),
    head_kind!("lantern", 2, [1.26, 1.42], 0.88, 0.28, 0.92, 0.52, 1.0, 0.76, 0.56, 0.97, 0.62, 0.21),
    head_kind!("peanut", 2, [1.24, 1.4], 0.94, 0.24, 0.76, 0.48, 1.0, 0.74, 0.5, 0.965, 0.62, 0.21),
    head_kind!("bun", 2, [0.8, 0.98], 0.9, 0.4, 1.0, 0.6, 0.88, 0.82, 0.54, 0.97, 0.64, 0.2),
    head_kind!("shield", 1, [1.14, 1.3], 1.0, 0.16, 0.92, 0.5, 0.72, 0.8, 0.4, 0.96, 0.84, 0.1),
    head_kind!("gumdrop", 1, [1.1, 1.26], 0.62, 0.22, 0.92, 0.56, 1.0, 0.82, 0.62, 0.975, 0.5, 0.3),
];

/// The outline of a head. While a head is built its chin is at (0, 0), its
/// axis of symmetry is x = 0 and the top of its skull is at y = -H.
#[derive(Clone, Debug)]
pub struct Head {
    pub kind: &'static str,
    pub h: f64,
    /// The widest the outline reaches from the axis.
    pub w: f64,
    pub size: f64,
    /// The twelve points the outline runs through.
    pub pts: Vec<Pt>,
    /// The outline as a polyline, eight points to each stretch.
    pub poly: Vec<Pt>,
    pub outline: Contour,
    /// The middle of the head, from which hair and beard are laid out by angle.
    pub cy: f64,
    /// Where round the outline the pen pressed hardest.
    pub heavy: f64,
    /// How far the outline is from the middle, every three degrees.
    table: Vec<f64>,
}

impl Head {
    /// The half-width of the outline at height `y`.
    pub fn hw(&self, y: f64) -> f64 {
        half_width_at(&self.poly, y)
    }

    /// How far the outline is from the middle at an angle (0 up, 90 right).
    pub fn rim(&self, deg: f64) -> f64 {
        let mut v = (deg + 180.0) / 3.0;
        v = ((v % 120.0) + 120.0) % 120.0;
        let i = v.floor() as usize;
        let f = v - i as f64;
        self.table[i] + (self.table[i + 1] - self.table[i]) * f
    }

    /// A point given by its angle, `out` units outside the outline.
    pub fn at(&self, deg: f64, out: f64) -> Pt {
        self.at_depth(deg, out, 0.0)
    }

    /// A point given by its angle: `out` units outside the outline, or a
    /// share `depth` of the way in from it.
    pub fn at_depth(&self, deg: f64, out: f64, depth: f64) -> Pt {
        let u = up_dir(deg);
        let r = self.rim(deg) * (1.0 - depth) + out;
        pt(u.0 * r, self.cy + u.1 * r)
    }

    /// The angle at which the outline at height `y` is seen, on the right side.
    pub fn angle_at(&self, y: f64) -> f64 {
        let mut lo = 0.0;
        let mut hi = 180.0;
        for _ in 0..14 {
            let m = (lo + hi) / 2.0;
            if self.at(m, 0.0).y < y {
                lo = m;
            } else {
                hi = m;
            }
        }
        (lo + hi) / 2.0
    }
}

pub fn make_head(g: &Genes) -> Head {
    let o = dna::HEAD;
    let k = &HEAD_KINDS[g.pick(o, 0, &HEAD_KINDS.map(|k| k.w))];
    let size = g.lin(o, 5, 3, 0.72, 1.0);
    let long = lerp(k.long[0], k.long[1], g.unit(o + 1, 0, 3));
    let jaw = g.signed(o + 1, 3, 3);
    let cheek = g.signed(o + 1, 6, 2);
    let chin = g.signed(o + 2, 0, 3);
    let crown = g.signed(o + 2, 3, 3);
    let flat = g.signed(o + 2, 6, 2);
    let mut cw = k.cw * (1.0 + 0.06 * crown);
    let mut kw = k.kw * (1.0 + 0.035 * cheek);
    let mut jw = k.jw * (1.0 + 0.11 * jaw);
    let mut nw = k.nw * (1.0 + 0.3 * chin);
    nw = min(nw, jw * 0.86);
    let widest = max(max(cw, kw), jw);
    cw /= widest;
    kw /= widest;
    jw /= widest;
    nw /= widest;
    let h = 178.0 * size * (long / 1.25).sqrt();
    let w = h / (2.0 * long);
    let at = |x: f64, y: f64| pt(x * w, -h + y * h);
    let half = [
        pt(0.0, -h),
        at(
            cw * clamp(k.tx + 0.05 * flat, 0.4, 0.9),
            k.ty * k.yc * (1.0 - 0.22 * flat),
        ),
        at(cw, k.yc),
        at(kw, k.yk),
        at(jw, k.yj + 0.02 * jaw),
        at(nw, k.yn - 0.008 * chin),
        pt(0.0, 0.0),
    ];
    let mut pts = half.to_vec();
    pts.extend(half[1..6].iter().rev().map(|p| pt(-p.x, p.y)));
    let poly = sample(&pts, true, 8);
    let outline = curve(&pts, true).expect("a head outline has points");
    let cy = -h * 0.5;
    let heavy = if g.flag(o + 3, 0) { 0.36 } else { 0.64 };
    let mut widest_x: f64 = 0.0;
    for p in &poly {
        widest_x = max(widest_x, p.x.abs());
    }
    let table = (0..=120)
        .map(|k| ray_hit(&poly, 0.0, cy, up_dir(f64::from(k) * 3.0 - 180.0)))
        .collect();
    Head {
        kind: k.name,
        h,
        w: widest_x,
        size,
        pts,
        poly,
        outline,
        cy,
        heavy,
        table,
    }
}

/// The neck stands still while the head moves on it.
#[derive(Clone, Debug)]
pub struct Neck {
    pub w: f64,
    pub len: f64,
    pub yt: f64,
    pub pts: Vec<Pt>,
}

impl Neck {
    /// The line down one side of the neck.
    pub fn side(&self, sd: f64) -> [Pt; 3] {
        [
            pt(sd * self.w * 0.95, self.yt),
            pt(sd * self.w, self.len * 0.4),
            pt(sd * self.w * 1.1, self.len * 0.9),
        ]
    }
}

pub fn make_neck(g: &Genes, hd: &Head, style: HairStyle) -> Option<Neck> {
    let o = dna::NECK;
    // Hair that hangs below the chin hangs behind a neck; under a head with
    // none it would run on under the chin.
    let hangs = matches!(
        style,
        HairStyle::Bob | HairStyle::Long | HairStyle::Wavy | HairStyle::Mullet | HairStyle::Dreads
    );
    if !hangs && g.pick(o, 0, &[1, 7]) == 0 {
        return None;
    }
    let wide = g.unit(o, 3, 3);
    let tall = g.unit(o, 6, 2);
    let w = min(
        hd.hw(-hd.h * 0.1) * lerp(0.42, 0.7, wide),
        lerp(21.0, 33.0, wide) * hd.size,
    );
    let len = lerp(11.0, 40.0, tall) * hd.size;
    let yt = -hd.h * 0.17;
    let pts = vec![
        sharp(-w * 0.95, yt),
        sharp(w * 0.95, yt),
        pt(w, len * 0.4),
        sharp(w * 1.14, len),
        pt(0.0, len + 3.0),
        sharp(-w * 1.14, len),
        pt(-w, len * 0.4),
    ];
    Some(Neck { w, len, yt, pts })
}

/// Where the eyes, the base of the nose and the mouth sit.
#[derive(Clone, Copy, Debug)]
pub struct FaceLayout {
    pub eye_y: f64,
    pub mouth_y: f64,
    pub nose_y: f64,
}

pub fn layout_face(g: &Genes, hd: &Head) -> FaceLayout {
    let o = dna::FACE;
    let h = hd.h;
    let eye_y = -h + h * g.lin(o, 0, 3, 0.44, 0.6);
    let mut my = eye_y * (1.0 - g.lin(o, 3, 3, 0.52, 0.7));
    // A chin too narrow for a mouth pushes the mouth up.
    while hd.hw(my) < 24.0 && my > eye_y * 0.5 {
        my -= 2.0;
    }
    let nose_y = eye_y + (my - eye_y) * g.lin(o, 6, 2, 0.5, 0.72);
    FaceLayout {
        eye_y,
        mouth_y: my,
        nose_y,
    }
}

pub const EAR_KINDS: [&str; 7] = ["round", "jug", "pointed", "lobed", "small", "out", "square"];

/// The ears: closed shapes that start and end inside the head and are drawn
/// before it, so that the head's own outline finishes them.
#[derive(Clone, Debug)]
pub struct Ears {
    pub kind: &'static str,
    pub eh: f64,
    pub ew: f64,
    pub yc: f64,
    /// Which ear sits higher: -1, 1, or 0 for neither.
    pub odd: f64,
}

/// One ear's outline and the curl drawn in it.
pub struct EarShape {
    pub pts: Vec<Pt>,
    pub curl: Option<Vec<Pt>>,
}

pub fn make_ears(g: &Genes, hd: &Head, f: &FaceLayout) -> Ears {
    let o = dna::EARS;
    let kind = EAR_KINDS[g.pick(o, 0, &[4, 2, 3, 2, 2, 2, 1])];
    let size = g.unit(o, 4, 2);
    let low = g.unit(o, 6, 2);
    let stick = g.unit(o + 1, 0, 2);
    let odd = if g.flag(o + 1, 2) {
        g.side(o + 1, 3)
    } else {
        0.0
    };
    let small = kind == "small";
    let eh = hd.h
        * if small {
            lerp(0.1, 0.13, size)
        } else {
            lerp(0.14, 0.27, size)
        };
    let ew = (if kind == "jug" {
        lerp(16.0, 26.0, stick)
    } else if small {
        lerp(5.5, 8.0, stick)
    } else if kind == "out" {
        lerp(12.0, 19.0, stick)
    } else {
        lerp(8.0, 18.0, stick)
    }) * hd.size;
    // Between the brow and the base of the nose, where ears are.
    let yc = lerp(f.eye_y - eh * 0.05, f.eye_y + eh * 0.42, low);
    Ears {
        kind,
        eh,
        ew,
        yc,
        odd,
    }
}

impl Ears {
    pub fn shape(&self, hd: &Head, sd: f64) -> EarShape {
        let (kind, eh, ew) = (self.kind, self.eh, self.ew);
        let y0 = self.yc + if self.odd == sd { -hd.h * 0.022 } else { 0.0 };
        let yt = y0 - eh / 2.0;
        let yb = y0 + eh / 2.0;
        let x0 = hd.hw(y0);
        let xi = |v: f64| min(hd.hw(v), x0 + 2.0) - 5.0;
        let mut pts = match kind {
            "pointed" => vec![
                pt(xi(yt + 3.0), yt + 3.0),
                pt(x0 + ew * 0.42, yt - eh * 0.2),
                sharp(x0 + ew * 1.12, yt - eh * 0.5),
                pt(x0 + ew * 0.92, y0 - eh * 0.02),
                pt(x0 + ew * 0.4, yb - 1.0),
                pt(xi(yb), yb),
            ],
            "out" => vec![
                pt(xi(yt + 2.0), yt + 2.0),
                pt(x0 + ew * 0.7, yt - eh * 0.06),
                sharp(x0 + ew * 1.75, y0 - eh * 0.34),
                pt(x0 + ew * 1.0, y0 + eh * 0.16),
                pt(x0 + ew * 0.36, yb - 1.0),
                pt(xi(yb), yb),
            ],
            "lobed" => vec![
                pt(xi(yt), yt),
                pt(x0 + ew * 0.72, yt - eh * 0.1),
                pt(x0 + ew, y0 - eh * 0.16),
                pt(x0 + ew * 0.7, y0 + eh * 0.22),
                pt(x0 + ew * 0.72, yb + eh * 0.12),
                pt(x0 + ew * 0.3, yb + eh * 0.2),
                pt(xi(yb), yb - 1.0),
            ],
            "square" => vec![
                pt(xi(yt), yt),
                pt(x0 + ew * 0.86, yt - eh * 0.06),
                pt(x0 + ew, yt + eh * 0.16),
                pt(x0 + ew * 0.96, yb - eh * 0.2),
                pt(x0 + ew * 0.7, yb),
                pt(xi(yb), yb),
            ],
            "jug" => vec![
                pt(xi(yt + 2.0), yt + 2.0),
                pt(x0 + ew * 0.62, yt - eh * 0.16),
                pt(x0 + ew, yt + eh * 0.12),
                pt(x0 + ew * 0.9, y0 + eh * 0.12),
                pt(x0 + ew * 0.44, yb - 1.0),
                pt(xi(yb), yb),
            ],
            _ => vec![
                pt(xi(yt), yt),
                pt(x0 + ew * 0.72, yt - eh * 0.08),
                pt(x0 + ew, y0 - eh * 0.12),
                pt(x0 + ew * 0.78, y0 + eh * 0.3),
                pt(xi(yb), yb),
            ],
        };
        // Close the shape inside the head, where the head's fill covers it.
        pts.push(pt(x0 - 12.0, y0));
        let curl = match kind {
            "small" => None,
            "out" => Some(vec![
                pt(x0 + ew * 0.3, y0 - eh * 0.18),
                pt(x0 + ew * 0.95, y0 - eh * 0.16),
                pt(x0 + ew * 0.5, y0 + eh * 0.12),
            ]),
            _ => Some(vec![
                pt(x0 + ew * 0.22, y0 - eh * 0.26),
                pt(
                    x0 + ew * 0.62,
                    y0 - eh * if kind == "pointed" { 0.3 } else { 0.16 },
                ),
                pt(x0 + ew * 0.5, y0 + eh * 0.14),
                pt(x0 + ew * 0.2, y0 + eh * 0.2),
            ]),
        };
        let flip = |p: &Pt| Pt { x: sd * p.x, ..*p };
        EarShape {
            pts: pts.iter().map(flip).collect(),
            curl: curl.map(|c| c.iter().map(flip).collect()),
        }
    }
}
