//! The finished sprite: the creature cut from its frame, its cells sorted
//! into named layers, and the layers merged into rectangles in the order
//! the reference draws them.

use avatar_svg::{serialize_tree, Mood, Paint, Primitive, RenderTree, Rgb, Shape};

use crate::fit::Fit;
use crate::grid::{CH, CW, N};
use crate::palette::Palette;
use crate::sprite::{build_sprite, mood_index, Built, Memo};

/// Primitive coordinates per grid cell: the tree's canvas is one unit per
/// cell, and primitive coordinates carry 100 per canvas unit.
const CELL: i64 = 100;

/// One named layer of cells, as (x, y, colour index) in the frame.
pub(crate) struct Layer {
    pub(crate) group: &'static str,
    pub(crate) name: String,
    pub(crate) cells: Vec<(i32, i32, u8)>,
}

/// The finished sprite of one mood: its layers, and the built creature for
/// what the face settled.
pub(crate) struct Sprite<'a> {
    pub(crate) layers: Vec<Layer>,
    pub(crate) built: Built<'a>,
}

/// Builds the sprite of one mood in its frame and sorts its cells into layers.
pub(crate) fn finish<'a>(memo: &mut Memo<'a>, fit: &Fit, mood: Mood) -> Sprite<'a> {
    let k = mood_index(mood);
    memo.anger = fit.anger;
    let built = build_sprite(memo, mood, fit.lim, Some(fit.win), fit.eases[k]);
    let (sx, sy) = fit.shifts[k];
    let (wx, wy) = (fit.win.x0 - sx, fit.win.y0 - sy);
    let mut layers: Vec<Layer> = Vec::new();
    let layer = |layers: &mut Vec<Layer>, group: &'static str, name: &str| -> usize {
        if let Some(i) = layers
            .iter()
            .position(|l| l.group == group && l.name == name)
        {
            return i;
        }
        layers.push(Layer {
            group,
            name: name.to_owned(),
            cells: Vec::new(),
        });
        layers.len() - 1
    };
    let parts = &built.ctx.parts;
    let comp = &built.comp;
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by_key(|&i| (parts[i].z, parts[i].ord));
    for &i in &order {
        layer(&mut layers, parts[i].group, &parts[i].name);
    }
    let edge = layer(&mut layers, "outline", "edge");
    let seam = layer(&mut layers, "outline", "seam");
    for y in 0..CH {
        for x in 0..CW {
            let c = (y * CW + x) as usize;
            let o = comp.own[c];
            if o < 0 {
                continue;
            }
            let (gx, gy) = (x - wx, y - wy);
            if gx < 0 || gy < 0 || gx >= N || gy >= N {
                continue;
            }
            let part = &parts[o as usize];
            // A part drawn as line alone keeps its cells under its own name.
            let l = if comp.line[c] == 1 && !part.line_only {
                edge
            } else if comp.line[c] == 2 {
                seam
            } else {
                layer(&mut layers, part.group, &part.name)
            };
            layers[l].cells.push((gx, gy, comp.col[c]));
        }
    }
    for e in built.ctx.face.iter().chain(&built.ctx.marks) {
        // Mood marks stand in the frame; the creature's shift does not move them.
        let floating = e.group == "marks" && e.name != "tear";
        let (gx, gy) = if floating {
            (e.x - wx - sx, e.y - wy - sy)
        } else {
            (e.x - wx, e.y - wy)
        };
        if gx < 0 || gy < 0 || gx >= N || gy >= N {
            continue;
        }
        let l = layer(&mut layers, e.group, &e.name);
        layers[l].cells.push((gx, gy, e.col));
    }
    layers.retain(|l| !l.cells.is_empty());
    Sprite { layers, built }
}

/// A rectangle of cells of one colour: x, y, width, height, colour index.
type Rect = (i32, i32, i32, i32, u8);

/// A layer's name with its rectangles.
type NamedRects<'a> = (&'a str, Vec<Rect>);

/// Merges cells of one colour into as few rectangles as a greedy sweep
/// finds: runs along each row, then runs that repeat in the rows below.
/// Colours are taken in the order they first appear.
fn rects_of(list: &[(i32, i32, u8)]) -> Vec<Rect> {
    let mut by_col: Vec<(u8, Vec<bool>)> = Vec::new();
    for &(x, y, col) in list {
        let i = match by_col.iter().position(|(c, _)| *c == col) {
            Some(i) => i,
            None => {
                by_col.push((col, vec![false; (N * N) as usize]));
                by_col.len() - 1
            }
        };
        by_col[i].1[(y * N + x) as usize] = true;
    }
    let mut out = Vec::new();
    for (col, mut g) in by_col {
        for y in 0..N {
            for x in 0..N {
                if !g[(y * N + x) as usize] {
                    continue;
                }
                let mut w = 1;
                while x + w < N && g[(y * N + x + w) as usize] {
                    w += 1;
                }
                let mut h = 1;
                'grow: while y + h < N {
                    for i in 0..w {
                        if !g[((y + h) * N + x + i) as usize] {
                            break 'grow;
                        }
                    }
                    h += 1;
                }
                for j in 0..h {
                    for i in 0..w {
                        g[((y + j) * N + x + i) as usize] = false;
                    }
                }
                out.push((x, y, w, h, col));
            }
        }
    }
    out
}

