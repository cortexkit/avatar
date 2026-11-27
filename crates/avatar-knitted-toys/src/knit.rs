//! Knitting: the rows and stitches laid over each knitted part.
//!
//! Pieces are crocheted in rounds from the crown down, so their rows are the
//! fronts of horizontal rings seen a little from above. Tubes are crocheted
//! from the tip, so their rounds run across the spine and bulge towards the
//! tip. Snouts, soles and bellies are crocheted in rounds from the middle
//! out. All rounds of a part are laid on one set of columns; near a crown or
//! a tip a round keeps half the columns once its radius is under half the
//! widest round's, and a quarter under a quarter. A stitch is a V: two legs
//! from its foot on this row up to the row before.

use crate::build::{Part, Toy};
use crate::dna::{hash_str, Stream};
use crate::geom::{
    add, atan2, clamp, closed_normals, cos, dist, hyp, js_round, lerp, max, max_of, min, mixp,
    resample, scl, sin, sq, sub, unit, Pt, PI, TAU,
};
use crate::palette::Yarn;
use crate::parts::HAND;
use crate::piece::{Piece, Tube};

/// Where a row lies: a function from the angle round the row to a point.
#[derive(Clone, Copy, Debug)]
pub enum RowAt {
    /// One point (a tube's tip, a flat round's middle).
    Point(Pt),
    /// The front of a piece's round: an arc whose middle dips by `sag`.
    Piece { ax: f64, rho: f64, y: f64, sag: f64 },
    /// A tube's round across the spine at `p`.
    Tube {
        p: Pt,
        nn: Pt,
        t: Pt,
        w: f64,
        bulge: f64,
    },
    /// A flat round at share `f` of the outer one.
    Ring { c: Pt, rx: f64, ry: f64, f: f64 },
}

#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub at: RowAt,
    pub n: usize,
    pub cols: f64,
    pub closed: bool,
    pub f: f64,
    pub y: f64,
    pub rho: f64,
    pub w: f64,
}

impl Row {
    pub fn at(&self, phi: f64) -> Pt {
        match self.at {
            RowAt::Point(p) => p,
            RowAt::Piece { ax, rho, y, sag } => [ax + rho * sin(phi), y + sag * cos(phi)],
            RowAt::Tube { p, nn, t, w, bulge } => {
                add(p, add(scl(nn, w * sin(phi)), scl(t, w * bulge * cos(phi))))
            }
            RowAt::Ring { c, rx, ry, f } => [c[0] + rx * f * cos(phi), c[1] + ry * f * sin(phi)],
        }
    }

    fn blank(at: RowAt) -> Self {
        Row {
            at,
            n: 0,
            cols: 0.0,
            closed: false,
            f: 0.0,
            y: 0.0,
            rho: 0.0,
            w: 0.0,
        }
    }
}

/// One stitch: its foot `b` and its legs' ends `l` and `r`, on row `row`.
#[derive(Clone, Copy, Debug)]
pub struct Stitch {
    pub row: usize,
    pub b: Pt,
    pub l: Pt,
    pub r: Pt,
}

fn even_cols(v: f64) -> f64 {
    max(4.0, 2.0 * js_round(v / 2.0))
}

fn col_level(f: f64) -> f64 {
    if f >= 0.5 {
        1.0
    } else if f >= 0.25 {
        2.0
    } else {
        4.0
    }
}

fn piece_rows(p: &Piece, rh: f64, sw: f64, curve: f64) -> Vec<Row> {
    let y0 = p.y_top;
    let y1 = p.y_bot;
    let h = y1 - y0;
    let n = max(4.0, js_round(h / rh)) as usize;
    let mut rows = Vec::with_capacity(n + 1);
    let mut prev_mid = f64::NEG_INFINITY;
    for r in 0..=n {
        let v = r as f64 / n as f64;
        let y = y0 + h * lerp(v, (1.0 - cos(PI * v)) / 2.0, 0.42);
        let rho = if r == 0 {
            0.0
        } else {
            p.half_w(min(y, y1 - 0.05))
        };
        let mut sag = rho * curve;
        // A row's middle stays below the middle of the row before.
        if y + sag < prev_mid + rh * 0.62 {
            sag = prev_mid + rh * 0.62 - y;
        }
        prev_mid = y + sag;
        rows.push(Row {
            y,
            rho,
            ..Row::blank(RowAt::Piece {
                ax: p.c[0],
                rho,
                y,
                sag,
            })
        });
    }
    let rho_max = max_of(rows.iter().map(|r| r.rho));
    let cols = even_cols(PI * rho_max / sw);
    for (r, row) in rows.iter_mut().enumerate() {
        row.n = if r == 0 {
            0
        } else {
            max(2.0, (cols / col_level(row.rho / rho_max)).floor()) as usize
        };
        row.cols = cols;
    }
    rows
}

