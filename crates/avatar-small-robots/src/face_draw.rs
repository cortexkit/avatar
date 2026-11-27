//! Drawing the face: eyes on a screen or behind lenses, their shutters, the
//! brows, the mouth, the cheeks and a tear.

use avatar_svg::Rgb;

use crate::draw::{clip_of, Ctx, El, Out};
use crate::face::{BrowKind, Eye, Face, Form, MouthKind, Tech};
use crate::geom::{line_path, poly_path, quad, quad_at, rrect, shift_pts, Cp, Path};
use crate::hull::Hull;
use crate::kit::{blend, hex, lerp, max, min, min3, Palette, P2};
use crate::mood::{EyeSpec, MoodSpec, Shut};

/// The outline of an eye scaled by `k`, `sx` and `sy`, about (x, y).
fn eye_pts(e: &Eye, x: f64, y: f64, k: f64, sx: f64, sy: f64) -> Vec<Cp> {
    shift_pts(
        &quad(
            -e.ry * k * sy,
            e.ry * k * sy,
            e.rx * k * sx,
            e.rx * k * sx,
            e.qx * k * sx,
            e.qy * k * sy,
            e.qx * k * sx,
            e.qy * k * sy,
        ),
        x,
        y,
    )
}

fn side_class(e: &Eye) -> (&'static str, &'static str) {
    if e.side < 0.0 {
        ("rb-eye rb-eye-l", "rb-lid rb-lid-l")
    } else if e.side > 0.0 {
        ("rb-eye rb-eye-r", "rb-lid rb-lid-r")
    } else {
        ("rb-eye rb-eye-c", "rb-lid rb-lid-c")
    }
}

struct Lid {
    upper: bool,
    d: Path,
    edge: Path,
}

/// The shutters over one eye whose middle is (x, y): each a closed path and
/// the line along its edge. The upper one comes down to a slanted line (a
/// V on an eye in the middle); the lower one comes up as a hill.
fn lid_paths(e: &Eye, x: f64, y: f64, rx: f64, ry: f64, s: &EyeSpec) -> Vec<Lid> {
    let mut out = Vec::new();
    if s.upper > 0.01 {
        let x0 = x - rx * 1.6;
        let x1 = x + rx * 1.6;
        let yc = y - ry + 2.0 * ry * s.upper;
        let rise = s.slant * rx * 1.6;
        let mid = e.side == 0.0;
        let yl = if mid {
            yc - rise * 0.6
        } else {
            yc + rise * e.side
        };
        let yr = if mid {
            yc - rise * 0.6
        } else {
            yc - rise * e.side
        };
        let ym = if mid { yc + rise * 0.6 } else { yc };
        out.push(Lid {
            upper: true,
            d: line_path(
                &[
                    [x0, y - ry * 2.4],
                    [x1, y - ry * 2.4],
                    [x1, yr],
                    [x, ym],
                    [x0, yl],
                ],
                true,
            ),
            edge: line_path(&[[x1, yr], [x, ym], [x0, yl]], false),
        });
    }
    if s.lower > 0.01 {
        let x0 = x - rx * 1.15;
        let x1 = x + rx * 1.15;
        let yb = y + ry - 2.0 * ry * s.lower;
        let a = s.arch * ry * 0.55;
        let curve = |d: &mut Path| d.quad_to([x0, yb + a], [x, yb - 3.0 * a], [x1, yb + a]);
        let mut d = Path::new();
        d.move_to([x0, y + ry * 2.4]);
        d.line_to([x0, yb + a]);
        curve(&mut d);
        d.line_to([x1, y + ry * 2.4]);
        d.close();
        let mut edge = Path::new();
        edge.move_to([x0, yb + a]);
        curve(&mut edge);
        out.push(Lid {
            upper: false,
            d,
            edge,
        });
    }
    out
}

/// The stroke that stands for a shut eye: flat, an arch, a sag, or a
/// chevron that points at the middle of the face.
fn shut_path(e: &Eye, x: f64, y: f64, w: f64, h: f64, form: Shut) -> Path {
    let mut d = Path::new();
    match form {
        Shut::ArcUp => {
            d.move_to([x - w, y + h * 0.5]);
            d.quad_to([x - w, y + h * 0.5], [x, y - h * 1.5], [x + w, y + h * 0.5]);
            d
        }
        Shut::ArcDown => {
            d.move_to([x - w, y - h * 0.3]);
            d.quad_to([x - w, y - h * 0.3], [x, y + h * 1.3], [x + w, y - h * 0.3]);
            d
        }
        Shut::Chevron if e.side != 0.0 => line_path(
            &[
                [x + e.side * w * 0.8, y - h],
                [x - e.side * w * 0.6, y],
                [x + e.side * w * 0.8, y + h],
            ],
            false,
        ),
        _ => line_path(&[[x - w, y], [x + w, y]], false),
    }
}

