//! Offscreen map pass on whatever adapter the machine has; skipped where there is none (CI
//! installs lavapipe).

use ceteris_app::camera::Camera;
use ceteris_app::fill::Fill;
use ceteris_app::map::{Layer, Map, Picked, Style};
use ceteris_app::tokens::{INK, PAPER, SEA};
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

fn picked(
    map: &mut Map,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    camera: &Camera,
    style: Style,
    at: [u32; 2],
) -> Picked {
    map.render(device, queue, [100, 100], camera, style, Some(at));
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    map.poll_pick(device).expect("pick finished")
}

fn pick(
    map: &mut Map,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    camera: &Camera,
    at: [u32; 2],
) -> Option<u32> {
    picked(map, device, queue, camera, Style::default(), at).province
}

/// Feature 0 covers x in [0.1, 0.5], feature 1 x in [0.5, 0.9], both y in [0.3, 0.7]: two
/// provinces sharing an edge at x = 0.5. Counties split feature 0 at x = 0.3.
fn neighbours() -> (Fill, Fill) {
    let square = |x0: f32, x1: f32| [[x0, 0.3], [x1, 0.3], [x1, 0.7], [x0, 0.7]];
    let quads = |n: u32| {
        (0..n)
            .flat_map(|q| [0, 1, 2, 0, 2, 3].map(|k| 4 * q + k))
            .collect()
    };
    let provinces = Fill {
        xy: [square(0.1, 0.5), square(0.5, 0.9)].concat(),
        feature: vec![0, 0, 0, 0, 1, 1, 1, 1],
        index: quads(2),
        features: 2,
    };
    let counties = Fill {
        xy: [square(0.1, 0.3), square(0.3, 0.5), square(0.5, 0.9)].concat(),
        feature: [0, 1, 2].iter().flat_map(|&f| [f; 4]).collect(),
        index: quads(3),
        features: 3,
    };
    (provinces, counties)
}

fn is_ink(px: [u8; 4], strength: f32) -> bool {
    // Ink blended over RED at `strength`.
    px.iter()
        .zip(INK.iter().zip(RED))
        .take(3)
        .all(|(&got, (&ink, base))| {
            let want = f32::from(ink) * strength + f32::from(base) * (1.0 - strength);
            (f32::from(got) - want).abs() <= 2.0
        })
}

