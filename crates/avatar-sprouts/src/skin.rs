//! Coat and skin: what grows along the edge of the body (thorns, spines,
//! fuzz, bracts) and the marks inside its outline that say what kind of
//! plant organ it is.

use crate::body::{walk_outline, Body, Kind};
use crate::geometry::{
    box_of, curve, curve_v, disc, line_path, oval, path_item, pt, shape, Node, PathD, Pt, Style,
};
use crate::maths::{lerp, rot, unit, TAU, UP, V};
use crate::mood::Mood;
use crate::organs::lens;
use crate::palette::{Palette, WHITE};
use crate::reader::{dna, Reader};

pub(crate) struct CoatSite {
    p: V,
    n: V,
    k: f64,
}

pub(crate) struct Coat {
    pub(crate) kind: &'static str,
    len: f64,
    sites: Vec<CoatSite>,
}

pub(crate) fn build_coat(r: &Reader<'_>, b: &Body) -> Coat {
    let o = dna::COAT;
    let kinds = crate::body::spec(b.kind).coats;
    let weights: Vec<u32> = kinds.iter().map(|k| k.1).collect();
    let kind = kinds[r.pick(o, &weights)].0;
    let mut coat = Coat {
        kind,
        len: 0.0,
        sites: Vec::new(),
    };
    if kind == "none" {
        return coat;
    }
    let gap = match kind {
        "fuzz" => r.lin(o + 1, 5.5, 8.0),
        "bracts" => r.lin(o + 1, 15.0, 21.0),
        "spines" => r.lin(o + 1, 13.0, 19.0),
        _ => r.lin(o + 1, 17.0, 26.0),
    };
    coat.len = match kind {
        "fuzz" => r.lin(o + 2, 3.5, 6.0),
        "thorns" => r.lin(o + 2, 7.0, 11.5),
        _ => r.lin(o + 2, 6.0, 10.0),
    };
    let mut i = 0_usize;
    let y0 = b.top + b.h * if kind == "bracts" { 0.1 } else { 0.04 };
    for s in walk_outline(b, gap, y0, b.bottom - b.h * 0.12) {
        if s.p[0] < 3.0 {
            continue;
        }
        coat.sites.push(CoatSite {
            p: s.p,
            n: s.n,
            k: if kind == "fuzz" {
                0.75 + 0.5 * ((i * 7) % 5) as f64 / 4.0
            } else {
                r.lin(o + 8 + (i % 40), 0.55, 1.45)
            },
        });
        i += 1;
    }
    coat
}

pub(crate) fn coat_items(coat: &Coat, p: &Palette, m: &Mood) -> Vec<Node> {
    if coat.kind == "none" || coat.sites.is_empty() {
        return Vec::new();
    }
    let len = coat.len
        * if coat.kind == "fuzz" {
            1.0 + (m.bristle - 1.0) * 0.5
        } else {
            m.bristle
        };
    let mut pts = Vec::new();
    let mut d = PathD::default();
    for side in [1.0, -1.0] {
        for s in &coat.sites {
            let q = [side * s.p[0], s.p[1]];
            let n = [side * s.n[0], s.n[1]];
            let l = len * s.k;
            if coat.kind == "thorns" || coat.kind == "bracts" {
                let v = if coat.kind == "bracts" {
                    unit(n[0] * 0.7, n[1] * 0.7 - 0.75)
                } else {
                    unit(n[0], n[1] - 0.25)
                };
                let t = [-v[1], v[0]];
                let w = coat.len * if coat.kind == "bracts" { 0.62 } else { 0.4 };
                let base = [q[0] - n[0] * 2.5, q[1] - n[1] * 2.5];
                let tri = [
                    [base[0] - t[0] * w, base[1] - t[1] * w],
                    [base[0] + v[0] * (l + 2.5), base[1] + v[1] * (l + 2.5)],
                    [base[0] + t[0] * w, base[1] + t[1] * w],
                ];
                d.append(line_path(&tri, true));
                pts.extend([tri[1], tri[0], tri[2]]);
            } else {
                let angles: &[f64] = if coat.kind == "spines" {
                    &[-0.45, 0.0, 0.45]
                } else {
                    &[0.0]
                };
                for &a in angles {
                    let v = rot(
                        if coat.kind == "spines" {
                            n
                        } else {
                            unit(n[0], n[1] + 0.15)
                        },
                        a,
                    );
                    let k = if a != 0.0 { 0.8 } else { 1.0 };
                    let e = [q[0] + v[0] * l * k, q[1] + v[1] * l * k];
                    d.push_line([q[0] - n[0], q[1] - n[1]], e);
                    pts.push(e);
                }
            }
        }
    }
    let cls = format!("sp-coat sp-{}", coat.kind);
    let bx = box_of(pts);
    let style = match coat.kind {
        "thorns" => Style::filled(p.thorn, p.outline, 1.1, &cls).round_join(),
        "bracts" => Style::filled(p.tone_down, p.outline, 1.2, &cls).round_join(),
        _ => Style::line(
            p.outline,
            if coat.kind == "spines" { 1.3 } else { 1.1 },
            &cls,
        )
        .round_cap(),
    };
    vec![path_item(d, bx, style)]
}