/// A point along a tube's spine at arc length s, with its tangent, normal
/// and half-width.
fn tube_at(tb: &Tube, cum: &[f64], s: f64) -> (Pt, Pt, Pt, f64) {
    let sp = &tb.spine;
    let n = sp.len();
    let mut i = 0;
    while i + 2 < n && cum[i + 1] < s {
        i += 1;
    }
    let mut den = cum[i + 1] - cum[i];
    if den == 0.0 {
        den = 1.0;
    }
    let u = clamp((s - cum[i]) / den, 0.0, 1.0);
    (
        mixp(sp[i], sp[i + 1], u),
        unit(mixp(tb.tan[i], tb.tan[i + 1], u)),
        unit(mixp(tb.nrm[i], tb.nrm[i + 1], u)),
        lerp(tb.ws[i], tb.ws[i + 1], u),
    )
}

fn spine_cum(sp: &[Pt]) -> Vec<f64> {
    let mut cum = vec![0.0];
    for i in 1..sp.len() {
        cum.push(cum[i - 1] + dist(sp[i - 1], sp[i]));
    }
    cum
}

fn tube_rows(tb: &Tube, rh: f64, sw: f64, bulge: f64) -> Vec<Row> {
    let cum = spine_cum(&tb.spine);
    let total = cum[tb.spine.len() - 1];
    let nr = max(3.0, js_round(total / rh));
    let (tp, tt, _, _) = tube_at(tb, &cum, total);
    let tip_pt = add(
        tp,
        scl(
            tt,
            if tb.tip_w > 0.6 {
                tb.tip_w * 0.85
            } else {
                tb.tip_w * 0.6
            },
        ),
    );
    let mut rows = vec![Row::blank(RowAt::Point(tip_pt))];
    let mut r = 1.0;
    while r <= nr + 1.0 {
        let s = total * (1.0 - (r - 0.6) / nr);
        let (p, t, nn, w) = tube_at(tb, &cum, clamp(s, 0.0, total));
        rows.push(Row {
            w,
            ..Row::blank(RowAt::Tube { p, nn, t, w, bulge })
        });
        r += 1.0;
    }
    let w_max = max_of(rows.iter().map(|r| r.w));
    let ratio = PI * w_max / sw;
    let cols = max(2.0, even_cols(ratio) - if ratio < 3.0 { 2.0 } else { 0.0 });
    for (r, row) in rows.iter_mut().enumerate() {
        row.n = if r == 0 {
            0
        } else {
            max(2.0, (cols / col_level(row.w / w_max)).floor()) as usize
        };
        row.cols = cols;
    }
    rows
}

/// Flat rounds from the middle out, six more stitches each round.
fn ring_rows(c: Pt, rx: f64, ry: f64, rh: f64) -> Vec<Row> {
    let nr = max(2.0, js_round(max(rx, ry) / rh)) as usize;
    let mut rows = vec![Row {
        closed: true,
        ..Row::blank(RowAt::Point(c))
    }];
    for r in 1..=nr {
        let f = (r as f64 - 0.15) / (nr as f64 - 0.15);
        rows.push(Row {
            closed: true,
            f,
            n: 6 * r,
            ..Row::blank(RowAt::Ring { c, rx, ry, f })
        });
    }
    rows
}

fn towards(a: Pt, b: Pt, max_len: f64) -> Pt {
    let d = sub(b, a);
    let l = hyp(d[0], d[1]);
    if l <= max_len {
        b
    } else {
        add(a, scl(d, max_len / l))
    }
}

/// The stitches of one part. `drift` turns the columns a little each round,
/// the spiral a crocheted piece shows.
fn stitches_of(rows: &[Row], rh: f64, drift: f64, rnd: &mut Stream) -> Vec<Stitch> {
    let mut out = Vec::new();
    let max_len = rh * 1.4;
    for r in 1..rows.len() {
        let row = &rows[r];
        let prev = &rows[r - 1];
        let n = row.n;
        if n == 0 {
            continue;
        }
        let span = if row.closed { TAU } else { PI };
        let start = if row.closed { 0.0 } else { -PI / 2.0 };
        let d = span / n as f64;
        let shift = if row.closed {
            0.0
        } else {
            (r as f64 * drift * PI / row.cols) % d
        };
        for j in -1..=n as i64 {
            let phi = start + (j as f64 + 0.5) * d + shift;
            if !row.closed && (phi < -PI / 2.0 - 0.02 || phi > PI / 2.0 + 0.02) {
                continue;
            }
            if row.closed && (j < 0 || j >= n as i64) {
                continue;
            }
            let jit = (rnd.draw() - 0.5) * 0.1;
            let b = row.at(phi + jit * d);
            let l = towards(b, prev.at(phi - d * 0.47), max_len);
            let rr = towards(b, prev.at(phi + d * 0.47), max_len);
            out.push(Stitch {
                row: r,
                b,
                l: mixp(b, l, 0.93),
                r: mixp(b, rr, 0.93),
            });
        }
    }
    out
}