#[test]
fn fills_take_feature_colours_and_pick_feature_ids() {
    let Some((device, queue)) = device() else {
        eprintln!("skipping: no wgpu adapter");
        return;
    };
    let mut map = Map::new(&device, &queue, &two_squares(), None, &[0, 1]);
    map.set_colours(&queue, Layer::Provinces, &[RED, BLUE]);
    // World [0, 1] fills the 100 x 100 viewport, so world x maps to pixel 100 x.
    let camera = Camera::fit(100.0, 100.0);
    let target = map
        .render(&device, &queue, [100, 100], &camera, Style::default(), None)
        .expect("first frame allocates");
    assert!(
        map.render(&device, &queue, [100, 100], &camera, Style::default(), None)
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
    map.set_colours(&queue, Layer::Provinces, &[BLUE, RED]);
    map.render(&device, &queue, [100, 100], &camera, Style::default(), None);
    assert_eq!(read_pixel(&device, &queue, &target, 25, 50), BLUE);
}

#[test]
fn the_world_wraps_across_the_antimeridian() {
    let Some((device, queue)) = device() else {
        eprintln!("skipping: no wgpu adapter");
        return;
    };
    let mut map = Map::new(&device, &queue, &two_squares(), None, &[0, 1]);
    map.set_colours(&queue, Layer::Provinces, &[RED, BLUE]);
    let mut camera = Camera::fit(100.0, 100.0);
    // Centre on x = 0: the left half of the viewport shows the previous copy's feature 1.
    camera.centre = [0.0, 0.5];
    let target = map
        .render(&device, &queue, [100, 100], &camera, Style::default(), None)
        .expect("allocates");
    assert_eq!(read_pixel(&device, &queue, &target, 25, 50), BLUE);
    assert_eq!(read_pixel(&device, &queue, &target, 75, 50), RED);
}

#[test]
fn the_globe_shows_the_near_side_only() {
    let Some((device, queue)) = device() else {
        eprintln!("skipping: no wgpu adapter");
        return;
    };
    let mut map = Map::new(&device, &queue, &two_squares(), None, &[0, 1]);
    map.set_colours(&queue, Layer::Provinces, &[RED, BLUE]);
    let mut camera = Camera::fit(100.0, 100.0);
    camera.globe = true;
    // Radius 100 / 2 pi px; centred on feature 0 (x 0.25), feature 1 (x 0.75) is behind.
    camera.scale = 100.0 * std::f64::consts::TAU / 2.0;
    camera.centre = [0.25, 0.5];
    let target = map
        .render(&device, &queue, [100, 100], &camera, Style::default(), None)
        .expect("allocates");
    assert_eq!(read_pixel(&device, &queue, &target, 50, 50), RED);
    assert_eq!(pick(&mut map, &device, &queue, &camera, [50, 50]), Some(0));
    camera.centre = [0.75, 0.5];
    map.render(&device, &queue, [100, 100], &camera, Style::default(), None);
    assert_eq!(read_pixel(&device, &queue, &target, 50, 50), BLUE);
    // Off the disc is paper, and picks nothing.
    let corner = read_pixel(&device, &queue, &target, 2, 2);
    assert_eq!(corner, [PAPER[0], PAPER[1], PAPER[2], 255]);
    assert_eq!(pick(&mut map, &device, &queue, &camera, [2, 2]), None);
}

#[test]
fn borders_follow_ids_and_counties_pick() {
    let Some((device, queue)) = device() else {
        eprintln!("skipping: no wgpu adapter");
        return;
    };
    let (provinces, counties) = neighbours();
    let camera = Camera::fit(100.0, 100.0);
    let style = Style {
        county_fill: false,
        county_borders: true,
        ..Style::default()
    };
    for (countries, edge_strength) in [([0, 0], 0.35), ([0, 1], 0.85)] {
        let mut map = Map::new(&device, &queue, &provinces, Some(&counties), &countries);
        map.set_colours(&queue, Layer::Provinces, &[RED, RED]);
        let target = map
            .render(&device, &queue, [100, 100], &camera, style, None)
            .expect("allocates");
        // The province edge at x = 50: one pixel inside feature 0 sees feature 1 to its right.
        let edge = read_pixel(&device, &queue, &target, 49, 50);
        assert!(
            is_ink(edge, edge_strength),
            "{edge:?} at strength {edge_strength}"
        );
        // The county edge at x = 30, within province 0.
        assert!(is_ink(read_pixel(&device, &queue, &target, 29, 50), 0.18));
        // Interior pixels keep the fill.
        assert_eq!(read_pixel(&device, &queue, &target, 40, 50), RED);
        assert_eq!(
            picked(&mut map, &device, &queue, &camera, style, [20, 50]),
            Picked {
                province: Some(0),
                county: Some(0),
            }
        );
        assert_eq!(
            picked(&mut map, &device, &queue, &camera, style, [40, 50]).county,
            Some(1)
        );
    }
}

#[test]
fn county_fill_replaces_province_colours() {
    let Some((device, queue)) = device() else {
        eprintln!("skipping: no wgpu adapter");
        return;
    };
    let (provinces, counties) = neighbours();
    let mut map = Map::new(&device, &queue, &provinces, Some(&counties), &[0, 0]);
    map.set_colours(&queue, Layer::Provinces, &[RED, RED]);
    map.set_colours(&queue, Layer::Counties, &[BLUE, RED, BLUE]);
    let camera = Camera::fit(100.0, 100.0);
    let style = Style {
        county_fill: true,
        county_borders: false,
        ..Style::default()
    };
    let target = map
        .render(&device, &queue, [100, 100], &camera, style, None)
        .expect("allocates");
    assert_eq!(read_pixel(&device, &queue, &target, 20, 50), BLUE);
    assert_eq!(read_pixel(&device, &queue, &target, 40, 50), RED);
}

#[test]
fn selection_and_origin_outline_inside_the_edge_and_hover_lightens() {
    let Some((device, queue)) = device() else {
        eprintln!("skipping: no wgpu adapter");
        return;
    };
    let mut map = Map::new(&device, &queue, &two_squares(), None, &[0, 1]);
    map.set_colours(&queue, Layer::Provinces, &[RED, BLUE]);
    let camera = Camera::fit(100.0, 100.0);
    let style = Style {
        selected: Some(0),
        origin: Some(1),
        hover: Some(0),
        ..Style::default()
    };
    let target = map
        .render(&device, &queue, [100, 100], &camera, style, None)
        .expect("allocates");
    // Feature 0 spans pixels 10 to 39: two pixels inside its left edge are ink, the third is not.
    let ink = [INK[0], INK[1], INK[2], 255];
    assert_eq!(read_pixel(&device, &queue, &target, 10, 50), ink);
    assert_eq!(read_pixel(&device, &queue, &target, 11, 50), ink);
    let hovered = read_pixel(&device, &queue, &target, 25, 50);
    let lightened = |base: u8, paper: u8| (0.3 * f32::from(paper) + 0.7 * f32::from(base)).round();
    for c in 0..3 {
        assert!(
            (f32::from(hovered[c]) - lightened(RED[c], PAPER[c])).abs() <= 2.0,
            "{hovered:?}"
        );
    }
    assert_eq!(
        read_pixel(&device, &queue, &target, 61, 50),
        [PAPER[0], PAPER[1], PAPER[2], 255]
    );
    assert_eq!(read_pixel(&device, &queue, &target, 75, 50), BLUE);
}
