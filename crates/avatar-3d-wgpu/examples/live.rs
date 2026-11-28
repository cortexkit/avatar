//! A live window on the 3D small robots: the stage robot, or the plaza of
//! 33 robots, moving with the idle layer on at the display's refresh rate,
//! drawn by the same renderer as the review sheets and shown on the
//! window's surface with `Renderer::present`, which draws each frame
//! offscreen and copies it onto the surface on the GPU.
//!
//! ```text
//! cargo run --release -p cortexkit-avatar-3d-wgpu --example live
//! ```
//!
//! The keys and mouse are in [`HELP`], which is printed at start. The
//! window's title shows the robot, its mood, the reaction playing, the
//! status loop and the frame time (from one frame to the next, and the
//! posing and drawing within it); nothing but the robots is drawn in the
//! window.
//!
//! It starts on the stage robot, genome 14 of seed 501, from its stage
//! camera. The arrows step through the genomes of the seed shown, which is
//! 501 until G picks another, and go back to the stage from the plaza. On
//! the plaza (genomes 0 to 32 of seed 503, placed as the plaza sheet
//! places them) every robot takes each mood, reaction and loop, robot `i`
//! running `i` times [`PLAZA_LAG`] behind the first, so a change ripples
//! across the plaza.

#[path = "shared/robots.rs"]
mod robots;

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use avatar_3d::motion::{Actor, Pose, Reaction, StatusLoop};
use avatar_3d::robot::{build, Robot, AZIMUTH, ELEVATION};
use avatar_3d_wgpu::plaza::{self, Plaza};
use avatar_3d_wgpu::view::View;
use avatar_3d_wgpu::{GpuRobot, Item, RenderError, Renderer, Scene};
use avatar_small_robots::{mood_name, Mood, MOODS};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

use robots::{genome, place_plaza, BACKGROUND, PLAZA_ROBOTS, PLAZA_SEED, STATED};

/// The keys and the mouse.
const HELP: &str = "\
keys
  1 to 7       mood: neutral, working, happy, waiting, blocked, sad, sleepy
  H A D B P    reaction: hello, ask, done, blocked, poke
  W Q X Z      status loop: working, waiting, blocked, sleepy
  0            stop the status loop
  Left Right   the previous or next genome of the seed shown (501 at start)
  G            a random seed, from its genome 0
  V            the plaza of seed 503, or back to the stage robot
  Esc          quit
mouse
  drag         orbit the camera round the robot
  scroll       zoom";

/// Seconds each plaza robot runs behind the one before it.
const PLAZA_LAG: f64 = 0.04;
/// Degrees the camera turns per logical pixel dragged.
const DRAG_DEGREES: f64 = 0.3;
/// The camera's lowest and highest elevation, in degrees.
const ELEVATION_RANGE: (f64, f64) = (-30.0, 85.0);
/// The nearest and farthest the camera stands, as shares of the scene's
/// own distance.
const ZOOM_RANGE: (f64, f64) = (0.25, 4.0);
/// Seconds between updates of the window's title.
const TITLE_EVERY: f64 = 0.25;

/// The events given so far, each at its time on the window's clock. An
/// actor keeps no history, so each frame replays them onto a fresh actor
/// for every robot shown, and a robot switched to picks up the mood, the
/// loop and any reaction still playing.
#[derive(Default)]
struct Script {
    moods: Vec<(f64, Mood)>,
    loops: Vec<(f64, Option<StatusLoop>)>,
    reactions: Vec<(f64, Reaction)>,
}

