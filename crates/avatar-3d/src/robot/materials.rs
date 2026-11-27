//! Materials: the 2D colours with 3D finishes (how each surface shines),
//! and the status
//! colour table.

use avatar_small_robots::layout::{MoodLayout, PaletteLayout, Status};
use avatar_small_robots::Rgb;

/// How a surface shines under the shading model the renderer implements:
/// a wrapped key light, a fill, darkening at grazing angles and a
/// highlight. `gloss` (a
/// depth trait) moves the shine within each finish's range.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Finish {
    /// The finish's name, one of [`FINISHES`].
    pub name: &'static str,
    /// Strength of the highlight.
    pub specular: f64,
    /// Sharpness of the highlight (a Phong exponent).
    pub shininess: f64,
    /// How much grazing angles darken toward the dark tone.
    pub edge: f64,
    /// How much the highlight takes the base colour (0 white, 1 tinted).
    pub metal: f64,
    /// How far the shadow side moves toward the colour's own dark tone.
    pub dark: f64,
}

/// The finish names: enamel paint, soft vinyl, metal, rubber, ink, glass and
/// glowing lamps.
pub const FINISHES: [&str; 7] = ["paint", "vinyl", "metal", "rubber", "ink", "glass", "glow"];

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

impl Finish {
    /// The finish named `name` at `gloss`, or `None` for an unknown name.
    pub fn new(name: &str, gloss: f64) -> Option<Self> {
        let f = |name, specular, shininess, edge, metal, dark| Self {
            name,
            specular,
            shininess,
            edge,
            metal,
            dark,
        };
        Some(match name {
            "paint" => f(
                "paint",
                lerp(0.25, 0.42, gloss),
                lerp(48.0, 76.0, gloss),
                0.3,
                0.0,
                0.5,
            ),
            "vinyl" => f(
                "vinyl",
                lerp(0.12, 0.2, gloss),
                lerp(28.0, 40.0, gloss),
                0.3,
                0.0,
                0.45,
            ),
            "metal" => f(
                "metal",
                lerp(0.35, 0.55, gloss),
                lerp(44.0, 70.0, gloss),
                0.42,
                0.55,
                0.6,
            ),
            "rubber" => f("rubber", 0.05, 14.0, 0.35, 0.0, 0.5),
            "ink" => f("ink", 0.08, 20.0, 0.1, 0.0, 0.3),
            "glass" => f("glass", 0.7, 90.0, 0.15, 0.0, 0.3),
            "glow" => f("glow", 0.35, 60.0, 0.05, 0.0, 0.0),
            _ => return None,
        })
    }
}

/// What makes a material give light of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Emission {
    /// Lit only by the scene.
    None,
    /// A status light: its colour comes from [`StatusTable`] entry `light`,
    /// and the pose's glow scales its strength.
    Status {
        /// Index into [`StatusTable::lights`].
        light: usize,
    },
    /// A jet's flame: the pose's thrust scales its strength.
    Thrust,
}

/// A material: a 2D colour with a finish.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Material {
    /// Which palette colour or light it is, such as `paint`, `metal` or
    /// `bulb`.
    pub name: &'static str,
    /// The sRGB base colour, exactly the 2D colour. A status light's base
    /// is its colour in the neutral mood; its table entry gives the rest.
    pub base: Rgb,
    /// How it shines.
    pub finish: Finish,
    /// Whether and how it glows.
    pub emission: Emission,
}

/// Which 2D rule a status light's colour follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LightKind {
    /// Ball and arrow tips, the dish lamp and the chest light: the status
    /// colour.
    Lamp,
    /// The bulb: the status colour when `lit`, else a pale tint of it.
    Bulb,
    /// The beacon dome: the status colour when `rays`, else glass tinted by
    /// it.
    Beacon,
    /// A chest LED, coloured by one of the pose's four LED statuses.
    Led {
        /// Which of the four LED outputs it shows.
        slot: usize,
    },
}

/// One status-painted material and every colour it can show.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct StatusLight {
    /// Index into [`super::Robot::materials`].
    pub material: usize,
    /// The 2D rule it follows.
    pub kind: LightKind,
    /// The sRGB colour for each status (in [`Status::ALL`] order), `lit`
    /// flag and `rays` flag: `colours[status][lit][rays]`. An LED reads its
    /// own slot's status.
    pub colours: [[[Rgb; 2]; 2]; 7],
}

/// The status colours, resolved once so a renderer only looks them up.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct StatusTable {
    /// Each status-painted material.
    pub lights: Vec<StatusLight>,
    /// The LED colour for each status, in [`Status::ALL`] order.
    pub leds: [Rgb; 7],
}

impl StatusTable {
    /// The colour of light `light` for a status and the two flags.
    pub fn colour(&self, light: usize, status: Status, lit: bool, rays: bool) -> Rgb {
        self.lights[light].colours[status_index(status)][usize::from(lit)][usize::from(rays)]
    }
}

