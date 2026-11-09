//! Inputs of the GE reference model, read from a `ceteris export` directory (export format
//! [`EXPORT_FORMAT`], 15).

use std::fs::File;
use std::ops::{Index, IndexMut};
use std::path::Path;

use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt64Type};
use arrow_array::{Array, RecordBatch};
use arrow_cast::{CastOptions, cast_with_options};
use arrow_ipc::reader::FileReader;
use arrow_schema::DataType;

use crate::demand::{self, Pigl, Weights};

pub const EXPORT_FORMAT: &str = "15";

/// Dense row-major matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct Mat {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f64>,
}

impl Mat {
    pub fn zeros(rows: usize, cols: usize) -> Mat {
        Mat::filled(rows, cols, 0.0)
    }

    pub fn filled(rows: usize, cols: usize, value: f64) -> Mat {
        Mat {
            rows,
            cols,
            data: vec![value; rows * cols],
        }
    }

    pub fn identity(n: usize) -> Mat {
        let mut m = Mat::zeros(n, n);
        for i in 0..n {
            m[(i, i)] = 1.0;
        }
        m
    }

    pub fn from_fn(rows: usize, cols: usize, f: impl Fn(usize, usize) -> f64) -> Mat {
        let mut data = Vec::with_capacity(rows * cols);
        for r in 0..rows {
            for c in 0..cols {
                data.push(f(r, c));
            }
        }
        Mat { rows, cols, data }
    }

    pub fn from_rows(rows: &[Vec<f64>]) -> Mat {
        let cols = rows.first().map_or(0, Vec::len);
        assert!(rows.iter().all(|r| r.len() == cols), "ragged rows");
        Mat {
            rows: rows.len(),
            cols,
            data: rows.concat(),
        }
    }

    pub fn row(&self, r: usize) -> &[f64] {
        &self.data[r * self.cols..(r + 1) * self.cols]
    }

    pub fn row_mut(&mut self, r: usize) -> &mut [f64] {
        &mut self.data[r * self.cols..(r + 1) * self.cols]
    }

    /// The rows `index` of `self`, in that order.
    pub fn select_rows(&self, index: &[usize]) -> Mat {
        let mut data = Vec::with_capacity(index.len() * self.cols);
        for &r in index {
            data.extend_from_slice(self.row(r));
        }
        Mat {
            rows: index.len(),
            cols: self.cols,
            data,
        }
    }

    pub fn transpose(&self) -> Mat {
        Mat::from_fn(self.cols, self.rows, |r, c| self[(c, r)])
    }

    pub fn matmul(&self, other: &Mat) -> Mat {
        assert_eq!(self.cols, other.rows, "matmul shapes");
        let mut out = Mat::zeros(self.rows, other.cols);
        for r in 0..self.rows {
            for k in 0..self.cols {
                let a = self[(r, k)];
                for c in 0..other.cols {
                    out[(r, c)] += a * other[(k, c)];
                }
            }
        }
        out
    }

    pub fn matvec(&self, x: &[f64]) -> Vec<f64> {
        assert_eq!(self.cols, x.len(), "matvec shapes");
        (0..self.rows)
            .map(|r| self.row(r).iter().zip(x).map(|(a, b)| a * b).sum())
            .collect()
    }

    /// Sum of each row, in numpy's `sum(axis=1)` order.
    pub fn row_sums(&self) -> Vec<f64> {
        (0..self.rows).map(|r| pairwise_sum(self.row(r))).collect()
    }
}

/// numpy's pairwise summation of a contiguous run: sequential below 8 values, eight running
/// sums combined as a tree up to 128, halves split at a multiple of 8 beyond. numpy starts the
/// reduction from +0, so a run of -0 sums to +0.
pub fn pairwise_sum(a: &[f64]) -> f64 {
    0.0 + pairwise(a)
}

fn pairwise(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut res = 0.0;
        for v in a {
            res += v;
        }
        res
    } else if n <= 128 {
        let (blocks, rest) = a.as_chunks::<8>();
        let mut r = blocks[0];
        for chunk in &blocks[1..] {
            for (acc, v) in r.iter_mut().zip(chunk) {
                *acc += v;
            }
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        for v in rest {
            res += v;
        }
        res
    } else {
        let half = n / 2;
        let half = half - half % 8;
        pairwise(&a[..half]) + pairwise(&a[half..])
    }
}

impl Index<(usize, usize)> for Mat {
    type Output = f64;
    fn index(&self, (r, c): (usize, usize)) -> &f64 {
        &self.data[r * self.cols + c]
    }
}

impl IndexMut<(usize, usize)> for Mat {
    fn index_mut(&mut self, (r, c): (usize, usize)) -> &mut f64 {
        &mut self.data[r * self.cols + c]
    }
}

/// Domestic or access kernel legs of all traded goods. Domestic: row origin, col destination
/// province. Access: row province, col gateway (origin access read forward, destination access
/// backward). `inv_tau` is the share of shipped units that arrives over the leg.
#[derive(Clone, Debug)]
pub struct Legs {
    pub good: Vec<usize>,
    pub row: Vec<usize>,
    pub col: Vec<usize>,
    pub k: Vec<f64>,
    pub inv_tau: Vec<f64>,
}

