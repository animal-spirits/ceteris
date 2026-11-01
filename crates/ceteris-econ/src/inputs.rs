//! Static data of the P2 world (`specs/econ/SPEC.md`, State components and Calibration).
//! Everything here is fixed for a run and lives outside snapshots; [`EconInputs::digest`]
//! pins it in the snapshot instead.

use std::sync::Arc;

use ceteris_core::rows::Rows;
use ceteris_core::snapshot::{Encoder, Hash, state_hash};
use ceteris_trade::{
    BulkLegs, GoodLegs, Goods, Leg, LegSet, LegTaus, Network, TradeParams, Wedges,
};

use crate::calibration::Steady;
use crate::demand::PiglWeights;
use crate::params::EconParams;

/// The elasticities of each market's price loop at the calibrated point (ADR 0263; trade SPEC,
/// Stability region, Per-market gain), `[province * n_goods + g]`, 0 where no market is active.
#[derive(Clone, Debug, PartialEq)]
pub struct MarketElasticity {
    /// E: the own-price elasticity of the orders the market receives.
    pub demand: Vec<f64>,
    /// eps: the largest supply elasticity of the market's enterprises.
    pub supply: Vec<f64>,
    /// The limit gain of the market's coupled block (ADR 0267; trade SPEC, Block gain) in the
    /// default mode and frictionless: the gain at which the block is at the edge of stability,
    /// computed at the default kappa1 (P03a), P03d and wage parameters; infinite for a market in
    /// no block or one whose block is within its margin.
    pub limit: [Vec<f64>; 2],
}

impl MarketElasticity {
    /// J of cell `i` at employment speed `eta` (P41) under `trade`: E r + eta eps / (2 - eta), r
    /// the storage order's factor on the orders' elasticity at a competitively stored good's
    /// origin ([`TradeParams::order_response`], ADR 0091; 1 elsewhere).
    pub fn j(&self, i: usize, eta: f64, trade: &TradeParams) -> f64 {
        self.demand[i] * trade.order_response(i % trade.goods.n())
            + eta * self.supply[i] / (2.0 - eta)
    }

    /// kappa1_cell of cell `i` in a run under `params` and `trade` (ADRs 0263, 0267): the
    /// per-market cap at the run's eta, kappa1 and P03d, capped again at the share P03d of the
    /// block limit gain of the run's wage regime (frictionless or the wage rule). J carries the
    /// storage response of `trade`; the limit is the calibration's, at its storage parameters.
    pub fn price_gain(&self, i: usize, params: &EconParams, trade: &TradeParams) -> f64 {
        let mode = usize::from(params.frictionless);
        trade
            .price_gain(self.j(i, params.eta, trade))
            .min(trade.gain_share * self.limit[mode][i])
    }
}

/// Education levels of a pop: below secondary, secondary, tertiary.
pub const N_EDU: usize = 3;

/// Per-country data, indexed by country.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Countries {
    /// Final demand shares Omega_g from all 2000 final demand.
    pub omega: Rows<f64>,
    /// Input coefficients a_kg, units of k per unit of g, at `[k * n_goods + g]`.
    pub io: Rows<f64>,
    /// Labour share s_L, used for every sector.
    pub s_l: Vec<f64>,
    /// 2000 unemployment rate (data; the model runs on `u_star`).
    pub unemployment: Vec<f64>,
    /// Natural rate u*, the mean unemployment rate 1995 to 2005 (ADR 0089): the base point,
    /// the wage rule's wedge and the frictionless labour target.
    pub u_star: Vec<f64>,
    /// 2000 Gini of income, the dispersion of each pop's lognormal income (ADR 0021).
    pub gini: Vec<f64>,
    /// Calibrated PIGL weights (ADR 0089, stage 4); `None` inverts them at load at the data
    /// state's prices (`world::household_preferences`).
    pub pigl: Option<Vec<PiglWeights>>,
    /// Price level p_c (ADR 0098): the base price of the country's local goods, at which the
    /// data state starts them. See [`EconInputs::set_price_levels`].
    pub price_level: Vec<f64>,
}