/// Which yarn each row of a part is knitted in: stripes and colour changes
/// happen at whole rows.
fn row_yarns(g: &Toy, part: &Part, n_rows: usize) -> Vec<Yarn> {
    let st = &g.t.stripes;
    let pal = &g.pal;
    let main = part.yarn().clone();
    let mut out = vec![main.clone(); n_rows];
    let second = pal.second.as_ref().unwrap_or(&pal.accent);
    if part.tip_rows != 0.0 {
        let ty = if part.tip_yarn == Some("accent") {
            &pal.accent
        } else {
            second
        };
        let k = max(1.0, js_round(n_rows as f64 * part.tip_rows)) as usize;
        for slot in out.iter_mut().take(k) {
            *slot = ty.clone();
        }
    }
    if let Some(ringed) = &part.ringed {
        for (r, slot) in out.iter_mut().enumerate() {
            if r % 3 == 1 {
                *slot = ringed.clone();
            }
        }
    }
    if st.kind == "none" || part.yarn() != &pal.main {
        return out;
    }
    // Only ears, the tail and wings are striped.
    if !(part.group == "ears"
        || part.id == "tail"
        || (part.group == "arms" && g.t.arms.kind == "wing"))
    {
        return out;
    }
    let rows: Vec<usize> = (0..n_rows)
        .filter(|&r| (r as u32 + st.phase).is_multiple_of(st.period) && out[r] == main)
        .collect();
    // A part shows stripes only with at least three.
    if rows.len() < 3 {
        return out;
    }
    for r in rows {
        out[r] = second.clone();
    }
    out
}

pub fn knit_all(g: &mut Toy) {
    let s = g.fit.s;
    let rh = 6.6 * g.t.gauge / s;
    let sw = rh * 1.15;
    g.rh = rh;
    g.sw = sw;
    for i in 0..g.parts.len() {
        let part = &g.parts[i];
        let mut rnd = Stream::new(hash_str(&part.id, HAND));
        let rows = match part.kind {
            "piece" => piece_rows(
                part.piece.as_ref().expect("a piece part has its piece"),
                rh,
                sw,
                g.t.curve,
            ),
            "tube" => tube_rows(
                part.tube.as_ref().expect("a tube part has its tube"),
                rh,
                sw,
                0.34,
            ),
            "ring" => ring_rows(part.c, part.rx, part.ry, rh),
            _ => continue,
        };
        let rows_y: Vec<f64> = if part.kind == "piece" {
            rows.iter().map(|r| r.y).collect()
        } else {
            Vec::new()
        };
        let yarns = row_yarns(g, part, rows.len());
        let stitches = stitches_of(&rows, rh, g.t.drift, &mut rnd);
        let part = &mut g.parts[i];
        part.rows = rows;
        part.rows_y = rows_y;
        part.yarns = yarns;
        part.stitches = stitches;
        part.edge = scallop_edge(part, rh, sw);
    }
}

fn bump(f: f64) -> f64 {
    4.0 * f * (1.0 - f)
}

fn frac(v: f64) -> f64 {
    v - v.floor()
}

