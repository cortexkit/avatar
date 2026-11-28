//! The wgpu side: pipelines, a robot's buffers and textures on the GPU, and
//! offscreen frames read back as RGBA8 or drawn onto a window ([`surface`]).

mod surface;

use std::borrow::Cow;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};

use avatar_3d::motion::Pose;
use avatar_3d::robot::Robot;
use avatar_small_robots::Rgb;
use wgpu::util::DeviceExt;

use crate::colour::{dark_ratio, linear};
use crate::frame::{self, Shadow};
use crate::layers::{self, Residency};
use crate::upload::{convert, mip_chain, mip_levels};
use crate::view::View;
use crate::{DEPTH_FORMAT, SAMPLE_COUNT, SHADER, TARGET_FORMAT, TEXTURE_FORMAT};

/// Why the renderer could not start or finish a frame.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderError {
    /// No GPU adapter was found.
    NoAdapter(String),
    /// The adapter refused a device.
    NoDevice(String),
    /// The frame could not be read back.
    Readback(String),
    /// Waiting for the GPU to finish failed.
    Wait(String),
    /// A window's surface cannot be drawn to.
    Surface(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter(m) => write!(f, "no GPU adapter: {m}"),
            Self::NoDevice(m) => write!(f, "no GPU device: {m}"),
            Self::Readback(m) => write!(f, "readback failed: {m}"),
            Self::Wait(m) => write!(f, "waiting for the GPU failed: {m}"),
            Self::Surface(m) => write!(f, "the window's surface: {m}"),
        }
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
    /// `width × height × 4` bytes, RGBA8 sRGB, rows top first, opaque.
    pub pixels: Vec<u8>,
}

/// One robot in a scene, in a pose, standing at an offset from the origin.
#[derive(Clone, Copy, Debug)]
pub struct Item<'a> {
    /// The robot as built.
    pub robot: &'a Robot,
    /// The same robot uploaded with [`Renderer::upload`] or
    /// [`Renderer::upload_consuming`].
    pub gpu: &'a GpuRobot,
    /// Its pose this frame.
    pub pose: &'a Pose,
    /// Where its model origin stands in the scene, in model units.
    pub offset: [f64; 3],
}

/// What one frame draws.
#[derive(Clone, Copy, Debug)]
pub struct Scene<'a> {
    /// The camera; its aspect should be `width / height`.
    pub view: View,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// The sRGB colour behind everything.
    pub background: Rgb,
    /// The robots, each at most once: a robot's per-frame buffers hold one
    /// pose.
    pub items: &'a [Item<'a>],
}

/// The offscreen renderer: one device, its pipelines, and the frame targets
/// of the last size drawn.
pub struct Renderer {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: wgpu::AdapterInfo,
    robot_layout: wgpu::BindGroupLayout,
    robot_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    frame_buffer: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    targets: Mutex<Option<Targets>>,
    timer: Option<Timer>,
    /// What draws frames onto windows, made for the first one.
    blit: Mutex<Option<surface::Blit>>,
    /// Robot array textures replaced or given back, kept until the GPU has
    /// finished the frames that could sample them.
    retired: Mutex<Vec<Retired>>,
    /// The bytes of the textures in `retired`.
    retired_bytes: AtomicU64,
}

/// A robot array texture no later frame samples, which the GPU may still be
/// reading.
struct Retired {
    texture: wgpu::Texture,
    /// Its bytes, mip levels included.
    bytes: u64,
    /// Set once the GPU has finished every submission made up to its
    /// retirement.
    done: Arc<AtomicBool>,
}

/// The timestamp queries that time a drawn frame's scene pass on the GPU.
struct Timer {
    /// Two timestamps: the scene pass's start and end.
    queries: wgpu::QuerySet,
    /// Where the queries resolve to, on the GPU.
    resolve: wgpu::Buffer,
    /// The resolved timestamps copied where the CPU can map them.
    readback: wgpu::Buffer,
    /// Nanoseconds per timestamp tick.
    period: f64,
}

/// The bytes of two resolved timestamps.
const TIMESTAMP_BYTES: u64 = 2 * wgpu::QUERY_SIZE as u64;

impl fmt::Debug for Renderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Renderer")
            .field("adapter", &self.info.name)
            .field("backend", &self.info.backend)
            .finish_non_exhaustive()
    }
}

