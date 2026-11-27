//! The working canvas and the pixel shapes every part is cut from.
//!
//! A creature is built on a canvas larger than the finished sprite, so that a
//! part may overshoot while it is being fitted. Columns run left to right and
//! rows top to bottom. The axis of symmetry lies between columns `AX - 1` and
//! `AX`, so a mirrored shape has an even width.
//!
//! The reference keeps every number as a JavaScript double. Integer work here
//! uses `i32`, and every step the reference does in fractions (shares,
//! divisions, square roots) is done in `f64` in the same order, so each cell
//! lands where the reference puts it.

/// The finished sprite is `N × N` cells.
pub(crate) const N: i32 = 24;
/// The working canvas.
pub(crate) const CW: i32 = 48;
pub(crate) const CH: i32 = 48;
/// The axis lies between columns `AX - 1` and `AX`.
pub(crate) const AX: i32 = 24;
/// The ground: a foot's lowest row is `GY - 1`.
pub(crate) const GY: i32 = 42;
/// Cells on the working canvas.
pub(crate) const CELLS: usize = (CW * CH) as usize;

/// Clamps as the reference does: below `lo` gives `lo`, above `hi` gives
/// `hi`. Unlike `i32::clamp` it accepts `lo > hi`, where `lo` wins for
/// values below it and `hi` for the rest.
pub(crate) fn clamp(v: i32, lo: i32, hi: i32) -> i32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// [`clamp`] for fractions.
pub(crate) fn clampf(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// Rounds halves away from zero, written exactly as the reference writes it
/// (`floor(v + 0.5)`), which differs from `f64::round` for a few values
/// just under one half.
pub(crate) fn rnd(v: f64) -> f64 {
    if v < 0.0 {
        -(-v + 0.5).floor()
    } else {
        (v + 0.5).floor()
    }
}

/// [`rnd`] for a value that is a cell count or a cell position.
pub(crate) fn rndi(v: f64) -> i32 {
    rnd(v) as i32
}

/// A cell position on the working canvas.
pub(crate) type Pt = (i32, i32);

/// The flat index of a cell. Like the reference, a column off the canvas
/// runs on into the next row rather than being refused.
pub(crate) fn idx(x: i32, y: i32) -> i32 {
    y * CW + x
}

/// Whether a cell lies on the working canvas.
pub(crate) fn inside(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < CW && y < CH
}

/// The bounding box of a mask's cells.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bounds {
    pub(crate) x0: i32,
    pub(crate) y0: i32,
    pub(crate) x1: i32,
    pub(crate) y1: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
}

/// A set of cells on the working canvas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Mask(Vec<u8>);

impl Mask {
    pub(crate) fn new() -> Self {
        Mask(vec![0; CELLS])
    }

    pub(crate) fn put(&mut self, x: i32, y: i32) {
        if inside(x, y) {
            self.0[idx(x, y) as usize] = 1;
        }
    }

    pub(crate) fn unput(&mut self, x: i32, y: i32) {
        if inside(x, y) {
            self.0[idx(x, y) as usize] = 0;
        }
    }

    pub(crate) fn has(&self, x: i32, y: i32) -> bool {
        inside(x, y) && self.0[idx(x, y) as usize] == 1
    }

    /// Reads a cell by flat index. An index off the canvas reads as empty,
    /// as an out-of-range read of the reference's array does.
    pub(crate) fn at(&self, c: i32) -> bool {
        c >= 0 && (c as usize) < CELLS && self.0[c as usize] != 0
    }

    /// Sets a cell by flat index. An index off the canvas is ignored, as a
    /// write past the end of the reference's typed array is.
    pub(crate) fn set_at(&mut self, c: i32) {
        if c >= 0 && (c as usize) < CELLS {
            self.0[c as usize] = 1;
        }
    }

    /// Cells `x0..=x1` of row `y`.
    pub(crate) fn span(&mut self, y: i32, x0: i32, x1: i32) {
        for x in x0..=x1 {
            self.put(x, y);
        }
    }

    pub(crate) fn block(&mut self, x0: i32, y0: i32, w: i32, h: i32) {
        for y in y0..y0 + h {
            self.span(y, x0, x0 + w - 1);
        }
    }

