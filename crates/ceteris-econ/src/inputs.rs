//! Static data of the P2 world (`specs/econ/SPEC.md`, State components and Calibration).
//! Everything here is fixed for a run and lives outside snapshots; [`EconInputs::digest`]
//! pins it in the snapshot instead.

use std::sync::Arc;

use ceteris_core::snapshot::{Encoder, Hash, state_hash};
use ceteris_trade::{Leg, N_CLASSES, N_GOODS, Network, TradeParams, Wedges};

use crate::calibration::Steady;
use crate::demand::PiglWeights;

/// Education levels of a pop: below secondary, secondary, tertiary.
pub const N_EDU: usize = 3;

/// Per-country data, indexed by country.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Countries {
    /// Final demand shares Omega_g from all 2000 final demand.
    pub omega: Vec<[f64; N_GOODS]>,
    /// Input coefficients a_kg, units of k per unit of g, at `[k * N_GOODS + g]`.
    pub io: Vec<[f64; N_GOODS * N_GOODS]>,
    /// Labour share s_L, used for every sector.
    pub s_l: Vec<f64>,
    /// 2000 unemployment rate (data; the model runs on `u_star`).
    pub unemployment: Vec<f64>,
    /// Natural rate u*, the mean unemployment rate 1995 to 2005 (ADR 0089): the base point,
    /// the wage rule's wedge and the frictionless labour target.
    pub u_star: Vec<f64>,
    /// 2000 Gini of income, the dispersion of each pop's lognormal income (ADR 0021).
    pub gini: Vec<f64>,
    /// Calibrated PIGL weights (ADR 0089, stage 4); `None` inverts them at load at prices 1
    /// (`world::household_preferences`).
    pub pigl: Option<Vec<PiglWeights>>,
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
    /// Base delivered unit price q_d of every good at the province: 1 for local goods; for
    /// traded goods, sourcing in the data state with prices 1, tariffs and iceberg included,
    /// infinite where no origin is reachable. See [`EconInputs::set_base_prices`].
    pub base_price: Vec<[f64; N_GOODS]>,
    /// Selection constant Gamma of every good at the province (ADR 0200): buyers of a traded
    /// good pay Gamma times the CES index P_d. Fixed where the world is calibrated, so that the
    /// buyer price there is the delivered unit price of the physical arrivals: the data state's
    /// `base_price / P_d` at prices 1 ([`EconInputs::set_base_prices`]), or the calibration
    /// tables' value. 1 for local goods and where no origin is reachable.
    pub gamma: Vec<[f64; N_GOODS]>,
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
    pub coef: Vec<[f64; N_GOODS]>,
    /// The 2000 vintage, which S09 FixedLeontief runs on: the country's ICIO cost shares at
    /// base delivered prices ([`EconInputs::set_coefficients`]) or at the calibrated prices
    /// (ADR 0089, [`EconInputs::calibrate`]).
    pub column: Vec<[f64; N_GOODS]>,
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
    pub network: Network,
    /// Weight per value w_g, tonnes per unit.
    pub weight: [f64; N_GOODS],
    pub wedges: Wedges,
    pub countries: Countries,
    pub provinces: ProvinceData,
    pub pops: PopData,
    pub enterprises: EntData,
    /// The calibrated steady state a world starts from (ADR 0089, stage 5); `None` starts
    /// from the data state. Not in the digest: snapshots carry the state.
    pub steady: Option<Arc<Steady>>,
    /// P05 per theta class of the calibration (ADR 0092), which its steady state was solved at;
    /// `None` uncalibrated. [`EconInputs::trade_params`] carries it into the trade parameters.
    pub border_nu: Option<[f64; N_CLASSES]>,
}