impl Script {
    fn actor<'r>(&self, robot: &'r Robot) -> Actor<'r> {
        let mut actor = Actor::new(robot, Mood::Neutral);
        for &(t, mood) in &self.moods {
            actor.set_mood(t, mood);
        }
        for &(t, status) in &self.loops {
            actor.set_loop(t, status);
        }
        for &(t, reaction) in &self.reactions {
            actor.play(t, reaction);
        }
        actor
    }

    fn mood(&self) -> Mood {
        self.moods.last().map_or(Mood::Neutral, |e| e.1)
    }

    fn status(&self) -> Option<StatusLoop> {
        self.loops.last().and_then(|e| e.1)
    }

    /// The reaction `robot` is playing at `t`, if any.
    fn reaction(&self, robot: &Robot, t: f64) -> Option<Reaction> {
        self.reactions
            .last()
            .filter(|(at, r)| t - at <= robot.motion.reaction(*r).duration)
            .map(|e| e.1)
    }
}

/// The camera's turn away from a scene's own view.
struct Orbit {
    /// Degrees round, toward +x.
    yaw: f64,
    /// Degrees up.
    pitch: f64,
    /// The distance, as a share of the scene's own.
    zoom: f64,
}

impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            zoom: 1.0,
        }
    }
}

impl Orbit {
    /// `base`, a view from `azimuth` and `elevation` degrees, turned and
    /// zoomed, for a frame of `aspect`.
    fn view(&self, base: View, azimuth: f64, elevation: f64, aspect: f64) -> View {
        let elevation = (elevation + self.pitch).clamp(ELEVATION_RANGE.0, ELEVATION_RANGE.1);
        let turned = base.orbit(azimuth + self.yaw, elevation);
        fit(stand_back(turned, self.zoom), aspect)
    }
}

/// The same view with the camera `k` times as far from its target.
fn stand_back(view: View, k: f64) -> View {
    View {
        eye: [0, 1, 2].map(|i| view.target[i] + k * (view.eye[i] - view.target[i])),
        ..view
    }
}

/// A view for a frame of another shape that still shows what it showed: a
/// wider frame shows more at the sides, and for a narrower one the camera
/// stands back until the sides fit again.
fn fit(view: View, aspect: f64) -> View {
    if aspect >= view.aspect {
        view.with_aspect(aspect)
    } else {
        stand_back(view, view.aspect / aspect).with_aspect(aspect)
    }
}

/// A robot built and uploaded.
struct Shown {
    seed: u64,
    index: u64,
    robot: Robot,
    gpu: GpuRobot,
}

impl Shown {
    fn new(renderer: &Renderer, seed: u64, index: u64) -> Self {
        let robot = build(&genome(seed, index));
        let gpu = renderer.upload(&robot);
        println!(
            "robot {seed}/{index}: {} gear{}",
            robot.gear.kind,
            if robot.gear.hover { ", hovering" } else { "" }
        );
        Self {
            seed,
            index,
            robot,
            gpu,
        }
    }
}

/// The plaza's robots, uploaded and placed.
struct PlazaShown {
    robots: Vec<Robot>,
    gpus: Vec<GpuRobot>,
    placed: Plaza,
}

/// The window, its surface and the renderer that draws on it.
struct Gfx {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    renderer: Renderer,
    /// The surface's size in pixels.
    size: (u32, u32),
}

impl Gfx {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, String> {
        let attributes = Window::default_attributes()
            .with_title("avatar 3D robots")
            .with_inner_size(LogicalSize::new(760.0, 760.0));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|e| format!("open the window: {e}"))?,
        );
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(event_loop.owned_display_handle()),
            ));
        let surface = instance
            .create_surface(Arc::clone(&window))
            .map_err(|e| format!("the window's surface: {e}"))?;
        let renderer = Renderer::for_surface(&instance, &surface).map_err(|e| e.to_string())?;
        let size = window.inner_size();
        let size = (size.width.max(1), size.height.max(1));
        renderer
            .configure(&surface, size.0, size.1)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            window,
            surface,
            renderer,
            size,
        })
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), RenderError> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        self.size = (width, height);
        self.renderer.configure(&self.surface, width, height)?;
        Ok(())
    }
}

