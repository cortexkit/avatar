//! Bounds in the same axes as a tree draws, including stroke and clip.

use avatar_svg::{Primitive, Segment, Shape, StrokeCap, StrokeJoin};

use crate::geom::{apply_m, IDENT, M6};
use crate::kit::P2;

use super::GRID;

#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    pub(super) min: P2,
    pub(super) max: P2,
}

impl Bounds {
    pub(super) fn empty() -> Self {
        Self {
            min: [f64::INFINITY; 2],
            max: [f64::NEG_INFINITY; 2],
        }
    }

    pub(super) fn nonempty(self) -> Option<Self> {
        (self.min[0] <= self.max[0] && self.min[1] <= self.max[1]).then_some(self)
    }

    pub(super) fn point(&mut self, p: P2) {
        for (i, v) in p.into_iter().enumerate() {
            self.min[i] = self.min[i].min(v);
            self.max[i] = self.max[i].max(v);
        }
    }

    pub(super) fn include(&mut self, other: Self) {
        if let Some(b) = other.nonempty() {
            self.point(b.min);
            self.point(b.max);
        }
    }

    pub(super) fn primitives(primitives: &[Primitive]) -> Self {
        let mut bounds = Self::empty();
        for p in primitives {
            bounds.include(Self::primitive(p));
        }
        bounds
    }

    pub(super) fn primitive(p: &Primitive) -> Self {
        let m = p.transform.as_ref().map_or(IDENT, |t| {
            let a = t.affine();
            let q = |v: i64| v as f64 / 4_294_967_296.0;
            [
                q(a.a.0),
                q(a.b.0),
                q(a.c.0),
                q(a.d.0),
                unit(a.tx),
                unit(a.ty),
            ]
        });
        let mut b = Self::shape(&p.shape, m);
        if let Some(s) = p.stroke {
            let path = matches!(p.shape, Shape::Path { .. } | Shape::CompoundPath { .. });
            let mut factor: f64 = 1.0;
            if path && s.cap == StrokeCap::Square {
                factor = 2.0_f64.sqrt();
            }
            if s.join == StrokeJoin::Miter && has_path_join(&p.shape) {
                // The renderers use a miter limit of four stroke radii.
                factor = 4.0;
            }
            let r = unit(s.width) * 0.5 * factor;
            for i in 0..2 {
                let pad = r * (m[i] * m[i] + m[i + 2] * m[i + 2]).sqrt();
                b.min[i] -= pad;
                b.max[i] += pad;
            }
        }
        if let Some(clip) = &p.clip {
            let clip = Self::shape(clip, m);
            for i in 0..2 {
                b.min[i] = b.min[i].max(clip.min[i]);
                b.max[i] = b.max[i].min(clip.max[i]);
            }
        }
        b
    }

    fn shape(shape: &Shape, m: M6) -> Self {
        let mut b = Self::empty();
        match shape {
            Shape::Rect { x, y, w, h } => {
                for x in [*x, x + w] {
                    for y in [*y, y + h] {
                        b.point(apply_m(m, [unit(x), unit(y)]));
                    }
                }
            }
            Shape::Circle { cx, cy, r } => b.ellipse(m, *cx, *cy, *r, *r),
            Shape::Ellipse { cx, cy, rx, ry } => b.ellipse(m, *cx, *cy, *rx, *ry),
            Shape::Path { segments, .. } => b.path(m, segments),
            Shape::CompoundPath { subpaths, .. } => {
                for contour in subpaths {
                    b.path(m, &contour.segments);
                }
            }
        }
        b
    }

    fn ellipse(&mut self, m: M6, x: i64, y: i64, rx: i64, ry: i64) {
        let c = apply_m(m, [unit(x), unit(y)]);
        let r: P2 = std::array::from_fn(|i| {
            let a = m[i] * unit(rx);
            let b = m[i + 2] * unit(ry);
            (a * a + b * b).sqrt()
        });
        self.point([c[0] - r[0], c[1] - r[1]]);
        self.point([c[0] + r[0], c[1] + r[1]]);
    }

    fn path(&mut self, m: M6, segments: &[Segment]) {
        let point = |x, y| apply_m(m, [unit(x), unit(y)]);
        let mut prev = [0.0; 2];
        for segment in segments {
            match *segment {
                Segment::MoveTo { x, y } | Segment::LineTo { x, y } => {
                    prev = point(x, y);
                    self.point(prev);
                }
                Segment::CubicTo {
                    cp1x,
                    cp1y,
                    cp2x,
                    cp2y,
                    x,
                    y,
                } => {
                    let end = point(x, y);
                    let cp1 = point(cp1x, cp1y);
                    let cp2 = point(cp2x, cp2y);
                    self.point(end);
                    for i in 0..2 {
                        let coords = [prev[i], cp1[i], cp2[i], end[i]];
                        for t in extrema(coords).into_iter().flatten() {
                            if t > 0.0 && t < 1.0 {
                                let value = cubic_at(coords, t);
                                self.min[i] = self.min[i].min(value);
                                self.max[i] = self.max[i].max(value);
                            }
                        }
                    }
                    prev = end;
                }
            }
        }
    }
}