/// Per-province data, indexed by province.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProvinceData {
    /// Same as `Network::province_country`.
    pub country: Vec<u16>,
    /// Pack province (graph node) of each province, for a world loaded from the pack
    /// ([`EconInputs::from_pack`]); empty for a generated world, whose provinces are no
    /// pack's.
    pub pack: Vec<u32>,
    pub population: Vec<f64>,
    /// Base wage w0, numeraire per effective labour unit. See [`EconInputs::set_base_point`].
    pub w0: Vec<f64>,
    /// Base delivered unit price q_d of every good at the province: the country's price level
    /// p_c for local goods (ADR 0098); for traded goods, sourcing in the data state with
    /// prices 1, tariffs and iceberg included, infinite where no origin is reachable. See
    /// [`EconInputs::set_base_prices`].
    pub base_price: Rows<f64>,
    /// Selection constant Gamma of every good at the province (ADR 0200): buyers of a traded
    /// good pay Gamma times the CES index P_d. Fixed where the world is calibrated, so that the
    /// buyer price there is the delivered unit price of the physical arrivals: the data state's
    /// `base_price / P_d` at prices 1 ([`EconInputs::set_base_prices`]), or the calibration
    /// tables' value. 1 for local goods and where no origin is reachable.
    pub gamma: Rows<f64>,
}

/// Pops, one per (province, education level), sorted by province.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PopData {
    pub province: Vec<u32>,
    /// Education level, `0..N_EDU`.
    pub edu: Vec<u8>,
    pub labour_force: Vec<f64>,
    pub population: Vec<f64>,
}

/// Enterprises, one per (province, sector), sorted by province and then sector.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntData {
    pub province: Vec<u32>,
    /// Sector, the good produced.
    pub good: Vec<u8>,
    /// Productivity A.
    pub a: Vec<f64>,
    /// Base value added VA0, numeraire per month.
    pub va0: Vec<f64>,
    /// Base employment L0, effective labour units. See [`EconInputs::set_base_point`].
    pub l0: Vec<f64>,
    /// Value added the calibration matches (pack VA for traded goods).
    pub va_target: Vec<f64>,
    /// Putty-clay input coefficients (ADR 0020): units of each good per unit of output,
    /// aggregated over the enterprise's capacity vintages. P2 has one vintage, `column`.
    pub coef: Rows<f64>,
    /// The 2000 vintage, which S09 FixedLeontief runs on: the country's ICIO cost shares at
    /// base delivered prices ([`EconInputs::set_coefficients`]) or at the calibrated prices
    /// (ADR 0089, [`EconInputs::calibrate`]).
    pub column: Rows<f64>,
    /// Value-added coefficient a_VA, real VA per unit of output: 1 - sum_k coef_k q_k at base
    /// prices for the base column, 1 - sum_k a_kg (the cost shares at the calibrated prices)
    /// for a calibrated one.
    pub a_va: Vec<f64>,
}

/// All static data of a P2 world.
///
/// The base point (`ProvinceData::w0`, `EntData::l0`) is data here, not derived at load: the
/// builder of the inputs fills it once with [`EconInputs::set_base_point`] (the data state) or
/// reads it from the calibration tables ([`EconInputs::calibrate`]), and `EconWorld::new` uses
/// it as given.
#[derive(Clone, Debug, Default)]
pub struct EconInputs {
    /// The goods of the pack (`econ_goods`); every goods-indexed field has one entry per good.
    pub goods: Goods,
    pub network: Network,
    /// Weight per value w_g, tonnes per unit.
    pub weight: Vec<f64>,
    pub wedges: Wedges,
    pub countries: Countries,
    pub provinces: ProvinceData,
    pub pops: PopData,
    pub enterprises: EntData,
    /// The calibrated steady state a world starts from (ADR 0089, stage 5); `None` starts
    /// from the data state. Not in the digest: snapshots carry the state.
    pub steady: Option<Arc<Steady>>,
    /// P05 per good of the calibration (ADRs 0092, 0276), which its steady state was solved at;
    /// `None` uncalibrated. [`EconInputs::trade_params`] carries it into the trade parameters.
    pub border_nu: Option<Vec<f64>>,
    /// The calibration's elasticities of each market's price loop (ADR 0263), which cap its
    /// price gain; `None` uncalibrated, where every market runs kappa1.
    pub market_elasticity: Option<MarketElasticity>,
}