/// An eye drawn by a screen: a lit shape, cut down by shutters of the
/// screen's own colour.
fn screen_eye(out: &mut Out, ctx: &Ctx, f: &Face, e: &Eye, s: &EyeSpec, m: &MoodSpec) {
    let on = f.on(m.screen);
    let back = f.back;
    let look = s.look;
    let rx = e.rx * s.sx;
    let ry = e.ry * s.sy;
    let x = e.x + look[0] * e.rx * 0.34;
    let y = e.y + look[1] * e.ry * 0.3;
    let (eye_class, lid_class) = side_class(e);
    if let Some(closed) = s.closed {
        let d = shut_path(e, x, y, e.rx * 1.1, max(e.ry * 0.55, 3.0), closed);
        out.group(eye_class, &[], |out| {
            out.put(El::path(&d).stroke(on, ctx.face_w).round());
        });
        return;
    }
    let d = poly_path(&eye_pts(e, x, y, 1.0, s.sx, s.sy));
    let ring = e.rim.filter(|_| ctx.fine);
    out.group(eye_class, &[], |out| {
        out.put(El::path(&d).class("rb-eye-lit").fill(on));
        if let Some(rim) = ring {
            let ix = rx - rim;
            let iy = ry - rim;
            let pr = min3(e.dot * s.pupil, ix - 0.6, iy - 0.6);
            out.put(El::ellipse(x, y, ix, iy).class("rb-eye-open").fill(back));
            out.put(
                El::circle(
                    [x + look[0] * (ix - pr) * 0.8, y + look[1] * (iy - pr) * 0.8],
                    pr,
                )
                .class("rb-pupil")
                .fill(on),
            );
        } else if e.hole != 0.0 && (ctx.fine || !e.hole_fine) {
            out.put(
                El::circle(
                    [x + look[0] * rx * 0.3, y + look[1] * ry * 0.3],
                    min(rx, ry) * e.hole * s.pupil,
                )
                .class("rb-pupil")
                .fill(back),
            );
        } else if ctx.fine && !f.lcd {
            out.put(
                El::ellipse(x - rx * 0.28, y - ry * 0.36, rx * 0.3, ry * 0.24)
                    .class("rb-eye-core")
                    .fill(f.on_lite(m.screen))
                    .opacity(0.85),
            );
        }
    });
    out.group(lid_class, &[], |out| {
        for lid in lid_paths(e, x, y, rx, ry, s) {
            let clip = clip_of(&d);
            out.put(
                El::path(&lid.d)
                    .class(if lid.upper {
                        "rb-lid-upper"
                    } else {
                        "rb-lid-lower"
                    })
                    .fill(back)
                    .clip(clip.clone()),
            );
            // A shutter that cuts a ring open closes it again along its own edge.
            if let Some(rim) = ring {
                out.put(
                    El::path(&lid.edge)
                        .class("rb-lid-edge")
                        .stroke(on, rim)
                        .round_join()
                        .clip(clip),
                );
            }
        }
    });
}

