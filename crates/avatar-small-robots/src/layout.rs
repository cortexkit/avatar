//! Fitted robot parts, mood poses and separate surface drawings. Geometry
//! uses pre-fit robot units: x increases right and y increases down. The
//! floor is y = 0 in the ground frame. Head geometry is relative to the
//! base of the head; the upper-body origin is the base of the lowest hull.
#![deny(missing_docs)]

mod bounds;
mod catalogue;
mod fittings;
mod layers;
mod palette;
mod support;
#[cfg(test)]
mod tests;

pub use crate::gear::{Limb as GearLimb, MultiLeg as GearMultiLeg, Shape as GearShape};
pub use crate::kit::{Material, Status};
pub use catalogue::*;
pub use fittings::*;
pub use layers::{Layer, Region, LAYER_MARGIN};
pub use palette::*;
pub use support::*;

use crate::draw::Out;
use crate::face_draw::{bezel_pts, mouth};
use crate::geom::poly_sample;
use crate::hull::Hull;
use crate::mood::mood_index;
use crate::robot::Robot;
use crate::{Creature, Mood, MouthKind, Tech, MOODS};

const GRID: f64 = 100.0;

/// Number of linear subdivisions of each rounded hull corner.
pub const HULL_CORNER_STEPS: u32 = 8;

/// A closed reference outline and placement of one of the three hulls.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct HullLayout {
    /// Decoded hull kind. The chassis names refer to the assembly, not its profile.
    pub kind: &'static str,
    /// Name of the outline profile actually used by the 2D generator.
    pub profile: &'static str,
    /// Outline in hull-local robot units, closed by connecting last to first.
    pub outline: Vec<[f64; 2]>,
    /// Hull base in the rest upper-body frame, before rise is applied.
    pub origin: [f64; 2],
    /// Half-width in robot units.
    pub half_width: f64,
    /// Height in robot units.
    pub height: f64,
}

impl HullLayout {
    fn new(kind: &'static str, hull: &Hull, y: f64) -> Self {
        Self {
            kind,
            profile: hull.kind,
            outline: poly_sample(&hull.pts, HULL_CORNER_STEPS),
            origin: [0.0, y],
            half_width: hull.w,
            height: hull.h,
        }
    }
}

/// Position and reference eye housing on the head.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct EyeLayout {
    /// -1 for left, 0 for middle, 1 for right as seen from the front.
    pub side: i8,
    /// Centre in head-local robot units.
    pub centre: [f64; 2],
    /// Fitted housing ellipse radii, including the 1.03 margin used by the fit.
    pub radii: [f64; 2],
    /// Housing ellipse in head-local robot units; close last to first.
    pub outline: Vec<[f64; 2]>,
}

/// Rest face and its separate containment targets in head-local robot units.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct FaceLayout {
    /// Decoded face unit.
    pub kind: Tech,
    /// Screen panel flag and eye style, if the head carries a screen.
    pub screen: Option<(bool, &'static str)>,
    /// Fitted non-stalk eyes in emitter order. Stalk eyes live on the fittings.
    pub eyes: Vec<EyeLayout>,
    /// Decoded mouth kind.
    pub mouth_kind: MouthKind,
    /// Fitted mouth centre, whether or not the mood moves the drawn line.
    pub mouth_centre: [f64; 2],
    /// Contours of the neutral mouth, in draw order. A line mouth has open
    /// contours; filled profiles and strip panels are closed.
    pub mouth_outlines: Vec<Contour>,
    /// Outside edge of the screen bezel if present; close last to first.
    pub bezel_outline: Option<Vec<[f64; 2]>>,
    /// Bounds of both face drawings across all moods, used only for the canvas.
    pub region: Option<Region>,
}

/// A sampled contour in head-local robot units.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Contour {
    /// Points in drawing order, without a repeated closing point.
    pub points: Vec<[f64; 2]>,
    /// Whether the last point connects to the first.
    pub closed: bool,
}

