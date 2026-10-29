//! The map pass. It renders into a texture that Slint shows as an `Image` (ADR 0088), on the
//! device Slint renders with.

use slint::wgpu_30::wgpu;

use crate::tokens::SEA;

// Slint composites in sRGB values, so the target stores them as they are.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct Map {
    target: Option<wgpu::Texture>,
}

impl Map {
    pub fn new() -> Self {
        Self { target: None }
    }

    /// Draws one frame at `width` x `height` physical pixels. Returns the texture when it was
    /// (re)allocated, so the caller hands the new one to Slint; otherwise the old one is reused.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) -> Option<wgpu::Texture> {
        let (width, height) = (width.max(1), height.max(1));
        let stale = self
            .target
            .as_ref()
            .is_none_or(|t| t.width() != width || t.height() != height);
        if stale {
            self.target = Some(target(device, width, height));
        }
        let texture = self.target.as_ref()?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("map") });
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("map"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(sea()),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        queue.submit([encoder.finish()]);
        stale.then(|| texture.clone())
    }
}

impl Default for Map {
    fn default() -> Self {
        Self::new()
    }
}

fn target(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("map target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
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
