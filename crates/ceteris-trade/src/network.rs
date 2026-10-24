//! The graph-independent trade network: leg costs between provinces and gateways, as
//! produced from `ceteris-geo` path costs, and the tariff wedges on gateway crossings.

use crate::goods::N_GOODS;

/// A least-cost path summary between two nodes. `cost_per_t` is USD per tonne at the
/// reference price level, `hours` the travel time along the same path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leg {
    pub from: u32,
    pub to: u32,
    pub cost_per_t: f64,
    pub hours: f64,
}

/// Leg lists of the two-level kernel.
///
/// - `domestic`: province o to province d, o != d, same country, both directions.
/// - `access`: province to a gateway of its country. The reverse leg (gateway to province)
///   is taken to cost the same.
/// - `crossings`: gateway to gateway of different countries, both directions.
///
/// `from` and `to` index provinces for `domestic`, province then gateway for `access`,
/// and gateways for `crossings`.
///
/// `route_taus`, when present, replaces the least-cost leg factors (S05) with route-choice
/// factors (ADR 0017).
#[derive(Clone, Debug, Default)]
pub struct Network {
    pub province_country: Vec<u16>,
    pub gateway_country: Vec<u16>,
    pub domestic: Vec<Leg>,
    pub access: Vec<Leg>,
    pub crossings: Vec<Leg>,
    pub route_taus: Option<LegTaus>,
}

/// Route-choice leg factors per good (ADR 0017, `ceteris_geo::route_choice`) and the
/// route-probability-weighted hours, aligned with the leg lists of [`Network`]: entry i of
/// `domestic` belongs to `Network::domestic[i]`, and so on. Factors are at least 1, infinite
/// for a leg with no route; local goods' entries are unused.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LegTaus {
    pub domestic: Vec<[f64; N_GOODS]>,
    pub access: Vec<[f64; N_GOODS]>,
    pub crossings: Vec<[f64; N_GOODS]>,
    pub domestic_hours: Vec<[f64; N_GOODS]>,
    pub access_hours: Vec<[f64; N_GOODS]>,
    pub crossing_hours: Vec<[f64; N_GOODS]>,
}

impl Network {
    pub fn n_provinces(&self) -> usize {
        self.province_country.len()
    }

    pub fn n_gateways(&self) -> usize {
        self.gateway_country.len()
    }
}

/// Applied tariffs as ad valorem rates, indexed
/// `[(importer * n_countries + exporter) * 2 + group]` with the group from
/// [`crate::goods::tariff_group`]. The country count is implied by the length:
/// `len == 2 * n_countries^2`.
#[derive(Clone, Debug, Default)]
pub struct Wedges {
    pub tariff: Vec<f32>,
}

impl Wedges {
    /// All-zero tariffs for `n_countries` countries.
    pub fn none(n_countries: usize) -> Self {
        Self {
            tariff: vec![0.0; 2 * n_countries * n_countries],
        }
    }

    pub fn n_countries(&self) -> usize {
        let n = (self.tariff.len() / 2).isqrt();
        assert_eq!(
            2 * n * n,
            self.tariff.len(),
            "tariff length must be 2 * n_countries^2"
        );
        n
    }

    pub fn rate(&self, importer: usize, exporter: usize, group: usize) -> f32 {
        let n = self.n_countries();
        self.tariff[(importer * n + exporter) * 2 + group]
    }
}