/// A camera lens or a lamp: a rim, the glass, an iris that glows and a
/// pupil, with painted shutters that close over the glass.
pub(crate) fn lens_eye(
    out: &mut Out,
    ctx: &Ctx,
    f: &Face,
    p: &Palette,
    e: &Eye,
    s: &EyeSpec,
    m: &MoodSpec,
) {
    let look = s.look;
    let r = min(e.rx, e.ry);
    let light = m.light;
    let (eye_class, lid_class) = side_class(e);
    // A lens on a stalk, or a cyclops's eye, has a metal housing; lenses
    // side by side sit behind a bevel of the hull's own paint.
    let housed = e.pod || f.tech == Tech::Mono;
    let (rim_base, rim_line) = if housed {
        (p.metal.base, p.metal.line)
    } else {
        (p.paint.dark, p.paint.line)
    };
    let g = if housed { 0.8 } else { 0.87 };
    let glass_d = poly_path(&eye_pts(e, e.x, e.y, g, 1.0, 1.0));
    out.group(eye_class, &[], |out| {
        out.put(
            El::path(&poly_path(&eye_pts(e, e.x, e.y, 1.0, 1.0, 1.0)))
                .class("rb-eye-rim")
                .fill(rim_base)
                .stroke(rim_line, ctx.line_w * 0.8)
                .round_join(),
        );
        let ix = e.x + look[0] * e.rx * 0.2;
        let iy = e.y + look[1] * e.ry * 0.2;
        if e.lamp {
            out.put(
                El::path(&glass_d)
                    .class("rb-eye-glass")
                    .fill(blend(p.glow_dim, p.glow, light)),
            );
            out.put(
                El::circle([ix, iy], r * (f.iris - 0.08))
                    .class("rb-eye-core")
                    .fill(blend(p.glow_dim, p.glow_lite, light)),
            );
            if f.lamp_pupil {
                out.put(
                    El::circle([ix, iy], r * (f.pupil - 0.05) * s.pupil)
                        .class("rb-pupil")
                        .fill(p.glass),
                );
            }
        } else {
            out.put(El::path(&glass_d).class("rb-eye-glass").fill(p.glass));
            out.put(
                El::circle([ix, iy], r * f.iris)
                    .class("rb-iris")
                    .fill(blend(p.glass, p.glow, lerp(0.4, 1.0, light))),
            );
            out.put(
                El::circle([ix, iy], r * f.pupil * s.pupil)
                    .class("rb-pupil")
                    .fill(p.glass),
            );
        }
        if r * ctx.px >= 2.6 {
            out.put(
                El::circle([e.x + f.glint * r * 0.3, e.y - r * 0.34], r * 0.13)
                    .class("rb-glint")
                    .fill(hex(0xffffff))
                    .opacity(0.9),
            );
        }
    });
    // The shutters are the hull's own paint, so a closed eye still belongs to the head.
    let clip = clip_of(&glass_d);
    let edge_w = max(1.5, ctx.line_w * 0.6);
    out.group(lid_class, &[], |out| {
        if let Some(closed) = s.closed {
            out.put(El::path(&glass_d).class("rb-lid-upper").fill(p.paint.base));
            let form = if closed == Shut::Chevron {
                Shut::Line
            } else {
                closed
            };
            out.put(
                El::path(&shut_path(
                    e,
                    e.x,
                    e.y + e.ry * 0.1,
                    e.rx * 0.8,
                    e.ry * 0.3,
                    form,
                ))
                .class("rb-lid-edge")
                .stroke(p.paint.line, edge_w)
                .round_cap()
                .clip(clip),
            );
        } else {
            for lid in lid_paths(e, e.x, e.y, e.rx * g, e.ry * g, s) {
                out.put(
                    El::path(&lid.d)
                        .class(if lid.upper {
                            "rb-lid-upper"
                        } else {
                            "rb-lid-lower"
                        })
                        .fill(p.paint.base)
                        .clip(clip.clone()),
                );
                out.put(
                    El::path(&lid.edge)
                        .class("rb-lid-edge")
                        .stroke(p.paint.line, edge_w)
                        .round()
                        .clip(clip.clone()),
                );
            }
        }
    });
}

/// A brow is a bar over an eye: lit on a screen, a metal plate on a hull.
fn brows(out: &mut Out, ctx: &Ctx, f: &Face, p: &Palette, m: &MoodSpec) {
    let Some(b) = m.brows else { return };
    if !f.brow_room || f.stalk.is_some() {
        return;
    }
    let screen = f.screen.is_some();
    let heavy = f.brow_kind == BrowKind::Heavy;
    out.group("rb-brows", &[], |out| {
        for e in &f.eyes {
            // The small lenses of a crown have none.
            if !e.big && f.tech == Tech::Tri && e.side != 0.0 && f.eyes[1].big {
                continue;
            }
            let len = e.rx * if screen { 1.0 } else { 0.95 };
            let skew = if b.skew != 0.0 && e.side == b.skew {
                2.5
            } else {
                0.0
            };
            let y0 = e.y - e.ry - (if screen { 4.5 } else { 6.0 }) - b.lift - skew;
            let rise = if e.side == 0.0 { 0.0 } else { b.slant * len };
            let d = if e.side == 0.0 {
                line_path(&[[e.x - len * 0.9, y0], [e.x + len * 0.9, y0]], false)
            } else {
                let inner = [e.x - e.side * len * 0.9, y0 + rise];
                let outer = [e.x + e.side * len * 0.9, y0 - rise];
                line_path(&[inner, outer], false)
            };
            let w = if screen {
                ctx.face_w * 0.85
            } else {
                max(if heavy { 5.6 } else { 4.0 }, ctx.face_w * 0.85)
            };
            let colour = if screen {
                f.on(m.screen)
            } else if p.dark {
                p.metal.dark
            } else {
                p.paint.line
            };
            let el = El::path(&d).class("rb-brow").stroke(colour, w);
            out.put(if screen {
                el.round_cap()
            } else {
                el.square_cap()
            });
        }
    });
}

