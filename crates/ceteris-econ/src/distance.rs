//! Population-weighted country-pair distance (ADR 0272; the CEPII `distw_harmonic` formula of
//! Head and Mayer 2002) from province centroids and populations.

use crate::inputs::EconInputs;
use ceteris_geo::pack::Pack;

const EARTH_RADIUS_KM: f64 = 6371.0088;

fn great_circle_km(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let s = libm::sin((p2 - p1) / 2.0);
    let t = libm::sin((lon2 - lon1).to_radians() / 2.0);
    let a = s * s + libm::cos(p1) * libm::cos(p2) * (t * t);
    2.0 * EARTH_RADIUS_KM * libm::asin(a.min(1.0).sqrt())
}

/// Row-major `n_countries` squared matrix of the harmonic population-weighted distance in km,
/// `(sum_ij (pop_i / pop_A) (pop_j / pop_B) / d_ij)^-1`: the diagonal and pairs without
/// population are NaN. A province pair at distance 0 (the same point) counts as 1 km.
pub fn country_distw(
    lon: &[f64],
    lat: &[f64],
    population: &[f64],
    country: &[u16],
    n_countries: usize,
) -> Vec<f64> {
    let n = lon.len();
    let mut people = vec![0.0; n_countries];
    for (&c, &p) in country.iter().zip(population) {
        people[usize::from(c)] += p;
    }
    let mut inv = vec![0.0; n_countries * n_countries];
    for i in 0..n {
        for j in 0..n {
            let (a, b) = (usize::from(country[i]), usize::from(country[j]));
            if a == b {
                continue;
            }
            let d = great_circle_km(lon[i], lat[i], lon[j], lat[j]).max(1.0);
            inv[a * n_countries + b] += population[i] * population[j] / d;
        }
    }
    (0..n_countries * n_countries)
        .map(|k| {
            let (a, b) = (k / n_countries, k % n_countries);
            let w = people[a] * people[b];
            if a != b && w > 0.0 && inv[k] > 0.0 {
                w / inv[k]
            } else {
                f64::NAN
            }
        })
        .collect()
}

impl EconInputs {
    /// [`country_distw`] of the world's provinces at the pack's centroids.
    pub fn country_distw(&self, pack: &Pack) -> Vec<f64> {
        let p = &self.provinces;
        let at = |v: &[f64]| -> Vec<f64> { p.pack.iter().map(|&i| v[i as usize]).collect() };
        country_distw(
            &at(&pack.provinces.lon),
            &at(&pack.provinces.lat),
            &p.population,
            &p.country,
            self.n_countries(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_province_each_is_the_great_circle_distance() {
        let d = country_distw(&[0.0, 90.0], &[0.0, 0.0], &[3.0, 5.0], &[0, 1], 2);
        let quarter = EARTH_RADIUS_KM * std::f64::consts::FRAC_PI_2;
        assert!((d[1] - quarter).abs() < 1e-9 && d[1] == d[2]);
        assert!(d[0].is_nan() && d[3].is_nan());
    }

    #[test]
    fn harmonic_mean_weights_by_population() {
        // country 0 has provinces at 0 and 90 degrees (weights 1:3), country 1 sits at 0 degrees
        let d = country_distw(
            &[0.0, 90.0, 0.0],
            &[0.0, 0.0, 0.0],
            &[1.0, 3.0, 1.0],
            &[0, 0, 1],
            2,
        );
        let q = EARTH_RADIUS_KM * std::f64::consts::FRAC_PI_2;
        let want = 1.0 / (0.25 / 1.0 + 0.75 / q);
        assert!((d[1] / want - 1.0).abs() < 1e-12);
    }
}
