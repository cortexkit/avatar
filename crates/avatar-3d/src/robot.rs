//! Small robots in 3D: the shipped 2D type `creature.small-robots` built as
//! plain-data meshes, materials, a skeleton, anchors, a camera and textures.
//!
//! A 3D robot is the same individual as its 2D drawing. Every part,
//! proportion, colour and face comes from the 2D type's layout view
//! ([`avatar_small_robots::layout::Layout`]); the 3D robot adds only depth,
//! read from the genome's depth bytes 6144 to 6153 ([`Depth`]), which the 2D
//! type never reads. Moods are not baked in: the builder makes the neutral
//! rest pose, and a pose's bones move the parts.
//!
//! # Space and units
//!
//! Model space has +y up, the floor at y = 0 and the face looking along
//! +z. x is the 2D x unmirrored as seen from the front, so a name ending in
//! `.L` is on the viewer's left (−x) and `.R` on the right (+x). One model
//! unit is 100 robot units, the units the 2D hulls are built in: about one
//! body width ([`UNIT`]). A point (x, y) of a 2D drawing frame whose ground
//! offset is `oy` (the frame's y plus `oy` is the 2D ground y, which grows
//! downward from the floor at 0) becomes the model point
//! `UNIT · (x, −(y + oy), z)`; [`to_model`] states it once. The 2D picture
//! fit is never applied, so robots keep their relative sizes.
//!
//! # Bones
//!
//! Every robot has `root` at the floor under the middle of its base (it
//! moves only to leave the ground), `body` at the base of the lowest hull
//! (the trunk's pivot) and `head` at the head's base (its pivot in the 2D
//! drawing; on a pod, which has no neck, it sits at the body's joint). The
//! rest are present with their parts:
//!
//! - `neck` at the neck's base;
//! - `upperarm.X`, `forearm.X` and `wrist.X` at the shoulder, elbow and
//!   wrist, and `grip.X` at the wrist for a hand's jaws;
//! - `antenna.N` at each antenna's socket, and `stalk.L`, `stalk.R` at each
//!   eye stalk's base;
//! - `needle` at a chest gauge's centre, and `cable` or `pipe` at the mount
//!   of what trails behind;
//! - `leg.X` at each hip and `foot.X` at its ankle, where X is `L`, `R`, `M`
//!   for a middle leg, or `L1`, `R1`, `L2`… for splayed pairs from the
//!   front; `spring.0` for a pogo spring; `wheel.L`, `wheel.R`, `wheel.M`,
//!   or `wheel.0`, `wheel.1`… for a track's road wheels; `jet.0` at a
//!   hover nozzle's mount, and `exhaust.0` at its outlet, carrying the
//!   flame or the rings below it.
//!
//! # Parts, bones and skinning
//!
//! Every vertex is given at rest, in model space. Bones have identity rest
//! rotations, so a bone's rest matrix is the translation to its joint.
//! A rigid part follows one bone; a skinned part carries up to four joint
//! indices and weights per vertex. With model-space joint matrices `M_j`
//! for a pose and rest matrices `R_j`, a vertex `v` moves to
//! `Σ w_j · M_j · R_j⁻¹ · v`, which for a rigid part is `M_b · R_b⁻¹ · v`.
//!
//! # Textures
//!
//! Faces, stalk eyes and hull marks are drawn by the 2D type into separate
//! layers and rasterized to 256×256 RGBA8 ([`TEXTURE_PX`]), sRGB colour
//! premultiplied by alpha, as [`avatar_raster::rasterize_tree`] returns them.
//! Each texture has a [`TextureBinding`]: the part it lies on, its canvas
//! origin and edge C in robot units, so `uv = (projection − origin) / C` on
//! both axes and a fragment outside `[0, 1]²` gets nothing from it.
#![deny(missing_docs)]

mod build;
mod camera;
mod depth;
mod digest;
mod fittings;
mod gear;
mod hulls;
mod limbs;
pub mod mapping;
mod materials;
mod rigging;
mod skeleton;
mod solids;
mod textures;

pub use camera::{Camera, AZIMUTH, ELEVATION, FILL, FOV_Y, FRAME_PX};
pub use depth::{Depth, BACK_KINDS, FIRST_BYTE as FIRST_DEPTH_BYTE, LAST_BYTE as LAST_DEPTH_BYTE};
pub use digest::mesh_bytes;
pub use materials::{Emission, Finish, LightKind, Material, StatusLight, StatusTable, FINISHES};