/// A status's index in its declaration order, which [`Status::ALL`] follows.
pub(super) fn status_index(status: Status) -> usize {
    Status::ALL
        .iter()
        .position(|s| *s == status)
        .expect("every status is in Status::ALL")
}

/// Collects a robot's materials, one per name.
pub(super) struct Materials<'a> {
    palette: &'a PaletteLayout,
    neutral: &'a MoodLayout,
    gloss: f64,
    pub list: Vec<Material>,
    pub lights: Vec<StatusLight>,
}

impl<'a> Materials<'a> {
    pub fn new(palette: &'a PaletteLayout, neutral: &'a MoodLayout, gloss: f64) -> Self {
        Self {
            palette,
            neutral,
            gloss,
            list: Vec::new(),
            lights: Vec::new(),
        }
    }

    fn add(&mut self, name: &'static str, base: Rgb, finish: &str, emission: Emission) -> usize {
        if let Some(i) = self.list.iter().position(|m| m.name == name) {
            return i;
        }
        self.list.push(Material {
            name,
            base,
            finish: Finish::new(finish, self.gloss).expect("finish names come from FINISHES"),
            emission,
        });
        self.list.len() - 1
    }

    /// The body paint.
    pub fn paint(&mut self) -> usize {
        self.add("paint", self.palette.paint.base, "paint", Emission::None)
    }

    /// The limb and fitting metal, with the finish its kind selects.
    pub fn metal(&mut self) -> usize {
        let finish = metal_finish(self.palette.metal_kind);
        self.add("metal", self.palette.metal.base, finish, Emission::None)
    }

    /// The secondary paint.
    pub fn accent(&mut self) -> usize {
        self.add("accent", self.palette.accent.base, "vinyl", Emission::None)
    }

    /// Rubber.
    pub fn rubber(&mut self) -> usize {
        self.add("rubber", self.palette.rubber.base, "rubber", Emission::None)
    }

    /// The magnet hand's red.
    pub fn magnet(&mut self) -> usize {
        self.add("magnet", self.palette.magnet.base, "paint", Emission::None)
    }

    /// The magnet hand's pole tips.
    pub fn magnet_tip(&mut self) -> usize {
        self.add(
            "magnet-tip",
            self.palette.magnet_tip.base,
            "metal",
            Emission::None,
        )
    }

    /// White grille and dial paint.
    pub fn white(&mut self) -> usize {
        self.add("white", self.palette.white, "paint", Emission::None)
    }

    /// The hazard plate's yellow, the colour the 2D chest draws.
    pub fn hazard(&mut self) -> usize {
        self.add(
            "hazard",
            Rgb {
                r: 0xf6,
                g: 0xc5,
                b: 0x31,
            },
            "paint",
            Emission::None,
        )
    }

    /// A material of another fitting's 2D colour: the arms' or the hose
    /// legs'. Its finish follows the palette material it matches.
    pub fn named(&mut self, name: &'static str, base: Rgb) -> usize {
        let p = self.palette;
        let finish = if base == p.metal.base {
            metal_finish(p.metal_kind)
        } else if base == p.rubber.base {
            "rubber"
        } else if base == p.accent.base {
            "vinyl"
        } else {
            "paint"
        };
        self.add(name, base, finish, Emission::None)
    }

    /// The jet flame, in the glow colour.
    pub fn flame(&mut self) -> usize {
        self.add("flame", self.palette.glow, "glow", Emission::Thrust)
    }

    /// A status-painted material of the given kind.
    pub fn light(&mut self, name: &'static str, kind: LightKind) -> usize {
        if let Some(i) = self.list.iter().position(|m| m.name == name) {
            return i;
        }
        let p = self.palette;
        let colours: [[[Rgb; 2]; 2]; 7] = Status::ALL.map(|status| {
            let c = p.status(status);
            [false, true].map(|lit| {
                [false, true].map(|rays| match kind {
                    LightKind::Lamp | LightKind::Led { .. } => c.status,
                    LightKind::Bulb => c.bulb[usize::from(lit)],
                    LightKind::Beacon => c.beacon[usize::from(rays)],
                })
            })
        });
        let light = self.lights.len();
        let n = self.neutral;
        let status = match kind {
            LightKind::Led { slot } => n.leds[slot],
            _ => n.status,
        };
        let base = colours[status_index(status)][usize::from(n.lit)][usize::from(n.rays)];
        let material = self.add(name, base, "glow", Emission::Status { light });
        self.lights.push(StatusLight {
            material,
            kind,
            colours,
        });
        material
    }

    pub fn table(&self) -> StatusTable {
        StatusTable {
            lights: self.lights.clone(),
            leds: Status::ALL.map(|s| self.palette.status(s).status),
        }
    }
}

/// The finish a metal kind selects; the mapping table's finish entries.
pub(super) fn metal_finish(kind: &str) -> &'static str {
    match kind {
        "enamel" => "paint",
        _ => "metal",
    }
}
