//! Resolved colours for the 2D robot and its status-painted fittings.

use crate::kit::{Palette, MAGNET, MAGNET_TIP};
use crate::Rgb;

use super::{Material, Status};

/// The decoded sRGB palette, without any renderer-specific finishes.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct PaletteLayout {
    /// Body-colour recipe (0 pastel, 2 vivid, 3 dark, 4 muted).
    pub recipe: u8,
    /// Whether the palette uses dark paint.
    pub dark: bool,
    /// The exact body colour resolved by the genome.
    pub body: Rgb,
    /// Body paint material.
    pub paint: Material,
    /// Limb and fitting metal kind.
    pub metal_kind: &'static str,
    /// Limb and fitting metal material.
    pub metal: Material,
    /// Secondary paint recipe index.
    pub accent_kind: u32,
    /// Secondary paint material.
    pub accent: Material,
    /// Rubber material.
    pub rubber: Material,
    /// Magnet hand material.
    pub magnet: Material,
    /// Magnet hand pole-tip material.
    pub magnet_tip: Material,
    /// Legible dark-paint ink.
    pub ink: Rgb,
    /// White grille paint.
    pub white: Rgb,
    /// Unlit lens or display glass colour.
    pub glass: Rgb,
    /// Full-strength glow colour.
    pub glow: Rgb,
    /// Light tint of the glow colour.
    pub glow_lite: Rgb,
    /// Dark tint of the glow colour.
    pub glow_dim: Rgb,
    /// Status-dependent fills in [`Status::ALL`] order.
    pub status_colours: [StatusColours; Status::ALL.len()],
}

/// The sRGB fills for one status key.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct StatusColours {
    /// Fill for ball and arrow tips, dish lamp, chest lamp, and chest LEDs.
    pub status: Rgb,
    /// Bulb fills indexed by the `lit` flag (false, true).
    pub bulb: [Rgb; 2],
    /// Beacon dome fills indexed by the `rays` flag (false, true).
    pub beacon: [Rgb; 2],
}

impl PaletteLayout {
    pub(super) fn new(p: &Palette) -> Self {
        Self {
            recipe: p.recipe,
            dark: p.dark,
            body: p.body,
            paint: p.paint,
            metal_kind: crate::kit::METAL_NAMES[p.metal_kind],
            metal: p.metal,
            accent_kind: p.accent_kind,
            accent: p.accent,
            rubber: p.rubber,
            magnet: MAGNET,
            magnet_tip: MAGNET_TIP,
            ink: p.ink,
            white: p.white,
            glass: p.glass,
            glow: p.glow,
            glow_lite: p.glow_lite,
            glow_dim: p.glow_dim,
            status_colours: Status::ALL.map(|s| StatusColours {
                status: p.status(s),
                bulb: [false, true].map(|lit| p.bulb(s, lit)),
                beacon: [false, true].map(|rays| p.beacon(s, rays)),
            }),
        }
    }

    /// Look up the resolved sRGB fills for a status key.
    pub fn status(&self, status: Status) -> &StatusColours {
        &self.status_colours[status as usize]
    }
}