fn has_path_join(shape: &Shape) -> bool {
    let joined = |segments: &[Segment], closed| {
        (closed && segments.len() > 2)
            || segments
                .windows(2)
                .any(|pair| pair.iter().all(|s| !matches!(s, Segment::MoveTo { .. })))
    };
    match shape {
        Shape::Path {
            segments, closed, ..
        } => joined(segments, *closed),
        Shape::CompoundPath { subpaths, .. } => {
            subpaths.iter().any(|s| joined(&s.segments, s.closed))
        }
        _ => false,
    }
}

fn unit(v: i64) -> f64 {
    v as f64 / GRID
}

fn extrema([p, q, r, s]: [f64; 4]) -> [Option<f64>; 2] {
    let a = (s - r) - 2.0 * (r - q) + (q - p);
    let b = 2.0 * ((r - q) - (q - p));
    let c = q - p;
    if a.abs() <= f64::EPSILON * (b.abs() + c.abs()) {
        return [(b != 0.0).then(|| -c / b), None];
    }
    let d = b * b - 4.0 * a * c;
    if d < 0.0 {
        return [None, None];
    }
    let q = -0.5 * (b + d.sqrt().copysign(b));
    [Some(q / a), (q != 0.0).then(|| c / q)]
}

fn cubic_at([p, q, r, s]: [f64; 4], t: f64) -> f64 {
    let u = 1.0 - t;
    u * u * (u * p + 3.0 * t * q) + t * t * (3.0 * u * r + t * s)
}

// Mouth contours use the same number of subdivisions as a rounded hull
// corner; the housing ellipse has eight subdivisions per quadrant.
pub(super) fn contours(shape: &Shape) -> Vec<super::Contour> {
    use super::{Contour, HULL_CORNER_STEPS};
    let path = |segments: &[Segment], closed| {
        let mut contours = Vec::new();
        let mut points = Vec::new();
        for segment in segments {
            match *segment {
                Segment::MoveTo { x, y } => {
                    if !points.is_empty() {
                        contours.push(Contour {
                            points: std::mem::take(&mut points),
                            closed,
                        });
                    }
                    points.push([unit(x), unit(y)]);
                }
                Segment::LineTo { x, y } => points.push([unit(x), unit(y)]),
                Segment::CubicTo {
                    cp1x,
                    cp1y,
                    cp2x,
                    cp2y,
                    x,
                    y,
                } => {
                    let [px, py] = *points.last().expect("a cubic follows its starting point");
                    for step in 1..=HULL_CORNER_STEPS {
                        let t = f64::from(step) / f64::from(HULL_CORNER_STEPS);
                        points.push([
                            cubic_at([px, unit(cp1x), unit(cp2x), unit(x)], t),
                            cubic_at([py, unit(cp1y), unit(cp2y), unit(y)], t),
                        ]);
                    }
                }
            }
        }
        if !points.is_empty() {
            if closed && points.first() == points.last() {
                points.pop();
            }
            contours.push(Contour { points, closed });
        }
        contours
    };
    let closed = |points| {
        vec![Contour {
            points,
            closed: true,
        }]
    };
    match shape {
        Shape::Path {
            segments, closed, ..
        } => path(segments, *closed),
        Shape::CompoundPath { subpaths, .. } => subpaths
            .iter()
            .flat_map(|s| path(&s.segments, s.closed))
            .collect(),
        Shape::Rect { x, y, w, h } => closed(vec![
            [unit(*x), unit(*y)],
            [unit(x + w), unit(*y)],
            [unit(x + w), unit(y + h)],
            [unit(*x), unit(y + h)],
        ]),
        Shape::Circle { cx, cy, r } => closed(ellipse([unit(*cx), unit(*cy)], [unit(*r); 2])),
        Shape::Ellipse { cx, cy, rx, ry } => {
            closed(ellipse([unit(*cx), unit(*cy)], [unit(*rx), unit(*ry)]))
        }
    }
}

pub(super) fn ellipse([x, y]: P2, [rx, ry]: P2) -> Vec<P2> {
    let count = super::HULL_CORNER_STEPS * 4;
    (0..count)
        .map(|i| {
            let u = crate::geom::up_dir(f64::from(i) * 360.0 / f64::from(count));
            [x + u[0] * rx, y + u[1] * ry]
        })
        .collect()
}
