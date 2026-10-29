//! Map benchmark (`CETERIS_BENCH=1`), the in-app measurement of ADR 0011 stage 1: frame
//! interval over a scripted camera path, and the CPU cost of a full recolour. Results go to
//! stdout, then the app quits.

use std::time::Instant;

use slint::wgpu_30::wgpu;

use crate::camera::Camera;
use crate::map::Map;

const FRAMES: usize = 600;
/// A recolour every `RECOLOUR`-th frame.
const RECOLOUR: usize = 4;

pub struct Bench {
    frame: usize,
    last: Option<Instant>,
    intervals: Vec<f64>,
    recolours: Vec<f64>,
}

impl Bench {
    pub fn new() -> Self {
        Self {
            frame: 0,
            last: None,
            intervals: Vec::with_capacity(FRAMES),
            recolours: Vec::new(),
        }
    }

    /// Advances the script one frame and returns the camera to draw with.
    pub fn frame(
        &mut self,
        map: &Map,
        queue: &wgpu::Queue,
        features: usize,
        viewport: [f64; 2],
        base: Camera,
    ) -> Camera {
        let now = Instant::now();
        if let Some(last) = self.last.replace(now) {
            self.intervals
                .push(now.duration_since(last).as_secs_f64() * 1e3);
        }
        let t = self.frame as f64 / FRAMES as f64;
        let mut camera = base;
        camera.pan(t * viewport[0] * 2.0, 0.0);
        camera.zoom_at(
            viewport,
            [viewport[0] / 2.0, viewport[1] / 2.0],
            (t * 6.0).exp2(),
        );
        if self.frame.is_multiple_of(RECOLOUR) {
            let start = Instant::now();
            let shift = self.frame as u32;
            let colours: Vec<[u8; 4]> = (0..features as u32)
                .map(|i| {
                    let h = i
                        .wrapping_mul(2_654_435_761)
                        .wrapping_add(shift.wrapping_mul(40_503));
                    [(h >> 24) as u8, (h >> 16) as u8, (h >> 8) as u8, 255]
                })
                .collect();
            map.set_colours(queue, &colours);
            self.recolours.push(start.elapsed().as_secs_f64() * 1e3);
        }
        self.frame += 1;
        camera
    }

    pub fn done(&self) -> bool {
        self.frame >= FRAMES
    }

    pub fn report(&self) {
        let stats = |name: &str, v: &[f64]| {
            let mut v = v.to_vec();
            v.sort_by(f64::total_cmp);
            let q = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
            println!(
                "CETERIS_BENCH {name}: n {} p50 {:.2} ms p95 {:.2} ms max {:.2} ms",
                v.len(),
                q(0.5),
                q(0.95),
                q(1.0)
            );
        };
        stats("frame interval", &self.intervals);
        stats("recolour upload", &self.recolours);
        let p50 = {
            let mut v = self.intervals.clone();
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        };
        println!("CETERIS_BENCH fps p50 {:.0}", 1e3 / p50);
    }
}

impl Default for Bench {
    fn default() -> Self {
        Self::new()
    }
}