/// Border crossings of all traded goods, row and col gateways: `inv_loss` is the share of shipped
/// units that arrives (border loss included), `inv_duty` 1 / (1 + tariff), `lag` transit months.
#[derive(Clone, Debug)]
pub struct Crossings {
    pub good: Vec<usize>,
    pub row: Vec<usize>,
    pub col: Vec<usize>,
    pub k: Vec<f64>,
    pub inv_loss: Vec<f64>,
    pub inv_duty: Vec<f64>,
    pub lag: Vec<usize>,
}

/// Per-country monthly growth rates of the moving reference (ADR 0243): labour force, labour-
/// augmenting productivity and price level. All zero is ADR 0089's steady state.
#[derive(Clone, Debug, PartialEq)]
pub struct Trends {
    pub n: Vec<f64>,
    pub g: Vec<f64>,
    pub pi: Vec<f64>,
}

impl Trends {
    pub fn zero(n_countries: usize) -> Self {
        Self {
            n: vec![0.0; n_countries],
            g: vec![0.0; n_countries],
            pi: vec![0.0; n_countries],
        }
    }

    pub fn is_zero(&self) -> bool {
        [&self.n, &self.g, &self.pi]
            .iter()
            .all(|v| v.iter().all(|&x| x == 0.0))
    }
}

/// G goods, P provinces, C countries, N gateways, E enterprises, K pops.
#[derive(Clone, Debug)]
pub struct Inputs {
    /// (G,) P01 per good times the theta scale P150, as the kernel uses it; 0 for local goods
    /// (ADRs 0018 and 0101).
    pub theta: Vec<f64>,
    /// (G,) P01 theta class of each good (ADR 0023; 0 for local goods).
    pub theta_class: Vec<usize>,
    /// (G,) P05 per good (ADRs 0092, 0276), the border friction the crossings' `k` and
    /// `inv_loss` carry.
    pub border_nu: Vec<f64>,
    /// P06, within a PIGL category.
    pub sigma_demand: f64,
    pub pigl: Pigl,
    /// (G,) PIGL category of each good.
    pub category: Vec<usize>,
    pub sigma_fl: f64,
    pub traded: Vec<bool>,
    pub province_country: Vec<usize>,
    pub gateway_country: Vec<usize>,
    /// (N,) each gateway's share of its country's trade throughput, summing to 1 per country
    /// (ADR 0092); the crossings' `k` carry m_a m_b.
    pub gateway_mass: Vec<f64>,
    /// (P,) effective units, sum_k e_k LF_k.
    pub labour_supply: Vec<f64>,
    /// (P,) sum of the province's pops, rebate weights.
    pub population: Vec<f64>,
    pub w0: Vec<f64>,
    /// (P, G) base delivered unit price q_d (inf where unreachable); for local goods the
    /// country's price level p_c (ADR 0098), the price the data state starts them at.
    pub base_price: Mat,
    /// (P, G) selection constant Gamma of ADR 0200: buyers pay Gamma P_d. `None` prices them
    /// at the delivered unit price of the physical arrivals, 1 / alpha_d, as the calibration
    /// does at every pass (it then sets Gamma = q_d / P_d at the calibrated point).
    pub gamma: Option<Mat>,
    pub s_l: Vec<f64>,
    /// (C, G) labour share of each good (ADR 0104; the export's `s_l_g`); no rows where every
    /// sector takes `s_l`.
    pub s_l_good: Mat,
    /// (C, G) foreign-owned share of each good's profits and (C, C) each host's owners (ADRs
    /// 0309, 0310; the export's `foreign_g`, `owner_c`); no rows without foreign owners.
    pub foreign: Mat,
    pub owners: Mat,
    /// (C,) 2000 rate (data).
    pub unemployment: Vec<f64>,
    /// (C,) natural rate (ADR 0089): labour clears at (1 - u*) L^s.
    pub u_star: Vec<f64>,
    /// (C,) monthly trends of ADR 0243 (the export's `trend_n`, `trend_g`, `trend_pi`; zeros when
    /// the export has none).
    pub trends: Trends,
    pub gini: Vec<f64>,
    /// (C,) transfer share s_c of ADR 0100: households of country c receive s_c times world
    /// income (`countries.transfer`, 0 where the export has none).
    pub transfer: Vec<f64>,
    /// (C, G)
    pub omega: Mat,
    /// (C, G) government's and investment's parts of `omega` (ADR 0099, RFC 0301; the export's
    /// `gov_g`, `inv_g`); no rows where households buy all final demand.
    pub gov: Mat,
    pub inv: Mat,
    /// Per country (G, G): `io[c][(k, g)]` = a_kg.
    pub io: Vec<Mat>,
    /// (P, G): origin markets of traded goods, province markets of local goods.
    pub active: Vec<Vec<bool>>,
    pub ent_province: Vec<usize>,
    pub ent_good: Vec<usize>,
    pub ent_a: Vec<f64>,
    pub ent_va0: Vec<f64>,
    pub ent_l0: Vec<f64>,
    pub pop_province: Vec<usize>,
    /// (K,) e_k LF_k: the pop's share of its province's wages and dividends.
    pub pop_effective: Vec<f64>,
    pub pop_population: Vec<f64>,
    /// (K,) share of its country's profit pool (the export's `dividend`, by wealth, ADR 0104);
    /// empty where it is the population share, [`Inputs::dividend_shares`].
    pub pop_dividend: Vec<f64>,
    pub domestic: Legs,
    pub access: Legs,
    pub crossing: Crossings,
    /// (E,) value added the calibration matches.
    pub ent_va_target: Option<Vec<f64>>,
    /// (G,) P02 merchant stock target in months of orders.
    pub coverage: Option<Vec<f64>>,
    /// (G,) P56 storage loss delta per month of the competitively stored goods (ADR 0091), 0
    /// elsewhere: at a steady state their origins' output covers sales and spoilage.
    pub storage_loss: Vec<f64>,
    /// The other storage parameters of the export (ADR 0091): they move only the dynamics, which
    /// the calibration's block map linearizes.
    pub storage: Storage,
    /// (K,) education level.
    pub pop_edu: Option<Vec<usize>>,
    pub static_digest: String,
    /// A calibrated steady state replaces the base column (E, G), its a_VA and the PIGL weights
    /// inverted at the data state's prices.
    pub calibrated_coef: Option<Mat>,
    pub calibrated_a_va: Option<Vec<f64>>,
    pub calibrated_weights: Option<Weights>,
}

