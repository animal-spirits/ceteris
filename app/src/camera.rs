//! Globe camera (ADR 0250): an orthographic sphere seen from above the centre, north up. World
//! points are Web Mercator (0 to 1, y down), the pack's encoding of lon/lat.

use std::f64::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Longitude and latitude at the viewport centre, radians.
    pub centre: [f64; 2],
    /// Sphere radius in physical pixels.
    pub scale: f64,
}

/// Deepest zoom: about 2 m per pixel at the equator.
const MAX_SCALE: f64 = 3.2e6;
/// The pack's Mercator clamp, 85.05 degrees.
pub const MAX_LAT: f64 = 1.484_422_229_745_332_4;

pub fn lon_lat(m: [f64; 2]) -> [f64; 2] {
    [(m[0] - 0.5) * TAU, (PI * (1.0 - 2.0 * m[1])).sinh().atan()]
}

fn wrap(lon: f64) -> f64 {
    (lon + PI).rem_euclid(TAU) - PI
}

impl Camera {
    /// The globe filling 90% of the shorter side, centred on 0, 0.
    pub fn fit(width: f64, height: f64) -> Self {
        Self {
            centre: [0.0, 0.0],
            scale: 0.45 * width.min(height).max(1.0),
        }
    }

    /// The lon/lat under a screen point; `None` off the disc.
    pub fn screen_to_lon_lat(&self, viewport: [f64; 2], px: [f64; 2]) -> Option<[f64; 2]> {
        let x = (px[0] - viewport[0] / 2.0) / self.scale;
        let yr = (viewport[1] / 2.0 - px[1]) / self.scale;
        let zr = (1.0 - x * x - yr * yr).sqrt();
        if zr.is_nan() {
            return None;
        }
        let (s, c) = self.centre[1].sin_cos();
        let y = yr * c + zr * s;
        let z = zr * c - yr * s;
        Some([wrap(self.centre[0] + x.atan2(z)), y.asin()])
    }

    /// Where a world point lands on screen, in physical pixels; `None` on the far side.
    pub fn project(&self, viewport: [f64; 2], xy: [f64; 2]) -> Option<[f64; 2]> {
        let [lon, lat] = lon_lat(xy);
        let dl = lon - self.centre[0];
        let (x, y, z) = (lat.cos() * dl.sin(), lat.sin(), lat.cos() * dl.cos());
        let (s, c) = self.centre[1].sin_cos();
        let yr = y * c - z * s;
        let zr = y * s + z * c;
        (zr > 0.0).then(|| {
            [
                viewport[0] / 2.0 + x * self.scale,
                viewport[1] / 2.0 - yr * self.scale,
            ]
        })
    }

    /// Rotates the sphere so the point under the pointer near the centre follows it.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        let [lon, lat] = self.centre;
        self.centre = [
            wrap(lon - dx / (self.scale * lat.cos())),
            (lat + dy / self.scale).clamp(-MAX_LAT, MAX_LAT),
        ];
    }

    /// Zooms by `factor` keeping the lon/lat under `px` fixed, or the centre when `px` is off
    /// the disc.
    pub fn zoom_at(&mut self, viewport: [f64; 2], px: [f64; 2], factor: f64) {
        let min = 0.25 * viewport[0].min(viewport[1]).max(1.0);
        let before = self.screen_to_lon_lat(viewport, px);
        self.scale = (self.scale * factor).clamp(min, MAX_SCALE);
        let Some(before) = before else { return };
        // Fixed-point on the centre; converges in a few steps for any point on the disc.
        for _ in 0..8 {
            let Some(after) = self.screen_to_lon_lat(viewport, px) else {
                return;
            };
            self.centre = [
                wrap(self.centre[0] + wrap(before[0] - after[0])),
                (self.centre[1] + before[1] - after[1]).clamp(-MAX_LAT, MAX_LAT),
            ];
        }
    }

    /// Shader uniform: centre lon/lat, then k scaling the unit sphere (y up) to clip space.
    pub fn uniform(&self, viewport: [f64; 2]) -> [f32; 4] {
        [
            self.centre[0] as f32,
            self.centre[1] as f32,
            (2.0 * self.scale / viewport[0]) as f32,
            (2.0 * self.scale / viewport[1]) as f32,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: [f64; 2] = [800.0, 600.0];

    fn close(a: [f64; 2], b: [f64; 2], tol: f64) -> bool {
        (a[0] - b[0]).abs() < tol && (a[1] - b[1]).abs() < tol
    }

    #[test]
    fn max_lat_is_the_mercator_clamp() {
        assert!((lon_lat([0.5, 0.0])[1] - MAX_LAT).abs() < 1e-15);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let mut c = Camera::fit(VIEW[0], VIEW[1]);
        c.centre = [2.0, 0.6];
        let px = [510.0, 140.0];
        let before = c.screen_to_lon_lat(VIEW, px).unwrap();
        c.zoom_at(VIEW, px, 3.5);
        let after = c.screen_to_lon_lat(VIEW, px).unwrap();
        assert!(close(before, after, 1e-9), "{before:?} {after:?}");
    }

    #[test]
    fn pan_moves_the_point_under_the_pointer_and_wraps() {
        let mut c = Camera::fit(VIEW[0], VIEW[1]);
        c.scale = 1e4;
        c.centre = [1.0, 0.5];
        let w = c.screen_to_lon_lat(VIEW, [400.0, 300.0]).unwrap();
        c.pan(5.0, 3.0);
        let moved = c.screen_to_lon_lat(VIEW, [405.0, 303.0]).unwrap();
        assert!(close(w, moved, 1e-7), "{w:?} {moved:?}");
        c.pan(1e9, 0.0);
        assert!((-PI..PI).contains(&c.centre[0]));
        c.pan(0.0, 1e9);
        assert_eq!(c.centre[1], MAX_LAT);
    }

    #[test]
    fn project_inverts_screen_to_lon_lat() {
        let mut c = Camera::fit(VIEW[0], VIEW[1]);
        c.centre = [-2.5, -0.4];
        c.zoom_at(VIEW, [100.0, 500.0], 2.0);
        let px = [233.0, 411.0];
        let [lon, lat] = c.screen_to_lon_lat(VIEW, px).unwrap();
        let m = [
            lon / TAU + 0.5,
            0.5 - (lat.tan() + 1.0 / lat.cos()).ln() / TAU,
        ];
        let back = c.project(VIEW, m).unwrap();
        assert!(close(back, px, 1e-6), "{back:?}");
    }

    #[test]
    fn the_centre_is_mid_screen_north_is_up_and_the_far_side_hidden() {
        let mut c = Camera::fit(VIEW[0], VIEW[1]);
        c.centre = lon_lat([0.3, 0.4]);
        let mid = c.project(VIEW, [0.3, 0.4]).unwrap();
        assert!(close(mid, [400.0, 300.0], 1e-9));
        assert!(c.project(VIEW, [0.8, 0.6]).is_none());
        assert!(c.project(VIEW, [0.3, 0.3]).unwrap()[1] < 300.0);
        assert!(c.screen_to_lon_lat(VIEW, [5.0, 5.0]).is_none());
    }

    #[test]
    fn zoom_out_stops_at_a_quarter_of_the_short_side() {
        let mut c = Camera::fit(VIEW[0], VIEW[1]);
        c.zoom_at(VIEW, [400.0, 300.0], 1e-6);
        assert_eq!(c.scale, 150.0);
    }
}