/// A robot's geometry, textures and per-frame buffers on the GPU.
#[derive(Debug)]
pub struct GpuRobot {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    uniform: wgpu::Buffer,
    joints: wgpu::Buffer,
    materials: wgpu::Buffer,
    /// Per texture, its canvas and its layer ([`layers::binding_data`]).
    bindings: wgpu::Buffer,
    /// Per texture, its canvas as `bindings` holds it.
    canvases: Vec<[f32; 3]>,
    /// The resident textures, one layer each.
    texture: wgpu::Texture,
    /// Which layer of `texture` holds each texture.
    residency: Residency,
    group: wgpu::BindGroup,
    bones: usize,
    material_count: usize,
    /// Bytes of the array texture holding the resident textures, mip
    /// levels included.
    pub texture_bytes: u64,
    /// Bytes of vertex and index memory.
    pub geometry_bytes: u64,
    /// Bytes of the buffers rewritten every frame (placement, skin
    /// matrices, material colours) and of the texture bindings.
    pub frame_bytes: u64,
    /// Triangles drawn.
    pub triangles: u32,
}

impl GpuRobot {
    /// Which of the robot's textures are resident, at which layer and edge.
    pub fn residency(&self) -> &Residency {
        &self.residency
    }
}

struct Targets {
    width: u32,
    height: u32,
    colour: wgpu::TextureView,
    resolve: wgpu::Texture,
    resolve_view: wgpu::TextureView,
    depth: wgpu::TextureView,
    readback: wgpu::Buffer,
    padded_row: u32,
}

/// One vertex as the robot pipeline reads it.
const VERTEX_BYTES: u64 = 80;
/// One shadow instance: centre, radii, opacity.
const SHADOW_BYTES: u64 = 24;
/// The no-texture marker of a vertex's marks slot.
const NO_TEXTURE: u32 = u32::MAX;

/// What a submitted frame is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    /// An image to read back.
    Image,
    /// A frame timed on the GPU, not read back.
    Timed,
    /// A frame drawn onto a window, neither read back nor timed.
    Shown,
}

/// A frame [`Renderer::draw`] submitted, which the GPU may still be
/// drawing.
#[must_use = "wait for the frame before drawing the next one"]
#[derive(Debug)]
pub struct Submitted<'r> {
    renderer: &'r Renderer,
}

impl Submitted<'_> {
    /// Waits until the GPU has finished the frame and returns how long its
    /// scene pass took there, in seconds, from timestamps written at the
    /// pass's start and end. `None` when the device has no timestamp
    /// queries, or they read back as no time at all.
    pub fn wait(self) -> Result<Option<f64>, RenderError> {
        let renderer = self.renderer;
        renderer.wait_idle()?;
        let Some(timer) = &renderer.timer else {
            return Ok(None);
        };
        // The timestamps are resolved only once the frame has finished. On
        // Metal, a resolve recorded after the pass in the frame's own
        // submission read the end timestamp before the GPU had written it,
        // so each frame's end came back with the next frame.
        let mut encoder = renderer
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame time"),
            });
        encoder.resolve_query_set(&timer.queries, 0..2, &timer.resolve, 0);
        encoder.copy_buffer_to_buffer(&timer.resolve, 0, &timer.readback, 0, TIMESTAMP_BYTES);
        renderer.queue.submit([encoder.finish()]);
        let slice = timer.readback.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            // The receiver waits below, so the send cannot fail.
            let _ = tx.send(r);
        });
        renderer.wait_idle()?;
        rx.recv()
            .map_err(|e| RenderError::Readback(e.to_string()))?
            .map_err(|e| RenderError::Readback(e.to_string()))?;
        let ticks = {
            let data = slice
                .get_mapped_range()
                .map_err(|e| RenderError::Readback(e.to_string()))?;
            let stamp = |i: usize| {
                let mut bytes = [0; 8];
                bytes.copy_from_slice(&data[8 * i..8 * i + 8]);
                u64::from_le_bytes(bytes)
            };
            stamp(1).checked_sub(stamp(0))
        };
        timer.readback.unmap();
        Ok(ticks
            .filter(|t| *t > 0)
            .map(|t| t as f64 * timer.period * 1e-9))
    }
}

/// The bytes of the frame targets [`Renderer`] keeps for a frame of this
/// size: the multisampled colour and depth, the resolved colour, and the
/// readback buffer with its rows padded for the copy.
pub fn target_bytes(width: u32, height: u32) -> u64 {
    let pixels = u64::from(width) * u64::from(height);
    let samples = u64::from(SAMPLE_COUNT);
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_row = u64::from((4 * width).div_ceil(align) * align);
    // Rgba8UnormSrgb colour and Depth32Float depth both take four bytes a
    // sample.
    4 * pixels * samples + 4 * pixels * samples + 4 * pixels + padded_row * u64::from(height)
}

