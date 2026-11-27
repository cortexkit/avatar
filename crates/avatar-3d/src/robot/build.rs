//! The builder: one pass over the layout that makes every part, bone and
//! texture.

use avatar_small_robots::layout::{Frame, Layout};
use avatar_small_robots::{decode, Creature, Mood, GENOME_LEN};

use super::depth::Depth;
use super::materials::Materials;
use super::solids::V3;
use super::{camera, fittings, gear, hulls, limbs, skeleton, textures};
use super::{BuildOptions, GearInfo, Part, Robot, Skin, UNIT};
use crate::mesh::Mesh;

/// How a new part follows the skeleton.
pub(super) enum SkinSpec {
    Rigid(usize),
    /// Joint indices and weights for each vertex, in vertex order.
    Skinned(Vec<[u32; 4]>, Vec<[f64; 4]>),
}

/// A part before its mesh is scaled to model units.
pub(super) struct NewPart {
    pub name: String,
    pub entry: (&'static str, &'static str),
    pub role: &'static str,
    pub class: &'static str,
    pub frame: Frame,
    pub placement: [f64; 2],
    pub material: usize,
    /// In robot space: robot units, +y up, the floor at 0.
    pub mesh: Mesh,
    pub skin: SkinSpec,
}

pub(super) struct Builder<'a> {
    pub creature: &'a Creature,
    pub layout: &'a Layout,
    pub depth: Depth,
    pub mats: Materials<'a>,
    pub parts: Vec<Part>,
    /// Name, parent and joint in robot space.
    pub bones: Vec<(String, Option<usize>, V3)>,
    /// Half the depth of each hull's solid, for placing fittings on it.
    pub head_half_depth: f64,
    pub chassis_half_depth: Option<f64>,
}

impl<'a> Builder<'a> {
    /// The ground offset of a 2D frame at rest: the frame's y plus this is
    /// the ground y.
    pub fn oy(&self, frame: Frame) -> f64 {
        let rise = self.layout.rise;
        match frame {
            Frame::Ground => 0.0,
            Frame::UpperBody => -rise,
            _ => self.layout.head.origin[1] - rise,
        }
    }

    /// A point of a 2D frame, at depth `z`, in robot space.
    pub fn at(&self, frame: Frame, [x, y]: [f64; 2], z: f64) -> V3 {
        [x, -(y + self.oy(frame)), z].map(|v| v + 0.0)
    }

    /// Adds a bone and returns its index.
    pub fn bone(&mut self, name: impl Into<String>, parent: Option<usize>, joint: V3) -> usize {
        self.bones.push((name.into(), parent, joint));
        self.bones.len() - 1
    }

    /// The index of a bone added earlier.
    pub fn find(&self, name: &str) -> usize {
        self.bones
            .iter()
            .position(|b| b.0 == name)
            .unwrap_or_else(|| panic!("bone {name} is added before its parts"))
    }

    /// The half depth of the hull a fitting in `frame` sits on.
    pub fn body_half_depth(&self) -> f64 {
        self.chassis_half_depth.unwrap_or(self.head_half_depth)
    }

    /// Scales a part to model units and adds it; returns its index.
    pub fn add(&mut self, p: NewPart) -> usize {
        let mut mesh = p.mesh;
        for v in &mut mesh.positions {
            *v = v.map(|c| c * UNIT + 0.0);
        }
        let skin = match p.skin {
            SkinSpec::Rigid(bone) => Skin::Rigid { bone },
            SkinSpec::Skinned(joints, weights) => Skin::Skinned { joints, weights },
        };
        self.parts.push(Part {
            name: p.name,
            entry: p.entry,
            role: p.role,
            class: p.class,
            frame: p.frame,
            placement: p.placement,
            material: p.material,
            mesh,
            skin,
        });
        self.parts.len() - 1
    }

    /// Adds a rigid part.
    #[allow(clippy::too_many_arguments)]
    pub fn rigid(
        &mut self,
        name: impl Into<String>,
        entry: (&'static str, &'static str),
        role: &'static str,
        class: &'static str,
        frame: Frame,
        placement: [f64; 2],
        material: usize,
        bone: usize,
        mesh: Mesh,
    ) -> usize {
        self.add(NewPart {
            name: name.into(),
            entry,
            role,
            class,
            frame,
            placement,
            material,
            mesh,
            skin: SkinSpec::Rigid(bone),
        })
    }
}

/// Builds a robot; see [`super::build_with`].
pub(super) fn robot(genome: &[u8; GENOME_LEN], options: BuildOptions) -> Robot {
    let creature = decode(genome);
    let layout = Layout::new(&creature);
    let depth = Depth::new(genome);
    super::solids::take_stand_ins();
    let mut b = Builder {
        creature: &creature,
        layout: &layout,
        depth,
        mats: Materials::new(&layout.palette, layout.mood(Mood::Neutral), depth.gloss),
        parts: Vec::new(),
        bones: Vec::new(),
        head_half_depth: 0.0,
        chassis_half_depth: None,
    };
    skeleton::trunk(&mut b);
    hulls::build(&mut b);
    fittings::joints(&mut b);
    fittings::stalks(&mut b);
    fittings::masts(&mut b);
    fittings::sides(&mut b);
    limbs::arms(&mut b);
    fittings::chest(&mut b);
    fittings::back(&mut b);
    let grounded = gear::build(&mut b);
    let anchors = skeleton::anchors(&b);
    let textures = textures::build(&b, options.textures);
    let bones = skeleton::finish(&b.bones);
    let camera = camera::stage(&b.parts);
    let status = b.mats.table();
    let stand_ins = super::solids::take_stand_ins();
    Robot {
        depth,
        parts: b.parts,
        materials: b.mats.list,
        status,
        bones,
        anchors,
        grounded,
        gear: GearInfo {
            kind: layout.gear.kind,
            hover: layout.gear.hover,
        },
        textures,
        camera,
        stand_ins,
    }
}