enum Geo {
    Ellipse([f64; 4]),
    Line(Path, bool),
    Shape(Path),
}

/// The line of a mouth `w` to each side of (x, y), curved by `c` from -1
/// (a frown) to 1 (a smile); with `open`, a closed shape whose lower lip
/// hangs below it.
#[allow(clippy::too_many_arguments)]
fn mouth_geometry(form: Form, x: f64, y: f64, w: f64, dh: f64, c: f64, open: f64) -> Geo {
    let yc = y - c * dh;
    let ym = y + c * dh;
    let p0 = [x - w, yc];
    let p2 = [x + w, yc];
    let ctl = [x, 2.0 * ym - yc];
    if form == Form::O {
        let r = max(2.6, w * 0.5);
        return Geo::Ellipse([x, y + dh * 0.2, r, r * (0.75 + 0.5 * open)]);
    }
    if form == Form::Zig {
        let a = max(2.0, dh * 0.7);
        return Geo::Line(
            line_path(
                &[
                    [x - w, y + a],
                    [x - w / 2.0, y - a],
                    [x, y + a],
                    [x + w / 2.0, y - a],
                    [x + w, y + a],
                ],
                false,
            ),
            false,
        );
    }
    if form == Form::Wave {
        let a = max(2.0, dh * 0.6);
        let mut d = Path::new();
        d.move_to([x - w, y + a * 0.3]);
        for i in 0..4 {
            let x0 = x - w + f64::from(i) * w / 2.0;
            let bend = if i % 2 == 1 { a } else { -a };
            d.quad_to(
                [x0, y + a * 0.3],
                [x0 + w / 4.0, y + a * 0.3 + bend * 1.6],
                [x0 + w / 2.0, y + a * 0.3],
            );
        }
        return Geo::Line(d, false);
    }
    let mut d = Path::new();
    if open > 0.05 {
        // The upper lip is flatter than the line would be.
        let up = [x, 2.0 * (y + c * dh * 0.25) - yc];
        let low = [x, 2.0 * (ym + open * dh * 1.5) - yc];
        d.move_to(p0);
        d.quad_to(p0, up, p2);
        d.quad_to(p2, low, p0);
        d.close();
        return Geo::Shape(d);
    }
    if form == Form::Cat && c > 0.2 {
        let mid = [x, yc + c * dh * 0.5];
        d.move_to(p0);
        d.quad_to(p0, [x - w / 2.0, ym + c * dh * 1.3], mid);
        d.quad_to(mid, [x + w / 2.0, ym + c * dh * 1.3], p2);
        return Geo::Line(d, false);
    }
    if form == Form::Steps && (c * dh).abs() >= 1.6 {
        return Geo::Line(
            line_path(
                &[
                    p0,
                    [x - w * 0.52, yc],
                    [x - w * 0.52, ym],
                    [x + w * 0.52, ym],
                    [x + w * 0.52, yc],
                    p2,
                ],
                false,
            ),
            true,
        );
    }
    if form == Form::Vee {
        return Geo::Line(line_path(&[p0, [x, ym + c * dh * 0.7], p2], false), false);
    }
    d.move_to(p0);
    d.quad_to(p0, ctl, p2);
    // A flat mouth ends in two small ticks, like a slot cut with a square tool.
    if form == Form::Flat {
        d.move_to([x - w, yc - 2.6]);
        d.line_to([x - w, yc + 2.6]);
        d.move_to([x + w, yc - 2.6]);
        d.line_to([x + w, yc + 2.6]);
    }
    Geo::Line(d, false)
}

fn mouth_class(kind: MouthKind) -> &'static str {
    match kind {
        MouthKind::Line => "rb-mouth rb-mouth-line",
        MouthKind::Strip => "rb-mouth rb-mouth-strip",
        MouthKind::Slot => "rb-mouth rb-mouth-slot",
        MouthKind::Grid => "rb-mouth rb-mouth-grid",
        MouthKind::None => "rb-mouth",
    }
}