/// Seconds a frame takes, smoothed.
#[derive(Default)]
struct FrameTimes {
    last: Option<Instant>,
    /// From one frame to the next.
    interval: f64,
    /// Posing and drawing, until the frame is handed to the surface.
    work: f64,
}

impl FrameTimes {
    fn record(&mut self, start: Instant, done: Instant) {
        let blend = |old: f64, new: f64| {
            if old == 0.0 {
                new
            } else {
                0.9 * old + 0.1 * new
            }
        };
        if let Some(last) = self.last {
            self.interval = blend(self.interval, (start - last).as_secs_f64());
        }
        self.work = blend(self.work, (done - start).as_secs_f64());
        self.last = Some(start);
    }
}

/// What a key asks for.
#[derive(Clone, Copy, Debug)]
enum Command {
    Mood(Mood),
    Play(Reaction),
    /// Start a status loop, or stop the one running.
    Loop(Option<StatusLoop>),
    /// The next genome of the seed shown, or the previous one.
    Step(bool),
    RandomSeed,
    /// The plaza, or back to the stage robot.
    Plaza,
    Quit,
}

/// The command a pressed key gives, as [`HELP`] lists them. Moods and
/// stopping the loop go by the key's place, so the digit row works on
/// every keyboard layout; letters go by the character the layout types.
fn command(event: &KeyEvent) -> Option<Command> {
    if let PhysicalKey::Code(code) = event.physical_key {
        let digit = match code {
            KeyCode::Digit0 | KeyCode::Numpad0 => Some(0),
            KeyCode::Digit1 | KeyCode::Numpad1 => Some(1),
            KeyCode::Digit2 | KeyCode::Numpad2 => Some(2),
            KeyCode::Digit3 | KeyCode::Numpad3 => Some(3),
            KeyCode::Digit4 | KeyCode::Numpad4 => Some(4),
            KeyCode::Digit5 | KeyCode::Numpad5 => Some(5),
            KeyCode::Digit6 | KeyCode::Numpad6 => Some(6),
            KeyCode::Digit7 | KeyCode::Numpad7 => Some(7),
            _ => None,
        };
        match digit {
            Some(0) => return Some(Command::Loop(None)),
            Some(d) => return Some(Command::Mood(MOODS[d - 1])),
            None => {}
        }
    }
    match &event.logical_key {
        Key::Named(NamedKey::Escape) => Some(Command::Quit),
        Key::Named(NamedKey::ArrowLeft) => Some(Command::Step(false)),
        Key::Named(NamedKey::ArrowRight) => Some(Command::Step(true)),
        Key::Character(c) => match c.to_lowercase().as_str() {
            "h" => Some(Command::Play(Reaction::Hello)),
            "a" => Some(Command::Play(Reaction::Ask)),
            "d" => Some(Command::Play(Reaction::Done)),
            "b" => Some(Command::Play(Reaction::Blocked)),
            "p" => Some(Command::Play(Reaction::Poke)),
            "w" => Some(Command::Loop(Some(StatusLoop::Working))),
            "q" => Some(Command::Loop(Some(StatusLoop::Waiting))),
            "x" => Some(Command::Loop(Some(StatusLoop::Blocked))),
            "z" => Some(Command::Loop(Some(StatusLoop::Sleepy))),
            "g" => Some(Command::RandomSeed),
            "v" => Some(Command::Plaza),
            _ => None,
        },
        _ => None,
    }
}

struct Live {
    gfx: Option<Gfx>,
    clock: Instant,
    script: Script,
    stage: Option<Shown>,
    /// The plaza's robots, being built on another thread until first shown.
    plaza_build: Option<JoinHandle<Vec<Robot>>>,
    plaza: Option<PlazaShown>,
    show_plaza: bool,
    orbit: Orbit,
    dragging: bool,
    cursor: Option<PhysicalPosition<f64>>,
    times: FrameTimes,
    title_at: f64,
}

