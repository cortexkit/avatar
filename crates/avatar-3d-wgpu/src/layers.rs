//! Which of a robot's textures are on the GPU, and at which layer.
//!
//! The renderer keeps a robot's textures ([`Robot::textures`]) in one
//! mipmapped array texture. A texture is resident when a layer of that
//! array holds its pixels; where a texture that is not resident lies,
//! nothing is drawn. The bindings buffer the shader reads gives each
//! texture's canvas and, in its fourth lane, the layer holding it or −1
//! ([`binding_data`]), so the layers need not follow the binding order.
//!
//! [`Renderer::upload`] makes every texture resident at its own index, as
//! the `review_sheets` example draws every mood's faces at once.
//! [`Renderer::upload_consuming`] makes resident
//! only the textures that have pixels, packed. [`Renderer::restock`] moves
//! a robot to a new resident set, as [`Residency::plan`] lays it out.
//!
//! [`Renderer::upload`]: crate::Renderer::upload
//! [`Renderer::upload_consuming`]: crate::Renderer::upload_consuming
//! [`Renderer::restock`]: crate::Renderer::restock

use avatar_3d::robot::{Robot, TEXTURE_PX_MAX, TEXTURE_PX_MIN};

use crate::upload::mip_levels;

/// Where each texture of a robot lies in its array texture: one layer,
/// or none when it is not resident. Every layer has the same edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Residency {
    /// The edge of every layer, in pixels.
    px: u32,
    /// Per texture, in [`Robot::textures`] order, the layer holding it.
    layers: Vec<Option<u32>>,
}

/// How [`crate::Renderer::restock`] moves a robot from one resident set to
/// another: a new array texture holding exactly the new set, filled by
/// copies from the current one and by fresh uploads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The new resident set.
    pub residency: Residency,
    /// For each texture kept, in the order given: its layer in the current
    /// array texture and in the new one. Each is copied on the GPU with
    /// every mip level.
    pub copies: Vec<(u32, u32)>,
    /// For each texture uploaded fresh, in the order given: its layer in
    /// the new array texture.
    pub uploads: Vec<u32>,
}

impl Residency {
    /// Each of `bindings` textures resident at its own index, every layer
    /// `px` pixels a side.
    ///
    /// # Panics
    ///
    /// Panics unless `px` is a power of two from [`TEXTURE_PX_MIN`] to
    /// [`TEXTURE_PX_MAX`].
    pub fn every(bindings: usize, px: u32) -> Self {
        check_px(px);
        Self {
            px,
            layers: (0..bindings).map(|i| Some(i as u32)).collect(),
        }
    }

    /// The textures `resident` names, of `bindings` in all, packed in
    /// binding order: the k-th smallest index at layer k. The order of
    /// `resident` does not matter.
    ///
    /// # Panics
    ///
    /// Panics when `resident` names a texture twice or one past
    /// `bindings`, and as [`Residency::every`] for `px`.
    pub fn packed(bindings: usize, px: u32, resident: &[usize]) -> Self {
        check_px(px);
        let mut layers = vec![None; bindings];
        for &b in resident {
            assert!(b < bindings, "texture {b} of a robot with {bindings}");
            assert!(layers[b].is_none(), "texture {b} is named twice");
            layers[b] = Some(0);
        }
        for (next, layer) in layers.iter_mut().flatten().enumerate() {
            *layer = next as u32;
        }
        Self { px, layers }
    }

    /// The edge of every layer, in pixels.
    pub fn px(&self) -> u32 {
        self.px
    }

    /// The number of textures the robot has, resident or not.
    pub fn bindings(&self) -> usize {
        self.layers.len()
    }

    /// The layer holding texture `binding`, or `None` when it is not
    /// resident or the robot has no such texture.
    pub fn layer(&self, binding: usize) -> Option<u32> {
        self.layers.get(binding).copied().flatten()
    }

    /// The resident textures, in layer order.
    pub fn resident(&self) -> Vec<usize> {
        let mut resident: Vec<(u32, usize)> = self
            .layers
            .iter()
            .enumerate()
            .filter_map(|(b, l)| l.map(|l| (l, b)))
            .collect();
        resident.sort_unstable();
        resident.into_iter().map(|(_, b)| b).collect()
    }

    /// The number of resident textures.
    pub fn layer_count(&self) -> u32 {
        self.layers.iter().flatten().count() as u32
    }

    /// The layers the array texture has: one per resident texture, and one
    /// left transparent when none is, since an array texture cannot be
    /// empty.
    pub fn texture_layers(&self) -> u32 {
        self.layer_count().max(1)
    }

    /// The bytes of the array texture, every mip level included.
    pub fn texture_bytes(&self) -> u64 {
        u64::from(self.texture_layers()) * layer_bytes(self.px)
    }

