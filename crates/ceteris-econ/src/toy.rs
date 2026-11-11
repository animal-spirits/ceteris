//! Toy-world builder: N countries of P provinces on a line, G goods with a chosen traded/local
//! split, per-country technology, demand weights, freight cost and tariff, giving the same
//! [`EconInputs`] as [`crate::fixture`], in its data state (no steady state attached). The toy
//! zoo's simulator tier runs on it (`reports/toy-models-and-literature-baseline-2026-11-11.md`).
//!
//! Layout: country `c` occupies `[c * W, (c + 1) * W)` on the x axis, its provinces evenly
//! spaced along it; each country has a west and an east gateway, and the east gateway of `c`
//! joins the west gateway of `c + 1` by one land crossing. Legs cost `cost_per_tkm` USD per
//! tonne-km and move at 40 km/h. With identical countries, trade is balanced by symmetry.

use ceteris_core::rows::Rows;
use ceteris_trade::{BulkLegs, Goods, Leg, Network, TradeParams, Wedges, equal_gateway_mass};

use crate::inputs::{Countries, EconInputs, EntData, N_EDU, PopData, ProvinceData, TRANSFER_YEARS};
use crate::params::EconParams;

const KMH: f64 = 40.0;
/// Km between neighbouring provinces and between a country's end provinces and its gateways.
const SPACING_KM: f64 = 200.0;
const GATEWAY_KM: f64 = 5.0;

type Point = (f64, f64);

#[derive(Clone, Debug)]
pub struct ToyWorld {
    pub countries: usize,
    pub provinces: usize,
    pub goods: Goods,
    /// Inhabitants of every province.
    pub population: f64,
    /// Final demand shares, the same in every country; sums to 1.
    pub omega: Vec<f64>,
    /// Tonnes per USD of each good.
    pub weight: Vec<f64>,
    /// Monthly value added per million inhabitants, millions, of each good.
    pub va_per_million: Vec<f64>,
    /// Technology multiplier of each country's value added in each good, `[c][g]`; 0 for a
    /// traded good the country does not make. Local goods must be positive.
    pub tech: Vec<Vec<f64>>,
    /// Input coefficients `[k * G + g]`, the same in every country.
    pub io: Vec<f64>,
    pub cost_per_tkm: f64,
    /// Ad valorem tariff on every international flow.
    pub tariff: f64,
    pub labour_share: f64,
    pub unemployment: f64,
    pub gini: f64,
    pub edu_share: [f64; N_EDU],
    /// Labour force per inhabitant.
    pub participation: f64,
}

impl ToyWorld {
    /// `n_goods` goods of which the first `traded` are traded; none storable, all in PIGL
    /// category 1.
    pub fn goods(n_goods: usize, traded: usize) -> Goods {
        Goods::new(
            (0..n_goods).map(|g| format!("g{g}")).collect(),
            traded,
            vec![false; n_goods],
            vec![0; n_goods],
            vec![1; n_goods],
        )
    }

    /// Identical countries, uniform demand, every country making every good, no intermediate
    /// use beyond 10% own-good input, no tariff.
    pub fn new(countries: usize, provinces: usize, goods: Goods) -> Self {
        let n = goods.n();
        let weight = (0..n)
            .map(|g| if goods.is_traded(g) { 0.001 } else { 0.0 })
            .collect();
        let mut io = vec![0.0; n * n];
        for g in 0..n {
            io[g * n + g] = 0.1;
        }
        Self {
            countries,
            provinces,
            population: 1e6,
            omega: vec![1.0 / n as f64; n],
            weight,
            va_per_million: vec![10.0; n],
            tech: vec![vec![1.0; n]; countries],
            io,
            cost_per_tkm: 0.02,
            tariff: 0.0,
            labour_share: 0.55,
            unemployment: 0.06,
            gini: 0.30,
            edu_share: [0.40, 0.40, 0.20],
            participation: 0.47,
            goods,
        }
    }

    pub fn omega(mut self, omega: Vec<f64>) -> Self {
        self.omega = omega;
        self
    }

    pub fn tech(mut self, country: usize, good: usize, a: f64) -> Self {
        self.tech[country][good] = a;
        self
    }

    pub fn io(mut self, input: usize, output: usize, a: f64) -> Self {
        let n = self.goods.n();
        self.io[input * n + output] = a;
        self
    }

    pub fn cost_per_tkm(mut self, cost: f64) -> Self {
        self.cost_per_tkm = cost;
        self
    }

    pub fn tariff(mut self, tariff: f64) -> Self {
        self.tariff = tariff;
        self
    }