use avatar_small_robots::layout::Frame;
use avatar_small_robots::{Mood, GENOME_LEN};

use crate::mesh::Mesh;
use crate::motion::Motion;

/// Model units per robot unit: 100 robot units, about one body width, make
/// one model unit.
pub const UNIT: f64 = 0.01;

/// The edge of every texture in pixels.
pub const TEXTURE_PX: u32 = 256;

/// Bytes of one RGBA8 texture.
pub const TEXTURE_BYTES: usize = (TEXTURE_PX * TEXTURE_PX * 4) as usize;

/// Moves a point of a 2D drawing frame, with depth `z` in robot units, to
/// model space. `oy` is the frame's ground offset at rest: the frame's y
/// plus `oy` is the 2D ground y, which grows downward from the floor at 0.
pub fn to_model([x, y]: [f64; 2], z: f64, oy: f64) -> [f64; 3] {
    [UNIT * x, -UNIT * (y + oy), UNIT * z].map(|v| v + 0.0)
}

/// A robot built from a genome, as plain data.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Robot {
    /// The decoded depth traits.
    pub depth: Depth,
    /// Every part, in build order: hulls, joints, face fittings, top, sides,
    /// arms and hands, chest, back, then running gear.
    pub parts: Vec<Part>,
    /// Materials, which parts index.
    pub materials: Vec<Material>,
    /// Resolved colours of every status-painted material and of the chest
    /// LEDs, so no renderer re-implements the 2D status blends.
    pub status: StatusTable,
    /// Bones in parent-first order.
    pub bones: Vec<Bone>,
    /// Named attachment points.
    pub anchors: Vec<Anchor>,
    /// Indices of the parts that touch the floor at rest; empty when the
    /// gear hovers.
    pub grounded: Vec<usize>,
    /// The running gear's kind and whether it hovers.
    pub gear: GearInfo,
    /// Textures with their bindings. Pixels are present unless the robot was
    /// built with [`BuildOptions::textures`] off.
    pub textures: Vec<Texture>,
    /// The three-quarter stage camera that frames this robot.
    pub camera: Camera,
    /// Mood postures, idle, reactions and status loops, for
    /// [`crate::motion::Actor`] to pose the robot with.
    pub motion: Motion,
    /// Solids the [`crate::mesh`] module refused for this genome's numbers,
    /// each replaced
    /// by a simpler stand-in so that building never fails. Empty for every
    /// genome the tests build.
    pub stand_ins: Vec<&'static str>,
}

impl Robot {
    /// The number of triangles over every part.
    pub fn triangle_count(&self) -> usize {
        self.parts.iter().map(|p| p.mesh.triangles.len()).sum()
    }

    /// The bytes of every texture, whether or not its pixels were drawn.
    pub fn texture_bytes(&self) -> usize {
        self.textures.len() * TEXTURE_BYTES
    }

    /// The index of the bone with this name.
    pub fn bone(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }

    /// The anchor with this name.
    pub fn anchor(&self, name: &str) -> Option<&Anchor> {
        self.anchors.iter().find(|a| a.name == name)
    }
}

/// How to build a robot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct BuildOptions {
    /// Whether to rasterize the textures. Off, every texture keeps its
    /// binding and has no pixels; the geometry is the same.
    pub textures: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self { textures: true }
    }
}

impl BuildOptions {
    /// Options that leave the textures undrawn.
    pub fn untextured() -> Self {
        Self { textures: false }
    }
}

/// Builds the robot a stored genome describes, with every texture drawn.
/// It never fails: every bevel is clamped, down to zero if needed.
pub fn build(genome: &[u8; GENOME_LEN]) -> Robot {
    build_with(genome, BuildOptions::default())
}

/// Builds the robot a stored genome describes with the given options.
pub fn build_with(genome: &[u8; GENOME_LEN], options: BuildOptions) -> Robot {
    build::robot(genome, options)
}

