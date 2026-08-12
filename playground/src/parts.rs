//! Top-slot part families for grid 1: the silhouette makers.
//!
//! Every part is drawn as an SVG path in the blob's frame, anchored to the
//! upper surface. Parts are mirrored pairs unless noted.

use crate::body::{width_at, BodyParams};

/// Which top-slot family a cell wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopPart {
    None,
    /// Rounded teardrop ears.
    Ears,
    /// Curved tapered horns.
    Horns,
    /// Stalk + ball antennae.
    Antennae,
    /// Scraggly three-tuft mohawk (ugly-cute probe).
    Tuft,
    /// Single chunky rhino-style nub, off-center (asymmetry probe).
    Nub,
}

pub const ALL: [TopPart; 6] = [
    TopPart::None,
    TopPart::Ears,
    TopPart::Horns,
    TopPart::Antennae,
    TopPart::Tuft,
    TopPart::Nub,
];

/// Emits SVG path elements for the part, `fill` pre-formatted.
pub fn draw(part: TopPart, body: &BodyParams, cx: f64, cy: f64, fill: &str) -> String {
    let half_h = body.height / 2.0;
    // Anchor y: a bit below the crown so parts grow FROM the mass.
    let ay = -half_h * 0.82;
    let aw = width_at(body, ay); // half-width at anchor line
    match part {
        TopPart::None => String::new(),
        TopPart::Ears => {
            let mut s = String::new();
            for side in [-1.0, 1.0] {
                let bx = cx + side * aw * 0.62;
                let by = cy + ay;
                let len = body.height * 0.34;
                let wid = body.w_top * 0.34;
                // Teardrop: base on the crown, rounded tip leaning outward.
                s.push_str(&format!(
                    "<path d='M{:.1} {:.1} C{:.1} {:.1} {:.1} {:.1} {:.1} {:.1} C{:.1} {:.1} {:.1} {:.1} {:.1} {:.1} Z' fill='{fill}'/>",
                    bx - wid * 0.5, by,
                    bx - wid * 0.55, by - len * 0.75,
                    bx + side * wid * 0.18, by - len * 1.05,
                    bx + side * wid * 0.32, by - len * 0.98,
                    bx + side * wid * 0.62, by - len * 0.55,
                    bx + wid * 0.55, by - len * 0.1,
                    bx + wid * 0.5, by,
                ));
            }
            s
        }
        TopPart::Horns => {
            let mut s = String::new();
            for side in [-1.0, 1.0] {
                let bx = cx + side * aw * 0.55;
                let by = cy + ay;
                let len = body.height * 0.30;
                let wid = body.w_top * 0.22;
                // Curved taper: base chord, outer arc to a point, inner arc back.
                s.push_str(&format!(
                    "<path d='M{:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1} Z' fill='{fill}'/>",
                    bx - side * wid, by,
                    bx + side * wid * 0.2, by - len * 0.55,
                    bx + side * wid * 1.7, by - len,
                    bx + side * wid * 0.9, by - len * 0.45,
                    bx + side * wid, by,
                ));
            }
            s
        }
        TopPart::Antennae => {
            let mut s = String::new();
            for side in [-1.0, 1.0] {
                let bx = cx + side * aw * 0.35;
                let by = cy + ay;
                let len = body.height * 0.36;
                let ball = body.height * 0.05;
                s.push_str(&format!(
                    "<path d='M{:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1}' stroke='{fill}' stroke-width='{:.1}' fill='none' stroke-linecap='round'/><circle cx='{:.1}' cy='{:.1}' r='{:.1}' fill='{fill}'/>",
                    bx, by,
                    bx + side * len * 0.35, by - len * 0.7,
                    bx + side * len * 0.55, by - len,
                    ball * 0.6,
                    bx + side * len * 0.55, by - len,
                    ball,
                ));
            }
            s
        }
        TopPart::Tuft => {
            let mut s = String::new();
            let by = cy + ay;
            for (dx, len, lean) in [(-0.3, 0.2, -0.4), (0.0, 0.3, 0.06), (0.28, 0.17, 0.45)] {
                let bx = cx + dx * aw;
                let l = body.height * len;
                let w = body.w_top * 0.2;
                s.push_str(&format!(
                    "<path d='M{:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1} Z' fill='{fill}'/>",
                    bx - w, by,
                    bx - w * 0.2 + lean * l, by - l * 0.9,
                    bx + lean * l * 1.3, by - l,
                    bx + w * 0.4 + lean * l * 0.4, by - l * 0.5,
                    bx + w, by,
                ));
            }
            s
        }
        TopPart::Nub => {
            let bx = cx + aw * 0.38; // off-center on purpose
            let by = cy + ay;
            let len = body.height * 0.3;
            let wid = body.w_top * 0.42;
            format!(
                "<path d='M{:.1} {:.1} C{:.1} {:.1} {:.1} {:.1} {:.1} {:.1} C{:.1} {:.1} {:.1} {:.1} {:.1} {:.1} Z' fill='{fill}'/>",
                bx - wid, by,
                bx - wid * 0.9, by - len * 1.1,
                bx + wid * 0.5, by - len * 1.15,
                bx + wid * 0.7, by - len * 0.6,
                bx + wid * 0.85, by - len * 0.2,
                bx + wid, by - len * 0.05,
                bx + wid, by,
            )
        }
    }
}