/// The edge of a knitted part as fabric has it: a row of small bumps, one
/// where each round turns away at the side of a piece or a tube, and one for
/// each stitch along a crown, a base or the rim of a flat round.
fn scallop_edge(part: &Part, rh: f64, sw: f64) -> Vec<Pt> {
    let r = resample(&part.outline, rh / 4.5, true);
    let n = closed_normals(&r);
    let lift: Box<dyn Fn(Pt, Pt) -> f64 + '_> = match part.kind {
        "piece" => {
            let ys = &part.rows_y;
            let x0 = part.piece.as_ref().expect("a piece part has its piece").c[0];
            Box::new(move |p: Pt, nrm: Pt| {
                let mut r = 1;
                while r < ys.len() - 1 && ys[r] < p[1] {
                    r += 1;
                }
                let gap = max(1e-6, ys[r] - ys[r - 1]);
                let side = min(gap, rh) * bump(clamp((p[1] - ys[r - 1]) / gap, 0.0, 1.0));
                // Where the edge runs level, the bumps are the stitches'
                // columns instead of the rows' ends.
                0.075
                    * (side * nrm[0] * nrm[0]
                        + sw * 0.8 * bump(frac((p[0] - x0) / sw + 0.5)) * nrm[1] * nrm[1])
            })
        }
        "tube" => {
            let sp = &part.tube.as_ref().expect("a tube part has its tube").spine;
            let cum = spine_cum(sp);
            let total = cum[sp.len() - 1];
            let nr = max(3.0, js_round(total / rh));
            Box::new(move |p: Pt, _| {
                // The point's place along the spine: its projection on the
                // nearest spine segment.
                let mut best = f64::INFINITY;
                let mut s_at = 0.0;
                for i in 1..sp.len() {
                    let a = sp[i - 1];
                    let b = sp[i];
                    let ex = b[0] - a[0];
                    let ey = b[1] - a[1];
                    let mut den = ex * ex + ey * ey;
                    if den == 0.0 {
                        den = 1e-12;
                    }
                    let t = clamp(((p[0] - a[0]) * ex + (p[1] - a[1]) * ey) / den, 0.0, 1.0);
                    let d = sq(p[0] - a[0] - ex * t) + sq(p[1] - a[1] - ey * t);
                    if d < best {
                        best = d;
                        s_at = cum[i - 1] + (cum[i] - cum[i - 1]) * t;
                    }
                }
                0.075 * min(total / nr, rh) * bump(frac(nr * (1.0 - s_at / total) + 0.6))
            })
        }
        _ => {
            let k = 6.0 * (part.rows.len() - 1) as f64;
            let c = part.c;
            let (rx, ry) = (part.rx, part.ry);
            Box::new(move |p: Pt, _| {
                0.065
                    * sw
                    * bump(frac(
                        atan2((p[1] - c[1]) / ry, (p[0] - c[0]) / rx) / TAU * k + 0.5,
                    ))
            })
        }
    };
    r.iter()
        .zip(&n)
        .map(|(&p, &nrm)| add(p, scl(nrm, lift(p, nrm))))
        .collect()
}

/// The area of a run of rows [r0, r1], as a polygon reaching past the
/// part's outline (it is drawn clipped to the outline).
pub fn band_poly(part: &Part, r0: usize, r1: usize) -> Vec<Pt> {
    let rows = &part.rows;
    let a = rows[r0.saturating_sub(1)];
    let b = rows[r1.min(rows.len() - 1)];
    const N: usize = 24;
    let mut pts = Vec::new();
    if part.kind == "ring" {
        let fa = if r0 == 0 { 0.0 } else { rows[r0 - 1].f };
        let fb = rows[r1].f;
        for i in 0..=N * 2 {
            let ang = i as f64 / (N * 2) as f64 * TAU;
            pts.push([
                part.c[0] + part.rx * (fb + 0.05) * cos(ang),
                part.c[1] + part.ry * (fb + 0.05) * sin(ang),
            ]);
        }
        if fa > 0.0 {
            for i in (0..=N * 2).rev() {
                let ang = i as f64 / (N * 2) as f64 * TAU;
                pts.push([
                    part.c[0] + part.rx * fa * cos(ang),
                    part.c[1] + part.ry * fa * sin(ang),
                ]);
            }
        }
        return pts;
    }
    // A row's ends are carried on past the outline along its chord.
    let ext = |row: &Row, sgn: f64, k: f64| {
        let pa = row.at(-PI / 2.0);
        let pb = row.at(PI / 2.0);
        let c = unit(sub(pb, pa));
        add(if sgn > 0.0 { pb } else { pa }, scl(c, sgn * k))
    };
    let far = 40.0;
    if r0 == 0 {
        // From the very first row: everything up to row r1, past the tip.
        for i in 0..=N {
            pts.push(b.at(-PI / 2.0 + i as f64 / N as f64 * PI));
        }
        let e1 = ext(&b, 1.0, far);
        let e0 = ext(&b, -1.0, far);
        let tip = a.at(0.0);
        let mid = b.at(0.0);
        let away = scl(unit(sub(tip, mid)), far * 2.0);
        pts.extend([e1, add(e1, away), add(e0, away), e0]);
        return pts;
    }
    for i in 0..=N {
        pts.push(a.at(-PI / 2.0 + i as f64 / N as f64 * PI));
    }
    pts.push(ext(&a, 1.0, far));
    pts.push(ext(&b, 1.0, far));
    for i in (0..=N).rev() {
        pts.push(b.at(-PI / 2.0 + i as f64 / N as f64 * PI));
    }
    pts.push(ext(&b, -1.0, far));
    pts.push(ext(&a, -1.0, far));
    pts
}
