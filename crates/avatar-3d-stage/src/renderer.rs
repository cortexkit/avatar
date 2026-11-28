//! The seam between the scene and the GPU: what the stage asks a renderer
//! to do, and what a frame hands it.
//!
//! The scene core decides everything a frame shows on the CPU and calls a
//! [`Renderer`] only from the thread running a render call, one call at a
//! time, so a renderer needs no locks of its own for the stage's sake.
//! [`crate::fake::FakeRenderer`] implements it without a GPU for tests.

use std::fmt;

use avatar_3d::motion::Pose;
use avatar_3d::robot::Robot;

/// Where a stage's frames go. A stage has one, chosen when it is made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    /// A window's or layer's surface: [`crate::Stage::render_to_surface`].
    Surface,
    /// Images read back to the CPU: [`crate::Stage::render_to_image`].
    Image,
}

/// Why a renderer could not do what it was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The surface is lost or out of date. The renderer has reconfigured it
    /// and the next frame may draw.
    SurfaceLost,
    /// The GPU device is lost. The stage calls [`Renderer::recover`] on its
    /// next render call that gets that far.
    DeviceLost,
    /// Anything else.
    Render(RenderError),
}

/// A renderer error other than a lost surface or device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderError {
    /// What went wrong, for logs.
    pub message: String,
}

impl RenderError {
    /// An error with this message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RenderError {}

/// A frame read back from the GPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes of sRGB RGBA8, rows top first, tightly
    /// packed.
    pub pixels: Vec<u8>,
}

/// One robot a frame draws.
#[derive(Debug)]
#[non_exhaustive]
pub struct DrawItem<'a, G> {
    /// The creature's key.
    pub key: &'a str,
    /// The build of the key this robot is.
    pub generation: u64,
    /// The robot as built.
    pub robot: &'a Robot,
    /// The same robot as [`Renderer::upload`] returned it.
    pub gpu: &'a G,
    /// Its pose this frame. Its face set is drawn as the pose gives it
    /// until drawn moods are tracked, which hold a new face until a blink.
    pub pose: &'a Pose,
    /// Where its model origin stands, in model units. Every robot stands
    /// at the origin until the plaza is laid out.
    pub offset: [f64; 3],
}

/// What one frame draws. The camera joins it with the stage camera and the
/// plaza layout.
#[derive(Debug)]
#[non_exhaustive]
pub struct Frame<'a, G> {
    /// The frame's width in device pixels; never 0.
    pub width_px: u32,
    /// The frame's height in device pixels; never 0.
    pub height_px: u32,
    /// Device pixels per view point.
    pub scale: f64,
    /// The host's `target_time` for this frame.
    pub target_time: f64,
    /// Stage time, which the poses were taken at.
    pub stage_time: f64,
    /// The robots, each once, in cast order.
    pub items: &'a [DrawItem<'a, G>],
}

/// What the stage needs from the GPU. Every call comes from the thread
/// running a render call, or from a command thread while no render call
/// runs (pause releases at once), never from two threads at a time.
///
/// A drawn frame calls [`Renderer::begin`], then [`Renderer::upload`] for
/// robots whose builds landed and [`Renderer::retire`] for robots no
/// longer shown, then [`Renderer::submit`], then [`Renderer::present`] or
/// [`Renderer::read_back`] as the target is.
pub trait Renderer: Send + 'static {
    /// A robot's resources on the GPU.
    type Robot: Send + 'static;

    /// Where the renderer's frames go.
    fn target(&self) -> Target;

    /// Whether the device times frames with timestamp queries.
    fn timestamps(&self) -> bool;

    /// Starts a frame of this size: reconfigures the surface or the frame
    /// targets when the size changed, and acquires the surface's next
    /// drawable. A failure here leaves the frame uncommitted.
    fn begin(&mut self, width_px: u32, height_px: u32, scale: f64) -> Result<(), Failure>;

    /// Uploads a newly built robot. A renderer may take the robot's texture
    /// pixels, so none stay on the CPU after upload.
    fn upload(
        &mut self,
        key: &str,
        generation: u64,
        robot: &mut Robot,
    ) -> Result<Self::Robot, Failure>;

    /// Gives back a robot no later frame draws. Its resources are freed
    /// once the GPU has finished the frames already submitted with it.
    fn retire(&mut self, robot: Self::Robot);

    /// Draws a frame and submits it to the GPU.
    fn submit(&mut self, frame: &Frame<'_, Self::Robot>) -> Result<(), Failure>;

    /// Presents the submitted frame on the surface.
    fn present(&mut self) -> Result<(), Failure>;

    /// Reads the submitted frame back.
    fn read_back(&mut self) -> Result<Image, Failure>;

    /// The GPU times, in seconds, of earlier presented or read-back frames
    /// whose timestamp results arrived since the last call. Empty without
    /// timestamp queries.
    fn take_gpu_times(&mut self) -> Vec<f64>;

    /// Frees every GPU resource but the surface: frame and readback targets
    /// and anything retired. The stage retires every robot first. Called
    /// when the stage is paused; the next frame after resume makes what it
    /// needs again.
    fn release(&mut self);

    /// Makes a new device after [`Failure::DeviceLost`]. The stage drops
    /// every robot uploaded before without retiring it, and uploads them
    /// again as they are rebuilt.
    fn recover(&mut self) -> Result<(), Failure>;

    /// An estimate of the GPU memory the renderer holds, in bytes.
    fn memory_bytes(&self) -> u64;
}