    /// The bytes of the resident textures' layers, every mip level
    /// included: [`Residency::texture_bytes`] less the transparent layer an
    /// empty set still takes.
    pub fn resident_bytes(&self) -> u64 {
        u64::from(self.layer_count()) * layer_bytes(self.px)
    }

    /// Lays out the change to a resident set of `keep` and `fresh` at `px`
    /// pixels a side, packed in binding order ([`Residency::packed`]). The
    /// textures in `keep` are copied from their current layers, so they
    /// must be resident now at the same edge; the textures in `fresh` are
    /// uploaded from new pixels, which may replace a resident texture's.
    /// A change of edge therefore keeps nothing: every texture the new set
    /// needs is uploaded fresh at the new edge.
    ///
    /// # Panics
    ///
    /// Panics when a texture in `keep` is not resident, when `keep` is not
    /// empty and `px` differs from the current edge, when a texture is
    /// named twice in `keep` and `fresh` together, and as
    /// [`Residency::packed`].
    pub fn plan(&self, px: u32, keep: &[usize], fresh: &[usize]) -> Plan {
        assert!(
            keep.is_empty() || px == self.px,
            "a change of edge from {} to {px} px uploads every layer fresh, \
             so it cannot keep {} layers",
            self.px,
            keep.len()
        );
        for &b in keep {
            assert!(
                self.layer(b).is_some(),
                "texture {b} is kept but not resident"
            );
        }
        let named: Vec<usize> = keep.iter().chain(fresh).copied().collect();
        let residency = Self::packed(self.bindings(), px, &named);
        let layer = |b: usize| residency.layer(b).expect("every named texture is resident");
        Plan {
            copies: keep
                .iter()
                .map(|&b| (self.layer(b).expect("checked above"), layer(b)))
                .collect(),
            uploads: fresh.iter().map(|&b| layer(b)).collect(),
            residency,
        }
    }
}

/// Refuses a layer edge a robot cannot be built at.
fn check_px(px: u32) {
    assert!(
        px.is_power_of_two() && (TEXTURE_PX_MIN..=TEXTURE_PX_MAX).contains(&px),
        "a layer edge is a power of two from {TEXTURE_PX_MIN} to {TEXTURE_PX_MAX} px, not {px}"
    );
}

/// The bytes of one RGBA8 layer `px` pixels a side with its mip chain.
pub fn layer_bytes(px: u32) -> u64 {
    (0..mip_levels(px))
        .map(|level| {
            let edge = u64::from((px >> level).max(1));
            4 * edge * edge
        })
        .sum()
}

/// The edge every texture of a robot shares, in pixels: [`TEXTURE_PX`] for
/// a robot without textures.
///
/// # Panics
///
/// Panics when two of its textures have different edges.
///
/// [`TEXTURE_PX`]: avatar_3d::robot::TEXTURE_PX
pub fn edge(robot: &Robot) -> u32 {
    let px = robot
        .textures
        .first()
        .map_or(avatar_3d::robot::TEXTURE_PX, |t| t.px);
    assert!(
        robot.textures.iter().all(|t| t.px == px),
        "every texture of a robot shares one edge"
    );
    px
}

/// Each texture's canvas as the bindings buffer holds it: origin x, origin
/// y and edge, in projection units.
pub(crate) fn canvases(robot: &Robot) -> Vec<[f32; 3]> {
    robot
        .textures
        .iter()
        .map(|t| {
            let b = t.binding;
            [b.origin[0], b.origin[1], f64::from(b.extent)].map(|v| v as f32)
        })
        .collect()
}

/// The bindings buffer's bytes for these canvases and resident set.
pub(crate) fn canvas_data(canvases: &[[f32; 3]], residency: &Residency) -> Vec<u8> {
    assert_eq!(
        canvases.len(),
        residency.bindings(),
        "the resident set covers every texture"
    );
    let mut data = Vec::with_capacity(16 * canvases.len().max(1));
    for (b, canvas) in canvases.iter().enumerate() {
        let lane = residency.layer(b).map_or(-1.0, |l| l as f32);
        for v in canvas.iter().copied().chain([lane]) {
            data.extend_from_slice(&v.to_le_bytes());
        }
    }
    // A storage buffer cannot be empty.
    if data.is_empty() {
        data.resize(16, 0);
    }
    data
}

/// A robot's bindings buffer as the shader reads it: per texture, in
/// [`Robot::textures`] order, four little-endian f32 values: its canvas's
/// origin x and y and its edge, in projection units, then the layer of the
/// array texture holding it, or −1 when it is not resident. A robot without
/// textures gets one zeroed entry, since a storage buffer cannot be empty.
///
/// # Panics
///
/// Panics when `residency` is for another number of textures.
pub fn binding_data(robot: &Robot, residency: &Residency) -> Vec<u8> {
    canvas_data(&canvases(robot), residency)
}