impl Live {
    fn new() -> Self {
        let plaza_build = std::thread::spawn(|| {
            (0..PLAZA_ROBOTS)
                .map(|i| build(&genome(PLAZA_SEED, i)))
                .collect()
        });
        Self {
            gfx: None,
            clock: Instant::now(),
            script: Script::default(),
            stage: None,
            plaza_build: Some(plaza_build),
            plaza: None,
            show_plaza: false,
            orbit: Orbit::default(),
            dragging: false,
            cursor: None,
            times: FrameTimes::default(),
            title_at: f64::NEG_INFINITY,
        }
    }

    fn now(&self) -> f64 {
        self.clock.elapsed().as_secs_f64()
    }

    /// Shows genome `index` of `seed` on the stage.
    fn show(&mut self, seed: u64, index: u64) {
        let Some(gfx) = &self.gfx else { return };
        self.stage = Some(Shown::new(&gfx.renderer, seed, index));
        if self.show_plaza {
            self.show_plaza = false;
            self.orbit = Orbit::default();
        }
        self.title_at = f64::NEG_INFINITY;
    }

    /// Switches between the stage and the plaza, building the plaza the
    /// first time.
    fn toggle_plaza(&mut self) {
        let Some(gfx) = &self.gfx else { return };
        if self.plaza.is_none() {
            let Some(handle) = self.plaza_build.take() else {
                return;
            };
            println!("plaza: waiting for its {PLAZA_ROBOTS} robots");
            let robots = handle.join().expect("the plaza's robots build");
            let gpus = robots.iter().map(|r| gfx.renderer.upload(r)).collect();
            let (_, placed) = place_plaza(&robots);
            self.plaza = Some(PlazaShown {
                robots,
                gpus,
                placed,
            });
        }
        self.show_plaza = !self.show_plaza;
        self.orbit = Orbit::default();
        self.title_at = f64::NEG_INFINITY;
    }