impl EconInputs {
    /// The default trade parameters with the calibrated border friction, where there is one:
    /// the parameters the calibrated steady state holds under.
    pub fn trade_params(&self) -> TradeParams {
        let default = TradeParams::default();
        TradeParams {
            border_nu: self.border_nu.unwrap_or(default.border_nu),
            ..default
        }
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
        self.countries.io[c][k * N_GOODS + g]
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

    /// Fills every enterprise's 2000 column with [`EconInputs::cost_share_column`], its
    /// coefficients with that one vintage (ADR 0020) and a_VA with [`EconInputs::base_a_va`].
    /// Needs the base prices.
    pub fn set_coefficients(&mut self) {
        let column: Vec<[f64; N_GOODS]> = (0..self.n_enterprises())
            .map(|e| self.cost_share_column(e))
            .collect();
        self.enterprises.a_va = (0..self.n_enterprises())
            .map(|e| self.base_a_va(self.enterprises.province[e] as usize, &column[e]))
            .collect();
        self.enterprises.coef = column.clone();
        self.enterprises.column = column;
    }

    /// The ICIO column of enterprise `e`'s country and sector, a_kg for every k: cost shares
    /// of output at base prices.
    pub fn country_column(&self, e: usize) -> [f64; N_GOODS] {
        let c = self.country_of(self.enterprises.province[e] as usize);
        let g = usize::from(self.enterprises.good[e]);
        std::array::from_fn(|k| self.a(c, k, g))
    }

    /// The 2000 column of enterprise `e`, which S09 FixedLeontief runs on.
    pub fn base_column(&self, e: usize) -> [f64; N_GOODS] {
        self.enterprises.column[e]
    }

    /// Physical coefficients of enterprise `e` from its country's cost shares (owner decision,
    /// 2026-10-25): coef_k = a_kg / q_k, q_k the base delivered price of k at its province, so
    /// at base prices every input's cost share is the ICIO share; 0 where k is unreachable.
    pub fn cost_share_column(&self, e: usize) -> [f64; N_GOODS] {
        let q = &self.provinces.base_price[self.enterprises.province[e] as usize];
        let a = self.country_column(e);
        std::array::from_fn(|k| if a[k] > 0.0 { a[k] / q[k] } else { 0.0 })
    }

    /// Value-added coefficient of coefficients `coef` at the base prices of province `i`:
    /// a_VA = 1 - sum_k coef_k q_k over reachable inputs (1 - sum_k a_kg for the base column).
    pub fn base_a_va(&self, i: usize, coef: &[f64; N_GOODS]) -> f64 {
        let q = &self.provinces.base_price[i];
        1.0 - (0..N_GOODS)
            .filter(|&k| coef[k] > 0.0)
            .map(|k| coef[k] * q[k])
            .sum::<f64>()
    }

    /// Whether every enterprise's coefficients are its 2000 column (one vintage), so that
    /// both S09 settings read the same numbers.
    pub fn one_vintage(&self) -> bool {
        self.enterprises.coef == self.enterprises.column
    }

    /// Value-added coefficient a_VA = 1 - sum_k a_kg of sector `g` in country `c`.
    pub fn a_va(&self, c: usize, g: usize) -> f64 {
        1.0 - (0..N_GOODS).map(|k| self.a(c, k, g)).sum::<f64>()
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
        let (nc, np) = (self.n_countries(), self.n_provinces());
        let c = &self.countries;
        assert!(
            c.omega.len() == nc
                && c.io.len() == nc
                && c.unemployment.len() == nc
                && c.u_star.len() == nc
                && c.gini.len() == nc
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
                (0.0..1.0).contains(&c.gini[k]),
                "country {k}: Gini {}",
                c.gini[k]
            );
            assert!(
                c.omega[k].iter().all(|&w| w >= 0.0) && c.omega[k].iter().any(|&w| w > 0.0),
                "country {k}: final demand shares {:?}",
                c.omega[k]
            );
            assert!(
                c.io[k].iter().all(|&a| a >= 0.0),
                "country {k}: negative input coefficient"
            );
            for g in 0..N_GOODS {
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
            assert!(usize::from(e.good[k]) < N_GOODS, "enterprise {k}: good");
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
        // destructured so a new field cannot be left out of the digest
        let EconInputs {
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
        } = network;
        let Countries {
            omega,
            io,
            s_l,
            unemployment,
            u_star,
            gini,
            pigl,
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
        u16s(&mut e, province_country);
        u16s(&mut e, gateway_country);
        e.f64s(gateway_mass);
        for legs in [domestic, access, crossings] {
            encode_legs(&mut e, legs);
        }
        e.f64s(weight);
        e.count(wedges.tariff.len());
        wedges.tariff.iter().for_each(|t| e.u32(t.to_bits()));
        e.f64s(omega.as_flattened());
        e.f64s(io.as_flattened());
        e.f64s(s_l);
        e.f64s(unemployment);
        e.f64s(u_star);
        e.f64s(gini);
        u16s(&mut e, country);
        e.f64s(population);
        e.f64s(w0);
        e.f64s(base_price.as_flattened());
        e.f64s(gamma.as_flattened());
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
        e.f64s(coef.as_flattened());
        e.f64s(column.as_flattened());
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
        if let Some(nu) = border_nu {
            b"border-nu".iter().for_each(|&b| e.u8(b));
            e.f64s(nu);
        }
        // appended only when present, so generated worlds keep their digest
        if !pack.is_empty() {
            b"pack-provinces".iter().for_each(|&b| e.u8(b));
            e.u32s(pack);
        }
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
                e.f64s(list.as_flattened());
            }
        }
        // appended only when present, so networks without a bulk freight rate keep their digest
        if let Some(b) = bulk {
            b"bulk-legs".iter().for_each(|&b| e.u8(b));
            b.classes.iter().for_each(|&c| e.u8(u8::from(c)));
            for legs in [&b.domestic, &b.access, &b.crossings] {
                encode_legs(&mut e, legs);
            }
        }
        state_hash(&e.into_bytes())
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