impl Renderer {
    /// Starts a renderer on the system's preferred high-performance adapter.
    /// The `WGPU_BACKEND` environment variable can pick the backend.
    pub fn new() -> Result<Self, RenderError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        Self::on_instance(&instance, None)
    }

    /// Starts a renderer on `instance`'s preferred high-performance adapter
    /// among those that can present to `surface`, if one is given.
    fn on_instance(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, RenderError> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface,
            apply_limit_buckets: false,
        }))
        .map_err(|e| RenderError::NoAdapter(e.to_string()))?;
        // Timestamp queries time frames on the GPU when the adapter has
        // them ([`Renderer::draw`]); without them frames draw the same.
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("avatar-3d"),
            required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
            ..Default::default()
        }))
        .map_err(|e| RenderError::NoDevice(e.to_string()))?;
        Ok(Self::with_device(adapter, device, queue))
    }

    /// The adapter's name and backend.
    pub fn adapter(&self) -> &wgpu::AdapterInfo {
        &self.info
    }

    /// Whether frames drawn with [`Renderer::draw`] are timed on the GPU:
    /// the device has timestamp queries.
    pub fn timestamps(&self) -> bool {
        self.timer.is_some()
    }

    fn with_device(adapter: wgpu::Adapter, device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let info = adapter.get_info();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("robots"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });
        let uniform = |binding, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let storage = |binding, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let both = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let frag = wgpu::ShaderStages::FRAGMENT;
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[uniform(0, both)],
        });
        let robot_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("robot"),
            entries: &[
                uniform(0, both),
                storage(1, wgpu::ShaderStages::VERTEX),
                storage(2, frag),
                storage(3, frag),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: frag,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: frag,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let robot_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("robot"),
                bind_group_layouts: &[Some(&frame_layout), Some(&robot_layout)],
                immediate_size: 0,
            });
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow"),
                bind_group_layouts: &[Some(&frame_layout)],
                immediate_size: 0,
            });
        let robot_attributes = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x2,
            3 => Uint32x4,
            4 => Float32x4,
            5 => Uint32x4,
        ];
        let shadow_attributes = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x2,
            2 => Float32,
        ];
        let multisample = wgpu::MultisampleState {
            count: SAMPLE_COUNT,
            mask: !0,
            alpha_to_coverage_enabled: false,
        };
        let robot_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("robot"),
            layout: Some(&robot_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_robot"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: VERTEX_BYTES,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &robot_attributes,
                })],
            },
            // Open solids show their insides, and a bone may mirror its
            // part, so nothing is culled; the shader turns back faces'
            // normals round.
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample,
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_robot"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TARGET_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: SHADOW_BYTES,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &shadow_attributes,
                })],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            // The shadow lies on the floor behind whatever stands on it and
            // writes no depth, so overlapping shadows blend.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample,
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_shadow"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TARGET_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("layers"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 4,
            ..Default::default()
        });
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: 128,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });
        let timer = device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| Timer {
                queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("frame time"),
                    ty: wgpu::QueryType::Timestamp,
                    count: 2,
                }),
                resolve: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("frame time resolve"),
                    size: TIMESTAMP_BYTES,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("frame time readback"),
                    size: TIMESTAMP_BYTES,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                period: f64::from(queue.get_timestamp_period()),
            });
        Self {
            adapter,
            device,
            queue,
            info,
            robot_layout,
            robot_pipeline,
            shadow_pipeline,
            sampler,
            frame_buffer,
            frame_group,
            targets: Mutex::new(None),
            timer,
            blit: Mutex::new(None),
            retired: Mutex::new(Vec::new()),
            retired_bytes: AtomicU64::new(0),
        }
    }

    /// Uploads a robot: its geometry, its textures converted for sRGB
    /// sampling ([`crate::upload`]) with their mip chains, and per-frame
    /// buffers sized for its bones and materials.
    ///
    /// Every texture is resident at its own index ([`Residency::every`]),
    /// at the edge the robot's textures share. A texture built without
    /// pixels, such as a face of a mood outside the build's
    /// [`avatar_3d::robot::BuildOptions::moods`], takes a transparent layer
    /// that [`Renderer::restock`] can fill later. The robot keeps its
    /// pixels; [`Renderer::upload_consuming`] frees them instead.
    ///
    /// # Panics
    ///
    /// Panics when the robot's textures do not share one edge.
    pub fn upload(&self, robot: &Robot) -> GpuRobot {
        let residency = Residency::every(robot.textures.len(), layers::edge(robot));
        let texture = self.layer_texture(&residency);
        for (binding, t) in robot.textures.iter().enumerate() {
            if let (Some(pixels), Some(layer)) = (&t.pixels, residency.layer(binding)) {
                self.write_layer(&texture, layer, residency.px(), pixels);
            }
        }
        self.upload_with(robot, residency, texture)
    }

    /// Uploads a robot as [`Renderer::upload`] does, but takes each
    /// texture's pixels out of the robot and frees them once uploaded, so
    /// no copy stays on the CPU. Only the textures that had pixels are
    /// resident, packed in binding order ([`Residency::packed`]); the
    /// others draw nothing until [`Renderer::restock`] brings them in.
    ///
    /// # Panics
    ///
    /// As [`Renderer::upload`].
    pub fn upload_consuming(&self, robot: &mut Robot) -> GpuRobot {
        let resident: Vec<usize> = robot
            .textures
            .iter()
            .enumerate()
            .filter(|(_, t)| t.pixels.is_some())
            .map(|(binding, _)| binding)
            .collect();
        let residency = Residency::packed(robot.textures.len(), layers::edge(robot), &resident);
        let texture = self.layer_texture(&residency);
        for (binding, t) in robot.textures.iter_mut().enumerate() {
            if let (Some(pixels), Some(layer)) = (t.pixels.take(), residency.layer(binding)) {
                self.write_layer(&texture, layer, residency.px(), &pixels);
            }
        }
        self.upload_with(robot, residency, texture)
    }

    /// Moves an uploaded robot to a new resident set at `px` pixels a side:
    /// the textures in `keep`, copied from their current layers on the GPU
    /// with every mip level, and the textures in `fresh`, uploaded from
    /// their pixels (sRGB premultiplied by alpha, `px` × `px` RGBA8, as
    /// [`avatar_3d::robot::face_set`] returns them), which are freed once
    /// uploaded. A change of edge keeps nothing, so `fresh` then holds
    /// every texture the robot needs, marks and chest included.
    ///
    /// The robot gets a new array texture holding exactly the new set,
    /// packed in binding order ([`Residency::plan`]), its bindings buffer's
    /// layer lanes are rewritten, and its bind group is rebuilt. Nothing
    /// waits for the GPU. The old array texture is retired: it is freed
    /// once the GPU has finished every frame submitted before this call,
    /// the last that can sample it, and until then it counts in
    /// [`Renderer::retired_bytes`]. A frame drawn after this call samples
    /// the new set.
    ///
    /// # Panics
    ///
    /// As [`Residency::plan`], and when a texture in `fresh` does not have
    /// `px` × `px` × 4 bytes.
    pub fn restock(
        &self,
        gpu: &mut GpuRobot,
        px: u32,
        keep: &[usize],
        fresh: Vec<(usize, Vec<u8>)>,
    ) {
        self.free_retired();
        let named: Vec<usize> = fresh.iter().map(|(binding, _)| *binding).collect();
        let plan = gpu.residency.plan(px, keep, &named);
        if fresh.is_empty() && plan.residency == gpu.residency {
            return;
        }
        let texture = self.layer_texture(&plan.residency);
        for ((_, pixels), &layer) in fresh.into_iter().zip(&plan.uploads) {
            self.write_layer(&texture, layer, px, &pixels);
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("robot layers"),
            });
        let layer_copy = |texture, mip_level, z| wgpu::TexelCopyTextureInfo {
            texture,
            mip_level,
            origin: wgpu::Origin3d { x: 0, y: 0, z },
            aspect: wgpu::TextureAspect::All,
        };
        for &(from, to) in &plan.copies {
            for level in 0..mip_levels(px) {
                let edge = (px >> level).max(1);
                encoder.copy_texture_to_texture(
                    layer_copy(&gpu.texture, level, from),
                    layer_copy(&texture, level, to),
                    wgpu::Extent3d {
                        width: edge,
                        height: edge,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        self.queue.write_buffer(
            &gpu.bindings,
            0,
            &layers::canvas_data(&gpu.canvases, &plan.residency),
        );
        // The copies, the fresh layers and the new layer lanes go to the GPU
        // in one submission, after every frame already submitted.
        self.queue.submit([encoder.finish()]);
        gpu.group = self.robot_group(
            [&gpu.uniform, &gpu.joints, &gpu.materials, &gpu.bindings],
            &texture,
        );
        let old = std::mem::replace(&mut gpu.texture, texture);
        self.retire_texture(old, gpu.texture_bytes);
        gpu.texture_bytes = plan.residency.texture_bytes();
        gpu.residency = plan.residency;
    }

    /// Gives back an uploaded robot that no later frame draws. Its array
    /// texture is retired as [`Renderer::restock`] retires an old one; its
    /// buffers are dropped at once, and wgpu keeps them for as long as
    /// submitted frames use them.
    pub fn retire(&self, gpu: GpuRobot) {
        self.free_retired();
        // Submits any upload still queued for the robot, so that the
        // texture counts as retired until that upload is done too.
        self.queue.submit([]);
        self.retire_texture(gpu.texture, gpu.texture_bytes);
    }

    /// The bytes of robot array textures retired by [`Renderer::restock`]
    /// or [`Renderer::retire`] and not freed yet, mip levels included. A
    /// retired texture is freed on the first [`Renderer::restock`],
    /// [`Renderer::retire`], drawn frame or [`Renderer::finish_uploads`]
    /// after the GPU has finished the frames that could sample it.
    pub fn retired_bytes(&self) -> u64 {
        self.retired_bytes.load(Ordering::Acquire)
    }

    /// Keeps `texture` until the GPU has finished everything submitted so
    /// far, then lets [`Renderer::free_retired`] drop it.
    fn retire_texture(&self, texture: wgpu::Texture, bytes: u64) {
        let done = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&done);
        // wgpu runs this once the last submission so far has finished on
        // the GPU, from whichever later submit or poll notices it.
        self.queue
            .on_submitted_work_done(move || flag.store(true, Ordering::Release));
        self.retired_bytes.fetch_add(bytes, Ordering::AcqRel);
        self.retired
            .lock()
            .expect("the retired textures lock")
            .push(Retired {
                texture,
                bytes,
                done,
            });
    }

    /// Drops every retired texture the GPU has finished with, which frees
    /// it: no bind group a later frame uses refers to it.
    fn free_retired(&self) {
        let finished: Vec<Retired> = self
            .retired
            .lock()
            .expect("the retired textures lock")
            .extract_if(.., |r| r.done.load(Ordering::Acquire))
            .collect();
        for r in finished {
            drop(r.texture);
            self.retired_bytes.fetch_sub(r.bytes, Ordering::AcqRel);
        }
    }

    /// The rest of an upload once its array texture is made: the geometry,
    /// the per-frame buffers, the bindings buffer with `residency`'s layer
    /// lanes, and the bind group.
    fn upload_with(&self, robot: &Robot, residency: Residency, texture: wgpu::Texture) -> GpuRobot {
        let (vertices, indices) = vertex_data(robot);
        let device = &self.device;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("robot vertices"),
            contents: &vertices,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("robot indices"),
            contents: &bytes_u32(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("robot uniform"),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let joints = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("robot joints"),
            size: 128 * robot.bones.len().max(1) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let materials = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("robot materials"),
            size: 48 * robot.materials.len().max(1) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let canvases = layers::canvases(robot);
        // Rewritten when the resident set changes ([`Renderer::restock`]).
        let bindings = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("robot texture bindings"),
            contents: &layers::canvas_data(&canvases, &residency),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let group = self.robot_group([&uniform, &joints, &materials, &bindings], &texture);
        GpuRobot {
            index_count: indices.len() as u32,
            geometry_bytes: (vertices.len() + 4 * indices.len()) as u64,
            frame_bytes: uniform.size() + joints.size() + materials.size() + bindings.size(),
            triangles: (indices.len() / 3) as u32,
            vertices: vertex_buffer,
            indices: index_buffer,
            uniform,
            joints,
            materials,
            bindings,
            canvases,
            group,
            bones: robot.bones.len(),
            material_count: robot.materials.len(),
            texture_bytes: residency.texture_bytes(),
            texture,
            residency,
        }
    }

    /// A robot's bind group over its uniform, joints, materials and
    /// bindings buffers, in that order, and its array texture.
    fn robot_group(&self, buffers: [&wgpu::Buffer; 4], texture: &wgpu::Texture) -> wgpu::BindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let [uniform, joints, materials, bindings] = buffers;
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("robot"),
            layout: &self.robot_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: joints.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: materials.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: bindings.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// An empty array texture for a resident set, with a full mip chain.
    /// Its layers start transparent, as wgpu clears every new texture. It
    /// can be copied from, so a later [`Renderer::restock`] can keep its
    /// layers.
    fn layer_texture(&self, residency: &Residency) -> wgpu::Texture {
        let px = residency.px();
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("robot textures"),
            size: wgpu::Extent3d {
                width: px,
                height: px,
                depth_or_array_layers: residency.texture_layers(),
            },
            mip_level_count: mip_levels(px),
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// Queues one texture's pixels, converted ([`convert`]) with their mip
    /// chain, into a layer of an array texture `px` pixels a side.
    fn write_layer(&self, texture: &wgpu::Texture, layer: u32, px: u32, pixels: &[u8]) {
        assert_eq!(
            pixels.len(),
            4 * px as usize * px as usize,
            "a texture's pixels are {px} × {px} RGBA8"
        );
        for (level, data) in mip_chain(convert(pixels), px).iter().enumerate() {
            let edge = (px >> level).max(1);
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * edge),
                    rows_per_image: Some(edge),
                },
                wgpu::Extent3d {
                    width: edge,
                    height: edge,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Draws a scene and reads it back.
    ///
    /// # Panics
    ///
    /// Panics when an item's robot and uploaded robot disagree on their
    /// bones or materials, or when one uploaded robot appears twice.
    pub fn render(&self, scene: &Scene<'_>) -> Result<Image, RenderError> {
        let targets = self.submit(scene, Purpose::Image);
        self.read_back(targets.as_ref().expect("the frame targets were just made"))
    }

    /// Draws a scene without reading it back, and returns as soon as the
    /// frame is submitted, so the CPU's share of a frame can be timed apart
    /// from the GPU's. [`Submitted::wait`] then waits for the GPU and gives
    /// its time for the frame.
    ///
    /// # Panics
    ///
    /// As [`Renderer::render`].
    pub fn draw(&self, scene: &Scene<'_>) -> Submitted<'_> {
        drop(self.submit(scene, Purpose::Timed));
        Submitted { renderer: self }
    }

    /// Submits every upload still queued and waits until the GPU has
    /// finished it. [`Renderer::upload`] queues its texture copies, which
    /// otherwise run with the next frame. Every retired texture is freed
    /// ([`Renderer::retired_bytes`]).
    pub fn finish_uploads(&self) -> Result<(), RenderError> {
        self.queue.submit([]);
        self.wait_idle()?;
        self.free_retired();
        Ok(())
    }

    /// Waits until the GPU has finished everything submitted.
    fn wait_idle(&self) -> Result<(), RenderError> {
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map(drop)
            .map_err(|e| RenderError::Wait(e.to_string()))
    }

    /// Writes a scene's per-frame buffers, records its pass and submits it:
    /// for an image with the copy to the readback buffer, for a timed frame
    /// with the timestamps when the device has them. Returns the frame
    /// targets, still locked, for the readback.
    fn submit(&self, scene: &Scene<'_>, purpose: Purpose) -> MutexGuard<'_, Option<Targets>> {
        for (i, a) in scene.items.iter().enumerate() {
            assert!(
                scene.items[..i].iter().all(|b| !std::ptr::eq(a.gpu, b.gpu)),
                "an uploaded robot appears once per scene"
            );
        }
        let (near, far) = scene.view.clip_range();
        let mut frame_data = Vec::with_capacity(128);
        push_matrix(&mut frame_data, &scene.view.view_matrix());
        push_matrix(&mut frame_data, &scene.view.projection_matrix(near, far));
        self.queue.write_buffer(&self.frame_buffer, 0, &frame_data);
        let mut shadows = Vec::with_capacity(scene.items.len() * SHADOW_BYTES as usize);
        for item in scene.items {
            self.write_item(item);
            let Shadow {
                centre,
                radii,
                opacity,
            } = frame::shadow(item.robot, item.pose);
            for v in [
                centre[0] + item.offset[0],
                centre[1] + item.offset[1],
                centre[2] + item.offset[2],
                radii[0],
                radii[1],
                opacity,
            ] {
                shadows.extend_from_slice(&(v as f32).to_le_bytes());
            }
        }
        let shadow_buffer = (!shadows.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("shadows"),
                    contents: &shadows,
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });

        let mut guard = self.targets.lock().expect("the frame targets lock");
        if guard
            .as_ref()
            .is_none_or(|t| t.width != scene.width || t.height != scene.height)
        {
            *guard = Some(self.targets(scene.width, scene.height));
        }
        let targets = guard.as_ref().expect("the frame targets were just made");
        let timer = match purpose {
            Purpose::Timed => self.timer.as_ref(),
            Purpose::Image | Purpose::Shown => None,
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let bg = linear(scene.background);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.colour,
                    depth_slice: None,
                    resolve_target: Some(&targets.resolve_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: bg[0],
                            g: bg[1],
                            b: bg[2],
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: timer.map(|t| wgpu::RenderPassTimestampWrites {
                    query_set: &t.queries,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.robot_pipeline);
            pass.set_bind_group(0, &self.frame_group, &[]);
            for item in scene.items {
                pass.set_bind_group(1, &item.gpu.group, &[]);
                pass.set_vertex_buffer(0, item.gpu.vertices.slice(..));
                pass.set_index_buffer(item.gpu.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..item.gpu.index_count, 0, 0..1);
            }
            if let Some(buffer) = &shadow_buffer {
                pass.set_pipeline(&self.shadow_pipeline);
                pass.set_bind_group(0, &self.frame_group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, 0..scene.items.len() as u32);
            }
        }
        if purpose == Purpose::Image {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &targets.resolve,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &targets.readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(targets.padded_row),
                        rows_per_image: Some(scene.height),
                    },
                },
                wgpu::Extent3d {
                    width: scene.width,
                    height: scene.height,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.queue.submit([encoder.finish()]);
        self.free_retired();
        guard
    }

    /// Writes one item's per-frame buffers: placement and face textures,
    /// skin matrices and material colours.
    fn write_item(&self, item: &Item<'_>) {
        let robot = item.robot;
        let pose = item.pose;
        assert_eq!(
            item.gpu.bones,
            robot.bones.len(),
            "the uploaded robot's bones"
        );
        assert_eq!(pose.bones.len(), robot.bones.len(), "the pose's bones");
        assert_eq!(
            item.gpu.material_count,
            robot.materials.len(),
            "the uploaded robot's materials"
        );
        assert_eq!(
            item.gpu.residency.bindings(),
            robot.textures.len(),
            "the uploaded robot's textures"
        );
        let mut uniform = Vec::with_capacity(80);
        let mut model = [0.0; 16];
        for k in [0, 5, 10, 15] {
            model[k] = 1.0;
        }
        model[12..15].copy_from_slice(&item.offset);
        push_matrix(&mut uniform, &model);
        let faces = frame::faces(robot, pose);
        let slot = |t: Option<usize>| t.map_or(-1, |i| i as i32);
        for v in [
            -1,
            slot(faces.head),
            slot(faces.stalks[0]),
            slot(faces.stalks[1]),
        ] {
            uniform.extend_from_slice(&v.to_le_bytes());
        }
        self.queue.write_buffer(&item.gpu.uniform, 0, &uniform);

        let mut joints = Vec::with_capacity(128 * robot.bones.len());
        for skin in frame::skin_matrices(robot, pose) {
            push_matrix(&mut joints, &skin);
            push_matrix(&mut joints, &frame::normal_matrix(&skin));
        }
        if !joints.is_empty() {
            self.queue.write_buffer(&item.gpu.joints, 0, &joints);
        }

        let mut materials = Vec::with_capacity(48 * robot.materials.len());
        for (i, m) in robot.materials.iter().enumerate() {
            let c = frame::colour(robot, i, pose);
            let base = linear(c);
            let dark = dark_ratio(c, m.finish.dark);
            let f = m.finish;
            for v in [
                base[0],
                base[1],
                base[2],
                frame::emission(robot, i, pose),
                dark[0],
                dark[1],
                dark[2],
                0.0,
                f.specular,
                f.shininess,
                f.edge,
                f.metal,
            ] {
                materials.extend_from_slice(&(v as f32).to_le_bytes());
            }
        }
        if !materials.is_empty() {
            self.queue.write_buffer(&item.gpu.materials, 0, &materials);
        }
    }

    fn targets(&self, width: u32, height: u32) -> Targets {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = |label, format, samples, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let colour = texture("colour", TARGET_FORMAT, SAMPLE_COUNT, attach);
        // The resolved frame is copied out for readback, or sampled to
        // draw it onto a window.
        let resolve = texture(
            "resolve",
            TARGET_FORMAT,
            1,
            attach | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = texture("depth", DEPTH_FORMAT, SAMPLE_COUNT, attach);
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_row = (4 * width).div_ceil(align) * align;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded_row) * u64::from(height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Targets {
            width,
            height,
            colour: colour.create_view(&Default::default()),
            resolve_view: resolve.create_view(&Default::default()),
            resolve,
            depth: depth.create_view(&Default::default()),
            readback,
            padded_row,
        }
    }

    fn read_back(&self, targets: &Targets) -> Result<Image, RenderError> {
        let slice = targets.readback.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            // The receiver waits below, so the send cannot fail.
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| RenderError::Readback(e.to_string()))?;
        rx.recv()
            .map_err(|e| RenderError::Readback(e.to_string()))?
            .map_err(|e| RenderError::Readback(e.to_string()))?;
        let mut pixels = Vec::with_capacity((4 * targets.width * targets.height) as usize);
        {
            let data = slice
                .get_mapped_range()
                .map_err(|e| RenderError::Readback(e.to_string()))?;
            for row in data.chunks(targets.padded_row as usize) {
                pixels.extend_from_slice(&row[..4 * targets.width as usize]);
            }
        }
        targets.readback.unmap();
        Ok(Image {
            width: targets.width,
            height: targets.height,
            pixels,
        })
    }
}

/// A robot's vertices and indices as the robot pipeline reads them.
///
/// Each vertex carries its position, normal and projection coordinate in
/// f32, its joints and weights (a rigid part's bone with weight one), and
/// its material, flags, face slot and shaded texture. The flags say
/// whether the triangle is tagged textured front and whether its outward
/// normal at rest has z > 0. A vertex shared by triangles with different
/// flags is split, so every triangle's flags hold over its whole surface.
/// The shaded texture (marks or a chest drawing) is set only on triangles
/// its binding's surface admits.
pub fn vertex_data(robot: &Robot) -> (Vec<u8>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut count: u32 = 0;
    for (index, part) in robot.parts.iter().enumerate() {
        let mesh = &part.mesh;
        let slot = frame::face_slot(robot, index);
        let marks =
            frame::marks(robot, index).map(|i| (i as u32, robot.textures[i].binding.surface));
        // The new index of each original vertex under each of the four
        // flag combinations.
        let mut split = vec![[u32::MAX; 4]; mesh.positions.len()];
        for (t, tri) in mesh.triangles.iter().enumerate() {
            let [a, b, c] = tri.map(|i| mesh.positions[i as usize]);
            let facing = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) > 0.0;
            let flags = u32::from(mesh.textured_front[t]) | u32::from(facing) << 1;
            let shaded = match marks {
                Some((i, surface))
                    if frame::surface_allows(surface, mesh.textured_front[t], facing) =>
                {
                    i
                }
                _ => NO_TEXTURE,
            };
            for &v in tri {
                let entry = &mut split[v as usize][flags as usize];
                if *entry == u32::MAX {
                    *entry = count;
                    count += 1;
                    let v = v as usize;
                    let (joints, weights) = match &part.skin {
                        avatar_3d::robot::Skin::Rigid { bone } => {
                            ([*bone as u32, 0, 0, 0], [1.0, 0.0, 0.0, 0.0])
                        }
                        avatar_3d::robot::Skin::Skinned { joints, weights } => {
                            (joints[v], weights[v])
                        }
                        _ => ([0; 4], [1.0, 0.0, 0.0, 0.0]),
                    };
                    let floats = mesh.positions[v]
                        .iter()
                        .chain(&mesh.normals[v])
                        .chain(&mesh.projection[v])
                        .copied();
                    for f in floats {
                        vertices.extend_from_slice(&(f as f32).to_le_bytes());
                    }
                    for j in joints {
                        vertices.extend_from_slice(&j.to_le_bytes());
                    }
                    for w in weights {
                        vertices.extend_from_slice(&(w as f32).to_le_bytes());
                    }
                    for i in [part.material as u32, flags, slot, shaded] {
                        vertices.extend_from_slice(&i.to_le_bytes());
                    }
                }
                indices.push(*entry);
            }
        }
    }
    debug_assert_eq!(vertices.len() as u64, u64::from(count) * VERTEX_BYTES);
    (vertices, indices)
}

fn push_matrix(out: &mut Vec<u8>, m: &[f64; 16]) {
    for v in m {
        out.extend_from_slice(&(*v as f32).to_le_bytes());
    }
}

fn bytes_u32(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
