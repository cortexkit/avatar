//! The 3D stage: a scene of small robots keyed by the caller's strings (an
//! agent's id, say), their motion, and a clock, drawn through a renderer.
//!
//! [`Stage`] is the whole interface a host uses. Its scene core decides on
//! the CPU everything a frame shows: the cast and each key's builds, the
//! actors and their commands, the clock and fps cap, pause, events and
//! statistics. It drives the GPU only through the [`Renderer`] trait;
//! [`fake::FakeRenderer`] implements that trait without a GPU, so every
//! rule here is tested on any machine.
//!
//! # The host contract
//!
//! - **Threads.** `create` (a constructor) may run on any thread. Commands
//!   ([`Stage::set_creatures`], [`Stage::set_state`], [`Stage::play`],
//!   [`Stage::set_view`], [`Stage::resize`], [`Stage::set_max_fps`],
//!   [`Stage::pause`], [`Stage::resume`]) and reads ([`Stage::events`],
//!   [`Stage::frame_stats`]) may come from any thread at any time, also
//!   while a render call runs on another. They never wait on a frame, a
//!   GPU fence, a build, or a lock held across GPU work: commands go into a
//!   lock-free queue, reads load what a frame published atomically, and
//!   events wait in their own lock-free queue.
//! - **Render calls** run on one thread at a time, the host's render
//!   thread driven by its display link. A call made while another runs is
//!   refused with [`RenderStatus::Busy`]. Render calls may block, so they
//!   belong off the main thread.
//! - **Time.** A render call's `target_time` is seconds on the platform's
//!   monotonic media clock (on Apple platforms, a `CADisplayLink`'s
//!   `targetTimestamp`). It is the stage's only time base: motion and
//!   events take time from it alone, and wall-clock time is used only for
//!   [`Stage::frame_stats`]. A command takes effect at the target time of
//!   the first frame drawn after it, and events are stamped with that
//!   time.
//! - **Pause** frees the GPU memory (robots, faces, frame targets) as soon
//!   as no frame is in flight: at once, or at the end of a running render
//!   call, with no further call from the host. Until [`Stage::resume`],
//!   render calls return [`RenderStatus::Paused`] at once without drawing.
//!   Resume rebuilds lazily: the next frames upload what the scene needs,
//!   and creatures appear as they are rebuilt.
//! - **Builds.** Robots build on worker threads and appear when ready,
//!   which depends on wall-clock time. [`StageOptions`] with
//!   [`BuildMode::Inline`] builds them inside the first render call that
//!   needs them instead, so a script driving the stage with a virtual
//!   clock gets the same frames every run. Face sets and new texture edges
//!   are rasterized the same way, on the workers or inline. A robot's
//!   reach box (the space it moves through, which the stage camera
//!   frames) and stage camera are worked out on the thread that builds
//!   it, never in a command or a read. [`Stage::events`] reports each
//!   creature ready.
//! - **Faces.** A face changes only to a face set already on the GPU, and
//!   only under a blink, so no frame shows a missing face, two faces, or
//!   lights from another mood than the face. [`Stage`] documents the rules:
//!   drawn moods, face commands held until their set is drawable, which
//!   sets stay resident, and the texture edge a robot is drawn at.
//!
//! [`Stage`] documents the order of steps within a render call, the commit
//! point, the creatures' lifecycle and destroy's bound.
//!
//! # Not yet here
//!
//! Every robot stands at the origin until the plaza (the view of the whole
//! cast, [`StageView::Plaza`]) is laid out, and frames carry no camera until the stage camera is added. With the camera
//! come the reads that depend on it: each drawn creature's head, face and
//! ground positions on screen, and the shapes a tap is tested against.
//! Until then a robot's texture edge follows the frame only when it is
//! shown alone at the default view ([`StageView::Stage`]); in the plaza
//! robots keep the edge they have, [`FIRST_EDGE`] unless staged before.

pub mod fake;

mod build;
mod clock;
mod events;
mod faces;
mod held;
mod renderer;
mod scene;
mod spec;
mod stage;
mod stats;

pub use build::{
    BuildMode, BuildRequest, Builder, FaceRequest, LayersRequest, RobotBuilder, StageOptions,
};
pub use clock::{DUE_SLACK, MAX_STEP, UNCAPPED_PERIOD};
pub use events::{CommandKind, Event, EVENT_CAPACITY};
pub use faces::{
    edge_target, FIRST_EDGE, HOLD_AT_MOST, LATE_AFTER, RELEASE_AFTER, SHRINK_AFTER, SHUT,
};
pub use renderer::{DrawItem, Failure, Frame, Image, LayerChange, RenderError, Renderer, Target};
pub use spec::{CreatureSpec, SkipReason, SMALL_ROBOTS, SMALL_ROBOTS_VERSION};
pub use stage::{InvalidArgument, RenderStatus, Stage, StageError, StageView};
pub use stats::{FrameStats, Series, Times, STATS_WINDOW};
