//! Frames on a window. A scene is drawn into the renderer's offscreen
//! targets as for any frame, and the resolved frame is then drawn onto the
//! surface's texture by one triangle that covers it ([`BLIT_SHADER`]). The
//! robot pipelines keep their one target format whatever format the
//! window's surface takes, and nothing is read back.

use super::{Purpose, RenderError, Renderer, Scene};
use crate::BLIT_SHADER;

/// What copies drawn frames onto windows: the shader, its bindings and a
/// pipeline for each target format met so far.
pub(super) struct Blit {
    shader: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
}

/// Whether a target format takes linear light: an sRGB format encodes on
/// write, and a floating-point one holds linear values. Any other format
/// stores what it is given, so the blit encodes to sRGB itself.
fn takes_linear(format: wgpu::TextureFormat) -> bool {
    use wgpu::TextureFormat as F;
    format.is_srgb() || matches!(format, F::Rgba16Float | F::Rgba32Float | F::Rg11b10Ufloat)
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

    /// The pipeline that draws onto targets of `format`, made the first
    /// time it is asked for.
    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        let at = match self.pipelines.iter().position(|(f, _)| *f == format) {
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
                        entry_point: Some(if takes_linear(format) {
                            "fs_linear"
                        } else {
                            "fs_encode"
                        }),
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
                self.pipelines.push((format, pipeline));
                self.pipelines.len() - 1
            }
        };
        &self.pipelines[at].1
    }
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
    /// (each at least 1) and returns the format chosen: the first sRGB
    /// format the surface offers, else its first. Frames are presented in
    /// step with the display's refresh, so [`Renderer::present`] paces a
    /// loop that draws as fast as it can to the display.
    pub fn configure(
        &self,
        surface: &wgpu::Surface<'_>,
        width: u32,
        height: u32,
    ) -> Result<wgpu::TextureFormat, RenderError> {
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
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: Vec::new(),
            },
        );
        Ok(format)
    }

    /// Draws a scene onto a surface texture and presents it. Draw the
    /// scene at the texture's size: a frame of another size is stretched to
    /// fill it.
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
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit"),
            layout: &blit.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&targets.resolve_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&blit.sampler),
                },
            ],
        });
        let pipeline = blit.pipeline(&self.device, target.format());
        let view = target.create_view(&Default::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("blit"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
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
    use crate::Item;

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
            view: View::from(&robot.camera).with_aspect(f64::from(width) / f64::from(height)),
            width,
            height,
            background: Rgb {
                r: 0x20,
                g: 0x90,
                b: 0xe0,
            },
            items: &items,
        };
        let image = renderer.render(&scene).expect("the frame reads back");
        for format in [
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
        ] {
            let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("window stand-in"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            renderer.draw_onto(&scene, &target);
            let mut shown = read(&renderer, &target);
            if matches!(
                format,
                wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm
            ) {
                for p in shown.chunks_mut(4) {
                    p.swap(0, 2);
                }
            }
            let worst = image
                .pixels
                .iter()
                .zip(&shown)
                .map(|(a, b)| a.abs_diff(*b))
                .max();
            assert!(
                worst.is_some_and(|w| w <= 1),
                "{format:?}: a channel differs by {worst:?}"
            );
        }
    }
}