impl EconInputs {
    /// The default trade parameters with the calibrated border friction, where there is one:
    /// the parameters the calibrated steady state holds under.
    pub fn trade_params(&self) -> TradeParams {
        let default = TradeParams::default();
        TradeParams {
            goods: self.goods.clone(),
            border_nu: self.border_nu.clone().unwrap_or(default.border_nu),
            ..default
        }
    }

    pub fn n_goods(&self) -> usize {
        self.goods.n()
    }

    pub fn n_countries(&self) -> usize {
        self.countries.s_l.len()
    }

    pub fn n_provinces(&self) -> usize {
        self.provinces.country.len()
    }

    pub fn n_pops(&self) -> usize {
        self.pops.province.len()
    }

    pub fn n_enterprises(&self) -> usize {
        self.enterprises.province.len()
    }

    /// Country of province `i`.
    pub fn country_of(&self, i: usize) -> usize {
        usize::from(self.provinces.country[i])
    }

    /// Input coefficient a_kg of country `c`: units of `k` per unit of `g`.
    pub fn a(&self, c: usize, k: usize, g: usize) -> f64 {
        self.countries.io[c][k * self.n_goods() + g]
    }

    /// Putty-clay input coefficient of enterprise `e`: units of `k` per unit of its output.
    pub fn coef(&self, e: usize, k: usize) -> f64 {
        self.enterprises.coef[e][k]
    }

    /// Fills `ProvinceData::base_price` and `ProvinceData::gamma` from sourcing at prices 1
    /// over the kernel of this network under `trade` ([`crate::world::base_delivered_prices`]).
    pub fn set_base_prices(&mut self, trade: &TradeParams) {
        (self.provinces.base_price, self.provinces.gamma) =
            crate::world::base_delivered_prices(self, trade);
    }

    /// Sets each country's price level p_c to `levels` (ADR 0098) and what follows from it in
    /// the data state: A = 1 / p_c for local enterprises (1 for traded ones), the base prices
    /// under `trade` and the coefficients, so that values, cost shares and the base point stay
    /// those of the data.
    pub fn set_price_levels(&mut self, levels: Vec<f64>, trade: &TradeParams) {
        self.countries.price_level = levels;
        for e in 0..self.n_enterprises() {
            self.enterprises.a[e] = 1.0 / self.base_output_price(e);
        }
        self.set_base_prices(trade);
        self.set_coefficients();
    }

    /// Price of good `g` at province `i` in the data state: 1 for traded goods, the country's
    /// price level p_c for local goods (ADR 0098).
    pub fn data_state_price(&self, i: usize, g: usize) -> f64 {
        if self.goods.is_traded(g) {
            1.0
        } else {
            self.countries.price_level[self.country_of(i)]
        }
    }

    /// p_o of enterprise `e`: the data state's price of its output at its province.
    pub fn base_output_price(&self, e: usize) -> f64 {
        let ent = &self.enterprises;
        self.data_state_price(ent.province[e] as usize, usize::from(ent.good[e]))
    }

