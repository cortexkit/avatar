// Shared palette formatting. Rust declares a private `hex` per module
// (svg.rs, eyes.rs, brows.rs, mouth.rs, lashes.rs); the TS port keeps one
// implementation so the pinned lowercase `#rrggbb` shape cannot drift
// between modules.

import type { Rgb } from "../phenotype";

/** Formats a color as Rust's `{:02x}{:02x}{:02x}`: lowercase, zero-padded. */
export function hexColor(color: Rgb): string {
  const channel = (value: bigint) => value.toString(16).padStart(2, "0");
  return `#${channel(color.r)}${channel(color.g)}${channel(color.b)}`;
}
