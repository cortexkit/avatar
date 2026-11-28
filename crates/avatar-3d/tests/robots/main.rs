//! Tests of the 3D small robots, through the public data only.

#[path = "../../examples/robot_pins/cases.rs"]
mod cases;
#[path = "../../examples/robot_pins/scripts.rs"]
mod scripts;

mod actor_state;
mod builder;
mod cameras;
mod containment;
mod faces;
mod ground;
mod mapping;
mod motion;
mod motion_faces;
mod motion_floor;
mod motion_values;
mod same_individual;
mod separation;
mod skinning;
mod textures;