    /// Sets the network's route taus to the kernel's least-cost factors (S05) under `trade`,
    /// from each good's legs as the kernel picks them (its good legs, ADR 0279, else the bulk
    /// legs for the bulk classes' goods, ADR 0247), with each good's leg hours, and drops the
    /// bulk and good legs, since route taus exclude both. The world then runs with the bulk
    /// freight rate off; its kernel is the one of these inputs at `trade`.
    pub fn fold_bulk_into_route_taus(&mut self, trade: &TradeParams) {
        let bulk = self.network.bulk.take();
        let good_legs = self.network.good_legs.take();
        let (weight, n) = (&self.weight, self.n_goods());
        let on_bulk = |g: usize| {
            bulk.as_ref()
                .is_some_and(|b| b.classes[usize::from(trade.theta_class[g])])
        };
        let factors = |base: &[Leg], bulk: Option<&Vec<Leg>>, pick: fn(&LegSet) -> &Vec<Leg>| {
            let leg = |i: usize, g: usize| match &good_legs {
                Some(gl) if g < gl.set.len() => &pick(&gl.sets[usize::from(gl.set[g])])[i],
                _ => bulk.filter(|_| on_bulk(g)).map_or(&base[i], |b| &b[i]),
            };
            let taus = Rows::from_flat(
                (0..base.len())
                    .flat_map(|i| {
                        (0..n).map(move |g| {
                            let l = leg(i, g);
                            // the kernel's least-cost factor, bit for bit
                            1.0 + l.cost_per_t * weight[g] + trade.time_cost[g] * l.hours / 24.0
                        })
                    })
                    .collect(),
                n,
            );
            let hours = Rows::from_flat(
                (0..base.len())
                    .flat_map(|i| (0..n).map(move |g| leg(i, g).hours))
                    .collect(),
                n,
            );
            (taus, hours)
        };
        let net = &self.network;
        let (domestic, domestic_hours) =
            factors(&net.domestic, bulk.as_ref().map(|b| &b.domestic), |s| {
                &s.domestic
            });
        let (access, access_hours) =
            factors(&net.access, bulk.as_ref().map(|b| &b.access), |s| &s.access);
        let (crossings, crossing_hours) =
            factors(&net.crossings, bulk.as_ref().map(|b| &b.crossings), |s| {
                &s.crossings
            });
        self.network.route_taus = Some(LegTaus {
            domestic,
            access,
            crossings,
            domestic_hours,
            access_hours,
            crossing_hours,
        });
    }

    /// Fills every enterprise's 2000 column with [`EconInputs::cost_share_column`], its
    /// coefficients with that one vintage (ADR 0020) and a_VA with [`EconInputs::base_a_va`].
    /// Needs the base prices.
    pub fn set_coefficients(&mut self) {
        let column = Rows::from_rows(
            self.n_goods(),
            (0..self.n_enterprises()).map(|e| self.cost_share_column(e)),
        );
        self.enterprises.a_va = (0..self.n_enterprises())
            .map(|e| self.base_a_va(e, &column[e]))
            .collect();
        self.enterprises.coef = column.clone();
        self.enterprises.column = column;
    }

    /// The ICIO column of enterprise `e`'s country and sector, a_kg for every k: cost shares
    /// of output at base prices.
    pub fn country_column(&self, e: usize) -> Vec<f64> {
        let c = self.country_of(self.enterprises.province[e] as usize);
        let g = usize::from(self.enterprises.good[e]);
        (0..self.n_goods()).map(|k| self.a(c, k, g)).collect()
    }

    /// The 2000 column of enterprise `e`, which S09 FixedLeontief runs on.
    pub fn base_column(&self, e: usize) -> &[f64] {
        &self.enterprises.column[e]
    }

    /// Physical coefficients of enterprise `e` from its country's cost shares (owner decision,
    /// 2026-10-25): coef_k = a_kg p_o / q_k, q_k the base delivered price of k at its province
    /// and p_o that of its output ([`EconInputs::base_output_price`], ADR 0098), so at base
    /// prices every input's cost share is the ICIO share; 0 where k is unreachable.
    pub fn cost_share_column(&self, e: usize) -> Vec<f64> {
        let q = &self.provinces.base_price[self.enterprises.province[e] as usize];
        let (a, p_o) = (self.country_column(e), self.base_output_price(e));
        (0..a.len())
            .map(|k| if a[k] > 0.0 { a[k] * p_o / q[k] } else { 0.0 })
            .collect()
    }

