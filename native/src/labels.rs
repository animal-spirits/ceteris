//! Map labels (ADR 0244): anchors computed once from the fills, placed on screen each time the
//! camera moves, drawn by Slint over the map.

use crate::camera::Camera;
use crate::fill::Fill;
use crate::web::Provinces;

pub struct Anchor {
    pub text: String,
    /// Web Mercator.
    pub xy: [f64; 2],
    /// Mercator area, for ranking and for when the label fits.
    pub area: f64,
}

pub struct Anchors {
    pub countries: Vec<Anchor>,
    pub provinces: Vec<Anchor>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub text: String,
    /// Physical pixels, the label's centre.
    pub at: [f64; 2],
    pub country: bool,
}

/// Per-feature area and area-weighted centroid of a fill's triangles.
fn centroids(fill: &Fill) -> Vec<(f64, [f64; 2])> {
    let mut acc = vec![(0.0, [0.0, 0.0]); fill.features as usize];
    for t in fill.index.as_chunks::<3>().0 {
        let [a, b, c] = t.map(|i| fill.xy[i as usize].map(f64::from));
        let area = ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0;
        let f = &mut acc[fill.feature[t[0] as usize] as usize];
        f.0 += area;
        for k in 0..2 {
            f.1[k] += area * (a[k] + b[k] + c[k]) / 3.0;
        }
    }
    acc.into_iter()
        .map(|(area, m)| (area, if area > 0.0 { m.map(|v| v / area) } else { m }))
        .collect()
}

fn find(parent: &mut [u32], mut i: u32) -> u32 {
    while parent[i as usize] != i {
        parent[i as usize] = parent[parent[i as usize] as usize];
        i = parent[i as usize];
    }
    i
}

impl Anchors {
    /// Provinces at their centroid. Countries at the province nearest the centroid of their
    /// largest landmass, so a country with distant islands or exclaves keeps its label on its
    /// main body. Provinces of one country that share a boundary vertex form a landmass.
    pub fn new(fill: &Fill, p: &Provinces) -> Self {
        let cents = centroids(fill);
        let n = p.country.len();
        let mut parent: Vec<u32> = (0..n as u32).collect();
        // Vertices sorted by position: equal positions are adjacent.
        let mut at: Vec<([u32; 2], u32)> = fill
            .xy
            .iter()
            .zip(&fill.feature)
            .map(|(xy, &f)| (xy.map(f32::to_bits), f))
            .collect();
        at.sort_unstable();
        for w in at.windows(2) {
            let ((ka, fa), (kb, fb)) = (w[0], w[1]);
            if ka == kb && p.country[fa as usize] == p.country[fb as usize] {
                let (a, b) = (find(&mut parent, fa), find(&mut parent, fb));
                parent[a as usize] = b;
            }
        }
        let mut mass = vec![0.0; n];
        for i in 0..n as u32 {
            let root = find(&mut parent, i);
            mass[root as usize] += cents[i as usize].0;
        }
        let countries = (0..p.countries.name.len() as u32)
            .filter_map(|c| {
                let members: Vec<u32> = (0..n as u32)
                    .filter(|&i| p.country[i as usize] == c)
                    .collect();
                let main = members
                    .iter()
                    .map(|&i| find(&mut parent, i))
                    .max_by(|&a, &b| {
                        mass[a as usize]
                            .total_cmp(&mass[b as usize])
                            .then(b.cmp(&a))
                    })?;
                let body: Vec<u32> = members
                    .into_iter()
                    .filter(|&i| find(&mut parent, i) == main)
                    .collect();
                let area: f64 = body.iter().map(|&i| cents[i as usize].0).sum();
                let centre = [0, 1].map(|k| {
                    body.iter()
                        .map(|&i| cents[i as usize].0 * cents[i as usize].1[k])
                        .sum::<f64>()
                        / area
                });
                let near = body.iter().copied().min_by(|&a, &b| {
                    let d = |i: u32| {
                        let c = cents[i as usize].1;
                        (c[0] - centre[0]).powi(2) + (c[1] - centre[1]).powi(2)
                    };
                    d(a).total_cmp(&d(b))
                })?;
                let total: f64 = (0..n)
                    .filter(|&i| p.country[i] == c)
                    .map(|i| cents[i].0)
                    .sum();
                Some(Anchor {
                    text: p.countries.name[c as usize].to_uppercase(),
                    xy: cents[near as usize].1,
                    area: total,
                })
            })
            .collect();
        let provinces = cents
            .iter()
            .zip(&p.name)
            .map(|(&(area, xy), name)| Anchor {
                text: name.clone(),
                xy,
                area,
            })
            .collect();
        Self {
            countries,
            provinces,
        }
    }

