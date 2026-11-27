//! Textures and their bindings, from the exported data alone.

use avatar_3d::robot::{
    build, build_with, BuildOptions, Robot, TextureBinding, TextureContent, TextureSurface,
    TEXTURE_BYTES,
};
use avatar_small_robots::layout::{Layout, LAYER_MARGIN};
use avatar_small_robots::{decode, Mood};

use crate::cases;

fn corpus(id: &str) -> cases::Genome {
    cases::corpus()
        .into_iter()
        .find(|(name, _)| name == id)
        .unwrap_or_else(|| panic!("{id} is a corpus genome"))
        .1
}

/// The texture coordinate of a projection point by the binding's stated
/// rule, `(projection − origin) / C`.
fn raw_uv(b: &TextureBinding, p: [f64; 2]) -> [f64; 2] {
    let c = f64::from(b.extent);
    [(p[0] - b.origin[0]) / c, (p[1] - b.origin[1]) / c]
}

/// The same, or `None` outside `[0, 1]²`, where the texture contributes
/// nothing.
fn uv(b: &TextureBinding, p: [f64; 2]) -> Option<[f64; 2]> {
    let uv = raw_uv(b, p);
    uv.iter().all(|v| (0.0..=1.0).contains(v)).then_some(uv)
}

fn close(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-12 && (a[1] - b[1]).abs() < 1e-12
}

#[test]
fn face_and_marks_bindings_map_their_canvases_by_hand() {
    // wild-port-0031's head carries a face canvas and a smaller marks canvas
    // at other fractional origins.
    let robot = build_with(&corpus("wild-port-0031"), BuildOptions::untextured());
    let head = robot.parts.iter().position(|p| p.name == "head").unwrap();
    let face = robot
        .textures
        .iter()
        .find(|t| {
            t.binding.content
                == TextureContent::Face {
                    mood: Mood::Neutral,
                    blinking: false,
                }
        })
        .unwrap()
        .binding;
    let marks = robot
        .textures
        .iter()
        .find(|t| t.binding.content == TextureContent::Marks && t.binding.part == head)
        .unwrap()
        .binding;
    assert_eq!(
        (face.part, face.surface),
        (head, TextureSurface::TexturedFront)
    );
    assert_eq!(marks.surface, TextureSurface::FacingFront);
    assert_eq!((face.origin, face.extent), ([-43.93, -90.73], 88));
    assert_eq!((marks.origin, marks.extent), ([-41.6, -20.8], 16));
    // Corners and centre of each canvas, worked out by hand from its origin
    // and edge.
    let cases: [(TextureBinding, [[f64; 2]; 5]); 2] = [
        (
            face,
            [
                [-43.93, -90.73],
                [44.07, -90.73],
                [-43.93, -2.73],
                [44.07, -2.73],
                [0.07, -46.73],
            ],
        ),
        (
            marks,
            [
                [-41.6, -20.8],
                [-25.6, -20.8],
                [-41.6, -4.8],
                [-25.6, -4.8],
                [-33.6, -12.8],
            ],
        ),
    ];
    let want = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0], [0.5, 0.5]];
    for (binding, points) in cases {
        for (p, w) in points.iter().zip(want) {
            let got = raw_uv(&binding, *p);
            assert!(close(got, w), "{p:?} maps to {got:?}, not {w:?}");
        }
        // The library's own lookup agrees inside the canvas.
        let centre = binding.uv(points[4]).expect("the centre is on the canvas");
        assert!(close(centre, [0.5, 0.5]));
        // One robot unit moves u and v by the same 1 / C: the canvas keeps
        // its aspect.
        let c = points[4];
        let base = uv(&binding, c).unwrap();
        let du = uv(&binding, [c[0] + 1.0, c[1]]).unwrap()[0] - base[0];
        let dv = uv(&binding, [c[0], c[1] + 1.0]).unwrap()[1] - base[1];
        assert!((du - 1.0 / f64::from(binding.extent)).abs() < 1e-12);
        assert!((du - dv).abs() < 1e-12);
        // Just outside the canvas there is no contribution.
        assert_eq!(uv(&binding, [points[0][0] - 0.5, c[1]]), None);
        assert_eq!(binding.uv([points[0][0] - 0.5, c[1]]), None);
        assert_eq!(uv(&binding, [c[0], points[3][1] + 0.5]), None);
    }
    // The face canvas's centre lies outside the small marks canvas.
    assert_eq!(uv(&marks, [0.07, -46.73]), None);
}

fn count(robot: &Robot, marks: bool) -> usize {
    robot
        .textures
        .iter()
        .filter(|t| (t.binding.content == TextureContent::Marks) == marks)
        .count()
}