pub(crate) struct Skin {
    pub(crate) kind: &'static str,
    pub(crate) items: Vec<Node>,
}

/// The skin's marks. They are the same in every mood, clipped to the body
/// and faded out round the face.
pub(crate) fn build_skin(r: &Reader<'_>, b: &Body, p: &Palette) -> Skin {
    let o = dna::SKIN;
    let kinds = crate::body::spec(b.kind).skins;
    let weights: Vec<u32> = kinds.iter().map(|k| k.1).collect();
    let kind = kinds[r.pick(o, &weights)].0;
    let mut items = Vec::new();
    let mut st = r.stream(o + 8, 40);
    let line = |sw: f64| Style::line(p.tone_down, sw, "sp-skin-line");
    let y_at = |t: f64| b.top + b.h * t;
    // A line of longitude: the same share of the half-width at every height.
    let meridian = |f: f64, t0: f64, t1: f64| -> Vec<Pt> {
        (0..=8)
            .map(|k| {
                let y = y_at(lerp(t0, t1, f64::from(k) / 8.0));
                pt(f * b.hw(y), y)
            })
            .collect()
    };
    // A line of latitude, sagging in the middle as a ring round the body
    // does when seen from a little above.
    let ring = |y: f64, f0: f64, f1: f64, sag: f64| -> Vec<Pt> {
        let w = b.hw(y);
        (0..=6)
            .map(|k| {
                let f = lerp(f0, f1, f64::from(k) / 6.0);
                pt(f * w, y + sag * (1.0 - f * f))
            })
            .collect()
    };
    // How many more marks than the fewest, for the skins that count them.
    let more = || f64::from(r.one(o + 1, 3));
    match kind {
        "veins" | "ribs" => {
            let ribbed = kind == "ribs" || b.kind == Kind::Pumpkin;
            let n = (if ribbed { 2.0 } else { 1.0 }) + more();
            let count = n as usize;
            for j in 0..count {
                for s in [1.0, -1.0] {
                    items.push(curve(
                        &meridian(s * (j as f64 + 1.0) / (n + 0.6), 0.05, 0.96),
                        line(1.5),
                    ));
                }
            }
            if ribbed {
                items.push(curve(&meridian(0.0, 0.04, 0.97), line(1.5)));
            }
            if kind == "ribs" {
                for j in 0..=count {
                    let sides: &[f64] = if j != 0 { &[1.0, -1.0] } else { &[1.0] };
                    for &s in sides {
                        for k in 0..6 {
                            let y = y_at(0.12 + 0.15 * f64::from(k) + (j % 2) as f64 * 0.07);
                            let x = s * j as f64 / (n + 0.6) * b.hw(y);
                            items.push(disc(
                                x,
                                y,
                                1.7,
                                Style::filled(p.pale, p.tone_down, 0.8, "sp-areole"),
                            ));
                        }
                    }
                }
            }
        }
        "stripes" => {
            let n = 2.0 + more();
            let tone = if r.chance(o + 2, 0.5) {
                p.tone_down
            } else {
                p.tone_up
            };
            for j in 0..n as usize {
                // Each stripe has its own width.
                let c = (j as f64 + 0.75) / (n + 0.5);
                let w = 0.2 / (n + 0.5) * lerp(0.6, 1.4, st.unit());
                for s in [1.0, -1.0] {
                    let mut pts = meridian(s * (c - w), 0.07, 0.94);
                    let mut back = meridian(s * (c + w), 0.07, 0.94);
                    back.reverse();
                    pts.extend(back);
                    items.push(shape(&pts, Style::fill(tone, "sp-skin-stripe").op(0.6)));
                }
            }
        }
        "speckles" => {
            // Two bytes to a speckle: how far down the body, and how far
            // across; the lowest bits of the second set its size.
            let n = 8.0 + 3.0 * more() + f64::from(st.next() % 6);
            for _ in 0..n as usize {
                let y = y_at(lerp(0.08, 0.92, st.unit()));
                let bb = st.next();
                let x = (f64::from(bb) / 127.5 - 1.0) * b.hw(y) * 0.86;
                items.push(disc(
                    x,
                    y,
                    lerp(1.8, 3.8, f64::from(bb & 3) / 3.0),
                    Style::fill(p.deep, "sp-speckle").op(0.5),
                ));
            }
        }
        "rings" => {
            if b.kind == Kind::Taproot {
                // The short scars a root carries where side roots left it.
                let n = 4.0 + 2.0 * more();
                for j in 0..n as usize {
                    let s = if j % 2 == 1 { 1.0 } else { -1.0 };
                    let y = y_at(lerp(0.44, 0.9, j as f64 / (n - 1.0)));
                    items.push(curve(
                        &ring(y, s * lerp(0.15, 0.4, st.unit()), s * 0.95, 2.0),
                        line(1.3),
                    ));
                }
            } else {
                // The nodes of a cane.
                let n = 2.0 + more();
                for j in 0..n as usize {
                    let y = y_at((j as f64 + 1.15) / (n + 1.1));
                    items.push(curve(&ring(y, -0.99, 0.99, 3.5), line(1.8)));
                    items.push(curve(
                        &ring(y + 3.0, -0.99, 0.99, 3.5),
                        Style::line(p.tone_up, 1.4, "sp-skin-line").op(0.8),
                    ));
                }
            }
        }
        "areoles" => {
            for row in 0..6 {
                let y = y_at(0.13 + 0.15 * f64::from(row));
                let w = b.hw(y);
                let fs: &[f64] = if row % 2 == 1 {
                    &[-0.5, 0.0, 0.5]
                } else {
                    &[-0.72, -0.26, 0.26, 0.72]
                };
                for &f in fs {
                    items.push(disc(f * w, y, 1.7, Style::fill(p.tone_down, "sp-areole")));
                }
            }
        }
        "scales" => {
            // Rows of bracts, each row set half a step across from the one
            // above. One bit for each bract and its mirror image says whether
            // the two are a shade darker.
            let dy = r.lin(o + 2, 15.0, 21.0);
            let pitch = r.lin(o + 3, 17.0, 24.0);
            let up = if r.chance(o + 4, 0.6) { -1.0 } else { 1.0 };
            let mut dark = Vec::new();
            let mut row = 0_usize;
            let mut k = 0_usize;
            let mut y = b.top + dy * 0.9;
            while y < b.bottom - 4.0 {
                let w = b.hw(y) + pitch;
                let mut x = if row % 2 == 1 { pitch / 2.0 } else { 0.0 };
                while x < w {
                    let shaded = st.bit(k);
                    k += 1;
                    let sides: &[f64] = if x > 0.1 { &[1.0, -1.0] } else { &[1.0] };
                    for &s in sides {
                        let arc = [
                            pt(s * x - pitch / 2.0, y),
                            pt(s * x, y + up * dy * 0.62),
                            pt(s * x + pitch / 2.0, y),
                        ];
                        if shaded {
                            let mut closed = arc.to_vec();
                            closed.push(pt(s * x, y + up * dy * 0.1));
                            dark.push(shape(
                                &closed,
                                Style::fill(p.tone_down, "sp-skin-shade").op(0.85),
                            ));
                        }
                        items.push(curve(&arc, line(1.5)));
                    }
                    x += pitch;
                }
                y += dy;
                row += 1;
            }
            dark.extend(items);
            items = dark;
        }
        "dimples" => {
            let n = 3.0 + more() + f64::from(st.next() % 3);
            for _ in 0..n as usize {
                let y = y_at(lerp(0.1, 0.9, st.unit()));
                let bb = st.next();
                let x = (f64::from(bb) / 127.5 - 1.0) * b.hw(y) * 0.8;
                let w = lerp(3.5, 6.0, f64::from(bb & 3) / 3.0);
                items.push(curve(
                    &[pt(x - w, y - 1.0), pt(x, y + w * 0.5), pt(x + w, y - 1.0)],
                    Style::line(p.deep, 1.3, "sp-skin-line").op(0.6),
                ));
                items.push(disc(
                    x,
                    y - 2.5,
                    1.2,
                    Style::fill(p.deep, "sp-speckle").op(0.6),
                ));
            }
        }
        "drupelets" => {
            // One bit for each drupelet and its mirror image says whether the
            // two catch the light.
            let rr = b.drupelet;
            let dy = rr * 1.5;
            let mut lit = Vec::new();
            let mut row = 0_usize;
            let mut k = 0_usize;
            let mut y = b.top + rr * 1.6;
            while y < b.bottom - rr * 0.6 {
                let w = b.hw(y);
                let mut x = if row % 2 == 1 { rr * 0.85 } else { 0.0 };
                while x < w {
                    let bright = st.bit(k);
                    k += 1;
                    let sides: &[f64] = if x > 0.1 { &[1.0, -1.0] } else { &[1.0] };
                    for &s in sides {
                        if bright {
                            lit.push(disc(
                                s * x,
                                y - rr * 0.1,
                                rr * 0.6,
                                Style::fill(p.pale, "sp-skin-shade").op(0.8),
                            ));
                        }
                        items.push(curve(
                            &[
                                pt(s * x - rr * 0.8, y - rr * 0.3),
                                pt(s * x, y + rr * 0.55),
                                pt(s * x + rr * 0.8, y - rr * 0.3),
                            ],
                            line(1.5),
                        ));
                    }
                    x += rr * 1.7;
                }
                y += dy;
                row += 1;
            }
            lit.extend(items);
            items = lit;
        }
        "tufts" => {
            // The scalloped lower edges of a shrub's lobes, and a few leaves
            // showing among them.
            for l in &b.lobes {
                items.push(curve(
                    &[
                        pt(l[0] - l[2] * 0.8, l[1] + l[2] * 0.5),
                        pt(l[0], l[1] + l[2] * 0.95),
                        pt(l[0] + l[2] * 0.8, l[1] + l[2] * 0.5),
                    ],
                    line(1.5),
                ));
            }
            let n = 5.0 + 2.0 * more();
            for _ in 0..n as usize {
                let y = y_at(lerp(0.1, 0.92, st.unit()));
                let bb = st.next();
                let x = (f64::from(bb) / 127.5 - 1.0) * b.hw(y) * 0.85;
                let v = rot(UP, f64::from(bb & 7) / 3.5 - 1.0);
                let mut leaf = lens([x, y], v, 8.0, 2.6, 0.15);
                *leaf.style_mut() = Style::fill(p.tone_down, "sp-skin-leaf").op(0.8);
                items.push(leaf);
            }
        }
        "florets" => {
            // The florets of a flower head: two rings of dots inside the rim.
            let c = [0.0, b.top + b.h / 2.0];
            for (k, n, rr) in [
                (0.84, 20.0 + 2.0 * more(), 1.7),
                (0.68, 14.0 + 2.0 * more(), 1.4),
            ] {
                for j in 0..n as usize {
                    let v = rot(UP, TAU * (j as f64 + if k < 0.8 { 0.5 } else { 0.0 }) / n);
                    items.push(disc(
                        c[0] + v[0] * b.w * k,
                        c[1] + v[1] * b.h / 2.0 * k,
                        rr,
                        Style::fill(p.deep, "sp-floret").op(0.5),
                    ));
                }
            }
        }
        "seam" => {
            let s = if r.chance(o + 2, 0.5) { 1.0 } else { -1.0 };
            items.push(curve(&meridian(s * 0.62, 0.05, 0.95), line(1.5)));
            for k in 0..b.seeds {
                let y = b.top + 8.0 + b.seed_h * (k as f64 + 0.5);
                let w = b.hw(y);
                items.push(curve_v(
                    &[
                        [-s * w * 0.62, y - b.seed_h * 0.22],
                        [-s * w * 0.4, y - b.seed_h * 0.34],
                        [-s * w * 0.14, y - b.seed_h * 0.3],
                    ],
                    Style::line(p.tone_up, 2.4, "sp-skin-line").op(0.85),
                ));
            }
        }
        _ => {
            if b.kind == Kind::Seed && r.chance(o + 2, 0.5) {
                // The scar where the seed was joined to its pod.
                items.push(oval(
                    0.0,
                    b.bottom - b.h * 0.15,
                    3.0,
                    6.5,
                    Style::fill(p.tone_down, "sp-skin-scar"),
                ));
            }
        }
    }
    // A soft highlight on a glossy skin.
    if r.chance(o + 5, 0.4) {
        let y = y_at(if b.kind == Kind::Bulb || b.kind == Kind::Gourd {
            0.5
        } else {
            0.24
        });
        let w = b.hw(y);
        items.push(oval(
            -w * 0.46,
            y,
            w * 0.16,
            b.h * 0.075,
            Style::fill(WHITE, "sp-gloss").op(0.22),
        ));
    }
    Skin { kind, items }
}