    /// Value-added coefficient of coefficients `coef` of enterprise `e` at the base prices of
    /// its province: a_VA = 1 - sum_k coef_k q_k / p_o over reachable inputs (1 - sum_k a_kg
    /// for the base column).
    pub fn base_a_va(&self, e: usize, coef: &[f64]) -> f64 {
        let q = &self.provinces.base_price[self.enterprises.province[e] as usize];
        1.0 - (0..coef.len())
            .filter(|&k| coef[k] > 0.0)
            .map(|k| coef[k] * q[k])
            .sum::<f64>()
            / self.base_output_price(e)
    }

    /// Whether every enterprise's coefficients are its 2000 column (one vintage), so that
    /// both S09 settings read the same numbers.
    pub fn one_vintage(&self) -> bool {
        self.enterprises.coef == self.enterprises.column
    }

    /// Value-added coefficient a_VA = 1 - sum_k a_kg of sector `g` in country `c`.
    pub fn a_va(&self, c: usize, g: usize) -> f64 {
        1.0 - (0..self.n_goods()).map(|k| self.a(c, k, g)).sum::<f64>()
    }

    /// Effective labour supply per province, L^s_i = sum_k e_k LF_k (P42 `edu_eff`).
    pub fn labour_supply(&self, edu_eff: &[f64; N_EDU]) -> Vec<f64> {
        let mut out = vec![0.0; self.n_provinces()];
        for k in 0..self.n_pops() {
            let i = self.pops.province[k] as usize;
            out[i] += edu_eff[usize::from(self.pops.edu[k])] * self.pops.labour_force[k];
        }
        out
    }

    /// Fills the base point from VA0 (ADR 0089): w0_i = sum_g s_L VA0_ig / ((1 - u*_i)
    /// L^s_i) and L0_ig = s_L VA0_ig / w0_i, so base wages clear base employment at the
    /// natural rate. Panics for a province with no value added or no labour supply.
    pub fn set_base_point(&mut self, edu_eff: &[f64; N_EDU]) {
        let supply = self.labour_supply(edu_eff);
        let mut bill = vec![0.0; self.n_provinces()];
        for e in 0..self.n_enterprises() {
            let i = self.enterprises.province[e] as usize;
            bill[i] += self.countries.s_l[self.country_of(i)] * self.enterprises.va0[e];
        }
        self.provinces.w0 = (0..self.n_provinces())
            .map(|i| {
                assert!(
                    bill[i] > 0.0 && supply[i] > 0.0,
                    "province {i} needs value added and labour for a base wage"
                );
                bill[i] / ((1.0 - self.countries.u_star[self.country_of(i)]) * supply[i])
            })
            .collect();
        self.enterprises.l0 = (0..self.n_enterprises())
            .map(|e| {
                let i = self.enterprises.province[e] as usize;
                self.countries.s_l[self.country_of(i)] * self.enterprises.va0[e]
                    / self.provinces.w0[i]
            })
            .collect();
    }

