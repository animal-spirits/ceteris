//! The map pass. It renders on the device Slint renders with, into a texture that Slint shows as
//! an `Image` (ADR 0088). A second target holds province and county ids, read back for picking
//! and by the border pass.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use slint::wgpu_30::wgpu;
use wgpu::util::DeviceExt;

use crate::camera::Camera;
use crate::fill::Fill;
use crate::tokens::{INK, PAPER, SEA};

// Slint composites sRGB values as stored, so the target is not an sRGB format.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const ID_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Uint;
const TEX_WIDTH: u32 = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Provinces,
    Counties,
}

/// What a frame shows beyond the fills.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Style {
    /// Fill with county colours instead of province colours.
    pub county_fill: bool,
    pub county_borders: bool,
    /// Provinces outlined in ink, outlined in paper, and lightened under the pointer.
    pub selected: Option<u32>,
    pub origin: Option<u32>,
    pub hover: Option<u32>,
    /// How far province colours have moved towards the next tick's, 0 to 1.
    pub mix: f32,
}

/// The features under a pixel; ids index the pack's tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Picked {
    pub province: Option<u32>,
    pub county: Option<u32>,
}

struct Geometry {
    xy: wgpu::Buffer,
    feature: wgpu::Buffer,
    index: wgpu::Buffer,
    indices: u32,
    features: u32,
    colours: wgpu::Texture,
    /// The next tick's colours, mixed in by `Style::mix` (ADR 0208).
    next: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

struct Targets {
    colour: wgpu::Texture,
    colour_view: wgpu::TextureView,
    id: wgpu::Texture,
    id_view: wgpu::TextureView,
    borders: wgpu::BindGroup,
}

struct Pick {
    buffer: wgpu::Buffer,
    ready: Arc<AtomicBool>,
    pending: bool,
}

struct Pipelines {
    background: wgpu::RenderPipeline,
    provinces: wgpu::RenderPipeline,
    county_ids: wgpu::RenderPipeline,
    county_fill: wgpu::RenderPipeline,
    borders: wgpu::RenderPipeline,
}

pub struct Map {
    pipelines: Pipelines,
    view_buffer: wgpu::Buffer,
    provinces: Geometry,
    counties: Option<Geometry>,
    countries: wgpu::TextureView,
    border_layout: wgpu::BindGroupLayout,
    targets: Option<Targets>,
    pick: Pick,
}

fn texture_2d(
    device: &wgpu::Device,
    label: &str,
    len: u32,
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: TEX_WIDTH,
            height: len.max(1).div_ceil(TEX_WIDTH),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// Writes `texels` (4 bytes each) from the start of a `TEX_WIDTH`-wide texture.
fn write_rows(queue: &wgpu::Queue, texture: &wgpu::Texture, texels: &[[u8; 4]]) {
    let mut padded = vec![[0u8; 4]; (texture.height() * TEX_WIDTH) as usize];
    padded[..texels.len()].copy_from_slice(texels);
    queue.write_texture(
        texture.as_image_copy(),
        padded.as_flattened(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * TEX_WIDTH),
            rows_per_image: None,
        },
        texture.size(),
    );
}

impl Map {
    /// `country` gives each province's country index, for country borders.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        provinces: &Fill,
        counties: Option<&Fill>,
        country: &[u32],
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("map.wgsl"));
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty,
            count: None,
        };
        let texture = |sample_type| wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let fill_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("map fills"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(
                    1,
                    texture(wgpu::TextureSampleType::Float { filterable: false }),
                ),
                entry(
                    2,
                    texture(wgpu::TextureSampleType::Float { filterable: false }),
                ),
            ],
        });
        let border_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("map borders"),
            entries: &[
                entry(0, texture(wgpu::TextureSampleType::Uint)),
                entry(1, texture(wgpu::TextureSampleType::Uint)),
            ],
        });
        let fill_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("map fills"),
            bind_group_layouts: &[Some(&fill_layout)],
            immediate_size: 0,
        });
        let border_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("map borders"),
                bind_group_layouts: &[Some(&fill_layout), Some(&border_layout)],
                immediate_size: 0,
            });
        let target = |format: wgpu::TextureFormat, write_mask| {
            Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask,
            })
        };
        let vertex_buffers = [
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
        ];
        let pipeline = |label,
                        layout: &wgpu::PipelineLayout,
                        vs,
                        fs,
                        buffers: &[Option<wgpu::VertexBufferLayout>],
                        targets: &[Option<wgpu::ColorTargetState>]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets,
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let all = wgpu::ColorWrites::ALL;
        let pipelines = Pipelines {
            background: pipeline(
                "map background",
                &fill_pipeline_layout,
                "vs_screen",
                "fs_bg",
                &[],
                &[target(FORMAT, all), target(ID_FORMAT, all)],
            ),
            provinces: pipeline(
                "province fills",
                &fill_pipeline_layout,
                "vs",
                "fs",
                &vertex_buffers,
                &[
                    target(FORMAT, all),
                    target(ID_FORMAT, wgpu::ColorWrites::RED),
                ],
            ),
            county_ids: pipeline(
                "county ids",
                &fill_pipeline_layout,
                "vs",
                "fs",
                &vertex_buffers,
                &[
                    target(FORMAT, wgpu::ColorWrites::empty()),
                    target(ID_FORMAT, wgpu::ColorWrites::GREEN),
                ],
            ),
            county_fill: pipeline(
                "county fills",
                &fill_pipeline_layout,
                "vs",
                "fs",
                &vertex_buffers,
                &[
                    target(FORMAT, all),
                    target(ID_FORMAT, wgpu::ColorWrites::GREEN),
                ],
            ),
            borders: pipeline(
                "map borders",
                &border_pipeline_layout,
                "vs_screen",
                "fs_borders",
                &[],
                &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: all,
                })],
            ),
        };
        let view_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map view"),
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let geometry = |label: &str, fill: &Fill| {
            let colours = texture_2d(
                device,
                label,
                fill.features,
                wgpu::TextureFormat::Rgba8Unorm,
            );
            let next = texture_2d(
                device,
                label,
                fill.features,
                wgpu::TextureFormat::Rgba8Unorm,
            );
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &fill_layout,
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
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(
                            &next.create_view(&Default::default()),
                        ),
                    },
                ],
            });
            let buffer = |contents: &[u8], usage| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents,
                    usage,
                })
            };
            let words = |w: &[u32]| -> Vec<u8> { w.iter().flat_map(|v| v.to_le_bytes()).collect() };
            let xy: Vec<u8> = fill
                .xy
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            Geometry {
                xy: buffer(&xy, wgpu::BufferUsages::VERTEX),
                feature: buffer(&words(&fill.feature), wgpu::BufferUsages::VERTEX),
                index: buffer(&words(&fill.index), wgpu::BufferUsages::INDEX),
                indices: fill.index.len() as u32,
                features: fill.features,
                colours,
                next,
                bind_group,
            }
        };
        let countries = texture_2d(
            device,
            "province countries",
            country.len() as u32,
            wgpu::TextureFormat::R32Uint,
        );
        let texels: Vec<[u8; 4]> = country.iter().map(|c| c.to_le_bytes()).collect();
        write_rows(queue, &countries, &texels);
        let provinces = geometry("provinces", provinces);
        let counties = counties.map(|c| geometry("counties", c));
        Self {
            pipelines,
            view_buffer,
            provinces,
            counties,
            countries: countries.create_view(&Default::default()),
            border_layout,
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

    pub fn features(&self, layer: Layer) -> u32 {
        match layer {
            Layer::Provinces => self.provinces.features,
            Layer::Counties => self.counties.as_ref().map_or(0, |c| c.features),
        }
    }

    /// Uploads one RGBA colour per feature of `layer`: 4 bytes each, and nothing else (ADR 0010).
    pub fn set_colours(&self, queue: &wgpu::Queue, layer: Layer, rgba: &[[u8; 4]]) {
        let geometry = match layer {
            Layer::Provinces => Some(&self.provinces),
            Layer::Counties => self.counties.as_ref(),
        };
        if let Some(g) = geometry {
            write_rows(queue, &g.colours, rgba);
        }
    }

    /// Uploads the next tick's province colours, which `Style::mix` blends towards.
    pub fn set_next_colours(&self, queue: &wgpu::Queue, rgba: &[[u8; 4]]) {
        write_rows(queue, &self.provinces.next, rgba);
    }

    /// Draws one frame at `size` physical pixels, and when `pick` is given starts reading the
    /// ids at that pixel. Returns the colour target when it was (re)allocated, so the caller
    /// hands the new one to Slint.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: [u32; 2],
        camera: &Camera,
        style: Style,
        pick: Option<[u32; 2]>,
    ) -> Option<wgpu::Texture> {
        let [width, height] = size.map(|s| s.max(1));
        let stale = self
            .targets
            .as_ref()
            .is_none_or(|t| t.colour.width() != width || t.colour.height() != height);
        if stale {
            self.targets = Some(self.targets(device, width, height));
        }
        let targets = self.targets.as_ref()?;
        let viewport = [f64::from(width), f64::from(height)];
        let [cx, cy, kx, ky] = camera.uniform(viewport);
        // Flat: world copies left and right of the centre that can reach the viewport.
        let reach = if camera.globe {
            0
        } else {
            (viewport[0] / (2.0 * camera.scale)).ceil() as i32 + 1
        };
        let id = |p: Option<u32>| p.map_or(0, |p| p + 1);
        let rgba = |c: [u8; 3]| c.into_iter().chain([255]).map(|v| f32::from(v) / 255.0);
        let uniform: Vec<u8> = [cx, cy, kx, ky, -(reach as f32)]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .chain(u32::from(camera.globe).to_le_bytes())
            .chain(style.mix.to_le_bytes())
            .chain([0; 4])
            .chain(
                rgba(SEA)
                    .chain(rgba(PAPER))
                    .chain(rgba(INK))
                    .flat_map(f32::to_le_bytes),
            )
            .chain(
                [
                    u32::from(style.county_borders),
                    id(style.selected),
                    id(style.origin),
                    id(style.hover),
                ]
                .into_iter()
                .flat_map(u32::to_le_bytes),
            )
            .collect();
        queue.write_buffer(&self.view_buffer, 0, &uniform);

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("map") });
        {
            let attachment = |view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    // The background pass covers every pixel, so nothing is cleared.
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("map"),
                color_attachments: &[
                    attachment(&targets.colour_view),
                    attachment(&targets.id_view),
                ],
                ..Default::default()
            });
            pass.set_bind_group(0, &self.provinces.bind_group, &[]);
            pass.set_pipeline(&self.pipelines.background);
            pass.draw(0..3, 0..1);
            let copies = 0..(2 * reach + 1) as u32;
            let mut draw = |pipeline, g: &Geometry| {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &g.bind_group, &[]);
                pass.set_vertex_buffer(0, g.xy.slice(..));
                pass.set_vertex_buffer(1, g.feature.slice(..));
                pass.set_index_buffer(g.index.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..g.indices, 0, copies.clone());
            };
            draw(&self.pipelines.provinces, &self.provinces);
            if let Some(counties) = &self.counties {
                let pipeline = if style.county_fill {
                    &self.pipelines.county_fill
                } else {
                    &self.pipelines.county_ids
                };
                draw(pipeline, counties);
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("map borders"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.colour_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipelines.borders);
            pass.set_bind_group(0, &self.provinces.bind_group, &[]);
            pass.set_bind_group(1, &targets.borders, &[]);
            pass.draw(0..3, 0..1);
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

    /// The finished pick, if one is. Never blocks.
    pub fn poll_pick(&mut self, device: &wgpu::Device) -> Option<Picked> {
        if !self.pick.pending {
            return None;
        }
        let _ = device.poll(wgpu::PollType::Poll);
        if !self.pick.ready.swap(false, Ordering::Acquire) {
            return None;
        }
        let ids = {
            let bytes = self.pick.buffer.slice(..8).get_mapped_range().ok()?;
            let word =
                |k: usize| u32::from_le_bytes([bytes[k], bytes[k + 1], bytes[k + 2], bytes[k + 3]]);
            [word(0), word(4)]
        };
        self.pick.buffer.unmap();
        self.pick.pending = false;
        Some(Picked {
            province: ids[0].checked_sub(1),
            county: ids[1].checked_sub(1),
        })
    }

    pub fn pick_pending(&self) -> bool {
        self.pick.pending
    }

    fn targets(&self, device: &wgpu::Device, width: u32, height: u32) -> Targets {
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
        let id_view = id.create_view(&Default::default());
        let borders = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("map borders"),
            layout: &self.border_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&id_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.countries),
                },
            ],
        });
        Targets {
            colour_view: colour.create_view(&Default::default()),
            colour,
            id,
            id_view,
            borders,
        }
    }
}