    pub(crate) fn union(&mut self, other: &Mask) {
        for (cell, &src) in self.0.iter_mut().zip(&other.0) {
            if src != 0 {
                *cell = 1;
            }
        }
    }

    /// The mask reflected across the axis.
    pub(crate) fn mirrored(&self) -> Mask {
        let mut out = Mask::new();
        for y in 0..CH {
            for x in 0..CW {
                if self.0[idx(x, y) as usize] != 0 {
                    out.0[idx(CW - 1 - x, y) as usize] = 1;
                }
            }
        }
        out
    }

    pub(crate) fn shifted(&self, dx: i32, dy: i32) -> Mask {
        if dx == 0 && dy == 0 {
            return self.clone();
        }
        let mut out = Mask::new();
        for y in 0..CH {
            for x in 0..CW {
                if self.0[idx(x, y) as usize] != 0 {
                    out.put(x + dx, y + dy);
                }
            }
        }
        out
    }

    pub(crate) fn bounds(&self) -> Option<Bounds> {
        let (mut x0, mut y0, mut x1, mut y1, mut n) = (CW, CH, -1, -1, 0);
        for y in 0..CH {
            for x in 0..CW {
                if self.0[idx(x, y) as usize] != 0 {
                    n += 1;
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                    y0 = y0.min(y);
                    y1 = y1.max(y);
                }
            }
        }
        (n > 0).then_some(Bounds {
            x0,
            y0,
            x1,
            y1,
            w: x1 - x0 + 1,
            h: y1 - y0 + 1,
        })
    }

    /// Rows of cells centred on the axis: `hw[i]` cells each side of it in
    /// row `top + i`.
    pub(crate) fn sym_rows(&mut self, top: i32, hw: &[i32]) {
        for (i, &w) in hw.iter().enumerate() {
            if w > 0 {
                self.span(top + i as i32, AX - w, AX + w - 1);
            }
        }
    }

    /// A disc of diameter `d` with the top-left corner of its box at (x, y).
    pub(crate) fn disc(&mut self, x: i32, y: i32, d: i32) {
        let rows = disc_rows(d);
        let count = rows.len() as i32;
        for (j, &w) in rows.iter().enumerate() {
            let off = (count - w) >> 1;
            self.span(y + j as i32, x + off, x + off + w - 1);
        }
    }

    /// A disc centred on the cell (cx, cy); an even disc reaches one cell
    /// further right and down.
    pub(crate) fn dot(&mut self, cx: i32, cy: i32, d: i32) {
        self.disc(cx - ((d - 1) >> 1), cy - ((d - 1) >> 1), d);
    }

    /// Stamps a disc at every point of a trail. `diam` gets the position
    /// along the trail (0 at its start, 1 at its end) and the point's index.
    pub(crate) fn stroke(&mut self, pts: &[Pt], diam: impl Fn(f64, usize) -> i32) {
        for (i, &(x, y)) in pts.iter().enumerate() {
            let u = if pts.len() > 1 {
                i as f64 / (pts.len() - 1) as f64
            } else {
                0.0
            };
            self.dot(x, y, diam(u, i));
        }
    }

    /// The flat indices of the mask's cells, in order.
    pub(crate) fn cells(&self) -> Vec<i32> {
        (0..CELLS as i32)
            .filter(|&c| self.0[c as usize] != 0)
            .collect()
    }
}

/// Small pixel discs by diameter: the width of each row.
const DISC: [&[i32]; 9] = [
    &[1],
    &[2, 2],
    &[1, 3, 1],
    &[2, 4, 4, 2],
    &[3, 5, 5, 5, 3],
    &[4, 6, 6, 6, 6, 4],
    &[3, 5, 7, 7, 7, 5, 3],
    &[4, 6, 8, 8, 8, 8, 6, 4],
    &[5, 7, 9, 9, 9, 9, 9, 7, 5],
];

/// The row widths of a disc; diameters outside 1..=9 are clamped.
pub(crate) fn disc_rows(d: i32) -> &'static [i32] {
    DISC[(clamp(d, 1, 9) - 1) as usize]
}

/// The eight directions a pen can step in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dir {
    E,
    Ne,
    N,
    Nw,
    W,
    Sw,
    S,
    Se,
}