pub(crate) fn mouth(out: &mut Out, ctx: &Ctx, f: &Face, p: &Palette, md: &MoodSpec) {
    let m = &f.mouth;
    if m.kind == MouthKind::None {
        return;
    }
    let mm = md.mouth;
    let c = mm.c;
    let open = mm.open;
    let w = m.w * mm.w;
    let form = mm.form.unwrap_or(m.form);
    let x = m.x;
    let y = m.y;
    out.group(mouth_class(m.kind), &[], |out| {
        let mut lw = ctx.face_w * 0.9;
        let mut clip = None;
        let ink: Rgb;
        if m.kind == MouthKind::Strip {
            // A small display of its own, set into the hull.
            let panel = poly_path(&m.panel);
            out.put(
                El::path(&panel)
                    .class("rb-mouth-panel")
                    .fill(p.glass)
                    .stroke(p.metal.line, ctx.line_w * 0.7)
                    .round_join(),
            );
            ink = blend(p.glass, p.glow, lerp(0.4, 1.0, md.light));
            clip = clip_of(&panel);
            lw = min(lw, 4.2);
        } else if m.kind == MouthKind::Line {
            ink = f.on(md.screen);
        } else {
            ink = if p.dark { p.ink } else { p.paint.line };
        }
        if m.kind == MouthKind::Grid {
            // A grille: a band along the line of the mouth with bars across it.
            let t = m.t * (1.0 + open * 0.7) * if mm.w != 0.0 && mm.w < 0.7 { 0.8 } else { 1.0 };
            let cc = if form == Form::Zig || form == Form::Wave {
                -0.35
            } else {
                c
            };
            let yc = y - cc * m.dh;
            let ym = y + cc * m.dh;
            let upper: [P2; 3] = [
                [x - w, yc - t / 2.0],
                [x, 2.0 * ym - yc - t / 2.0],
                [x + w, yc - t / 2.0],
            ];
            let lower: [P2; 3] = [
                [x + w, yc + t / 2.0],
                [x, 2.0 * ym - yc + t / 2.0],
                [x - w, yc + t / 2.0],
            ];
            let mut d = Path::new();
            d.move_to(upper[0]);
            d.quad_to(upper[0], upper[1], upper[2]);
            d.line_to(lower[0]);
            d.quad_to(lower[0], lower[1], lower[2]);
            d.close();
            out.put(
                El::path(&d)
                    .class("rb-mouth-grille")
                    .fill(p.white)
                    .stroke(ink, max(1.6, ctx.line_w * 0.7))
                    .round_join(),
            );
            if ctx.fine {
                let mut bars = Path::new();
                for i in 1..=m.bars {
                    let u = f64::from(i) / f64::from(m.bars + 1);
                    let q = quad_at(upper[0], upper[1], upper[2], u);
                    bars.move_to(q);
                    bars.line_to([q[0], q[1] + t]);
                }
                if open > 0.3 || form == Form::Zig {
                    let a = [x - w, yc];
                    let b = [x + w, yc];
                    bars.move_to(a);
                    bars.quad_to(a, [x, 2.0 * ym - yc], b);
                }
                out.put(El::path(&bars).class("rb-mouth-bars").stroke(ink, 1.5));
            }
            return;
        }
        if form == Form::Bar {
            // A display that is busy shows how far it has got.
            let h = min(6.4, max(4.4, m.dh * 1.5));
            let d = poly_path(&rrect(x - w, y - h / 2.0, x + w, y + h / 2.0, h / 2.0));
            out.put(
                El::rect(x - w, y - h / 2.0, 2.0 * w * mm.lit, h)
                    .class("rb-mouth-bar")
                    .fill(ink)
                    .clip(clip_of(&d)),
            );
            out.put(
                El::path(&d)
                    .class("rb-mouth-line")
                    .stroke(ink, min(lw * 0.5, 2.4))
                    .round_join(),
            );
            return;
        }
        let slot = m.kind == MouthKind::Slot;
        match mouth_geometry(form, x, y, w, m.dh, c, open) {
            Geo::Ellipse(g) => {
                let mut e = El::ellipse(g[0], g[1], g[2], g[3])
                    .class("rb-mouth-line")
                    .stroke(ink, lw * 0.85)
                    .clip(clip);
                if slot {
                    e = e.fill(p.paint.line);
                }
                out.put(e);
            }
            Geo::Shape(d) => {
                let fill = if slot {
                    if p.dark {
                        hex(0x191419)
                    } else {
                        p.paint.line
                    }
                } else {
                    ink
                };
                out.put(
                    El::path(&d)
                        .class("rb-mouth-open")
                        .fill(fill)
                        .stroke(ink, lw * 0.7)
                        .round_join()
                        .clip(clip),
                );
            }
            Geo::Line(d, square) => {
                let e = El::path(&d)
                    .class("rb-mouth-line")
                    .stroke(ink, lw)
                    .clip(clip);
                out.put(if square { e.square_cap() } else { e.round() });
            }
        }
    });
}

