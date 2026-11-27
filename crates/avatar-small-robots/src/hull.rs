//! Hulls: the outlines of heads and chassis modules, and how much room they
//! have at each height.

use crate::geom::{box_of, cp, half_width_at, poly_sample, quad, shift_pts, top_at, Cp};
use crate::kit::{lerp, max, min, P2};

/// The hull shapes a head can have, in the order its gene picks them.
pub const HULL_NAMES: [&str; 15] = [
    "box", "soft", "capsule", "dome", "cup", "bucket", "pyramid", "octagon", "hexagon", "ball",
    "egg", "acorn", "tee", "shield", "can",
];

/// How tall each kind of head may be for its full width, least and most.
pub(crate) fn hull_aspect(kind: &str) -> [f64; 2] {
    match kind {
        "box" => [0.62, 1.06],
        "soft" => [0.7, 1.02],
        "capsule" => [0.52, 0.72],
        "dome" => [0.7, 1.1],
        "cup" => [0.66, 0.96],
        "bucket" => [0.66, 1.02],
        "pyramid" => [0.66, 1.02],
        "octagon" => [0.7, 1.02],
        "hexagon" => [0.6, 0.84],
        "ball" => [0.8, 1.04],
        "egg" => [0.9, 1.16],
        "acorn" => [0.84, 1.08],
        "tee" => [0.8, 1.06],
        "shield" => [0.84, 1.08],
        _ => [0.76, 1.1],
    }
}

/// The outline of a hull of one kind, `w` wide to each side and `h` tall,
/// standing on y = 0; `a` and `b` (0..1) vary the shape within its kind.
fn hull_pts(kind: &str, w: f64, h: f64, a: f64, b: f64) -> Vec<Cp> {
    let q = |wb: f64, wt: f64, bx: f64, by: f64, tx: f64, ty: f64| {
        quad(-h, 0.0, wb, wt, bx, by, tx, ty)
    };
    match kind {
        "soft" => {
            let k = lerp(0.36, 0.5, a);
            let t = lerp(0.9, 1.0, b);
            q(w * t, w, w * k, h * k, w * k, h * k)
        }
        "capsule" => q(w, w, h / 2.0, h / 2.0, h / 2.0, h / 2.0),
        "dome" => {
            let rb = lerp(4.0, 12.0, b);
            q(w, w, rb, rb, w, min(h - rb - 4.0, w * lerp(0.8, 1.15, a)))
        }
        "cup" => {
            let rt = lerp(5.0, 12.0, b);
            q(
                w * 0.9,
                w,
                w * 0.9,
                min(h - rt - 4.0, h * lerp(0.5, 0.7, a)),
                rt,
                rt,
            )
        }
        "bucket" => {
            let r = lerp(6.0, 16.0, b);
            q(w * lerp(0.62, 0.8, a), w, r, r, r, r)
        }
        "pyramid" => {
            let r = lerp(6.0, 16.0, b);
            q(w, w * lerp(0.6, 0.8, a), r, r, r, r)
        }
        "octagon" => {
            let c = lerp(0.24, 0.4, a) * min(w, h / 2.0);
            let r = lerp(1.5, 6.0, b);
            vec![
                cp(-w + c, 0.0, r),
                cp(-w, -c, r),
                cp(-w, -h + c, r),
                cp(-w + c, -h, r),
                cp(w - c, -h, r),
                cp(w, -h + c, r),
                cp(w, -c, r),
                cp(w - c, 0.0, r),
            ]
        }
        "hexagon" => {
            let c = w * lerp(0.3, 0.44, a);
            let r = lerp(3.0, 10.0, b);
            vec![
                cp(-w + c, 0.0, r),
                cp(-w, -h / 2.0, r),
                cp(-w + c, -h, r),
                cp(w - c, -h, r),
                cp(w, -h / 2.0, r),
                cp(w - c, 0.0, r),
            ]
        }
        "ball" => q(w, w, w, h / 2.0, w, h / 2.0),
        "egg" => {
            let wt = w * lerp(0.52, 0.7, a);
            q(w, wt, w, h * lerp(0.36, 0.44, b), wt, h * 0.5)
        }
        "acorn" => {
            let wb = w * lerp(0.52, 0.7, a);
            q(wb, w, wb, h * 0.5, w, h * lerp(0.36, 0.44, b))
        }
        "tee" => {
            let wj = w * lerp(0.58, 0.76, a);
            let hj = h * lerp(0.26, 0.38, b);
            vec![
                cp(-wj, 0.0, 7.0),
                cp(-wj, -hj, 4.0),
                cp(-w, -hj, 7.0),
                cp(-w, -h, 13.0),
                cp(w, -h, 13.0),
                cp(w, -hj, 7.0),
                cp(wj, -hj, 4.0),
                cp(wj, 0.0, 7.0),
            ]
        }
        "shield" => {
            let k = lerp(0.42, 0.56, a);
            let rt = lerp(8.0, 18.0, b);
            vec![
                cp(0.0, 0.0, lerp(12.0, 24.0, b)),
                cp(-w, -h * k, 14.0),
                cp(-w, -h, rt),
                cp(w, -h, rt),
                cp(w, -h * k, 14.0),
            ]
        }
        "can" => {
            let e = lerp(6.0, 10.0, a);
            q(w, w, w, e, w, e)
        }
        _ => {
            let r = lerp(5.0, 20.0, a);
            let t = lerp(-0.06, 0.06, b);
            q(w * (1.0 - max(0.0, t)), w * (1.0 + min(0.0, t)), r, r, r, r)
        }
    }
}

