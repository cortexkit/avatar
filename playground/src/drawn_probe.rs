use avatar_svg::wild::{draw_full, shape as wild, Creature};
use avatar_svg::{Mood, Segment, Shape, TransformOp};

fn numbers(values: impl IntoIterator<Item = f64>) -> String {
    values
        .into_iter()
        .map(|v| format!("{v:.3}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn point(shape: &Shape) -> [f64; 2] {
    let (x, y) = match shape {
        Shape::Path { segments, .. } => match segments[0] {
            Segment::MoveTo { x, y } => (x, y),
            _ => panic!("a drawn path must start with MoveTo"),
        },
        Shape::Ellipse { cx, cy, .. } => (*cx, *cy),
        _ => panic!("unexpected mouth geometry"),
    };
    [x as f64 / 100.0, y as f64 / 100.0]
}

/// Inspect drawn face and adjunct primitives instead of trusting placement
/// coordinates alone: a correct search can still precede the wrong drawing.
pub fn fields(c: &Creature) -> String {
    let tree = draw_full(c, Mood::Neutral, 120.0, 120.0);
    let body = tree.iter().find(|p| p.label == "wild-body").unwrap();
    let Shape::Path { segments, .. } = &body.shape else {
        panic!("body must be a path")
    };
    let points: Vec<_> = segments
        .iter()
        .map(|s| match *s {
            Segment::MoveTo { x, y } | Segment::LineTo { x, y } => {
                (x as f64 / 100.0, y as f64 / 100.0)
            }
            _ => panic!("body outline must be polygonal"),
        })
        .collect();
    let (x0, y0, x1, y1) = wild::bbox(&points);

    // The fit bounds include true cubic extrema, not just sampled vertices.
    // Report the fit factor as well as the independently drawn body bounds.
    let raw = wild::build_raw(&c.wb, c.dom);
    let mean_r = c.wb.spokes.iter().map(|s| s.r).sum::<f64>() / c.wb.spokes.len() as f64;
    let (mut bx0, mut by0, mut bx1, mut by1) = raw.bbox;
    for e in &c.exts {
        let ring_r = wild::radius_along(&raw.pts, e.theta).unwrap_or(mean_r);
        for (x, y) in wild::extremity_outline(e, ring_r, mean_r, false) {
            bx0 = bx0.min(x).min(-x);
            bx1 = bx1.max(x).max(-x);
            by0 = by0.min(y);
            by1 = by1.max(y);
        }
    }
    let scale = (172.0 / (bx1 - bx0).max(1e-6)).min(198.0 / (by1 - by0).max(1e-6));
    let blush = tree
        .iter()
        .filter(|p| p.label == "wild-blush")
        .flat_map(|p| {
            let Shape::Ellipse { cx, cy, rx, ry } = p.shape else {
                panic!("blush must be an ellipse")
            };
            [cx, cy, rx, ry].map(|v| v as f64 / 100.0)
        });
    let mouth = tree.iter().find(|p| p.label.starts_with("wild-mouth-"));
    let (mouth_point, mouth_scale) = mouth.map_or_else(
        || ("none".to_owned(), "none".to_owned()),
        |p| {
            let mut values = [1.0, 1.0, 0.0, 0.0];
            if let Some(transform) = &p.transform {
                for op in &transform.ops {
                    if let TransformOp::Scale { sx, sy } = op {
                        values[0] = sx.0 as f64 / (1_u64 << 32) as f64;
                        values[1] = sy.0 as f64 / (1_u64 << 32) as f64;
                    }
                }
                if let Some(TransformOp::Translate { tx, ty }) = transform.ops.first() {
                    values[2] = *tx as f64 / 100.0;
                    values[3] = *ty as f64 / 100.0;
                }
            }
            (
                numbers(point(&p.shape)),
                format!(
                    "{:.6},{:.6},{:.3},{:.3}",
                    values[0], values[1], values[2], values[3]
                ),
            )
        },
    );
    let sleepy = draw_full(c, Mood::Sleepy, 120.0, 120.0);
    let zz: Vec<_> = sleepy.iter().filter(|p| p.label == "wild-mood-z").collect();
    let zz_geometry = zz.iter().flat_map(|p| {
        let Shape::Path { segments, .. } = &p.shape else {
            panic!("Z glyph must be a path")
        };
        segments
            .iter()
            .flat_map(|s| match *s {
                Segment::MoveTo { x, y } | Segment::LineTo { x, y } => {
                    [x as f64 / 100.0, y as f64 / 100.0]
                }
                _ => panic!("Z glyph must use straight lines"),
            })
            .chain([
                p.stroke.unwrap().width as f64 / 100.0,
                p.opacity.unwrap().0 as f64 / (1_u64 << 32) as f64,
            ])
    });
    format!(
        " scale={scale:.6} drawn={} blush={} mouth={} mouthscale={} zzcount={} zz={}",
        numbers([x0, y0, x1, y1]),
        numbers(blush),
        mouth_point,
        mouth_scale,
        zz.len(),
        if zz.is_empty() {
            "none".to_owned()
        } else {
            numbers(zz_geometry)
        }
    )
}
