//! The graph-independent trade network: leg costs between provinces and gateways, as
//! produced from `ceteris-geo` path costs, and the tariff wedges on gateway crossings.

use crate::goods::N_GOODS;
use crate::params::N_CLASSES;

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
/// `gateway_mass` is each gateway's share m_a of its country's trade throughput, summing to 1
/// over the country's gateways (ADR 0092): a crossing a to b weighs `m_a m_b`, so splitting a
/// gateway into copies that share its mass changes no access.
///
/// `route_taus`, when present, replaces the least-cost leg factors (S05) with route-choice
/// factors (ADR 0017).
///
/// `bulk`, when present, gives the theta classes it names their own least-cost legs at the bulk
/// freight rate (ADR 0247).
///
/// `good_legs`, when present, gives each traded good its least-iceberg legs (ADR 0279), which
/// the kernel prices it on in place of the base or bulk lists.
#[derive(Clone, Debug, Default)]
pub struct Network {
    pub province_country: Vec<u16>,
    pub gateway_country: Vec<u16>,
    pub gateway_mass: Vec<f64>,
    pub domestic: Vec<Leg>,
    pub access: Vec<Leg>,
    pub crossings: Vec<Leg>,
    pub route_taus: Option<LegTaus>,
    pub bulk: Option<BulkLegs>,
    pub good_legs: Option<GoodLegs>,
}

/// Least-iceberg legs per traded good (ADR 0279): `sets[set[g]]` holds good g's legs, on the
/// paths that minimise `w_g c + lambda_g h / 24`, with the base lists' endpoints in their order;
/// `set[g]` is `u8::MAX` for a local good. Goods with the same `kappa` (lambda_g / (24 w_g)) and
/// the same bulk freight rate share a set. `kappa` records each traded good's value the sets were
/// built with (0 for local goods); the kernel refuses weights and time costs that give another.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GoodLegs {
    pub set: [u8; N_GOODS],
    pub kappa: [f64; N_GOODS],
    pub sets: Vec<LegSet>,
}

/// One set of domestic, access and crossing legs, aligned with the lists of [`Network`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LegSet {
    pub domestic: Vec<Leg>,
    pub access: Vec<Leg>,
    pub crossings: Vec<Leg>,
}

/// kappa_g = lambda_g / (24 w_g), the value of an hour per tonne that ranks a good's paths
/// (ADR 0279).
pub fn kappa(weight: f64, time_cost: f64) -> f64 {
    time_cost / (24.0 * weight)
}

/// Least-cost legs at the bulk freight rate (P154, P155, ADR 0247), for the goods of the theta
/// classes in `classes`: the same endpoints in the same order as the base lists of
/// [`Network`], with the cost per tonne and hours of the least-cost path on the graph repriced
/// at that rate. `sea` and `port` are the P154 and P155 multipliers the legs were built with;
/// the kernel refuses parameters that give these classes another rate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BulkLegs {
    pub classes: [bool; N_CLASSES],
    pub sea: f64,
    pub port: f64,
    pub domestic: Vec<Leg>,
    pub access: Vec<Leg>,
    pub crossings: Vec<Leg>,
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

/// Gateway masses 1 / k_A: each of the k_A gateways of country A an equal share.
pub fn equal_gateway_mass(gateway_country: &[u16]) -> Vec<f64> {
    let n = gateway_country
        .iter()
        .max()
        .map_or(0, |&c| usize::from(c) + 1);
    let mut count = vec![0u32; n];
    for &c in gateway_country {
        count[usize::from(c)] += 1;
    }
    gateway_country
        .iter()
        .map(|&c| 1.0 / f64::from(count[usize::from(c)]))
        .collect()
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