    /// Labels that fit, largest first, dropping any that would overlap one already placed.
    /// `em` is the text size in physical pixels.
    pub fn place(&self, camera: &Camera, viewport: [f64; 2], em: f64) -> Vec<Placed> {
        let mut candidates: Vec<(&Anchor, bool)> = self
            .countries
            .iter()
            .map(|a| (a, true))
            .chain(self.provinces.iter().map(|a| (a, false)))
            .collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.area.total_cmp(&a.0.area)));
        let mut boxes: Vec<[f64; 4]> = Vec::new();
        let mut placed = Vec::new();
        for (a, country) in candidates {
            // Width of the feature on screen, roughly; a label wider than its feature is left out.
            let extent = a.area.sqrt() * camera.scale;
            let width = em * 0.6 * a.text.chars().count() as f64 * if country { 1.25 } else { 1.0 };
            if extent < width * if country { 0.5 } else { 1.5 } {
                continue;
            }
            let Some(at) = camera.project(viewport, a.xy) else {
                continue;
            };
            let b = [
                at[0] - width / 2.0,
                at[1] - em,
                at[0] + width / 2.0,
                at[1] + em,
            ];
            if b[2] < 0.0 || b[0] > viewport[0] || b[3] < 0.0 || b[1] > viewport[1] {
                continue;
            }
            if boxes
                .iter()
                .any(|o| b[0] < o[2] && o[0] < b[2] && b[1] < o[3] && o[1] < b[3])
            {
                continue;
            }
            boxes.push(b);
            placed.push(Placed {
                text: a.text.clone(),
                at,
                country,
            });
        }
        placed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::Countries;

    fn square(x0: f32, y0: f32, side: f32) -> [[f32; 2]; 4] {
        [
            [x0, y0],
            [x0 + side, y0],
            [x0 + side, y0 + side],
            [x0, y0 + side],
        ]
    }

    /// Country 0: provinces 0 and 1 share an edge (area 2e-4 together); province 2 is a larger
    /// island (1.5e-4) far away, so it is not the main body. Country 1 is province 3.
    fn world() -> (Fill, Provinces) {
        let squares = [
            square(0.40, 0.40, 0.01),
            square(0.41, 0.40, 0.01),
            square(0.80, 0.80, 0.0122),
            square(0.10, 0.10, 0.05),
        ];
        let fill = Fill {
            xy: squares.concat(),
            feature: (0..4).flat_map(|f| [f; 4]).collect(),
            index: (0..4)
                .flat_map(|q| [0, 1, 2, 0, 2, 3].map(|k| 4 * q + k))
                .collect(),
            features: 4,
        };
        let provinces = Provinces {
            id: (0..4).map(|i| format!("P{i}")).collect(),
            name: ["Westmark", "Eastmark", "Isle", "Other"]
                .map(String::from)
                .to_vec(),
            country: vec![0, 0, 0, 1],
            countries: Countries {
                code: vec!["AAA".into(), "BBB".into()],
                name: vec!["Alpha".into(), "Beta".into()],
                colour: vec![0, 1],
                ..Default::default()
            },
            ..Default::default()
        };
        (fill, provinces)
    }

    #[test]
    fn a_country_label_sits_on_its_largest_landmass() {
        let (fill, p) = world();
        let a = Anchors::new(&fill, &p);
        assert_eq!(a.countries[0].text, "ALPHA");
        let [x, y] = a.countries[0].xy;
        assert!(
            (0.40..0.42).contains(&x) && (0.40..0.41).contains(&y),
            "{x} {y}"
        );
        assert!((a.countries[0].area - 3.4884e-4).abs() < 1e-8);
        let isle = a.provinces[2].xy;
        assert!((isle[0] - 0.8061).abs() < 1e-6 && (isle[1] - 0.8061).abs() < 1e-6);
    }

    #[test]
    fn labels_that_do_not_fit_or_overlap_are_dropped() {
        let (fill, p) = world();
        let a = Anchors::new(&fill, &p);
        let view = [800.0, 800.0];
        let camera = Camera::fit(800.0, 800.0);
        let world_view: Vec<String> = a
            .place(&camera, view, 12.0)
            .into_iter()
            .map(|l| l.text)
            .collect();
        // At world scale only Beta (40 px wide feature) has room.
        assert_eq!(world_view, ["BETA"]);
        let mut close = camera;
        close.scale = 20_000.0;
        close.centre = [0.41, 0.405];
        let near: Vec<Placed> = a.place(&close, view, 12.0);
        let texts: Vec<&str> = near.iter().map(|l| l.text.as_str()).collect();
        // Alpha first; Westmark overlaps it at the shared anchor and is dropped.
        assert_eq!(texts, ["ALPHA", "Eastmark"]);
        assert!(near.iter().all(|l| (0.0..800.0).contains(&l.at[0])));
    }
}
