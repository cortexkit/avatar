//! Frames on a window, and the pass that converts a frame drawn over a
//! transparent background before it is read back. A scene is drawn into the
//! renderer's offscreen targets as for any frame, and the resolved frame is
//! then drawn onto the target by one triangle that covers it
//! ([`BLIT_SHADER`]). The robot pipelines keep their one target format
//! whatever format the window's surface takes, and a frame shown on a
//! window is not read back.

use super::{Alpha, Purpose, RenderError, Renderer, Scene, IMAGE_FORMAT};
use crate::BLIT_SHADER;

/// How [`Renderer::configure`] set a window's surface up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceSetup {
    /// The format of the surface's textures.
    pub format: wgpu::TextureFormat,
    /// How the window system composites the surface's alpha with what lies
    /// behind the window.
    pub alpha_mode: wgpu::CompositeAlphaMode,
}

/// What copies drawn frames onto windows and converts transparent frames
/// for read-back: the shader, its bindings and a pipeline for each target
/// format and fragment entry point met so far.
pub(super) struct Blit {
    shader: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    pipelines: Vec<((wgpu::TextureFormat, &'static str), wgpu::RenderPipeline)>,
}

/// Whether a target format takes linear light: an sRGB format encodes on
/// write, and a floating-point one holds linear values. Any other format
/// stores what it is given, so the blit encodes to sRGB itself.
fn takes_linear(format: wgpu::TextureFormat) -> bool {
    use wgpu::TextureFormat as F;
    format.is_srgb() || matches!(format, F::Rgba16Float | F::Rgba32Float | F::Rg11b10Ufloat)
}

/// The composite alpha mode to configure a surface with: `wanted` when the
/// surface supports it, else opaque when it supports that, else the first
/// mode it lists. `Auto` is never among the listed modes, so asking for it
/// gets the fallback.
fn alpha_mode(
    supported: &[wgpu::CompositeAlphaMode],
    wanted: wgpu::CompositeAlphaMode,
) -> wgpu::CompositeAlphaMode {
    use wgpu::CompositeAlphaMode as M;
    [wanted, M::Opaque]
        .into_iter()
        .find(|m| supported.contains(m))
        .or_else(|| supported.first().copied())
        .unwrap_or(M::Auto)
}

impl Blit {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });
        let frag = wgpu::ShaderStages::FRAGMENT;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: frag,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: frag,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        // Linear filtering, so a frame of another size than the target is
        // stretched smoothly; at the same size every sample lands on a
        // texel's centre and copies it exactly.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            shader,
            layout,
            pipeline_layout,
            sampler,
            pipelines: Vec::new(),
        }
    }

    /// The pipeline that draws onto targets of `format` with the fragment
    /// entry point `entry`, made the first time it is asked for.
    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        entry: &'static str,
    ) -> &wgpu::RenderPipeline {
        let at = match self
            .pipelines
            .iter()
            .position(|(k, _)| *k == (format, entry))
        {
            Some(at) => at,
            None => {
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("blit"),
                    layout: Some(&self.pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &self.shader,
                        entry_point: Some("vs_blit"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &self.shader,
                        entry_point: Some(entry),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                });
                self.pipelines.push(((format, entry), pipeline));
                self.pipelines.len() - 1
            }
        };
        &self.pipelines[at].1
    }

    /// Binds a resolved frame for the blit's entry points to read.
    fn group(&self, device: &wgpu::Device, frame: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(frame),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

/// Records one pass that draws the blit's covering triangle onto `target`.
fn cover(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
    target: &wgpu::TextureView,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("blit"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                // The triangle covers every pixel, so the clear never shows.
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
}

impl Renderer {
    /// Starts a renderer on `instance`'s preferred high-performance adapter
    /// among those that can present to `surface`, which must come from the
    /// same instance. The `WGPU_BACKEND` environment variable can pick the
    /// backend when the instance is made from the environment.
    pub fn for_surface(
        instance: &wgpu::Instance,
        surface: &wgpu::Surface<'_>,
    ) -> Result<Self, RenderError> {
        Self::on_instance(instance, Some(surface))
    }

    /// Configures `surface` for this renderer at `width` × `height` pixels
    /// (each at least 1) and returns how. The format is the first sRGB
    /// format the surface offers, else its first, so shading stays in
    /// linear light with the sRGB encoding on write. The composite alpha
    /// mode is `alpha_mode` when the surface supports it, else opaque when
    /// it supports that, else the first mode it lists.
    ///
    /// To show frames with a transparent background over what lies behind
    /// the window, ask for [`wgpu::CompositeAlphaMode::PreMultiplied`]: the
    /// frames carry linear colour premultiplied by alpha, encoded on write.
    /// When the setup reports another mode, the window does not show
    /// through, so draw an opaque background.
    ///
    /// Frames are presented in step with the display's refresh, so
    /// [`Renderer::present`] paces a loop that draws as fast as it can to
    /// the display.
    pub fn configure(
        &self,
        surface: &wgpu::Surface<'_>,
        width: u32,
        height: u32,
        alpha_mode: wgpu::CompositeAlphaMode,
    ) -> Result<SurfaceSetup, RenderError> {
        let caps = surface.get_capabilities(&self.adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| {
                RenderError::Surface(format!(
                    "the adapter {} cannot present to it",
                    self.info.name
                ))
            })?;
        let alpha_mode = self::alpha_mode(&caps.alpha_modes, alpha_mode);
        surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                color_space: wgpu::SurfaceColorSpace::Auto,
                width: width.max(1),
                height: height.max(1),
                desired_maximum_frame_latency: 2,
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode,
                view_formats: Vec::new(),
            },
        );
        Ok(SurfaceSetup { format, alpha_mode })
    }

    /// Draws a scene onto a surface texture and presents it. Draw the
    /// scene at the texture's size: a frame of another size is stretched to
    /// fill it. Alpha reaches the surface as drawn, so a frame with a
    /// transparent background shows through where nothing covers it on a
    /// surface that composites premultiplied alpha.
    ///
    /// # Panics
    ///
    /// As [`Renderer::render`].
    pub fn present(&self, scene: &Scene<'_>, frame: wgpu::SurfaceTexture) {
        self.draw_onto(scene, &frame.texture);
        self.queue.present(frame);
    }

    /// Draws a scene into the offscreen targets, then onto `target`, which
    /// must allow rendering to it, and submits both.
    fn draw_onto(&self, scene: &Scene<'_>, target: &wgpu::Texture) {
        let guard = self.submit(scene, Purpose::Shown);
        let targets = guard.as_ref().expect("the frame targets were just made");
        let mut blit = self.blit.lock().expect("the blit lock");
        let blit = blit.get_or_insert_with(|| Blit::new(&self.device));
        let group = blit.group(&self.device, &targets.resolve_view);
        let format = target.format();
        let entry = if takes_linear(format) {
            "fs_linear"
        } else {
            "fs_encode"
        };
        let pipeline = blit.pipeline(&self.device, format, entry);
        let view = target.create_view(&Default::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("blit"),
            });
        cover(&mut encoder, pipeline, &group, &view);
        self.queue.submit([encoder.finish()]);
    }

    /// Records the pass that converts a resolved frame drawn over a
    /// transparent background into `image`, a plain RGBA8 target, with its
    /// colour straight or premultiplied as `alpha` says.
    pub(super) fn alpha_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::TextureView,
        image: &wgpu::TextureView,
        alpha: Alpha,
    ) {
        let mut blit = self.blit.lock().expect("the blit lock");
        let blit = blit.get_or_insert_with(|| Blit::new(&self.device));
        let group = blit.group(&self.device, frame);
        let entry = match alpha {
            Alpha::Premultiplied => "fs_premultiplied",
            Alpha::Straight => "fs_straight",
        };
        let pipeline = blit.pipeline(&self.device, IMAGE_FORMAT, entry);
        cover(encoder, pipeline, &group, image);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use avatar_3d::motion::Actor;
    use avatar_3d::robot::build;
    use avatar_small_robots::{Mood, Rgb, GENOME_LEN};

    use super::*;
    use crate::view::View;
    use crate::{Background, Item};

    /// Reads a 4-byte-per-pixel texture back, rows top first.
    fn read(renderer: &Renderer, texture: &wgpu::Texture) -> Vec<u8> {
        let (width, height) = (texture.width(), texture.height());
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let row = (4 * width).div_ceil(align) * align;
        let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test readback"),
            size: u64::from(row * height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = renderer.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            texture.size(),
        );
        renderer.queue.submit([encoder.finish()]);
        let (tx, rx) = mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        renderer.wait_idle().expect("the GPU finishes");
        rx.recv().expect("the map").expect("the buffer maps");
        let data = buffer
            .slice(..)
            .get_mapped_range()
            .expect("the mapped bytes");
        data.chunks(row as usize)
            .flat_map(|r| r[..4 * width as usize].to_vec())
            .collect()
    }

    /// The formats a window's surface may take: sRGB or not, in RGBA or
    /// BGRA order.
    const WINDOW_FORMATS: [wgpu::TextureFormat; 4] = [
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Bgra8Unorm,
    ];

    /// Draws a scene onto a window-like target of `format` at the scene's
    /// size and reads the target back, in RGBA order.
    fn shown(renderer: &Renderer, scene: &Scene<'_>, format: wgpu::TextureFormat) -> Vec<u8> {
        let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("window stand-in"),
            size: wgpu::Extent3d {
                width: scene.width,
                height: scene.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        renderer.draw_onto(scene, &target);
        let mut shown = read(renderer, &target);
        if matches!(
            format,
            wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm
        ) {
            for p in shown.chunks_mut(4) {
                p.swap(0, 2);
            }
        }
        shown
    }

    /// The largest difference of any channel between two pixel buffers.
    fn worst(a: &[u8], b: &[u8]) -> Option<u8> {
        assert_eq!(a.len(), b.len());
        a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max()
    }

    /// Drawn onto a window-like target, sRGB or not, in RGBA or BGRA order,
    /// a frame matches the same frame read back, within the rounding of
    /// one decode and encode.
    #[test]
    #[ignore = "needs a GPU adapter"]
    fn a_frame_drawn_onto_a_target_matches_the_frame_read_back() {
        let renderer = Renderer::new().expect("a GPU adapter and device");
        let robot = build(&[0x5a; GENOME_LEN]);
        let gpu = renderer.upload(&robot);
        let pose = Actor::new(&robot, Mood::Happy).evaluate(0.0);
        let (width, height) = (160, 120);
        let items = [Item {
            robot: &robot,
            gpu: &gpu,
            pose: &pose,
            offset: [0.0; 3],
        }];
        let scene = Scene {
            view: View::from(&robot.camera()).with_aspect(f64::from(width) / f64::from(height)),
            width,
            height,
            background: Background::Opaque(Rgb {
                r: 0x20,
                g: 0x90,
                b: 0xe0,
            }),
            shadows: true,
            items: &items,
        };
        let image = renderer.render(&scene).expect("the frame reads back");
        for format in WINDOW_FORMATS {
            let worst = worst(&image.pixels, &shown(&renderer, &scene, format));
            assert!(
                worst.is_some_and(|w| w <= 1),
                "{format:?}: a channel differs by {worst:?}"
            );
        }
    }

    /// A frame with a transparent background reaches a window-like target
    /// with its alpha unchanged and its colour still premultiplied, as the
    /// frame target holds it: within the rounding of one decode and encode
    /// of the frame's own pixels, in any of the formats.
    #[test]
    #[ignore = "needs a GPU adapter"]
    fn a_transparent_frame_reaches_a_target_with_its_alpha() {
        let renderer = Renderer::new().expect("a GPU adapter and device");
        let robot = build(&[0x5a; GENOME_LEN]);
        let gpu = renderer.upload(&robot);
        let pose = Actor::new(&robot, Mood::Happy).evaluate(0.0);
        let (width, height) = (160, 120);
        let items = [Item {
            robot: &robot,
            gpu: &gpu,
            pose: &pose,
            offset: [0.0; 3],
        }];
        let scene = Scene {
            view: View::from(&robot.camera()).with_aspect(f64::from(width) / f64::from(height)),
            width,
            height,
            background: Background::Transparent,
            shadows: true,
            items: &items,
        };
        for format in WINDOW_FORMATS {
            let shown = shown(&renderer, &scene, format);
            // The frame as drawn, from the targets the draw just used.
            let drawn = {
                let guard = renderer.targets.lock().expect("the frame targets lock");
                read(&renderer, &guard.as_ref().expect("frame targets").resolve)
            };
            let alphas: Vec<u8> = drawn.chunks(4).map(|p| p[3]).collect();
            assert!(
                alphas.contains(&0)
                    && alphas.contains(&255)
                    && alphas.iter().any(|a| (1..255).contains(a)),
                "the frame has clear, covered and partly covered pixels"
            );
            let worst = worst(&drawn, &shown);
            assert!(
                worst.is_some_and(|w| w <= 1),
                "{format:?}: a channel differs by {worst:?}"
            );
        }
    }

    /// The surface gets the composite alpha mode asked for when it supports
    /// it, else opaque, else whatever it supports first.
    #[test]
    fn the_composite_alpha_mode_falls_back_to_opaque_then_the_first_listed() {
        use wgpu::CompositeAlphaMode as M;
        assert_eq!(
            alpha_mode(&[M::Opaque, M::PreMultiplied], M::PreMultiplied),
            M::PreMultiplied
        );
        assert_eq!(
            alpha_mode(&[M::Opaque, M::PreMultiplied], M::Opaque),
            M::Opaque
        );
        assert_eq!(
            alpha_mode(&[M::PostMultiplied, M::Opaque], M::PreMultiplied),
            M::Opaque
        );
        assert_eq!(alpha_mode(&[M::Inherit], M::PreMultiplied), M::Inherit);
        assert_eq!(
            alpha_mode(&[M::Inherit, M::Opaque], M::Auto),
            M::Opaque,
            "Auto is never listed, so it gets the fallback"
        );
        assert_eq!(alpha_mode(&[], M::PreMultiplied), M::Auto);
    }
}
