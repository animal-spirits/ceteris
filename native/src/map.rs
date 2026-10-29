//! The map pass. It renders on the device Slint renders with, into a texture that Slint shows as
//! an `Image` (ADR 0088). A second target holds feature ids for picking.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use slint::wgpu_30::wgpu;
use wgpu::util::DeviceExt;

use crate::camera::Camera;
use crate::fill::Fill;
use crate::tokens::SEA;

// Slint composites sRGB values as stored, so the target is not an sRGB format.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const ID_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Uint;
const TEX_WIDTH: u32 = 1024;

struct Targets {
    colour: wgpu::Texture,
    colour_view: wgpu::TextureView,
    id: wgpu::Texture,
    id_view: wgpu::TextureView,
}

struct Pick {
    buffer: wgpu::Buffer,
    ready: Arc<AtomicBool>,
    pending: bool,
}

pub struct Map {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    view_buffer: wgpu::Buffer,
    colours: wgpu::Texture,
    xy: wgpu::Buffer,
    feature: wgpu::Buffer,
    index: wgpu::Buffer,
    indices: u32,
    features: u32,
    targets: Option<Targets>,
    pick: Pick,
}

impl Map {
    pub fn new(device: &wgpu::Device, fill: &Fill) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("map.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("map"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("map fills"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("map"),
                    bind_group_layouts: &[Some(&layout)],
                    immediate_size: 0,
                }),
            ),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: 8,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2],
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: 4,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![1 => Uint32],
                    }),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(FORMAT.into()), Some(ID_FORMAT.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let view_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map view"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let features = fill.features.max(1);
        let colours = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("feature colours"),
            size: wgpu::Extent3d {
                width: TEX_WIDTH,
                height: features.div_ceil(TEX_WIDTH),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("map"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &colours.create_view(&Default::default()),
                    ),
                },
            ],
        });
        let vertices = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let xy: Vec<u8> = fill
            .xy
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let words = |w: &[u32]| -> Vec<u8> { w.iter().flat_map(|v| v.to_le_bytes()).collect() };
        Self {
            pipeline,
            bind_group,
            view_buffer,
            colours,
            xy: vertices("fill xy", &xy, wgpu::BufferUsages::VERTEX),
            feature: vertices(
                "fill feature",
                &words(&fill.feature),
                wgpu::BufferUsages::VERTEX,
            ),
            index: vertices("fill index", &words(&fill.index), wgpu::BufferUsages::INDEX),
            indices: fill.index.len() as u32,
            features,
            targets: None,
            pick: Pick {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("pick"),
                    size: 256,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                ready: Arc::new(AtomicBool::new(false)),
                pending: false,
            },
        }
    }

    /// Uploads one RGBA colour per feature: 4 bytes each, and nothing else (ADR 0010).
    pub fn set_colours(&self, queue: &wgpu::Queue, rgba: &[[u8; 4]]) {
        let mut texels = vec![[0u8; 4]; (self.features.div_ceil(TEX_WIDTH) * TEX_WIDTH) as usize];
        texels[..rgba.len()].copy_from_slice(rgba);
        queue.write_texture(
            self.colours.as_image_copy(),
            texels.as_flattened(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * TEX_WIDTH),
                rows_per_image: None,
            },
            self.colours.size(),
        );
    }

    /// Draws one frame at `size` physical pixels, and when `pick` is given starts reading the
    /// feature id at that pixel. Returns the colour target when it was (re)allocated, so the
    /// caller hands the new one to Slint.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: [u32; 2],
        camera: &Camera,
        pick: Option<[u32; 2]>,
    ) -> Option<wgpu::Texture> {
        let [width, height] = size.map(|s| s.max(1));
        let stale = self
            .targets
            .as_ref()
            .is_none_or(|t| t.colour.width() != width || t.colour.height() != height);
        if stale {
            self.targets = Some(targets(device, width, height));
        }
        let targets = self.targets.as_ref()?;
        let viewport = [f64::from(width), f64::from(height)];
        let [cx, cy, kx, ky] = camera.uniform(viewport);
        // World copies left and right of the centre that can reach the viewport.
        let reach = (viewport[0] / (2.0 * camera.scale)).ceil() as i32 + 1;
        let uniform: Vec<u8> = [cx, cy, kx, ky, -(reach as f32), 0.0, 0.0, 0.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        queue.write_buffer(&self.view_buffer, 0, &uniform);

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("map") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("map"),
                color_attachments: &[
                    Some(attachment(&targets.colour_view, sea())),
                    Some(attachment(&targets.id_view, wgpu::Color::TRANSPARENT)),
                ],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.xy.slice(..));
            pass.set_vertex_buffer(1, self.feature.slice(..));
            pass.set_index_buffer(self.index.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.indices, 0, 0..(2 * reach + 1) as u32);
        }
        let pick = pick.filter(|&[x, y]| !self.pick.pending && x < width && y < height);
        if let Some([x, y]) = pick {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &targets.id,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &self.pick.buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: None,
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
        queue.submit([encoder.finish()]);
        if pick.is_some() {
            let ready = self.pick.ready.clone();
            self.pick.pending = true;
            self.pick
                .buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| {
                    if r.is_ok() {
                        ready.store(true, Ordering::Release);
                    }
                });
        }
        stale.then(|| targets.colour.clone())
    }

    /// The finished pick, if one is: `Some(None)` over the sea, `Some(Some(feature))` over a
    /// feature. Never blocks.
    pub fn poll_pick(&mut self, device: &wgpu::Device) -> Option<Option<u32>> {
        if !self.pick.pending {
            return None;
        }
        let _ = device.poll(wgpu::PollType::Poll);
        if !self.pick.ready.swap(false, Ordering::Acquire) {
            return None;
        }
        let id = {
            let bytes = self.pick.buffer.slice(..4).get_mapped_range().ok()?;
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        };
        self.pick.buffer.unmap();
        self.pick.pending = false;
        Some(id.checked_sub(1))
    }

    pub fn pick_pending(&self) -> bool {
        self.pick.pending
    }
}

fn attachment(view: &wgpu::TextureView, clear: wgpu::Color) -> wgpu::RenderPassColorAttachment<'_> {
    wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(clear),
            store: wgpu::StoreOp::Store,
        },
    }
}

fn targets(device: &wgpu::Device, width: u32, height: u32) -> Targets {
    let texture = |label, format| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    };
    let colour = texture("map colour", FORMAT);
    let id = texture("map ids", ID_FORMAT);
    Targets {
        colour_view: colour.create_view(&Default::default()),
        id_view: id.create_view(&Default::default()),
        colour,
        id,
    }
}

fn sea() -> wgpu::Color {
    let c = |v: u8| f64::from(v) / 255.0;
    wgpu::Color {
        r: c(SEA[0]),
        g: c(SEA[1]),
        b: c(SEA[2]),
        a: 1.0,
    }
}