/// One mesh of a robot with one material, hung on one bone or skinned.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Part {
    /// A name unique within the robot, such as `head` or `arm.L.upper`.
    pub name: String,
    /// The mapping entry the part realises: a catalogue field and value
    /// ([`mapping::MAPPING`]).
    pub entry: (&'static str, &'static str),
    /// The component the part is, such as `head`, `mast` or `foot`. For a
    /// part of an assembly it is one of the assembly's members
    /// ([`mapping::Kind::Assembly`]).
    pub role: &'static str,
    /// The 2D drawing class of the fitting it comes from, or empty.
    pub class: &'static str,
    /// The 2D frame its placement is given in.
    pub frame: Frame,
    /// Where the 2D layout places the fitting, in `frame`'s robot units.
    pub placement: [f64; 2],
    /// Index into [`Robot::materials`].
    pub material: usize,
    /// The geometry at rest, in model space. Projection coordinates are the
    /// rest-local 2D (x, y) in robot units of the part's texture frame for
    /// the hulls and stalk eyes, and the solid's own build coordinates
    /// elsewhere.
    pub mesh: Mesh,
    /// How the part follows the skeleton.
    pub skin: Skin,
}

/// How a part follows the skeleton.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Skin {
    /// Every vertex follows one bone.
    Rigid {
        /// Index into [`Robot::bones`].
        bone: usize,
    },
    /// Each vertex blends up to four bones; unused slots have weight zero.
    Skinned {
        /// Bone indices, one set per vertex.
        joints: Vec<[u32; 4]>,
        /// Weights summing to one, one set per vertex.
        weights: Vec<[f64; 4]>,
    },
}

/// One bone of the skeleton.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Bone {
    /// The bone's name, from the list in this module's documentation, such
    /// as `head` or
    /// `upperarm.L` (`.L` is −x).
    pub name: String,
    /// Index of the parent bone; `None` only for `root`.
    pub parent: Option<usize>,
    /// The joint's rest position in model space.
    pub joint: [f64; 3],
    /// Rest matrix, joint to model, column-major: the translation to
    /// `joint`, since every rest rotation is the identity.
    pub rest: [f64; 16],
}

/// A named point that follows a bone.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Anchor {
    /// `ground`, `headTop`, `face`, `hand.L` or `hand.R`.
    pub name: &'static str,
    /// The bone it follows; `None` for `ground`, which stays at the
    /// contact point whatever the root does.
    pub bone: Option<usize>,
    /// Offset from the bone's joint at rest, in model units.
    pub offset: [f64; 3],
}

/// The running gear as a whole.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct GearInfo {
    /// The decoded gear kind.
    pub kind: &'static str,
    /// Whether the gear hovers in neutral; hover gear lands in sleepy.
    pub hover: bool,
}

/// One 256×256 RGBA8 texture and where it lies.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Texture {
    /// Where the texture is sampled.
    pub binding: TextureBinding,
    /// `TEXTURE_BYTES` bytes, rows top first, sRGB premultiplied by alpha;
    /// `None` when built without textures.
    pub pixels: Option<Vec<u8>>,
}

/// What a texture shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextureContent {
    /// The face in a mood, as drawn or blinking.
    Face {
        /// The mood the face is drawn in.
        mood: Mood,
        /// Whether every open eye is drawn shut.
        blinking: bool,
    },
    /// One stalk eye in a mood: 0 is the left (−x) eye.
    StalkEye {
        /// Which eye.
        eye: usize,
        /// The mood it is drawn in.
        mood: Mood,
        /// Whether it is drawn shut.
        blinking: bool,
    },
    /// The surface marks of a hull, shaded like the paint under them.
    Marks,
}

/// Which triangles of its part a texture may cover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextureSurface {
    /// Only triangles tagged textured front ([`Mesh::textured_front`]).
    TexturedFront,
    /// Every triangle whose outward normal has z > 0.
    FacingFront,
}

/// Where a texture lies: `uv = (projection − origin) / extent` on both
/// axes, over the part's projection coordinates. On one surface marks draw
/// over the paint (shaded like it) and faces over marks (unlit).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct TextureBinding {
    /// Index into [`Robot::parts`].
    pub part: usize,
    /// What the texture shows.
    pub content: TextureContent,
    /// Which triangles may show it.
    pub surface: TextureSurface,
    /// The canvas's top-left corner in the part's projection coordinates
    /// (robot units, y down).
    pub origin: [f64; 2],
    /// The canvas edge C in robot units; the canvas is square.
    pub extent: u32,
}

impl TextureBinding {
    /// The texture coordinate of a projection coordinate, or `None` outside
    /// the canvas, where the texture contributes nothing.
    pub fn uv(&self, projection: [f64; 2]) -> Option<[f64; 2]> {
        let c = f64::from(self.extent);
        let uv = [
            (projection[0] - self.origin[0]) / c,
            (projection[1] - self.origin[1]) / c,
        ];
        uv.iter().all(|v| (0.0..=1.0).contains(v)).then_some(uv)
    }
}
