//! Spatial chunks of a fill for culling: triangles regrouped by a 2-degree Mercator grid cell,
//! each chunk bounded by a cap on the unit sphere. Built when the app loads, so the pack's
//! `.fill` format is unchanged.

use std::ops::Range;

use crate::camera::{Camera, lon_lat};
use crate::fill::Fill;

const COLS: usize = 180;
const ROWS: usize = 90;

#[derive(Clone, Copy, Debug)]
struct Chunk {
    /// Index range in `Chunks::index`.
    start: u32,
    end: u32,
    /// Unit vector to the cap's centre and the cap's angular radius.
    centre: [f64; 3],
    radius: f64,
}

pub struct Chunks {
    /// The fill's triangles reordered so each chunk's are contiguous.
    pub index: Vec<u32>,
    chunks: Vec<Chunk>,
}

fn unit(m: [f32; 2]) -> [f64; 3] {
    let [lon, lat] = lon_lat(m.map(f64::from));
    [lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos()]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl Chunks {
    pub fn new(fill: &Fill) -> Self {
        let tris = fill.index.as_chunks::<3>().0;
        let cell = |t: &[u32; 3]| {
            let [x, y] =
                [0, 1].map(|k| t.iter().map(|&i| fill.xy[i as usize][k]).sum::<f32>() / 3.0);
            let col = ((x * COLS as f32) as usize).min(COLS - 1);
            let row = ((y * ROWS as f32) as usize).min(ROWS - 1);
            row * COLS + col
        };
        // Counting sort by cell keeps each cell's triangles in their original order.
        let cells: Vec<usize> = tris.iter().map(cell).collect();
        let mut start = vec![0usize; COLS * ROWS + 1];
        for &c in &cells {
            start[c + 1] += 1;
        }
        for c in 0..COLS * ROWS {
            start[c + 1] += start[c];
        }
        let mut next = start.clone();
        let mut index = vec![0u32; fill.index.len()];
        for (t, &c) in tris.iter().zip(&cells) {
            index[3 * next[c]..3 * next[c] + 3].copy_from_slice(t);
            next[c] += 1;
        }
        let chunks = (0..COLS * ROWS)
            .filter(|&c| start[c + 1] > start[c])
            .map(|c| {
                let ids = &index[3 * start[c]..3 * start[c + 1]];
                let sum = ids.iter().fold([0.0; 3], |s, &i| {
                    let p = unit(fill.xy[i as usize]);
                    [s[0] + p[0], s[1] + p[1], s[2] + p[2]]
                });
                let norm = dot(sum, sum).sqrt();
                let centre = if norm > 0.0 {
                    sum.map(|v| v / norm)
                } else {
                    [0.0, 0.0, 1.0]
                };
                let radius = ids
                    .iter()
                    .map(|&i| {
                        dot(centre, unit(fill.xy[i as usize]))
                            .clamp(-1.0, 1.0)
                            .acos()
                    })
                    .fold(0.0, f64::max);
                Chunk {
                    start: (3 * start[c]) as u32,
                    end: (3 * start[c + 1]) as u32,
                    centre,
                    radius,
                }
            })
            .collect();
        Self { index, chunks }
    }

    /// Index ranges of the chunks that face the viewer and overlap the viewport, adjacent ones
    /// merged.
    pub fn visible(&self, camera: &Camera, viewport: [f64; 2]) -> Vec<Range<u32>> {
        let [lon, lat] = camera.centre;
        let (sl, cl) = lon.sin_cos();
        let (sp, cp) = lat.sin_cos();
        // The camera's right, up and towards-viewer axes in world coordinates.
        let right = [cl, 0.0, -sl];
        let up = [-sp * sl, cp, -sp * cl];
        let view = [cp * sl, sp, cp * cl];
        let half = viewport.map(|v| v / 2.0 / camera.scale);
        let mut out: Vec<Range<u32>> = Vec::new();
        for c in &self.chunks {
            // Orthographic projection shortens distances, so the chunk lies within a disc of
            // the cap's chord around its centre's projection.
            let reach = 2.0 * (c.radius / 2.0).sin();
            let facing =
                c.radius >= std::f64::consts::FRAC_PI_2 || dot(c.centre, view) > -c.radius.sin();
            let on_screen = dot(c.centre, right).abs() <= half[0] + reach
                && dot(c.centre, up).abs() <= half[1] + reach;
            if !(facing && on_screen) {
                continue;
            }
            match out.last_mut() {
                Some(r) if r.end == c.start => r.end = c.end,
                _ => out.push(c.start..c.end),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grid of small squares over the whole Mercator plane, one feature each.
    fn world(n: usize) -> Fill {
        let mut xy = Vec::new();
        let mut index = Vec::new();
        let mut feature = Vec::new();
        for r in 0..n {
            for c in 0..n {
                let (x, y, s) = (c as f32 / n as f32, r as f32 / n as f32, 1.0 / n as f32);
                let base = xy.len() as u32;
                xy.extend([[x, y], [x + s, y], [x + s, y + s], [x, y + s]]);
                feature.extend([(r * n + c) as u32; 4]);
                index.extend([0, 1, 2, 0, 2, 3].map(|k| base + k));
            }
        }
        Fill {
            xy,
            feature,
            index,
            features: (n * n) as u32,
        }
    }

    fn triangles(chunks: &Chunks, ranges: &[Range<u32>]) -> Vec<[u32; 3]> {
        let mut t: Vec<[u32; 3]> = ranges
            .iter()
            .flat_map(|r| {
                chunks.index[r.start as usize..r.end as usize]
                    .as_chunks::<3>()
                    .0
                    .to_vec()
            })
            .collect();
        t.sort_unstable();
        t
    }

    #[test]
    fn reordering_keeps_every_triangle() {
        let fill = world(40);
        let chunks = Chunks::new(&fill);
        let mut before: Vec<[u32; 3]> = fill.index.as_chunks::<3>().0.to_vec();
        before.sort_unstable();
        let all: Vec<Range<u32>> = chunks.chunks.iter().map(|c| c.start..c.end).collect();
        assert_eq!(triangles(&chunks, &all), before);
    }

    #[test]
    fn every_triangle_that_reaches_the_screen_is_drawn() {
        let fill = world(60);
        let chunks = Chunks::new(&fill);
        let viewport = [800.0, 600.0];
        for (lon, lat, scale) in [
            (0.0, 0.0, 250.0),
            (2.0, 0.9, 900.0),
            (-3.0, -1.2, 5_000.0),
            (3.1, 0.3, 40_000.0),
        ] {
            let camera = Camera {
                centre: [lon, lat],
                scale,
            };
            let drawn = triangles(&chunks, &chunks.visible(&camera, viewport));
            for t in fill.index.as_chunks::<3>().0 {
                let seen = t.iter().any(|&i| {
                    camera
                        .project(viewport, fill.xy[i as usize].map(f64::from))
                        .is_some_and(|p| {
                            (0.0..viewport[0]).contains(&p[0]) && (0.0..viewport[1]).contains(&p[1])
                        })
                });
                if seen {
                    let mut s = *t;
                    s.sort_unstable();
                    assert!(
                        drawn.iter().any(|d| {
                            let mut d = *d;
                            d.sort_unstable();
                            d == s
                        }),
                        "{t:?} at {lon} {lat} {scale}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_far_side_and_off_screen_chunks_are_skipped() {
        let fill = world(60);
        let chunks = Chunks::new(&fill);
        let total = fill.index.len() as u32;
        let count = |camera: &Camera| -> u32 {
            chunks
                .visible(camera, [800.0, 600.0])
                .iter()
                .map(|r| r.end - r.start)
                .sum()
        };
        let world_view = count(&Camera {
            centre: [0.0, 0.0],
            scale: 250.0,
        });
        assert!(world_view < total * 3 / 5, "{world_view} of {total}");
        let close = count(&Camera {
            centre: [0.0, 0.0],
            scale: 20_000.0,
        });
        assert!(close < total / 50, "{close} of {total}");
    }
}