impl Dir {
    pub(crate) fn delta(self) -> Pt {
        match self {
            Dir::E => (1, 0),
            Dir::Ne => (1, -1),
            Dir::N => (0, -1),
            Dir::Nw => (-1, -1),
            Dir::W => (-1, 0),
            Dir::Sw => (-1, 1),
            Dir::S => (0, 1),
            Dir::Se => (1, 1),
        }
    }
}

/// The cells a pen visits from (x, y), taking `k` steps in each direction
/// in turn. A count of zero or less takes no step.
pub(crate) fn trail(mut x: i32, mut y: i32, moves: &[(Dir, i32)]) -> Vec<Pt> {
    let mut pts = vec![(x, y)];
    for &(d, k) in moves {
        let (dx, dy) = d.delta();
        for _ in 0..k {
            x += dx;
            y += dy;
            pts.push((x, y));
        }
    }
    pts
}

/// `total` unit steps shared over `n` slots as evenly as possible. A
/// negative `order` puts the larger shares first, a positive one the smaller
/// first, and zero spreads them evenly along the slots, which draws a
/// straight slope.
pub(crate) fn spread(total: i32, n: i32, order: i32) -> Vec<i32> {
    let mut out = Vec::new();
    if n <= 0 {
        return out;
    }
    let (t, nf) = (f64::from(total), f64::from(n));
    if order == 0 {
        for i in 0..n {
            let i = f64::from(i);
            let next = ((i + 1.0) * t / nf + 0.5).floor();
            let this = (i * t / nf + 0.5).floor();
            out.push((next - this) as i32);
        }
        return out;
    }
    let base = (t / nf).floor() as i32;
    let extra = total - base * n;
    for i in 0..n {
        out.push(base + i32::from(i < extra));
    }
    if order > 0 {
        out.reverse();
    }
    out
}

/// Running totals of a list, starting from zero: `[0, a, a + b, ...]`.
pub(crate) fn running(list: &[i32]) -> Vec<i32> {
    let mut out = vec![0];
    for &v in list {
        out.push(out[out.len() - 1] + v);
    }
    out
}

/// Rebuilds a convex arc of half-widths, from its widest row to its tip, so
/// that its runs of cells change length steadily: the arc is cut into edges
/// (r rows at one width, then a drop of s cells), an edge that is neither a
/// pure vertical nor a pure horizontal run is split into ones that are, and
/// the edges are put in order of slope. The result keeps the height and the
/// first width, and is convex.
pub(crate) fn clean_arc(arr: &[i32]) -> Vec<i32> {
    let n = arr.len();
    if n < 2 {
        return arr.to_vec();
    }
    let mut w = arr.to_vec();
    for i in 1..n {
        if w[i] > w[i - 1] {
            w[i] = w[i - 1];
        }
    }
    let mut edges: Vec<(i32, i32)> = Vec::new();
    let mut r = 0;
    for i in 0..n - 1 {
        r += 1;
        let s = w[i] - w[i + 1];
        if s > 0 {
            if r >= 2 && s >= 2 {
                if r >= s {
                    edges.extend(spread(r, s, -1).into_iter().map(|rows| (rows, 1)));
                } else {
                    edges.extend(spread(s, r, -1).into_iter().map(|step| (1, step)));
                }
            } else {
                edges.push((r, s));
            }
            r = 0;
        }
    }
    // The rows left at the tip: the last one closes the arc across the axis,
    // the others stand upright.
    if r > 0 {
        edges.push((r, 0));
    }
    edges.push((1, w[n - 1]));
    // Every edge spans at least one row, so comparing slopes by cross
    // multiplication is a total order; ties keep the taller edge first.
    edges.sort_by(|p, q| (p.1 * q.0 - q.1 * p.0).cmp(&0).then((q.0 - p.0).cmp(&0)));
    let mut out = Vec::with_capacity(n);
    let mut cur = w[0];
    for (er, es) in edges {
        for _ in 0..er {
            out.push(cur);
        }
        cur -= es;
    }
    out
}

/// The corner shapes a lobe's top and base can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    Round,
    Boxy,
    Pointed,
    Square,
}