/// Competitive storage's parameters besides P56 delta (ADR 0091), per good: P56 money cost k,
/// P144 psi, P145 zeta, and P146 g_s; empty where the inputs carry none.
#[derive(Clone, Debug, Default)]
pub struct Storage {
    pub cost: Vec<f64>,
    pub speculation: Vec<f64>,
    pub convenience: Vec<f64>,
    pub gain: f64,
}

fn bincount(index: &[usize], weights: &[f64], n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n];
    for (&i, &w) in index.iter().zip(weights) {
        out[i] += w;
    }
    out
}

impl Inputs {
    pub fn n_provinces(&self) -> usize {
        self.province_country.len()
    }

    pub fn n_goods(&self) -> usize {
        self.traded.len()
    }

    pub fn n_countries(&self) -> usize {
        self.s_l.len()
    }

    pub fn n_gateways(&self) -> usize {
        self.gateway_country.len()
    }

    pub fn ent_country(&self) -> Vec<usize> {
        self.ent_province
            .iter()
            .map(|&p| self.province_country[p])
            .collect()
    }

    /// (P, G) prices of the data state, as `EconInputs::data_state_price`: 1 for traded goods,
    /// the country's price level p_c for local goods (their base price, ADR 0098).
    pub fn data_state_price(&self) -> Mat {
        Mat::from_fn(self.n_provinces(), self.n_goods(), |p, g| {
            if self.traded[g] {
                1.0
            } else {
                self.base_price[(p, g)]
            }
        })
    }

    /// (E,) p_o, the data state's price of each enterprise's output at its province.
    pub fn ent_base_price(&self) -> Vec<f64> {
        let price = self.data_state_price();
        (0..self.ent_good.len())
            .map(|e| price[(self.ent_province[e], self.ent_good[e])])
            .collect()
    }

    /// (E, G): physical input coefficients, as `EconInputs::base_column`: the sector's ICIO cost
    /// share a_kg times p_o over the base delivered price of k at the enterprise's province, 0
    /// where the sector does not use k.
    pub fn ent_inputs(&self) -> Mat {
        if let Some(coef) = &self.calibrated_coef {
            return coef.clone();
        }
        let country = self.ent_country();
        let own = self.ent_base_price();
        Mat::from_fn(self.ent_good.len(), self.n_goods(), |e, k| {
            let a = self.io[country[e]][(k, self.ent_good[e])];
            if a > 0.0 {
                a * own[e] / self.base_price[(self.ent_province[e], k)]
            } else {
                0.0
            }
        })
    }

    /// 1 - sum_k coef_k q_k / p_o at base prices, as `EconInputs::base_a_va` (1 - sum_k a_kg).
    pub fn ent_a_va(&self) -> Vec<f64> {
        if let Some(a_va) = &self.calibrated_a_va {
            return a_va.clone();
        }
        let (coef, own) = (self.ent_inputs(), self.ent_base_price());
        (0..coef.rows)
            .map(|e| {
                let mut cost = 0.0;
                for (k, &c) in coef.row(e).iter().enumerate() {
                    if c > 0.0 {
                        cost += c * self.base_price[(self.ent_province[e], k)];
                    }
                }
                1.0 - cost / own[e]
            })
            .collect()
    }

    /// (E,) labour share of each enterprise: its country's for its good (ADR 0104), else
    /// national.
    pub fn ent_s_l(&self) -> Vec<f64> {
        let c = self.ent_country();
        (0..c.len())
            .map(|e| {
                if self.s_l_good.rows == 0 {
                    self.s_l[c[e]]
                } else {
                    self.s_l_good[(c[e], self.ent_good[e])]
                }
            })
            .collect()
    }