#[test]
fn hulls_without_marks_have_no_marks_texture() {
    // wild-port-0005 is a pod whose head carries no marks: its 14 textures
    // are the face pair of each of the seven moods.
    let genome = corpus("wild-port-0005");
    let creature = decode(&genome);
    assert!(creature.head_marks().is_none() && creature.chassis_marks().is_none());
    let robot = build_with(&genome, BuildOptions::untextured());
    assert_eq!((count(&robot, true), count(&robot, false)), (0, 14));
    assert_eq!(robot.texture_bytes(), 14 * TEXTURE_BYTES);
    // A chassis without rivets gets no texture of its own.
    for (label, genome) in cases::cases(cases::committed_prefix()) {
        let creature = decode(&genome);
        let robot = build_with(&genome, BuildOptions::untextured());
        for (name, layer) in [
            ("head", creature.head_marks()),
            ("chassis", creature.chassis_marks()),
        ] {
            let part = robot.parts.iter().position(|p| p.name == name);
            let textures = robot
                .textures
                .iter()
                .filter(|t| {
                    Some(t.binding.part) == part && t.binding.content == TextureContent::Marks
                })
                .count();
            assert_eq!(
                textures,
                usize::from(layer.is_some() && part.is_some()),
                "{label}: {name}"
            );
        }
    }
}

#[test]
fn built_textures_hold_their_pixels() {
    let robot = build(&corpus("wild-port-0031"));
    // The face pair of each of the seven moods, the head's and the
    // chassis's marks, and the drawing on its chest plate.
    let of = |want: fn(&TextureContent) -> bool| {
        robot
            .textures
            .iter()
            .filter(|t| want(&t.binding.content))
            .count()
    };
    assert_eq!(
        (
            of(|c| matches!(c, TextureContent::Face { .. })),
            of(|c| *c == TextureContent::Marks),
            of(|c| *c == TextureContent::Chest),
        ),
        (14, 2, 1)
    );
    assert_eq!(robot.textures.len(), 17);
    for t in &robot.textures {
        let pixels = t.pixels.as_ref().expect("a full build draws every texture");
        assert_eq!(pixels.len(), TEXTURE_BYTES);
        assert!(
            pixels.chunks(4).any(|p| p[3] > 0),
            "{:?} is blank",
            t.binding.content
        );
    }
    let untextured = build_with(&corpus("wild-port-0031"), BuildOptions::untextured());
    assert!(untextured.textures.iter().all(|t| t.pixels.is_none()));
    let bindings = |r: &Robot| r.textures.iter().map(|t| t.binding).collect::<Vec<_>>();
    assert_eq!(bindings(&robot), bindings(&untextured));
}

#[test]
fn chest_plates_carry_their_drawing_and_lit_chests_do_not() {
    let mut painted = 0;
    for (label, genome) in cases::cases(cases::committed_prefix()) {
        let creature = decode(&genome);
        let kind = Layout::new(&creature).chest.kind;
        let robot = build_with(&genome, BuildOptions::untextured());
        let chests: Vec<_> = robot
            .textures
            .iter()
            .filter(|t| t.binding.content == TextureContent::Chest)
            .collect();
        // A lamp, gauge or LED chest is geometry that moves or lights up,
        // so it is not drawn a second time.
        let plate = !matches!(kind, "none" | "light" | "gauge" | "leds");
        assert_eq!(chests.len(), usize::from(plate), "{label}: {kind}");
        let Some(t) = chests.first() else { continue };
        painted += 1;
        let layer = creature.chest_layer().expect("a plate chest has a drawing");
        let b = t.binding;
        assert_eq!(robot.parts[b.part].name, "chest", "{label}");
        assert_eq!(
            (b.origin, b.extent, b.surface),
            (
                layer.region.origin,
                layer.region.extent,
                TextureSurface::TexturedFront
            ),
            "{label}"
        );
        // The plate's textured front holds the whole drawing.
        let mesh = &robot.parts[b.part].mesh;
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for (i, tri) in mesh.triangles.iter().enumerate() {
            if mesh.textured_front[i] {
                for v in tri {
                    let p = mesh.projection[*v as usize];
                    for k in 0..2 {
                        lo[k] = lo[k].min(p[k]);
                        hi[k] = hi[k].max(p[k]);
                    }
                }
            }
        }
        let r = layer.region;
        for k in 0..2 {
            assert!(
                lo[k] <= r.origin[k] + LAYER_MARGIN + 1e-9
                    && hi[k] >= r.origin[k] + r.size[k] - LAYER_MARGIN - 1e-9,
                "{label}: the plate {lo:?}..{hi:?} misses the drawing {r:?}"
            );
        }
    }
    assert!(painted > 0, "no case has a plate chest");
}