/// A drop: a tear, or a bead of coolant.
fn drop_path(x: f64, y: f64, s: f64) -> Path {
    let mut d = Path::new();
    d.move_to([x, y - s]);
    d.cubic_to(
        [x + s * 0.7, y + s * 0.1],
        [x + s * 0.55, y + s],
        [x, y + s],
    );
    d.cubic_to(
        [x - s * 0.55, y + s],
        [x - s * 0.7, y + s * 0.1],
        [x, y - s],
    );
    d.close();
    d
}

pub(crate) fn bezel_pts(f: &Face) -> Option<Vec<crate::geom::Cp>> {
    let s = f.screen.as_ref()?;
    let b = f.bezel;
    Some(if s.oval {
        shift_pts(
            &quad(
                -s.h - b,
                s.h + b,
                s.w + b,
                s.w + b,
                s.w + b,
                s.h + b,
                s.w + b,
                s.h + b,
            ),
            0.0,
            s.cy,
        )
    } else {
        rrect(-s.w - b, s.cy - s.h - b, s.w + b, s.cy + s.h + b, s.r + b)
    })
}

/// The whole face, in the head's coordinates. Eyes on stalks are drawn
/// with their stalks, not here.
pub(crate) fn face(out: &mut Out, ctx: &Ctx, f: &Face, p: &Palette, hull: &Hull, m: &MoodSpec) {
    if let Some(s) = &f.screen {
        out.group("rb-screen", &[], |out| {
            if ctx.fine {
                let pts = bezel_pts(f).expect("a screen has a bezel");
                out.put(
                    El::path(&poly_path(&pts))
                        .class("rb-bezel")
                        .fill(p.metal.base)
                        .stroke(p.metal.line, ctx.line_w * 0.75)
                        .round_join(),
                );
            }
            // The glass is plain: it shows the eyes and the mouth and nothing else.
            out.put(
                El::path(&poly_path(&s.pts))
                    .class("rb-glass")
                    .fill(f.back)
                    .stroke(
                        if ctx.fine { f.back_line } else { p.metal.line },
                        if ctx.fine { 1.4 } else { ctx.line_w * 0.7 },
                    )
                    .round_join(),
            );
        });
    }
    if f.mouth.kind != MouthKind::None && f.screen.is_none() {
        mouth(out, ctx, f, p, m);
    }
    out.group("rb-eyes", &[], |out| {
        for e in &f.eyes {
            let spec = m.eye_for(e.side);
            if f.screen.is_some() {
                screen_eye(out, ctx, f, e, &spec, m);
            } else {
                lens_eye(out, ctx, f, p, e, &spec, m);
            }
        }
    });
    brows(out, ctx, f, p, m);
    if f.screen.is_some() {
        mouth(out, ctx, f, p, m);
    }
    if m.blush {
        if let Some(cheeks) = &f.cheeks {
            out.group("rb-cheeks", &[], |out| {
                for c in cheeks {
                    let e = El::ellipse(c[0], c[1], c[2], c[2] * 0.56);
                    out.put(if f.screen.is_some() {
                        e.fill(blend(
                            f.back,
                            if f.lcd { f.on_base } else { hex(0xff8fb0) },
                            if f.lcd { 0.35 } else { 0.85 },
                        ))
                    } else {
                        e.fill(p.blush).opacity(0.85)
                    });
                }
            });
        }
    }
    if m.tear {
        if let Some(t) = f.tear_at(m.tear_side, hull) {
            let d = drop_path(t[0], t[1], t[2]);
            out.group("rb-tear", &[], |out| {
                let e = El::path(&d);
                out.put(if f.screen.is_some() {
                    let e = e.fill(blend(f.back, p.drop, if f.lcd { 0.0 } else { 0.95 }));
                    if f.lcd {
                        e.stroke(f.on_base, 1.4)
                    } else {
                        e
                    }
                } else {
                    e.fill(p.drop).stroke(p.drop_edge, 1.2)
                });
            });
        }
    }
}