    /// (C,) each country's profit pool from each enterprise's profit `profit` (E,): the
    /// domestic part (1 - f) of its own enterprises' and its owner share of every host's
    /// foreign-owned part f (ADR 0310), as `world::base_profit_pool`.
    pub fn profit_pool(&self, profit: &[f64]) -> Vec<f64> {
        let c = self.ent_country();
        let n_c = self.n_countries();
        let mut pool = vec![0.0; n_c];
        if self.foreign.rows == 0 {
            for (e, &x) in profit.iter().enumerate() {
                pool[c[e]] += x;
            }
            return pool;
        }
        let mut abroad = vec![0.0; n_c];
        for (e, &x) in profit.iter().enumerate() {
            let f = self.foreign[(c[e], self.ent_good[e])];
            pool[c[e]] += (1.0 - f) * x;
            abroad[c[e]] += f * x;
        }
        for (h, &paid) in abroad.iter().enumerate().filter(|(_, x)| **x > 0.0) {
            for (o, slot) in pool.iter_mut().enumerate() {
                *slot += self.owners[(h, o)] * paid;
            }
        }
        pool
    }

    /// (P, G) final demand shares of each province's country.
    pub fn omega_p(&self) -> Mat {
        self.omega.select_rows(&self.province_country)
    }

    /// Flat indices `province * G + good` of active cells, row-major: the price unknowns.
    pub fn active_cells(&self) -> Vec<usize> {
        let g = self.n_goods();
        self.active
            .iter()
            .enumerate()
            .flat_map(|(p, row)| {
                row.iter()
                    .enumerate()
                    .filter(|&(_, &on)| on)
                    .map(move |(j, _)| p * g + j)
            })
            .collect()
    }

    /// (P,) effective labour employed at the natural rate, (1 - u*) L^s: the frictionless wage
    /// clears labour there (ADR 0089, as Rust's E1).
    pub fn employed_supply(&self) -> Vec<f64> {
        self.province_country
            .iter()
            .zip(&self.labour_supply)
            .map(|(&c, &l)| (1.0 - self.u_star[c]) * l)
            .collect()
    }

    /// Household income per province at the base point, rebates aside: w0 (1 - u*) L^s wages
    /// plus its pops' dividend shares of their country's profit pool sum (1 - s_L) VA0
    /// (national ownership; by population, or by wealth, ADR 0104), as `world::base_income`.
    pub fn base_income(&self) -> Vec<f64> {
        let c = &self.province_country;
        let n_c = self.n_countries();
        let profit: Vec<f64> = self
            .ent_s_l()
            .iter()
            .zip(&self.ent_va0)
            .map(|(s, va)| (1.0 - s) * va)
            .collect();
        let pool = self.profit_pool(&profit);
        let share = if self.pop_dividend.is_empty() {
            let people = bincount(c, &self.population, n_c);
            (0..self.n_provinces())
                .map(|p| {
                    if people[c[p]] > 0.0 {
                        self.population[p] / people[c[p]]
                    } else {
                        0.0
                    }
                })
                .collect()
        } else {
            bincount(&self.pop_province, &self.pop_dividend, self.n_provinces())
        };
        (0..self.n_provinces())
            .map(|p| {
                self.w0[p] * (1.0 - self.u_star[c[p]]) * self.labour_supply[p]
                    + share[p] * pool[c[p]]
            })
            .collect()
    }

    /// (C,) base spending per person: base income over the pops' population, 0 for a country
    /// without people, as `world::base_spend`.
    pub fn base_spend(&self) -> Vec<f64> {
        let c = &self.province_country;
        let n_c = self.n_countries();
        let income = bincount(c, &self.base_income(), n_c);
        let pop_c: Vec<usize> = self.pop_province.iter().map(|&p| c[p]).collect();
        let people = bincount(&pop_c, &self.pop_population, n_c);
        income
            .iter()
            .zip(&people)
            .map(|(&i, &n)| if n > 0.0 { i / n } else { 0.0 })
            .collect()
    }

