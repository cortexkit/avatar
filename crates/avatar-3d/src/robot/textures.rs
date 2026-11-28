//! The face, stalk-eye and marks textures and their bindings.

use avatar_raster::{rasterize_tree, ImageOptions};
use avatar_small_robots::layout::{Layer, Layout};
use avatar_small_robots::{decode, Creature, Mood, GENOME_LEN, MOODS};

use super::build::Builder;
use super::fittings::stalk_name;
use super::{BuildOptions, Texture, TextureBinding, TextureContent, TextureSurface};

/// A texture before its part's index is looked up and its pixels are drawn:
/// the part's name, the binding's other fields and the layer to draw.
struct Planned {
    part: &'static str,
    content: TextureContent,
    surface: TextureSurface,
    origin: [f64; 2],
    layer: Layer,
}

/// The face sets, which lead the robot's textures: for each mood a face
/// pair (as drawn, then blinking); then on a stalk robot a pair for each
/// stalk eye in each mood. Only the creature and its layout are read, so
/// one mood's set can be drawn without building the robot, with the same
/// binding indices the built robot has.
fn face_plan(c: &Creature, layout: &Layout) -> Vec<Planned> {
    let mut out = Vec::new();
    for mood in MOODS {
        for blinking in [false, true] {
            if let Some(layer) = c.face_layer(mood, blinking) {
                out.push(Planned {
                    part: "head",
                    content: TextureContent::Face { mood, blinking },
                    surface: TextureSurface::TexturedFront,
                    origin: layer.region.origin,
                    layer,
                });
            }
        }
    }
    for (eye, part) in ["stalk.L.eye", "stalk.R.eye"].into_iter().enumerate() {
        // The fittings give every stalk of the layout an eye part named
        // after its side, so the layout says which eye parts the robot has.
        let built = layout
            .top
            .stalks
            .iter()
            .any(|s| format!("{}.eye", stalk_name(s)) == part);
        if !built {
            continue;
        }
        let rest = layout.top.stalks[eye].rest.eye_centre;
        for mood in MOODS {
            // The pod hangs on its stalk's bone, so its projection stays at
            // rest; each mood's drawing is shifted back by how far that
            // mood moves the eye.
            let moved = layout.mood(mood).stalks[eye].eye_centre;
            for blinking in [false, true] {
                if let Some(layer) = c.stalk_eye_layer(eye, mood, blinking) {
                    let o = layer.region.origin;
                    out.push(Planned {
                        part,
                        content: TextureContent::StalkEye {
                            eye,
                            mood,
                            blinking,
                        },
                        surface: TextureSurface::TexturedFront,
                        origin: [o[0] - (moved[0] - rest[0]), o[1] - (moved[1] - rest[1])],
                        layer,
                    });
                }
            }
        }
    }
    out
}

/// The mood of a face or stalk-eye texture, or `None` for marks and the
/// chest, which every mood shares.
fn face_mood(content: TextureContent) -> Option<Mood> {
    match content {
        TextureContent::Face { mood, .. } | TextureContent::StalkEye { mood, .. } => Some(mood),
        _ => None,
    }
}

/// Draws a layer at `px` × `px`: RGBA8, rows top first, sRGB premultiplied
/// by alpha.
fn rasterize(layer: &Layer, px: u32) -> Vec<u8> {
    rasterize_tree(&layer.tree, px, &ImageOptions::default())
        .expect("a layer tree rasterizes at every texture edge")
        .pixels
}

/// Every texture, in binding order: the face sets ([`face_plan`]), then the
/// head's and the chassis's marks where their marks calls return a drawing,
/// then the chest's drawing where the chest is a plate. With textures on,
/// the marks, the chest and the face sets of `options.moods` are drawn at
/// `options.texture_px`; every other texture keeps its binding without
/// pixels.
pub(super) fn build(b: &Builder, options: BuildOptions) -> Vec<Texture> {
    let c = b.creature;
    let part = |name: &str| b.parts.iter().position(|p| p.name == name);
    let mut planned = face_plan(c, b.layout);
    if let Some(layer) = c.head_marks() {
        planned.push(Planned {
            part: "head",
            content: TextureContent::Marks,
            surface: TextureSurface::FacingFront,
            origin: layer.region.origin,
            layer,
        });
    }
    if let (Some(layer), Some(_)) = (c.chassis_marks(), part("chassis")) {
        planned.push(Planned {
            part: "chassis",
            content: TextureContent::Marks,
            surface: TextureSurface::FacingFront,
            origin: layer.region.origin,
            layer,
        });
    }
    // Only a plate chest has a part named `chest`; its projection is in the
    // chest's frame, where the layer's canvas lies.
    if let (Some(layer), Some(_)) = (c.chest_layer(), part("chest")) {
        planned.push(Planned {
            part: "chest",
            content: TextureContent::Chest,
            surface: TextureSurface::TexturedFront,
            origin: layer.region.origin,
            layer,
        });
    }
    planned
        .into_iter()
        .map(|p| {
            let drawn =
                options.textures && face_mood(p.content).is_none_or(|m| options.moods.contains(m));
            Texture {
                binding: TextureBinding {
                    part: part(p.part).unwrap_or_else(|| {
                        panic!("the part {} a texture lies on is built", p.part)
                    }),
                    content: p.content,
                    surface: p.surface,
                    origin: p.origin,
                    extent: p.layer.region.extent,
                },
                px: options.texture_px,
                pixels: drawn.then(|| rasterize(&p.layer, options.texture_px)),
            }
        })
        .collect()
}

/// One mood's face set drawn at `px`, each texture with the index of its
/// binding in the robot's textures; see [`super::face_set`].
pub(super) fn face_set(genome: &[u8; GENOME_LEN], mood: Mood, px: u32) -> Vec<(usize, Vec<u8>)> {
    let creature = decode(genome);
    let layout = Layout::new(&creature);
    face_plan(&creature, &layout)
        .into_iter()
        .enumerate()
        .filter(|(_, p)| face_mood(p.content) == Some(mood))
        .map(|(index, p)| (index, rasterize(&p.layer, px)))
        .collect()
}