    pub fn population(mut self, people: f64) -> Self {
        self.population = people;
        self
    }

    fn leg(&self, from: usize, to: usize, a: Point, b: Point) -> Leg {
        let km = libm::hypot(a.0 - b.0, a.1 - b.1);
        Leg {
            from: from as u32,
            to: to as u32,
            cost_per_t: self.cost_per_tkm * km,
            hours: km / KMH,
        }
    }

    /// Trade parameters for this goods table: each good takes the P2 default of a P2 good of the
    /// same kind (traded from traded, local from local, in turn).
    pub fn trade_params(&self) -> TradeParams {
        let d = TradeParams::default();
        let (td, n) = (d.goods.traded, self.goods.n());
        let src: Vec<usize> = (0..n)
            .map(|g| {
                if self.goods.is_traded(g) {
                    g % td
                } else {
                    td + (g - self.goods.traded) % d.goods.n_local()
                }
            })
            .collect();
        let pick = |v: &[f64]| src.iter().map(|&s| v[s]).collect::<Vec<_>>();
        TradeParams {
            goods: self.goods.clone(),
            theta: pick(&d.theta),
            theta_class: src.iter().map(|&s| d.theta_class[s]).collect(),
            coverage: pick(&d.coverage),
            border_nu: pick(&d.border_nu),
            time_cost: pick(&d.time_cost),
            storage_loss: pick(&d.storage_loss),
            storage_cost: pick(&d.storage_cost),
            speculation: pick(&d.speculation),
            convenience: pick(&d.convenience),
            ..d
        }
    }

    /// The world and the trade parameters it was built under.
    pub fn build(&self) -> (EconInputs, TradeParams) {
        let trade = self.trade_params();
        (self.build_with(&trade), trade)
    }