    /// Panics with the first broken contract: lengths, sort orders, ranges.
    pub fn validate(&self) {
        let (nc, np, n) = (self.n_countries(), self.n_provinces(), self.n_goods());
        let widths = [
            self.weight.len(),
            self.border_nu.as_ref().map_or(n, Vec::len),
            self.countries.omega.width(),
            self.countries.io.width() / n.max(1),
            self.provinces.base_price.width(),
            self.provinces.gamma.width(),
            self.enterprises.coef.width(),
            self.enterprises.column.width(),
        ];
        assert!(
            widths.iter().all(|&w| w == n),
            "goods-indexed data not sized to the {n} goods: {widths:?}"
        );
        let c = &self.countries;
        assert!(
            c.omega.len() == nc
                && c.io.len() == nc
                && c.unemployment.len() == nc
                && c.u_star.len() == nc
                && c.gini.len() == nc
                && c.price_level.len() == nc
                && c.pigl.as_ref().is_none_or(|w| w.len() == nc),
            "country columns disagree in length"
        );
        assert_eq!(
            self.wedges.n_countries(),
            nc,
            "wedges cover a different country count"
        );
        for k in 0..nc {
            assert!(
                c.s_l[k] > 0.0 && c.s_l[k] < 1.0,
                "country {k}: labour share {}",
                c.s_l[k]
            );
            assert!(
                (0.0..1.0).contains(&c.unemployment[k]),
                "country {k}: unemployment {}",
                c.unemployment[k]
            );
            assert!(
                (0.0..1.0).contains(&c.u_star[k]),
                "country {k}: natural rate {}",
                c.u_star[k]
            );
            assert!(
                c.price_level[k] > 0.0 && c.price_level[k].is_finite(),
                "country {k}: price level {}",
                c.price_level[k]
            );
            assert!(
                (0.0..1.0).contains(&c.gini[k]),
                "country {k}: Gini {}",
                c.gini[k]
            );
            assert!(
                c.omega[k].iter().all(|&w| w >= 0.0) && c.omega[k].iter().any(|&w| w > 0.0),
                "country {k}: final demand shares {:?}",
                &c.omega[k]
            );
            assert!(
                c.io[k].iter().all(|&a| a >= 0.0),
                "country {k}: negative input coefficient"
            );
            for g in 0..self.n_goods() {
                assert!(
                    self.a_va(k, g) > 0.0,
                    "country {k} sector {g}: no value added"
                );
            }
        }

        let p = &self.provinces;
        assert!(
            p.population.len() == np
                && p.w0.len() == np
                && p.base_price.len() == np
                && p.gamma.len() == np,
            "province columns disagree in length; fill base prices with set_base_prices"
        );
        assert!(
            p.base_price
                .iter()
                .all(|q| q.iter().all(|&x| x > 0.0 && !x.is_nan())),
            "base delivered prices must be positive"
        );
        assert!(
            p.gamma
                .iter()
                .all(|g| g.iter().all(|&x| x > 0.0 && x.is_finite())),
            "selection constants must be positive and finite"
        );
        assert_eq!(
            p.country, self.network.province_country,
            "province countries differ from the network"
        );
        assert!(
            p.country.iter().all(|&k| usize::from(k) < nc),
            "province country out of range"
        );
        assert!(
            p.w0.iter().all(|&w| w.is_finite() && w > 0.0),
            "base wages must be finite and positive; fill them with set_base_point"
        );
        assert!(
            p.pack.is_empty() || (p.pack.len() == np && p.pack.is_sorted_by(|a, b| a < b)),
            "pack provinces must be one per province, increasing"
        );

        let pp = &self.pops;
        let n = pp.province.len();
        assert!(
            pp.edu.len() == n && pp.labour_force.len() == n && pp.population.len() == n,
            "pop columns disagree in length"
        );
        for k in 0..n {
            assert!(
                (pp.province[k] as usize) < np,
                "pop {k}: province out of range"
            );
            assert!(
                k == 0 || pp.province[k - 1] <= pp.province[k],
                "pops not sorted by province at {k}"
            );
            assert!(usize::from(pp.edu[k]) < N_EDU, "pop {k}: education level");
            assert!(
                pp.labour_force[k] >= 0.0 && pp.labour_force[k] <= pp.population[k],
                "pop {k}: labour force {} of population {}",
                pp.labour_force[k],
                pp.population[k]
            );
        }

        let e = &self.enterprises;
        let n = e.province.len();
        assert!(
            e.good.len() == n
                && e.a.len() == n
                && e.va0.len() == n
                && e.l0.len() == n
                && e.va_target.len() == n
                && e.coef.len() == n
                && e.column.len() == n
                && e.a_va.len() == n,
            "enterprise columns disagree in length"
        );
        for k in 0..n {
            assert!((e.province[k] as usize) < np, "enterprise {k}: province");
            assert!(
                usize::from(e.good[k]) < self.n_goods(),
                "enterprise {k}: good"
            );
            assert!(
                k == 0 || (e.province[k - 1], e.good[k - 1]) < (e.province[k], e.good[k]),
                "enterprises not sorted by (province, good) or duplicated at {k}"
            );
            assert!(
                e.a[k] > 0.0 && e.va0[k] >= 0.0 && e.l0[k] >= 0.0,
                "enterprise {k}: A {}, VA0 {}, L0 {}",
                e.a[k],
                e.va0[k],
                e.l0[k]
            );
            assert!(
                e.a[k].is_finite()
                    && e.va0[k].is_finite()
                    && e.l0[k].is_finite()
                    && e.va_target[k].is_finite(),
                "enterprise {k}: non-finite data"
            );
            for coef in [&e.coef[k], &e.column[k]] {
                assert!(
                    coef.iter().all(|a| a.is_finite() && *a >= 0.0),
                    "enterprise {k}: input coefficients {coef:?}; fill them with set_coefficients"
                );
            }
            assert!(
                e.a_va[k] > 0.0 && e.a_va[k] <= 1.0,
                "enterprise {k}: value-added coefficient {}",
                e.a_va[k]
            );
        }
    }