    /// Carries out one command at the window clock's present time.
    fn apply(&mut self, event_loop: &ActiveEventLoop, command: Command) {
        let t = self.now();
        let s = &mut self.script;
        match command {
            Command::Mood(mood) => s.moods.push((t, mood)),
            Command::Play(reaction) => s.reactions.push((t, reaction)),
            Command::Loop(status) => s.loops.push((t, status)),
            Command::Step(forward) => {
                if let Some(shown) = &self.stage {
                    let index = if forward {
                        shown.index + 1
                    } else {
                        shown.index.saturating_sub(1)
                    };
                    if index != shown.index {
                        self.show(shown.seed, index);
                    }
                }
            }
            Command::RandomSeed => {
                let seed = RandomState::new().hash_one(self.clock.elapsed()) % 1_000_000;
                self.show(seed, 0);
            }
            Command::Plaza => self.toggle_plaza(),
            Command::Quit => event_loop.exit(),
        }
        self.title_at = f64::NEG_INFINITY;
    }
    /// Draws and presents one frame.
    fn draw(&mut self) {
        let Some(gfx) = &mut self.gfx else { return };
        let frame = match gfx.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                let (w, h) = gfx.size;
                if let Err(e) = gfx.resize(w, h) {
                    eprintln!("live: {e}");
                }
                return;
            }
            _ => return,
        };
        let start = Instant::now();
        let t = self.clock.elapsed().as_secs_f64();
        let (width, height) = gfx.size;
        let aspect = f64::from(width) / f64::from(height);
        // Who is on screen: each robot with its upload, where it stands and
        // how many seconds it runs behind the clock.
        type Cast<'a> = Vec<(&'a Robot, &'a GpuRobot, [f64; 3], f64)>;
        let (view, cast): (View, Cast<'_>) = match (&self.plaza, self.show_plaza, &self.stage) {
            (Some(p), true, _) => (
                self.orbit
                    .view(p.placed.view, 0.0, plaza::ELEVATION, aspect),
                (0..p.robots.len())
                    .map(|i| {
                        (
                            &p.robots[i],
                            &p.gpus[i],
                            p.placed.offsets[i],
                            PLAZA_LAG * i as f64,
                        )
                    })
                    .collect(),
            ),
            (_, _, Some(s)) => (
                self.orbit
                    .view(View::from(&s.robot.camera), AZIMUTH, ELEVATION, aspect),
                vec![(&s.robot, &s.gpu, [0.0; 3], 0.0)],
            ),
            _ => return,
        };
        let poses: Vec<Pose> = cast
            .iter()
            .map(|(robot, _, _, lag)| self.script.actor(robot).evaluate(t - lag))
            .collect();
        let items: Vec<Item<'_>> = cast
            .iter()
            .zip(&poses)
            .map(|(&(robot, gpu, offset, _), pose)| Item {
                robot,
                gpu,
                pose,
                offset,
            })
            .collect();
        gfx.window.pre_present_notify();
        gfx.renderer.present(
            &Scene {
                view,
                width,
                height,
                background: BACKGROUND,
                items: &items,
            },
            frame,
        );
        self.times.record(start, Instant::now());
        if t - self.title_at >= TITLE_EVERY {
            self.title_at = t;
            let who = match (self.show_plaza, &self.stage) {
                (true, _) => format!("plaza of seed {PLAZA_SEED}, {PLAZA_ROBOTS} robots"),
                (false, Some(s)) => format!("robot {}/{} ({})", s.seed, s.index, s.robot.gear.kind),
                (false, None) => String::new(),
            };
            let title = format!(
                "{who} | mood {} | reaction {} | loop {} | frame {:.1} ms, drawing {:.1} ms",
                mood_name(self.script.mood()),
                self.script
                    .reaction(cast[0].0, t)
                    .map_or("none", Reaction::name),
                self.script.status().map_or("none", StatusLoop::name),
                1e3 * self.times.interval,
                1e3 * self.times.work,
            );
            gfx.window.set_title(&title);
        }
    }
}

impl ApplicationHandler for Live {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        match Gfx::new(event_loop) {
            Ok(gfx) => {
                let info = gfx.renderer.adapter();
                println!("live: {} on {:?}", info.name, info.backend);
                self.gfx = Some(gfx);
                self.show(STATED.0, STATED.1);
            }
            Err(e) => {
                eprintln!("live: {e}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gfx) = &mut self.gfx {
                    if let Err(e) = gfx.resize(size.width, size.height) {
                        eprintln!("live: {e}");
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                if let Some(command) = command(&event) {
                    self.apply(event_loop, command);
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => self.dragging = state == ElementState::Pressed,
            WindowEvent::CursorMoved { position, .. } => {
                if let (true, Some(last), Some(gfx)) = (self.dragging, self.cursor, &self.gfx) {
                    let scale = gfx.window.scale_factor();
                    self.orbit.yaw -= DRAG_DEGREES * (position.x - last.x) / scale;
                    self.orbit.pitch = (self.orbit.pitch
                        + DRAG_DEGREES * (position.y - last.y) / scale)
                        .clamp(-60.0, 75.0);
                }
                self.cursor = Some(position);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Scrolling up, away from the user, comes closer.
                let up = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    MouseScrollDelta::PixelDelta(p) => p.y / 40.0,
                };
                let step = 1.0 + 0.1 * up.abs();
                let zoom = if up > 0.0 {
                    self.orbit.zoom / step
                } else {
                    self.orbit.zoom * step
                };
                self.orbit.zoom = zoom.clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(gfx) = &self.gfx {
            gfx.window.request_redraw();
        }
    }
}

fn main() {
    println!("{HELP}");
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(e) => {
            eprintln!("live: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = event_loop.run_app(&mut Live::new()) {
        eprintln!("live: {e}");
        std::process::exit(1);
    }
}
