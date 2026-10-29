//! Offscreen map pass on whatever adapter the machine has; skipped where there is none (CI
//! installs lavapipe).

use ceteris_app::camera::Camera;
use ceteris_app::fill::Fill;
use ceteris_app::map::Map;
use ceteris_app::tokens::SEA;
use slint::wgpu_30::wgpu;

const RED: [u8; 4] = [200, 30, 40, 255];
const BLUE: [u8; 4] = [30, 60, 200, 255];

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

/// Feature 0 covers x in [0.1, 0.4], feature 1 covers x in [0.6, 0.9], both for y in [0.3, 0.7].
fn two_squares() -> Fill {
    let square = |x0: f32, x1: f32| [[x0, 0.3], [x1, 0.3], [x1, 0.7], [x0, 0.7]];
    Fill {
        xy: [square(0.1, 0.4), square(0.6, 0.9)].concat(),
        feature: vec![0, 0, 0, 0, 1, 1, 1, 1],
        index: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
        features: 2,
    }
}

fn read_pixel(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    x: u32,
    y: u32,
) -> [u8; 4] {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
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

fn pick(
    map: &mut Map,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    camera: &Camera,
    at: [u32; 2],
) -> Option<u32> {
    map.render(device, queue, [100, 100], camera, Some(at));
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    map.poll_pick(device).expect("pick finished")
}

#[test]
fn fills_take_feature_colours_and_pick_feature_ids() {
    let Some((device, queue)) = device() else {
        eprintln!("skipping: no wgpu adapter");
        return;
    };
    let mut map = Map::new(&device, &two_squares());
    map.set_colours(&queue, &[RED, BLUE]);
    // World [0, 1] fills the 100 x 100 viewport, so world x maps to pixel 100 x.
    let camera = Camera::fit(100.0, 100.0);
    let target = map
        .render(&device, &queue, [100, 100], &camera, None)
        .expect("first frame allocates");
    assert!(
        map.render(&device, &queue, [100, 100], &camera, None)
            .is_none()
    );

    assert_eq!(read_pixel(&device, &queue, &target, 25, 50), RED);
    assert_eq!(read_pixel(&device, &queue, &target, 75, 50), BLUE);
    let sea = read_pixel(&device, &queue, &target, 50, 10);
    assert_eq!(sea, [SEA[0], SEA[1], SEA[2], 255]);

    assert_eq!(pick(&mut map, &device, &queue, &camera, [25, 50]), Some(0));
    assert_eq!(pick(&mut map, &device, &queue, &camera, [75, 50]), Some(1));
    assert_eq!(pick(&mut map, &device, &queue, &camera, [50, 10]), None);

    // A recolour changes only the texture.
    map.set_colours(&queue, &[BLUE, RED]);
    map.render(&device, &queue, [100, 100], &camera, None);
    assert_eq!(read_pixel(&device, &queue, &target, 25, 50), BLUE);
}

#[test]
fn the_world_wraps_across_the_antimeridian() {
    let Some((device, queue)) = device() else {
        eprintln!("skipping: no wgpu adapter");
        return;
    };
    let mut map = Map::new(&device, &two_squares());
    map.set_colours(&queue, &[RED, BLUE]);
    let mut camera = Camera::fit(100.0, 100.0);
    // Centre on x = 0: the left half of the viewport shows the previous copy's feature 1.
    camera.centre = [0.0, 0.5];
    let target = map
        .render(&device, &queue, [100, 100], &camera, None)
        .expect("allocates");
    assert_eq!(read_pixel(&device, &queue, &target, 25, 50), BLUE);
    assert_eq!(read_pixel(&device, &queue, &target, 75, 50), RED);
}
