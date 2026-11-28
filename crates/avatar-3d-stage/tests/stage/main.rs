//! Tests of the stage's scene core through the fake renderer. None needs a
//! GPU; robots are built untextured, once per genome, and held or counted
//! by a test builder.

mod cast;
mod clock;
mod events;
mod outcomes;
mod pause;
mod stats;
mod support;
mod threads;