    /// (K,) each pop's share of its country's profit pool: `pop_dividend`, else its population
    /// share (0 in a country without people).
    pub fn dividend_shares(&self) -> Vec<f64> {
        if !self.pop_dividend.is_empty() {
            return self.pop_dividend.clone();
        }
        let country = self.pop_country();
        let people = bincount(&country, &self.pop_population, self.n_countries());
        (0..country.len())
            .map(|k| {
                let n = people[country[k]];
                if n > 0.0 {
                    self.pop_population[k] / n
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// (K,) country of each pop.
    pub fn pop_country(&self) -> Vec<usize> {
        self.pop_province
            .iter()
            .map(|&p| self.province_country[p])
            .collect()
    }

    /// (K,) base income per person of each pop, rebates aside (ADR 0094): w0 (1 - u*) times
    /// its effective labour, plus its dividend share of its country's profit pool sum (1 - s_L)
    /// VA0, over its population; 0 without people.
    pub fn pop_base_income(&self) -> Vec<f64> {
        let country = self.pop_country();
        let profit: Vec<f64> = self
            .ent_s_l()
            .iter()
            .zip(&self.ent_va0)
            .map(|(s, va)| (1.0 - s) * va)
            .collect();
        let pool = self.profit_pool(&profit);
        let dividend = self.dividend_shares();
        (0..country.len())
            .map(|k| {
                let (p, c, n) = (self.pop_province[k], country[k], self.pop_population[k]);
                if n > 0.0 {
                    let wage = self.w0[p] * (1.0 - self.u_star[c]) * self.pop_effective[k];
                    (wage + dividend[k] * pool[c]) / n
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// (C,) within-pop log income dispersion s_w (ADR 0094), from the Gini and the pops' base
    /// incomes.
    pub fn within_sigma(&self) -> Vec<f64> {
        demand::within_sigma(
            &self.gini,
            &self.pop_country(),
            &self.pop_population,
            &self.pop_base_income(),
        )
    }

    /// PIGL weights per country, as `world::household_preferences`: the calibrated ones, else
    /// inverted over the country's pops at the data state's prices and their base incomes
    /// (ADRs 0094 and 0098).
    pub fn pigl_weights(&self) -> Result<Weights, String> {
        if let Some(w) = &self.calibrated_weights {
            return Ok(w.clone());
        }
        let country = self.pop_country();
        let income = self.pop_base_income();
        let spend: Vec<f64> = income
            .iter()
            .zip(&self.pop_population)
            .map(|(m, n)| m * n)
            .collect();
        let price = self.data_state_price().select_rows(&self.pop_province);
        let (household, gov, inv) =
            self.final_demand_split(&Mat::filled(self.omega.rows, self.omega.cols, 1.0));
        let h = fixed_household_share(&gov, &inv, self.omega.rows);
        let income: Vec<f64> = income
            .iter()
            .zip(&country)
            .map(|(m, &c)| h[c] * m)
            .collect();
        let spend: Vec<f64> = spend.iter().zip(&country).map(|(m, &c)| h[c] * m).collect();
        let mut w = demand::invert_pops(
            &household,
            &demand::Pops {
                country: &country,
                price: &price,
                income: &income,
                spend: &spend,
            },
            &self.within_sigma(),
            &self.category,
            self.sigma_demand,
            &self.pigl,
        )?;
        (w.gov, w.inv) = (gov, inv);
        Ok(w)
    }

    /// The final demand split over its demanders (ADR 0099) with each country and good's final
    /// demand scaled by `factor` (C, G) (stage 1's balance, ADR 0318; 1 at the data):
    /// households' part of the scaled shares (C, G), and government's and investment's spending
    /// shares, their scaled parts over the country's scaled total (no rows without the split).
    pub fn final_demand_split(&self, factor: &Mat) -> (Mat, Mat, Mat) {
        let (n_c, n_g) = (self.omega.rows, self.omega.cols);
        let scaled = |m: &Mat| Mat::from_fn(n_c, n_g, |c, g| m[(c, g)] * factor[(c, g)]);
        let total = scaled(&self.omega);
        if self.gov.rows == 0 {
            return (total, Mat::zeros(0, n_g), Mat::zeros(0, n_g));
        }
        let (gov, inv) = (scaled(&self.gov), scaled(&self.inv));
        let sum = total.row_sums();
        let household = Mat::from_fn(n_c, n_g, |c, g| {
            (total[(c, g)] - gov[(c, g)] - inv[(c, g)]).max(0.0)
        });
        let per = |m: &Mat| Mat::from_fn(n_c, n_g, |c, g| m[(c, g)] / sum[c]);
        (household, per(&gov), per(&inv))
    }

    /// (P, G) base household consumption quantities at prices 1: weights of the world price index.
    pub fn index_weights(&self) -> Mat {
        let mut om = self.omega_p();
        let total = om.row_sums();
        let income = self.base_income();
        for p in 0..om.rows {
            for v in om.row_mut(p) {
                *v = *v / total[p] * income[p];
            }
        }
        om
    }

    fn base_y0(&self) -> Vec<f64> {
        let a_va = self.ent_a_va();
        (0..self.ent_a.len())
            .map(|e| self.ent_a[e] * self.ent_va0[e] / a_va[e])
            .collect()
    }

    /// (P, G) base output Y0 = A VA0 / a_VA of each cell's enterprises.
    pub fn base_output(&self) -> Mat {
        let mut out = Mat::zeros(self.n_provinces(), self.n_goods());
        for (e, y) in self.base_y0().into_iter().enumerate() {
            out[(self.ent_province[e], self.ent_good[e])] += y;
        }
        out
    }

    /// (P, G) households' base demand plus one month of base input use (Y0 = A VA0 / a_VA).
    pub fn base_demand(&self) -> Mat {
        let coef = self.ent_inputs();
        let mut out = self.index_weights();
        for (e, y) in self.base_y0().into_iter().enumerate() {
            for (o, c) in out
                .row_mut(self.ent_province[e])
                .iter_mut()
                .zip(coef.row(e))
            {
                *o += c * y;
            }
        }
        out
    }
}

struct Table {
    name: String,
    batches: Vec<RecordBatch>,
}

impl Table {
    fn open(dir: &Path, name: &str) -> Result<Table, String> {
        let path = dir.join(format!("{name}.arrow"));
        let file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let reader = FileReader::try_new(file, None).map_err(|e| format!("{name}: {e}"))?;
        let batches = reader
            .collect::<Result<_, _>>()
            .map_err(|e| format!("{name}: {e}"))?;
        Ok(Table {
            name: name.into(),
            batches,
        })
    }

    fn column(&self, col: &str, to: &DataType) -> Result<Vec<arrow_array::ArrayRef>, String> {
        self.batches
            .iter()
            .map(|b| {
                let a = b
                    .column_by_name(col)
                    .ok_or_else(|| format!("{}: no column {col}", self.name))?;
                if a.null_count() > 0 {
                    return Err(format!("{}.{col}: nulls", self.name));
                }
                // A safe cast turns a value out of the target's range into a null.
                let options = CastOptions {
                    safe: false,
                    ..CastOptions::default()
                };
                cast_with_options(a, to, &options).map_err(|e| format!("{}.{col}: {e}", self.name))
            })
            .collect()
    }

    /// Whether every batch has column `col`.
    fn has(&self, col: &str) -> bool {
        self.batches.iter().all(|b| b.column_by_name(col).is_some())
    }

    fn f64(&self, col: &str) -> Result<Vec<f64>, String> {
        Ok(self
            .column(col, &DataType::Float64)?
            .iter()
            .flat_map(|a| a.as_primitive::<Float64Type>().values().to_vec())
            .collect())
    }

    fn usize(&self, col: &str) -> Result<Vec<usize>, String> {
        self.column(col, &DataType::UInt64)?
            .iter()
            .flat_map(|a| a.as_primitive::<UInt64Type>().values().to_vec())
            .map(|v| usize::try_from(v).map_err(|e| format!("{}.{col}: {e}", self.name)))
            .collect()
    }

    fn bool(&self, col: &str) -> Result<Vec<bool>, String> {
        Ok(self
            .column(col, &DataType::Boolean)?
            .iter()
            .flat_map(|a| a.as_boolean().values().iter().collect::<Vec<_>>())
            .collect())
    }
}

fn legs(t: &Table, row: &str, col: &str) -> Result<Legs, String> {
    Ok(Legs {
        good: t.usize("good")?,
        row: t.usize(row)?,
        col: t.usize(col)?,
        k: t.f64("k")?,
        inv_tau: t.f64("inv_tau")?,
    })
}

fn crossings(t: &Table) -> Result<Crossings, String> {
    Ok(Crossings {
        good: t.usize("good")?,
        row: t.usize("a")?,
        col: t.usize("b")?,
        k: t.f64("k")?,
        inv_loss: t.f64("inv_loss")?,
        inv_duty: t.f64("inv_duty")?,
        lag: t.usize("lag")?,
    })
}

fn json_f64(v: &serde_json::Value, key: &str) -> Result<f64, String> {
    v[key]
        .as_f64()
        .ok_or_else(|| format!("params.json: econ.{key} is not a number"))
}

fn json_f64s(v: &serde_json::Value, key: &str) -> Result<Vec<f64>, String> {
    v[key]
        .as_array()
        .and_then(|a| a.iter().map(serde_json::Value::as_f64).collect())
        .ok_or_else(|| format!("params.json: econ.{key} is not a list of numbers"))
}

/// Reads and validates an export directory.
pub fn load(dir: impl AsRef<Path>) -> Result<Inputs, String> {
    let d = dir.as_ref();
    let path = d.join("params.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let params: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("params.json: {e}"))?;
    let format = params["format"].as_str().unwrap_or_default();
    if format != EXPORT_FORMAT {
        return Err(format!("export format {format}, expected {EXPORT_FORMAT}"));
    }
    let econ = &params["econ"];
    let goods = Table::open(d, "goods")?;
    let countries = Table::open(d, "countries")?;
    let provinces = Table::open(d, "provinces")?;
    let pops = Table::open(d, "pops")?;
    let ent = Table::open(d, "enterprises")?;
    let active = Table::open(d, "active")?;
    let io = Table::open(d, "country_io")?;
    let gateways = Table::open(d, "gateways")?;

    let n_goods = params["n_goods"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("params.json: n_goods is not a count")?;
    let n_c = countries.usize("country")?.len();
    let province_country = provinces.usize("country")?;
    let n_p = province_country.len();

    let mut a = vec![Mat::zeros(n_goods, n_goods); n_c];
    for (((c, k), g), v) in io
        .usize("country")?
        .into_iter()
        .zip(io.usize("input")?)
        .zip(io.usize("sector")?)
        .zip(io.f64("a")?)
    {
        a[c][(k, g)] = v;
    }
    let mut act = vec![vec![false; n_goods]; n_p];
    for ((p, g), on) in active
        .usize("province")?
        .into_iter()
        .zip(active.usize("good")?)
        .zip(active.bool("active")?)
    {
        act[p][g] = on;
    }
    let edu_eff = json_f64s(econ, "edu_eff")?;
    let pop_province = pops.usize("province")?;
    let pop_edu = pops.usize("edu")?;
    let pop_effective: Vec<f64> = pop_edu
        .iter()
        .zip(pops.f64("labour_force")?)
        .map(|(&e, lf)| edu_eff[e] * lf)
        .collect();
    let pop_population = pops.f64("population")?;
    let pop_dividend = if pops.has("dividend") {
        pops.f64("dividend")?
    } else {
        Vec::new()
    };
    let columns = |t: &Table, prefix: &str, rows: usize| -> Result<Mat, String> {
        let cols = (0..n_goods)
            .map(|g| t.f64(&format!("{prefix}_{g}")))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Mat::from_fn(rows, n_goods, |r, g| cols[g][r]))
    };
    let pigl_theta = json_f64s(econ, "pigl_theta")?;
    let pigl_theta: [f64; demand::N_CATEGORIES] = pigl_theta
        .try_into()
        .map_err(|_| "params.json: econ.pigl_theta needs one value per PIGL category")?;
    let inp = Inputs {
        theta: {
            let scale = json_f64(&params["trade"], "theta_scale")?;
            goods.f64("theta")?.into_iter().map(|t| t * scale).collect()
        },
        theta_class: goods.usize("theta_class")?,
        border_nu: json_f64s(&params["trade"], "border_nu")?,
        sigma_demand: json_f64(econ, "sigma_demand")?,
        pigl: Pigl {
            sigma_b: json_f64(econ, "pigl_sigma_b")?,
            phi: json_f64(econ, "pigl_phi")?,
            gamma: json_f64(econ, "pigl_gamma")?,
            theta: pigl_theta,
            eps: json_f64(econ, "pigl_eps")?,
        },
        category: goods.usize("category")?,
        sigma_fl: json_f64(econ, "sigma_fl")?,
        traded: goods.bool("traded")?,
        gateway_country: gateways.usize("country")?,
        gateway_mass: gateways.f64("mass")?,
        labour_supply: bincount(&pop_province, &pop_effective, n_p),
        population: bincount(&pop_province, &pop_population, n_p),
        w0: provinces.f64("w0")?,
        base_price: columns(&provinces, "base_price", n_p)?,
        gamma: Some(columns(&provinces, "gamma", n_p)?),
        s_l: countries.f64("s_l")?,
        s_l_good: if countries.has("s_l_0") {
            columns(&countries, "s_l", n_c)?
        } else {
            Mat::zeros(0, n_goods)
        },
        foreign: if countries.has("foreign_0") {
            columns(&countries, "foreign", n_c)?
        } else {
            Mat::zeros(0, n_goods)
        },
        owners: if countries.has("foreign_0") {
            let cols = (0..n_c)
                .map(|o| countries.f64(&format!("owner_{o}")))
                .collect::<Result<Vec<_>, _>>()?;
            Mat::from_fn(n_c, n_c, |h, o| cols[o][h])
        } else {
            Mat::zeros(0, n_c)
        },
        unemployment: countries.f64("unemployment")?,
        u_star: countries.f64("u_star")?,
        trends: Trends {
            n: if countries.has("trend_n") {
                countries.f64("trend_n")?
            } else {
                vec![0.0; n_c]
            },
            g: if countries.has("trend_g") {
                countries.f64("trend_g")?
            } else {
                vec![0.0; n_c]
            },
            pi: if countries.has("trend_pi") {
                countries.f64("trend_pi")?
            } else {
                vec![0.0; n_c]
            },
        },
        gini: countries.f64("gini")?,
        transfer: if countries.has("transfer") {
            countries.f64("transfer")?
        } else {
            vec![0.0; n_c]
        },
        omega: columns(&countries, "omega", n_c)?,
        gov: if countries.has("gov_0") {
            columns(&countries, "gov", n_c)?
        } else {
            Mat::zeros(0, n_goods)
        },
        inv: if countries.has("inv_0") {
            columns(&countries, "inv", n_c)?
        } else {
            Mat::zeros(0, n_goods)
        },
        io: a,
        active: act,
        ent_province: ent.usize("province")?,
        ent_good: ent.usize("good")?,
        ent_a: ent.f64("a")?,
        ent_va0: ent.f64("va0")?,
        ent_l0: ent.f64("l0")?,
        pop_province,
        pop_effective,
        pop_population,
        pop_dividend,
        domestic: legs(&Table::open(d, "kernel_domestic")?, "o", "d")?,
        access: legs(&Table::open(d, "kernel_access")?, "province", "gateway")?,
        crossing: crossings(&Table::open(d, "kernel_crossing")?)?,
        ent_va_target: Some(ent.f64("va_target")?),
        coverage: Some(goods.f64("coverage")?),
        storage_loss: goods.f64("storage_loss")?,
        storage: Storage {
            cost: json_f64s(&params["trade"], "storage_cost")?,
            speculation: json_f64s(&params["trade"], "speculation")?,
            convenience: json_f64s(&params["trade"], "convenience")?,
            gain: json_f64(&params["trade"], "storage_gain")?,
        },
        pop_edu: Some(pop_edu),
        static_digest: params["static_digest"]
            .as_str()
            .ok_or("params.json: static_digest is not a string")?
            .into(),
        province_country,
        calibrated_coef: None,
        calibrated_a_va: None,
        calibrated_weights: None,
    };
    validate(&inp)?;
    Ok(inp)
}

/// Rejects inputs whose Rust steady state the model does not encode.
pub fn validate(inp: &Inputs) -> Result<(), String> {
    let mut stuck = Vec::new();
    for (&p, &g) in inp.ent_province.iter().zip(&inp.ent_good) {
        // Rust keeps such a cell's price at its initial level, which is not homogeneous in
        // prices.
        if !inp.traded[g] && !inp.active[p][g] && !stuck.contains(&[p, g]) {
            stuck.push([p, g]);
        }
    }
    if !stuck.is_empty() {
        stuck.sort_unstable();
        return Err(format!(
            "inactive local cells with an enterprise: {stuck:?}"
        ));
    }
    // statrs' erf_inv gives an infinite income dispersion outside [0, 1) where scipy's gives NaN.
    let gini: Vec<usize> = (0..inp.gini.len())
        .filter(|&c| !(0.0..1.0).contains(&inp.gini[c]))
        .collect();
    if !gini.is_empty() {
        return Err(format!("Gini outside [0, 1) in countries {gini:?}"));
    }
    if inp.border_nu.len() != inp.traded.len() {
        return Err(format!(
            "{} border frictions for {} goods",
            inp.border_nu.len(),
            inp.traded.len()
        ));
    }
    if inp.gateway_mass.len() != inp.gateway_country.len() {
        return Err(format!(
            "{} gateway masses for {} gateways",
            inp.gateway_mass.len(),
            inp.gateway_country.len()
        ));
    }
    let mut mass = vec![0.0; inp.n_countries()];
    for (&c, &m) in inp.gateway_country.iter().zip(&inp.gateway_mass) {
        if !(m > 0.0 && m <= 1.0) {
            return Err(format!("a gateway of country {c} has mass {m}"));
        }
        mass[c] += m;
    }
    if let Some(c) = (0..mass.len()).find(|&c| mass[c] != 0.0 && (mass[c] - 1.0).abs() > 1e-12) {
        return Err(format!("gateway masses of country {c} sum to {}", mass[c]));
    }
    if inp.ent_a_va().iter().any(|&a| a <= 0.0) {
        return Err("an enterprise has a nonpositive value-added coefficient".into());
    }
    inp.pigl_weights().map(|_| ())
}

/// (C,) households' share of spending at government's and investment's spending shares `gov`,
/// `inv` (no rows without the split: 1), as `demand::Weights::household_share`.
pub fn fixed_household_share(gov: &Mat, inv: &Mat, n_c: usize) -> Vec<f64> {
    if gov.rows == 0 {
        return vec![1.0; n_c];
    }
    let (g, i) = (gov.row_sums(), inv.row_sums());
    g.iter().zip(&i).map(|(g, i)| 1.0 - g - i).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow_array::{ArrayRef, Int32Array, RecordBatch};

    use super::Table;

    fn table(values: Vec<i32>) -> Table {
        let column: ArrayRef = Arc::new(Int32Array::from(values));
        Table {
            name: "t".into(),
            batches: vec![RecordBatch::try_from_iter([("index", column)]).unwrap()],
        }
    }

    /// `numpy.sum` of `[1.0] + [1.1e-16] * (n - 1)` (order-sensitive: a sequential sum stays 1)
    /// and of `[1 / (i + 1) for i in range(n)]`, printed with `repr`.
    const NUMPY_SUMS: [(usize, f64, f64); 9] = [
        (3, 1.0, 1.8333333333333333),
        (8, 1.0000000000000007, 2.7178571428571425),
        (9, 1.0000000000000007, 2.8289682539682537),
        (10, 1.0000000000000007, 2.9289682539682538),
        (16, 1.0000000000000016, 3.3807289932289937),
        (17, 1.0000000000000016, 3.439552522640758),
        (128, 1.0000000000000124, 5.433147092589173),
        (129, 1.0000000000000133, 5.440899030573669),
        (300, 1.000000000000032, 6.282663880299504),
    ];

    #[test]
    fn pairwise_sum_matches_numpy() {
        for (n, tiny, harmonic) in NUMPY_SUMS {
            let mut a = vec![1.1e-16; n];
            a[0] = 1.0;
            assert_eq!(super::pairwise_sum(&a), tiny, "n = {n}");
            let h: Vec<f64> = (0..n).map(|i| 1.0 / (i + 1) as f64).collect();
            assert_eq!(super::pairwise_sum(&h), harmonic, "n = {n}");
            let zero = super::pairwise_sum(&vec![-0.0; n]);
            assert!(zero == 0.0 && zero.is_sign_positive(), "n = {n}");
        }
    }

    #[test]
    fn negative_index_is_an_error() {
        assert_eq!(table(vec![0, 3]).usize("index").unwrap(), [0, 3]);
        let err = table(vec![2, -1]).usize("index").unwrap_err();
        assert!(err.starts_with("t.index: "), "{err}");
    }
}