impl Corner {
    pub const ALL: &'static [Corner] =
        &[Corner::Round, Corner::Boxy, Corner::Pointed, Corner::Square];

    /// The reference's name for the shape.
    pub fn name(self) -> &'static str {
        match self {
            Corner::Round => "round",
            Corner::Boxy => "boxy",
            Corner::Pointed => "pointed",
            Corner::Square => "square",
        }
    }
}

/// How wide a lobe is at the fraction `u` of the way from its widest row
/// (0) to its tip (1), as a share of its greatest half-width.
fn shape_at(u: f64, kind: Corner) -> f64 {
    let u2 = u * u;
    let circle = (0f64).max(1.0 - u2).sqrt();
    let squircle = (0f64).max(1.0 - u2 * u2).sqrt().sqrt();
    let slope = 1.0 - u;
    match kind {
        Corner::Boxy => 0.5 * circle + 0.5 * squircle,
        Corner::Square => 0.15 * circle + 0.85 * squircle,
        Corner::Pointed => 0.6 * circle + 0.4 * slope,
        Corner::Round => circle,
    }
}

/// A lobe as half-widths per row, top to bottom. `a` is the greatest
/// half-width and `h` the rows; `peak` is how far down the widest row lies
/// (0..1), `top` and `bot` the shapes above and below it, and `cut` the rows
/// sliced off the bottom for a flat base.
pub(crate) fn lobe(a: i32, h: i32, peak: f64, top: Corner, bot: Corner, cut: i32) -> Vec<i32> {
    let full = h + cut;
    let p = clamp(rndi(f64::from(full) * peak), 1, 1.max(h - 1));
    let af = f64::from(a);
    let up: Vec<i32> = (0..p)
        .map(|k| {
            clamp(
                rndi(af * shape_at((f64::from(k) + 0.5) / f64::from(p), top)),
                1,
                a,
            )
        })
        .collect();
    let down: Vec<i32> = (0..full - p)
        .map(|k| {
            clamp(
                rndi(af * shape_at((f64::from(k) + 0.5) / f64::from(full - p), bot)),
                1,
                a,
            )
        })
        .collect();
    let mut rows = clean_arc(&up);
    rows.reverse();
    let down = clean_arc(&down);
    rows.extend_from_slice(&down[..(h - p).max(0) as usize]);
    rows
}

/// An axis-aligned oval inside the box at (x0, y0), `w` wide and `h` tall.
pub(crate) fn oval(m: &mut Mask, x0: i32, y0: i32, w: i32, h: i32) {
    let half = f64::from(h) / 2.0;
    for j in 0..h {
        let u = (f64::from(j) + 0.5 - half) / half;
        let mut n = rndi(f64::from(w) * (0f64).max(1.0 - u * u).sqrt());
        if (w - n) & 1 != 0 {
            n += 1;
        }
        n = clamp(n, if w & 1 != 0 { 1 } else { 2 }, w);
        m.span(y0 + j, x0 + ((w - n) >> 1), x0 + ((w - n) >> 1) + n - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rnd_rounds_halves_away_from_zero_like_floor_of_v_plus_a_half() {
        assert_eq!(rnd(2.5), 3.0);
        assert_eq!(rnd(-2.5), -3.0);
        assert_eq!(rnd(0.49999999999999994), 1.0);
    }

    #[test]
    fn clamp_accepts_an_empty_range_as_the_reference_does() {
        // Below the lower bound gives it; anything else above the upper
        // bound gives that.
        assert_eq!(clamp(5, 7, 3), 7);
        assert_eq!(clamp(9, 7, 3), 3);
    }

    #[test]
    fn spread_shares_steps_in_each_order() {
        assert_eq!(spread(5, 3, -1), [2, 2, 1]);
        assert_eq!(spread(5, 3, 1), [1, 2, 2]);
        assert_eq!(spread(5, 3, 0), [2, 1, 2]);
        assert!(spread(3, 0, 0).is_empty());
    }

    #[test]
    fn a_cleaned_arc_keeps_its_height_and_first_width() {
        let arc = clean_arc(&[6, 6, 5, 3, 2, 1]);
        assert_eq!(arc.len(), 6);
        assert_eq!(arc[0], 6);
    }
}
