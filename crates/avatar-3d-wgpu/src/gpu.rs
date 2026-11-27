//! The wgpu side: pipelines, a robot's buffers and textures on the GPU, and
//! offscreen frames read back as RGBA8.

use std::borrow::Cow;
use std::fmt;
use std::sync::{mpsc, Mutex};

use avatar_3d::motion::Pose;
use avatar_3d::robot::Robot;
use avatar_small_robots::Rgb;
use wgpu::util::DeviceExt;

use crate::colour::{dark_ratio, linear};
use crate::frame::{self, Shadow};
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
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter(m) => write!(f, "no GPU adapter: {m}"),
            Self::NoDevice(m) => write!(f, "no GPU device: {m}"),
            Self::Readback(m) => write!(f, "readback failed: {m}"),
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
    /// The same robot uploaded with [`Renderer::upload`].
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
}

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
    group: wgpu::BindGroup,
    bones: usize,
    material_count: usize,
    /// Bytes of texture memory, mip levels included.
    pub texture_bytes: u64,
    /// Bytes of vertex and index memory.
    pub geometry_bytes: u64,
    /// Triangles drawn.
    pub triangles: u32,
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

impl Renderer {
    /// Starts a renderer on the system's preferred high-performance adapter.
    /// The `WGPU_BACKEND` environment variable can pick the backend.
    pub fn new() -> Result<Self, RenderError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .map_err(|e| RenderError::NoAdapter(e.to_string()))?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("avatar-3d"),
            ..Default::default()
        }))
        .map_err(|e| RenderError::NoDevice(e.to_string()))?;
        Ok(Self::with_device(device, queue, info))
    }

    /// The adapter's name and backend.
    pub fn adapter(&self) -> &wgpu::AdapterInfo {
        &self.info
    }

    fn with_device(device: wgpu::Device, queue: wgpu::Queue, info: wgpu::AdapterInfo) -> Self {
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
        Self {
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
        }
    }

    /// Uploads a robot: its geometry, its textures converted for sRGB
    /// sampling ([`crate::upload`]) with their mip chains, and per-frame
    /// buffers sized for its bones and materials.
    pub fn upload(&self, robot: &Robot) -> GpuRobot {
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
        let mut binding_data = Vec::with_capacity(robot.textures.len().max(1) * 16);
        for t in &robot.textures {
            let b = t.binding;
            for v in [b.origin[0], b.origin[1], f64::from(b.extent), 0.0] {
                binding_data.extend_from_slice(&(v as f32).to_le_bytes());
            }
        }
        if binding_data.is_empty() {
            binding_data.resize(16, 0);
        }
        let bindings = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("robot texture bindings"),
            contents: &binding_data,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let (texture, texture_bytes) = self.upload_textures(robot);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        });
        GpuRobot {
            index_count: indices.len() as u32,
            geometry_bytes: (vertices.len() + 4 * indices.len()) as u64,
            triangles: (indices.len() / 3) as u32,
            vertices: vertex_buffer,
            indices: index_buffer,
            uniform,
            joints,
            materials,
            group,
            bones: robot.bones.len(),
            material_count: robot.materials.len(),
            texture_bytes,
        }
    }

    /// Every texture of a robot as one array texture, a layer per texture
    /// in [`Robot::textures`] order. A texture built without pixels is
    /// uploaded transparent.
    fn upload_textures(&self, robot: &Robot) -> (wgpu::Texture, u64) {
        let size = avatar_3d::robot::TEXTURE_PX;
        let layers = robot.textures.len().max(1) as u32;
        let levels = mip_levels(size);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("robot textures"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: layers,
            },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut bytes = 0;
        for layer in 0..layers {
            let converted = match robot
                .textures
                .get(layer as usize)
                .and_then(|t| t.pixels.as_ref())
            {
                Some(pixels) => convert(pixels),
                None => vec![0; avatar_3d::robot::TEXTURE_BYTES],
            };
            for (level, data) in mip_chain(converted, size).iter().enumerate() {
                let edge = (size >> level).max(1);
                bytes += data.len() as u64;
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
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
        (texture, bytes)
    }

    /// Draws a scene and reads it back.
    ///
    /// # Panics
    ///
    /// Panics when an item's robot and uploaded robot disagree on their
    /// bones or materials, or when one uploaded robot appears twice.
    pub fn render(&self, scene: &Scene<'_>) -> Result<Image, RenderError> {
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
        self.queue.submit([encoder.finish()]);
        self.read_back(targets)
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
        let resolve = texture(
            "resolve",
            TARGET_FORMAT,
            1,
            attach | wgpu::TextureUsages::COPY_SRC,
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