    /// BLAKE3 of a canonical encoding of every field.
    pub fn digest(&self) -> Hash {
        self.digest_with(true)
    }

    /// [`EconInputs::digest`] without the network, which snapshots name by
    /// [`network_digest`].
    pub fn base_digest(&self) -> Hash {
        self.digest_with(false)
    }

    fn digest_with(&self, with_network: bool) -> Hash {
        // destructured so a new field cannot be left out of the digest
        let EconInputs {
            goods,
            network,
            weight,
            wedges,
            countries,
            provinces,
            pops,
            enterprises,
            // the initial state, which snapshots carry
            steady: _,
            border_nu,
            market_elasticity,
        } = self;
        let Network {
            province_country,
            gateway_country,
            gateway_mass,
            domestic,
            access,
            crossings,
            route_taus,
            bulk,
            good_legs,
        } = network;
        let Countries {
            omega,
            io,
            s_l,
            unemployment,
            u_star,
            gini,
            pigl,
            price_level,
        } = countries;
        let ProvinceData {
            country,
            population,
            w0,
            pack,
            base_price,
            gamma,
        } = provinces;
        let PopData {
            province: pop_province,
            edu,
            labour_force,
            population: pop_population,
        } = pops;
        let EntData {
            province: ent_province,
            good,
            a,
            va0,
            l0,
            va_target,
            coef,
            column,
            a_va,
        } = enterprises;

        let mut e = Encoder::default();
        b"ceteris-econ-inputs-1".iter().for_each(|&b| e.u8(b));
        // the goods table only where it is not P2's, so P2 worlds keep their digest
        if *goods != Goods::p2() {
            b"goods".iter().for_each(|&b| e.u8(b));
            e.count(goods.n());
            for name in &goods.name {
                e.count(name.len());
                name.bytes().for_each(|b| e.u8(b));
            }
            e.count(goods.traded);
            e.bools(&goods.storable);
            goods.group.iter().for_each(|&g| e.u8(g));
        }
        if with_network {
            u16s(&mut e, province_country);
            u16s(&mut e, gateway_country);
            e.f64s(gateway_mass);
            for legs in [domestic, access, crossings] {
                encode_legs(&mut e, legs);
            }
        }
        e.f64s(weight);
        e.count(wedges.tariff.len());
        wedges.tariff.iter().for_each(|t| e.u32(t.to_bits()));
        e.f64s(omega.as_flat());
        e.f64s(io.as_flat());
        e.f64s(s_l);
        e.f64s(unemployment);
        e.f64s(u_star);
        e.f64s(gini);
        u16s(&mut e, country);
        e.f64s(population);
        e.f64s(w0);
        e.f64s(base_price.as_flat());
        e.f64s(gamma.as_flat());
        e.u32s(pop_province);
        e.count(edu.len());
        edu.iter().for_each(|&b| e.u8(b));
        e.f64s(labour_force);
        e.f64s(pop_population);
        e.u32s(ent_province);
        e.count(good.len());
        good.iter().for_each(|&b| e.u8(b));
        e.f64s(a);
        e.f64s(va0);
        e.f64s(l0);
        e.f64s(va_target);
        e.f64s(coef.as_flat());
        e.f64s(column.as_flat());
        e.f64s(a_va);
        if let Some(weights) = pigl {
            b"pigl-weights".iter().for_each(|&b| e.u8(b));
            for w in weights {
                e.f64(w.scale);
                e.f64s(&w.omega);
                e.f64s(&w.within);
                e.f64(w.sigma);
            }
        }
        // appended only where some country's is not 1, so worlds before ADR 0098 keep their digest
        if price_level.iter().any(|&p| p != 1.0) {
            b"price-level".iter().for_each(|&b| e.u8(b));
            e.f64s(price_level);
        }
        if let Some(nu) = border_nu {
            b"border-nu".iter().for_each(|&b| e.u8(b));
            e.f64s(nu);
        }
        // appended only when present, so uncalibrated worlds keep their digest
        if let Some(m) = market_elasticity {
            b"market-elasticity".iter().for_each(|&b| e.u8(b));
            e.f64s(&m.demand);
            e.f64s(&m.supply);
            e.f64s(&m.limit[0]);
            e.f64s(&m.limit[1]);
        }
        // appended only when present, so generated worlds keep their digest
        if !pack.is_empty() {
            b"pack-provinces".iter().for_each(|&b| e.u8(b));
            e.u32s(pack);
        }
        if with_network {
            encode_network_tail(&mut e, route_taus, bulk, good_legs);
        }
        state_hash(&e.into_bytes())
    }
}

