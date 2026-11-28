//! Tests of the stage's scene core through the fake renderer. None needs a
//! GPU; robots are built once per genome with stand-in pixels, and held or
//! counted by a test builder.

mod cast;
mod clock;
mod edges;
mod events;
mod faces;
mod held;
mod outcomes;
mod pause;
mod stats;
mod support;
mod threads;