    fn build_with(&self, trade: &TradeParams) -> EconInputs {
        let (nc, per, n) = (self.countries, self.provinces, self.goods.n());
        assert!(nc >= 1 && per >= 1, "a toy world needs a province");
        assert!(
            self.omega.len() == n
                && self.weight.len() == n
                && self.va_per_million.len() == n
                && self.io.len() == n * n
                && self.tech.len() == nc
                && self.tech.iter().all(|t| t.len() == n),
            "toy world columns not sized to {n} goods and {nc} countries"
        );
        assert!(
            self.tech
                .iter()
                .all(|t| (self.goods.traded..n).all(|g| t[g] > 0.0)),
            "a local good needs positive technology in every country"
        );
        let width = per as f64 * SPACING_KM;

        let mut province_country = Vec::new();
        let mut province_at: Vec<Point> = Vec::new();
        for c in 0..nc {
            for j in 0..per {
                province_country.push(c as u16);
                province_at.push((c as f64 * width + (j as f64 + 0.5) * SPACING_KM, 0.0));
            }
        }
        let mut gateway_country = Vec::new();
        let mut gateway_at: Vec<Point> = Vec::new();
        for c in 0..nc {
            let x = c as f64 * width;
            gateway_country.extend([c as u16, c as u16]);
            gateway_at.extend([(x + GATEWAY_KM, 0.0), (x + width - GATEWAY_KM, 0.0)]);
        }

        let np = province_country.len();
        let mut net = Network {
            province_country: province_country.clone(),
            gateway_country: gateway_country.clone(),
            gateway_mass: equal_gateway_mass(&gateway_country),
            ..Network::default()
        };
        for o in 0..np {
            for d in 0..np {
                if o != d && province_country[o] == province_country[d] {
                    net.domestic
                        .push(self.leg(o, d, province_at[o], province_at[d]));
                }
            }
            for (a, &gc) in gateway_country.iter().enumerate() {
                if gc == province_country[o] {
                    net.access
                        .push(self.leg(o, a, province_at[o], gateway_at[a]));
                }
            }
        }
        for c in 0..nc.saturating_sub(1) {
            let (a, b) = (2 * c + 1, 2 * c + 2);
            for (x, y) in [(a, b), (b, a)] {
                net.crossings
                    .push(self.leg(x, y, gateway_at[x], gateway_at[y]));
            }
        }
        // no sea legs, so the bulk legs are the base ones (ADR 0247)
        net.bulk = trade.bulk_rate().map(|(classes, sea, port)| BulkLegs {
            classes,
            sea,
            port,
            domestic: net.domestic.clone(),
            access: net.access.clone(),
            crossings: net.crossings.clone(),
        });

        let mut wedges = Wedges::none(nc);
        for imp in 0..nc {
            for exp in (0..nc).filter(|&e| e != imp) {
                for group in 0..2 {
                    wedges.tariff[(imp * nc + exp) * 2 + group] = self.tariff as f32;
                }
            }
        }

        let countries = Countries {
            omega: Rows::from_rows(n, vec![self.omega.clone(); nc]),
            gov: Rows::new(n),
            inv: Rows::new(n),
            io: Rows::from_rows(n * n, vec![self.io.clone(); nc]),
            s_l: vec![self.labour_share; nc],
            s_l_good: Rows::new(n),
            wealth: Rows::new(3),
            unemployment: vec![self.unemployment; nc],
            u_star: vec![self.unemployment; nc],
            gini: vec![self.gini; nc],
            pigl: None,
            price_level: vec![1.0; nc],
            transfer: Vec::new(),
            transfer_path: Rows::new(TRANSFER_YEARS),
            foreign: Rows::new(n),
            owners: Rows::new(nc),
            drift_a: Vec::new(),
            drift_f: Vec::new(),
        };

        let mut pops = PopData::default();
        let mut ent = EntData::default();
        for (i, &c) in province_country.iter().enumerate() {
            let c = usize::from(c);
            for (edu, share) in self.edu_share.iter().enumerate() {
                let people = self.population * share;
                pops.province.push(i as u32);
                pops.edu.push(edu as u8);
                pops.population.push(people);
                pops.labour_force.push(self.participation * people);
            }
            for g in (0..n).filter(|&g| self.tech[c][g] > 0.0) {
                let va0 = self.population / 1e6 * self.va_per_million[g] * self.tech[c][g];
                ent.province.push(i as u32);
                ent.good.push(g as u8);
                ent.a.push(1.0);
                ent.va0.push(va0);
                ent.va_target.push(va0);
            }
        }

        let mut inputs = EconInputs {
            goods: self.goods.clone(),
            network: net,
            weight: self.weight.clone(),
            wedges,
            countries,
            provinces: ProvinceData {
                country: province_country,
                population: vec![self.population; np],
                w0: Vec::new(),
                pack: Vec::new(),
                base_price: Rows::new(n),
                gamma: Rows::new(n),
            },
            pops,
            enterprises: ent,
            steady: None,
            border_nu: None,
            market_elasticity: None,
        };
        inputs.set_base_prices(trade);
        inputs.set_coefficients();
        inputs.set_base_point(&EconParams::default().edu_eff);
        inputs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_countries_one_province_one_good() {
        let (w, _) = ToyWorld::new(2, 1, ToyWorld::goods(2, 1)).build();
        w.validate();
        assert_eq!((w.n_countries(), w.n_provinces(), w.n_goods()), (2, 2, 2));
        assert_eq!(w.network.n_gateways(), 4);
        assert_eq!(w.network.crossings.len(), 2);
    }

    #[test]
    fn shapes_follow_the_arguments() {
        let (w, _) = ToyWorld::new(3, 4, ToyWorld::goods(5, 3)).build();
        w.validate();
        assert_eq!((w.n_countries(), w.n_provinces(), w.n_pops()), (3, 12, 36));
        assert_eq!(w.n_enterprises(), 12 * 5);
        assert_eq!(w.goods.traded, 3);
    }

    #[test]
    fn technology_zero_removes_a_traded_good_from_a_country() {
        let (w, _) = ToyWorld::new(2, 2, ToyWorld::goods(3, 2))
            .tech(1, 0, 0.0)
            .build();
        w.validate();
        let makes = |c: u16, g: u8| {
            (0..w.n_enterprises()).any(|e| {
                w.enterprises.good[e] == g
                    && w.provinces.country[w.enterprises.province[e] as usize] == c
            })
        };
        assert!(makes(0, 0) && !makes(1, 0) && makes(1, 1));
    }

    #[test]
    fn deterministic() {
        let a = ToyWorld::new(2, 3, ToyWorld::goods(4, 2));
        assert_eq!(a.build().0.digest(), a.build().0.digest());
    }

    #[test]
    fn base_wages_clear_base_employment() {
        let (w, _) = ToyWorld::new(2, 3, ToyWorld::goods(4, 2)).build();
        let supply = w.labour_supply(&EconParams::default().edu_eff);
        for (i, &s) in supply.iter().enumerate() {
            let l: f64 = (0..w.n_enterprises())
                .filter(|&e| w.enterprises.province[e] as usize == i)
                .map(|e| w.enterprises.l0[e])
                .sum();
            let u = w.countries.u_star[w.country_of(i)];
            assert!((l / ((1.0 - u) * s) - 1.0).abs() < 1e-12);
        }
    }
}