/// BLAKE3 of a canonical encoding of every field of `net`, the network [`EconInputs::digest`]
/// covers.
pub fn network_digest(net: &Network) -> Hash {
    let Network {
        province_country,
        gateway_country,
        gateway_mass,
        domestic,
        access,
        crossings,
        route_taus,
        bulk,
        good_legs,
    } = net;
    let mut e = Encoder::default();
    b"ceteris-econ-network-1".iter().for_each(|&b| e.u8(b));
    u16s(&mut e, province_country);
    u16s(&mut e, gateway_country);
    e.f64s(gateway_mass);
    for legs in [domestic, access, crossings] {
        encode_legs(&mut e, legs);
    }
    encode_network_tail(&mut e, route_taus, bulk, good_legs);
    state_hash(&e.into_bytes())
}

fn encode_network_tail(
    e: &mut Encoder,
    route_taus: &Option<LegTaus>,
    bulk: &Option<BulkLegs>,
    good_legs: &Option<GoodLegs>,
) {
    // appended only when present, so least-cost networks keep their digest
    if let Some(rt) = route_taus {
        b"route-taus".iter().for_each(|&b| e.u8(b));
        for list in [
            &rt.domestic,
            &rt.access,
            &rt.crossings,
            &rt.domestic_hours,
            &rt.access_hours,
            &rt.crossing_hours,
        ] {
            e.f64s(list.as_flat());
        }
    }
    // appended only when present, so networks without a bulk freight rate keep their digest
    if let Some(b) = bulk {
        b"bulk-legs".iter().for_each(|&b| e.u8(b));
        b.classes.iter().for_each(|&c| e.u8(u8::from(c)));
        for legs in [&b.domestic, &b.access, &b.crossings] {
            encode_legs(e, legs);
        }
    }
    // appended only when present, so least-cost networks keep their digest
    if let Some(gl) = good_legs {
        b"good-legs".iter().for_each(|&b| e.u8(b));
        gl.set.iter().for_each(|&s| e.u8(s));
        e.f64s(&gl.kappa);
        e.count(gl.sets.len());
        for s in &gl.sets {
            for legs in [&s.domestic, &s.access, &s.crossings] {
                encode_legs(e, legs);
            }
        }
    }
}

fn u16s(e: &mut Encoder, vs: &[u16]) {
    e.count(vs.len());
    vs.iter().for_each(|&v| e.u16(v));
}

fn encode_legs(e: &mut Encoder, legs: &[Leg]) {
    e.count(legs.len());
    for l in legs {
        e.u32(l.from);
        e.u32(l.to);
        e.f64(l.cost_per_t);
        e.f64(l.hours);
    }
}
