//! Flat Web Mercator camera. World units are Mercator (0 to 1, y down), wrapped in x.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// World point at the viewport centre.
    pub centre: [f64; 2],
    /// Physical pixels per world unit.
    pub scale: f64,
}

/// Deepest zoom: about 2 m per pixel at the equator.
const MAX_SCALE: f64 = 2.0e7;

impl Camera {
    /// The whole world fitted to a viewport.
    pub fn fit(width: f64, height: f64) -> Self {
        Self {
            centre: [0.5, 0.5],
            scale: width.min(height).max(1.0),
        }
    }

    pub fn screen_to_world(&self, viewport: [f64; 2], px: [f64; 2]) -> [f64; 2] {
        [
            self.centre[0] + (px[0] - viewport[0] / 2.0) / self.scale,
            self.centre[1] + (px[1] - viewport[1] / 2.0) / self.scale,
        ]
    }

    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.centre[0] = (self.centre[0] - dx / self.scale).rem_euclid(1.0);
        self.centre[1] = (self.centre[1] - dy / self.scale).clamp(0.0, 1.0);
    }

    /// Zooms by `factor` keeping the world point under `px` fixed.
    pub fn zoom_at(&mut self, viewport: [f64; 2], px: [f64; 2], factor: f64) {
        let min = viewport[0].min(viewport[1]).max(1.0) / 2.0;
        let before = self.screen_to_world(viewport, px);
        self.scale = (self.scale * factor).clamp(min, MAX_SCALE);
        let after = self.screen_to_world(viewport, px);
        self.centre[0] = (self.centre[0] + before[0] - after[0]).rem_euclid(1.0);
        self.centre[1] = (self.centre[1] + before[1] - after[1]).clamp(0.0, 1.0);
    }

    /// Shader uniform: world to clip is `clip = (world - centre) * k`.
    pub fn uniform(&self, viewport: [f64; 2]) -> [f32; 4] {
        [
            self.centre[0] as f32,
            self.centre[1] as f32,
            (2.0 * self.scale / viewport[0]) as f32,
            (-2.0 * self.scale / viewport[1]) as f32,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: [f64; 2] = [800.0, 600.0];

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let mut c = Camera::fit(VIEW[0], VIEW[1]);
        let px = [610.0, 140.0];
        let before = c.screen_to_world(VIEW, px);
        c.zoom_at(VIEW, px, 3.5);
        let after = c.screen_to_world(VIEW, px);
        assert!((before[0] - after[0]).abs() < 1e-12 && (before[1] - after[1]).abs() < 1e-12);
    }

    #[test]
    fn pan_moves_the_world_with_the_pointer_and_wraps() {
        let mut c = Camera::fit(VIEW[0], VIEW[1]);
        let w = c.screen_to_world(VIEW, [100.0, 100.0]);
        c.pan(50.0, 20.0);
        let moved = c.screen_to_world(VIEW, [150.0, 120.0]);
        assert!((w[1] - moved[1]).abs() < 1e-12);
        assert!(((w[0] - moved[0]).rem_euclid(1.0)).min((moved[0] - w[0]).rem_euclid(1.0)) < 1e-12);
        c.pan(1e6, 0.0);
        assert!((0.0..1.0).contains(&c.centre[0]));
    }

    #[test]
    fn zoom_out_stops_at_half_the_fitted_world() {
        let mut c = Camera::fit(VIEW[0], VIEW[1]);
        c.zoom_at(VIEW, [400.0, 300.0], 1e-6);
        assert_eq!(c.scale, 300.0);
    }
}
