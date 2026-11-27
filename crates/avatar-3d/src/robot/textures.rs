//! The face, stalk-eye and marks textures and their bindings.

use avatar_raster::{rasterize_tree, ImageOptions};
use avatar_small_robots::layout::Layer;
use avatar_small_robots::MOODS;

use super::build::Builder;
use super::{Texture, TextureBinding, TextureContent, TextureSurface, TEXTURE_PX};

/// Every texture: for each mood a face pair (as drawn, then blinking); on a
/// stalk robot a pair for each stalk eye in each mood; then the head's and
/// the chassis's marks where their marks calls return a drawing.
pub(super) fn build(b: &Builder, draw: bool) -> Vec<Texture> {
    let c = b.creature;
    let mut out = Vec::new();
    let part = |name: &str| b.parts.iter().position(|p| p.name == name);
    let head = part("head").expect("every robot has a head");
    let mut push = |layer: Layer, part: usize, content, surface, origin: [f64; 2]| {
        let pixels = draw.then(|| {
            rasterize_tree(&layer.tree, TEXTURE_PX, &ImageOptions::default())
                .expect("a layer tree rasterizes at the texture size")
                .pixels
        });
        out.push(Texture {
            binding: TextureBinding {
                part,
                content,
                surface,
                origin,
                extent: layer.region.extent,
            },
            pixels,
        });
    };
    for mood in MOODS {
        for blinking in [false, true] {
            if let Some(layer) = c.face_layer(mood, blinking) {
                let origin = layer.region.origin;
                push(
                    layer,
                    head,
                    TextureContent::Face { mood, blinking },
                    TextureSurface::TexturedFront,
                    origin,
                );
            }
        }
    }
    for (eye, name) in ["stalk.L.eye", "stalk.R.eye"].into_iter().enumerate() {
        let Some(index) = part(name) else { continue };
        let rest = b.layout.top.stalks[eye].rest.eye_centre;
        for mood in MOODS {
            // The pod hangs on its stalk's bone, so its projection stays at
            // rest; each mood's drawing is shifted back by how far that
            // mood moves the eye.
            let moved = b.layout.mood(mood).stalks[eye].eye_centre;
            for blinking in [false, true] {
                if let Some(layer) = c.stalk_eye_layer(eye, mood, blinking) {
                    let o = layer.region.origin;
                    let origin = [o[0] - (moved[0] - rest[0]), o[1] - (moved[1] - rest[1])];
                    push(
                        layer,
                        index,
                        TextureContent::StalkEye {
                            eye,
                            mood,
                            blinking,
                        },
                        TextureSurface::TexturedFront,
                        origin,
                    );
                }
            }
        }
    }
    if let Some(layer) = c.head_marks() {
        let origin = layer.region.origin;
        push(
            layer,
            head,
            TextureContent::Marks,
            TextureSurface::FacingFront,
            origin,
        );
    }
    if let (Some(layer), Some(chassis)) = (c.chassis_marks(), part("chassis")) {
        let origin = layer.region.origin;
        push(
            layer,
            chassis,
            TextureContent::Marks,
            TextureSurface::FacingFront,
            origin,
        );
    }
    out
}
