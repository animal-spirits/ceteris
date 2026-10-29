//! Offscreen map pass on whatever adapter the machine has; skipped where there is none (CI).

use ceteris_app::map::Map;
use ceteris_app::tokens::SEA;
use slint::wgpu_30::wgpu;

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

fn read_pixel(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> [u8; 4] {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
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
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map read"));
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let bytes = slice.get_mapped_range().expect("mapped range");
    [bytes[0], bytes[1], bytes[2], bytes[3]]
}

#[test]
fn clears_to_sea_and_reuses_the_target() {
    let Some((device, queue)) = device() else {
        eprintln!("skipping: no wgpu adapter");
        return;
    };
    let mut map = Map::new();
    let texture = map
        .render(&device, &queue, 64, 32)
        .expect("first frame allocates");
    assert!(map.render(&device, &queue, 64, 32).is_none());
    let px = read_pixel(&device, &queue, &texture);
    for (got, want) in px.iter().zip(SEA) {
        assert!(got.abs_diff(want) <= 1, "{px:?} vs {SEA:?}");
    }
    assert_eq!(px[3], 255);
    let resized = map
        .render(&device, &queue, 32, 32)
        .expect("resize reallocates");
    assert_eq!((resized.width(), resized.height()), (32, 32));
}