/// The order groups are drawn in, back to front.
const GROUP_ORDER: [&str; 20] = [
    "tail",
    "wings",
    "legs",
    "body",
    "arms",
    "ears",
    "horns",
    "antlers",
    "antennae",
    "frills",
    "coat",
    "eye-mounts",
    "outline",
    "eyes",
    "lids",
    "brows",
    "nose",
    "mouth",
    "cheeks",
    "marks",
];

/// The sprite as a render tree on a 24-cell pixel grid: for each layer, in
/// the reference's drawing order, the rectangles of the cells it shows.
/// Only the last cell drawn at a place is written, except that eyes stay
/// whole under their lids; so the topmost cell is the one that shows.
pub(crate) fn tree_of(sprite: &Sprite, palette: &Palette) -> RenderTree {
    let layers = &sprite.layers;
    let mut top = vec![usize::MAX; (N * N) as usize];
    for (li, l) in layers.iter().enumerate() {
        for &(x, y, _) in &l.cells {
            top[(y * N + x) as usize] = li;
        }
    }
    // Each group with its layers, by name, in the order they first appear.
    let mut groups: Vec<(&'static str, Vec<NamedRects>)> = Vec::new();
    for (li, l) in layers.iter().enumerate() {
        let keep = |&(x, y, _): &(i32, i32, u8)| {
            l.group == "eyes" || l.group == "lids" || top[(y * N + x) as usize] == li
        };
        // One cell per place: the last one at a place, in the order places
        // first appear.
        let mut seen: Vec<(i32, i32, u8)> = Vec::new();
        let mut at = vec![usize::MAX; (N * N) as usize];
        for &cell in l.cells.iter().filter(|cell| keep(cell)) {
            let p = (cell.1 * N + cell.0) as usize;
            if at[p] == usize::MAX {
                at[p] = seen.len();
                seen.push(cell);
            } else {
                seen[at[p]] = cell;
            }
        }
        let rects = rects_of(&seen);
        if rects.is_empty() {
            continue;
        }
        match groups.iter_mut().find(|(g, _)| *g == l.group) {
            Some((_, list)) => list.push((&l.name, rects)),
            None => groups.push((l.group, vec![(&l.name, rects)])),
        }
    }
    // Groups the order does not name go last, as in the reference.
    let rank = |g: &str| GROUP_ORDER.iter().position(|&o| o == g).unwrap_or(99);
    groups.sort_by_key(|(g, _)| rank(g));
    let mut primitives = Vec::new();
    for (_, list) in &groups {
        for (name, rects) in list {
            for &(x, y, w, h, col) in rects {
                primitives.push(Primitive {
                    shape: Shape::Rect {
                        x: i64::from(x) * CELL,
                        y: i64::from(y) * CELL,
                        w: i64::from(w) * CELL,
                        h: i64::from(h) * CELL,
                    },
                    transform: None,
                    paint: Paint::Solid {
                        color: cell_colour(palette, col),
                    },
                    stroke: None,
                    opacity: None,
                    clip: None,
                    effects: Vec::new(),
                    label: format!("px-{name}"),
                    role: None,
                });
            }
        }
    }
    RenderTree {
        canvas: N as u32,
        background: None,
        pixel_grid: Some(N as u32),
        primitives,
    }
}

/// The colour of a table entry. The empty entry only reaches a layer when
/// a lid is drawn over a cell the body does not cover; the reference then
/// writes the fill `null`, which resvg cannot parse and draws as black.
fn cell_colour(palette: &Palette, col: u8) -> Rgb {
    palette.colour(col).unwrap_or(Rgb { r: 0, g: 0, b: 0 })
}

/// The SVG a pixel-grid tree draws as at `size_px`, placed as
/// `avatar_raster::rasterize_tree` places it: each cell `size_px / cells`
/// whole pixels wide and the grid centred, with the odd spare pixel on the
/// right and bottom. Drawn by a plain SVG rasterizer at `size_px`, it gives
/// the pixels `rasterize_tree` gives. Only the root element differs from
/// `serialize_tree`'s output. A tree without a grid, or a size with less
/// than a pixel per cell, is serialized unchanged.
pub fn placed_svg(tree: &RenderTree, size_px: u32) -> Vec<u8> {
    let svg = serialize_tree(tree);
    let Some(cells) = tree.pixel_grid else {
        return svg;
    };
    let cell_px = size_px / cells;
    if cell_px == 0 {
        return serialize_tree(&RenderTree {
            pixel_grid: None,
            ..tree.clone()
        });
    }
    let margin = (size_px - cell_px * cells) / 2;
    let text = String::from_utf8(svg).expect("serialize_tree writes UTF-8");
    let old_root = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\"",
        tree.canvas, tree.canvas
    );
    // The canvas spans the grid, so a canvas unit is cells / canvas cells.
    let unit_px = f64::from(cell_px) * f64::from(cells) / f64::from(tree.canvas);
    let offset = if margin == 0 {
        "0".to_owned()
    } else {
        format!("-{}", f64::from(margin) / unit_px)
    };
    let extent = f64::from(size_px) / unit_px;
    let new_root = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size_px}\" height=\"{size_px}\" viewBox=\"{offset} {offset} {extent} {extent}\""
    );
    assert!(
        text.starts_with(&old_root),
        "serialize_tree writes the root element this replaces"
    );
    text.replacen(&old_root, &new_root, 1).into_bytes()
}