/// A head or a chassis module: its outline standing on y = 0, measured on
/// the outline itself, with its half-width tabled every half unit of height
/// and its top and underside every half unit across.
#[derive(Clone, Debug)]
pub(crate) struct Hull {
    pub(crate) kind: &'static str,
    pub(crate) w: f64,
    pub(crate) h: f64,
    pub(crate) a: f64,
    pub(crate) pts: Vec<Cp>,
    pub(crate) poly: Vec<P2>,
    wide: Vec<f64>,
    ny: f64,
    tops: Vec<f64>,
    bots: Vec<f64>,
    nx: f64,
    /// The half-width and height of a tee's narrower lower part.
    pub(crate) jaw_w: f64,
    pub(crate) jaw_h: f64,
}

pub(crate) fn make_hull(kind: &'static str, w0: f64, h0: f64, a: f64, b: f64) -> Hull {
    let mut pts = hull_pts(kind, w0, h0, a, b);
    let mut poly = poly_sample(&pts, 8);
    // A rounded corner does not reach the corner it rounds off: the outline
    // is moved so that its lowest point stands on y = 0.
    let bx = box_of(&poly);
    if bx[3] < -0.05 {
        pts = shift_pts(&pts, 0.0, -bx[3]);
        poly = poly.iter().map(|p| [p[0], p[1] - bx[3]]).collect();
    }
    let h = bx[3] - bx[1];
    let w = bx[2];
    let ny = (h * 2.0).ceil();
    let nx = (w * 2.0).ceil();
    let wide = (0..=ny as usize)
        .map(|i| half_width_at(&poly, -min(i as f64 / 2.0, h)))
        .collect();
    let flipped: Vec<P2> = poly.iter().map(|p| [p[0], -p[1]]).collect();
    let mut tops = Vec::new();
    let mut bots = Vec::new();
    for i in 0..=nx as usize {
        let x = min(i as f64 / 2.0, w - 0.01);
        tops.push(top_at(&poly, x));
        bots.push(-top_at(&flipped, x));
    }
    let (jaw_w, jaw_h) = if kind == "tee" {
        (w0 * lerp(0.58, 0.76, a), h0 * lerp(0.26, 0.38, b))
    } else {
        (0.0, 0.0)
    };
    Hull {
        kind,
        w,
        h,
        a,
        pts,
        poly,
        wide,
        ny,
        tops,
        bots,
        nx,
        jaw_w,
        jaw_h,
    }
}

impl Hull {
    /// The half-width at height `y` (negative above the base), 0 off the hull.
    pub(crate) fn hw_at(&self, y: f64) -> f64 {
        let v = -y * 2.0;
        if v.is_nan() || v < 0.0 || v > self.ny {
            return 0.0;
        }
        let i = v.floor();
        let f = v - i;
        let i = i as usize;
        let j = min(self.ny, (i + 1) as f64) as usize;
        self.wide[i] + (self.wide[j] - self.wide[i]) * f
    }

    fn across(&self, table: &[f64], x: f64) -> f64 {
        let v = min(x.abs() * 2.0, self.nx);
        let i = v.floor();
        let f = v - i;
        let i = i as usize;
        let j = min(self.nx, (i + 1) as f64) as usize;
        table[i] + (table[j] - table[i]) * f
    }

    /// The top of the outline at `x`.
    pub(crate) fn top_at(&self, x: f64) -> f64 {
        self.across(&self.tops, x)
    }

    /// The underside of the outline at `x`.
    pub(crate) fn bot_at(&self, x: f64) -> f64 {
        self.across(&self.bots, x)
    }
}

/// Where a face goes: the middle height, half-width and half-height of the
/// largest upright box the hull has room for, `margin` inside its outline
/// and at least `above` over its base.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FaceBox {
    pub(crate) cy: f64,
    pub(crate) fw: f64,
    pub(crate) fh: f64,
}

pub(crate) fn face_box_of(hull: &Hull, margin: f64, above: f64) -> FaceBox {
    let h = hull.h;
    let n = 30_i64;
    let nf = n as f64;
    let ws: Vec<f64> = (0..=n).map(|i| hull.hw_at(-h * i as f64 / nf)).collect();
    let mut best: Option<(f64, FaceBox)> = None;
    let start = max(2.0, (above / h * nf).ceil()) as i64;
    let mut i = start;
    while i < n - 5 {
        for j in i + 4..=n - 2 {
            let mut w = f64::INFINITY;
            for k in i..=j {
                if ws[k as usize] < w {
                    w = ws[k as usize];
                }
            }
            let fw = w - margin;
            let fh = h * (j - i) as f64 / nf / 2.0;
            if fw < 8.0 {
                continue;
            }
            // A face is at most 2.2 times as wide as tall and at most 1.15
            // times as tall as wide.
            let uw = min(fw, fh * 2.2);
            let uh = min(fh, uw * 1.15);
            let score = uw * uh * (1.0 - 0.3 * ((i + j) as f64 / (2.0 * nf) - 0.5).abs());
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((
                    score,
                    FaceBox {
                        cy: -h * (i + j) as f64 / nf / 2.0,
                        fw: uw,
                        fh: uh,
                    },
                ));
            }
        }
        i += 1;
    }
    best.map(|(_, b)| b).unwrap_or(FaceBox {
        cy: -h / 2.0,
        fw: hull.w * 0.6,
        fh: h * 0.3,
    })
}