impl FaceLayout {
    fn new(c: &Robot) -> Self {
        let f = &c.f;
        let eyes = f
            .eyes
            .iter()
            .map(|e| {
                let centre = [e.x, e.y];
                let radii = [e.rx * 1.03, e.ry * 1.03];
                EyeLayout {
                    side: e.side as i8,
                    centre,
                    radii,
                    outline: bounds::ellipse(centre, radii),
                }
            })
            .collect();
        let mut out = Out::new();
        mouth(&mut out, &layers::detail(), f, &c.p, c.mood(Mood::Neutral));
        let mouth_outlines = out
            .prims
            .iter()
            .filter(|p| match f.mouth.kind {
                MouthKind::Strip => p.label == "rb-mouth-panel",
                MouthKind::Grid => p.label == "rb-mouth-grille",
                _ => true,
            })
            .flat_map(|p| bounds::contours(&p.shape))
            .collect();
        Self {
            kind: f.tech,
            screen: f.screen.as_ref().map(|_| (f.lcd, f.eye_style)),
            eyes,
            mouth_kind: f.mouth.kind,
            mouth_centre: [f.mouth.x, f.mouth.y],
            mouth_outlines,
            bezel_outline: bezel_pts(f).map(|p| poly_sample(&p, HULL_CORNER_STEPS)),
            region: layers::face_region(c),
        }
    }
}

/// The independently resolved, owned view of one decoded robot.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Layout {
    /// Assembly plan name.
    pub plan: &'static str,
    /// Decoded size multiplier, already applied where the 2D type uses it.
    pub scale: f64,
    /// Height of the lowest module above the floor, in robot units at rest.
    pub rise: f64,
    /// Head outline and placement.
    pub head: HullLayout,
    /// Chassis outline and placement, absent on a pod.
    pub chassis: Option<HullLayout>,
    /// Bottom segment and its placement, present on a stack.
    pub pelvis: Option<HullLayout>,
    /// Upper edge of the chassis in the upper-body frame, if present.
    pub chassis_top: Option<f64>,
    /// Face traits and head-local reference outlines.
    pub face: FaceLayout,
    /// Top and tip choices and fitted mast/stalk placements in the head frame.
    pub top: TopLayout,
    /// Side choice and fitted head-local side placements.
    pub sides: SideLayout,
    /// Rest arm construction in the upper-body frame, left then right.
    pub arms: [Option<ArmLayout>; 2],
    /// Running gear construction and its rest placements.
    pub gear: GearLayout,
    /// Fitted chest panel in the head or upper-body frame.
    pub chest: ChestLayout,
    /// Head-local surface decor and upper-body chassis rivet placements.
    pub decor: DecorLayout,
    /// Fitted neck and waist, including the zero-width flush neck.
    pub joints: BodyJoints,
    /// Tail fitting, if there is one; its fitted frame depends on the kind.
    pub back: Option<BackLayout>,
    /// Resolved sRGB palette and status light colour table.
    pub palette: PaletteLayout,
    /// Resolved pose and carried mood fields, in [`MOODS`] order.
    pub moods: [MoodLayout; MOODS.len()],
}

impl Layout {
    /// Read the already fitted robot without applying the picture fit.
    pub fn new(creature: &Creature) -> Self {
        let c = &creature.robot;
        Self {
            plan: c.plan,
            scale: c.scale,
            rise: c.rise,
            head: HullLayout::new(c.head.kind, &c.head, c.head_y),
            chassis: c
                .body
                .as_ref()
                .zip(c.body_kind)
                .map(|(h, k)| HullLayout::new(k, h, c.body_y)),
            pelvis: c.pelvis.as_ref().map(|h| HullLayout::new(h.kind, h, 0.0)),
            chassis_top: c.body.as_ref().map(|_| c.body_top),
            face: FaceLayout::new(c),
            top: TopLayout::new(c),
            sides: SideLayout::new(c),
            arms: arms(c),
            gear: GearLayout::new(c),
            chest: ChestLayout::new(c),
            decor: DecorLayout::new(c),
            joints: BodyJoints::new(c),
            back: back(c),
            palette: PaletteLayout::new(&c.p),
            moods: MOODS.map(|m| MoodLayout::new(c, m)),
        }
    }

    /// Resolved pose of one mood, in pre-fit robot units and clockwise degrees.
    pub fn mood(&self, mood: Mood) -> &MoodLayout {
        &self.moods[mood_index(mood)]
    }
}
