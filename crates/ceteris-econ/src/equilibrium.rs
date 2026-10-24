//! Starting a run from the GE reference equilibrium (`python -m ceteris.reference solve`):
//! the solution's prices, wages, employment and output, with every stock and pop balance at
//! its steady-state value, so that a frictionless run starting there stays there.

use std::fs::{self, File};
use std::path::Path;

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::Float64Type;
use arrow_ipc::reader::FileReader;
use std::sync::Arc;

use ceteris_trade::goods::TRADED;
use ceteris_trade::source::{ARRIVAL_SLOTS, HOUSEHOLD, INTERMEDIATE};
use ceteris_trade::{N_GOODS, Sourced, TradeParams, dispatch, source};

use crate::export::ExportError;
use crate::inputs::EconInputs;
use crate::params::EconParams;
use crate::systems::COVER;
use crate::world::{EconWorld, MINT, static_digest, to_minor};

/// Relative change of every pop's income below which the steady-state income iteration
/// (tariff rebates and market cash depend on spending) stops.
const INCOME_TOLERANCE: f64 = 1e-15;
const INCOME_ITERATIONS: usize = 100;

/// A reference solution as written by `python -m ceteris.reference solve --out DIR`.
#[derive(Clone, Debug, PartialEq)]
pub struct Equilibrium {
    /// `[province * N_GOODS + g]`: posted price of active cells (origin markets of traded
    /// goods, province markets of local goods), 0 elsewhere.
    pub price: Vec<f64>,
    pub wage: Vec<f64>,
    pub employment: Vec<f64>,
    pub output: Vec<f64>,
    /// `static_digest` of the export the solution was solved from.
    pub static_digest: String,
}

fn read_batch(path: &Path) -> Result<RecordBatch, ExportError> {
    let reader = FileReader::try_new(File::open(path)?, None)?;
    let mut batches = reader.collect::<Result<Vec<_>, _>>()?;
    match batches.len() {
        1 => Ok(batches.remove(0)),
        n => Err(ExportError(format!(
            "{}: {n} record batches",
            path.display()
        ))),
    }
}

fn f64_column(batch: &RecordBatch, name: &str) -> Result<Vec<f64>, ExportError> {
    let column = batch
        .column_by_name(name)
        .ok_or_else(|| ExportError(format!("missing column {name}")))?;
    let values = column
        .as_primitive_opt::<Float64Type>()
        .ok_or_else(|| ExportError(format!("column {name} is not f64")))?;
    Ok(values.values().to_vec())
}

impl Equilibrium {
    /// Reads `prices`, `wages`, `enterprises` and the static digest of `solution.json`.
    pub fn read(dir: &Path) -> Result<Equilibrium, ExportError> {
        let table = |name: &str| read_batch(&dir.join(format!("{name}.arrow")));
        let json = fs::read_to_string(dir.join("solution.json"))?;
        let key = "\"static_digest\": \"";
        let start = json
            .find(key)
            .ok_or_else(|| ExportError("solution.json has no static_digest".into()))?
            + key.len();
        let static_digest = json[start..]
            .split('"')
            .next()
            .unwrap_or_default()
            .to_string();
        let enterprises = table("enterprises")?;
        Ok(Equilibrium {
            price: f64_column(&table("prices")?, "price")?,
            wage: f64_column(&table("wages")?, "wage")?,
            employment: f64_column(&enterprises, "employment")?,
            output: f64_column(&enterprises, "output")?,
            static_digest,
        })
    }
}

fn hex(hash: &[u8; 32]) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

/// Sourcing and full-fill dispatch of `spend` for traded good `g` at the world's prices.
fn ship(w: &EconWorld, g: usize, spend: &[[f64; 2]], out: &mut Sourced) {
    let np = w.inputs.n_provinces();
    let cells = || (0..np).map(|o| o * N_GOODS + g);
    let price: Vec<f64> = cells().map(|c| w.markets.price[c]).collect();
    let active: Vec<bool> = cells().map(|c| w.markets.active[c]).collect();
    source(&w.kernel, g, &price, &active, spend, out);
    dispatch(&w.kernel, g, &price, &vec![[1.0; 2]; np], out);
}

impl EconWorld {
    /// [`EconWorld::new`] moved to the steady state of `solution` by [`EconWorld::settle`].
    pub fn from_equilibrium(
        inputs: Arc<EconInputs>,
        params: EconParams,
        trade: TradeParams,
        solution: &Equilibrium,
    ) -> EconWorld {
        let mut w = EconWorld::new(inputs, params, trade);
        w.settle(solution);
        w
    }

    /// Moves a world fresh from [`EconWorld::new`] (crossing lags may have been set) to the
    /// steady state of `solution`, at the start of a tick: its prices (cells without a market
    /// keep price 1), wages, employment and output; the price index at those prices and the
    /// world price level, with no expected inflation; every buyer's pipeline holding the
    /// shipments of the last ticks; traded input stocks such that this tick's orders (up to
    /// the inventory position of E3) replace exactly one month of use, bought at the delivered
    /// unit price, a Y q_d, and local input stocks at their target, which E2 leaves untouched
    /// (the same-tick chain draws local use from output); origin inventories at coverage of output; and each pop
    /// holding last tick's dividends and rebates, so that with this tick's wages it spends
    /// exactly its steady-state income (found by iterating, since rebates and market cash
    /// depend on spending).
    ///
    /// The solution must come from the export of these inputs with these structural
    /// parameters (adjustment speeds, the frictionless and rationing switches are free);
    /// panics otherwise.
    pub fn settle(&mut self, solution: &Equilibrium) {
        let structural = static_digest(
            &self.inputs,
            &EconParams {
                sigma_demand: self.params.sigma_demand,
                pigl: self.params.pigl.clone(),
                sigma_fl: self.params.sigma_fl,
                edu_eff: self.params.edu_eff,
                ..EconParams::default()
            },
            &TradeParams {
                theta: self.trade.theta,
                coverage: self.trade.coverage,
                border_nu: self.trade.border_nu,
                time_cost: self.trade.time_cost,
                ..TradeParams::default()
            },
        );
        assert_eq!(
            solution.static_digest,
            hex(&structural),
            "the solution belongs to other inputs or parameters"
        );
        let (inp, lay) = (Arc::clone(&self.inputs), Arc::clone(&self.layout));
        let (inp, lay) = (&*inp, &*lay);
        let acc = &lay.accounts;
        let (np, nc, n_pops, n_ent) = (
            inp.n_provinces(),
            inp.n_countries(),
            inp.n_pops(),
            inp.n_enterprises(),
        );
        assert!(
            solution.price.len() == np * N_GOODS
                && solution.wage.len() == np
                && solution.employment.len() == n_ent
                && solution.output.len() == n_ent,
            "solution sizes do not match the inputs"
        );
        assert!(
            (0..N_GOODS).all(|g| self.pipeline.in_flight(g) == 0.0),
            "settle needs a world fresh from EconWorld::new"
        );

        for (cell, &p) in solution.price.iter().enumerate() {
            if self.markets.active[cell] {
                self.markets.price[cell] = p;
            }
        }
        self.wage.copy_from_slice(&solution.wage);
        self.employment.copy_from_slice(&solution.employment);
        self.output.copy_from_slice(&solution.output);
        for i in 0..np {
            let rate =
                lay.enterprises(i).map(|e| self.employment[e]).sum::<f64>() / lay.labour_supply[i];
            for k in lay.pops(i) {
                self.employed[k] = inp.pops.labour_force[k] * rate;
            }
        }
        self.refresh_price_index();
        // a steady state has no inflation: pi^e = 0 and the level at the solution's prices
        self.expected_inflation = 0.0;
        self.price_level = self.world_price_index();
        let price = |cell: usize| self.markets.price[cell];
        let index = self.price_index.clone();
        let traded: Vec<usize> = (0..N_GOODS).filter(|&g| TRADED[g]).collect();
        let mut out = Sourced::new();

        // enterprises: inputs used, traded inputs bought at a Y q_d, profits
        let mut ent_spend = vec![0.0; n_ent * N_GOODS];
        let mut output = vec![0.0; np * N_GOODS];
        let mut pool = vec![0.0; np];
        for e in 0..n_ent {
            let i = inp.enterprises.province[e] as usize;
            let g = usize::from(inp.enterprises.good[e]);
            let y = self.output[e];
            output[i * N_GOODS + g] += y;
            let mut cost = 0.0;
            for k in (0..N_GOODS).filter(|&k| lay.coef[e][k] > 0.0) {
                let (use_k, cell) = (lay.coef[e][k] * y, i * N_GOODS + k);
                if !TRADED[k] {
                    cost += price(cell) * use_k;
                } else if index[cell].is_finite() {
                    ent_spend[e * N_GOODS + k] = use_k * index[cell];
                    cost += ent_spend[e * N_GOODS + k];
                }
            }
            pool[i] += price(i * N_GOODS + g) * y - self.wage[i] * self.employment[e] - cost;
        }

        // households: income with rebates and market cash, by fixed point
        let edu_eff = self.params.edu_eff;
        let wages: Vec<f64> = (0..n_pops)
            .map(|k| {
                let i = inp.pops.province[k] as usize;
                self.wage[i] * edu_eff[usize::from(inp.pops.edu[k])] * self.employed[k]
            })
            .collect();
        let population: Vec<f64> = lay
            .country_pops
            .iter()
            .map(|pops| pops.iter().map(|&k| inp.pops.population[k as usize]).sum())
            .collect();
        let mut income: Vec<f64> = (0..n_pops)
            .map(|k| wages[k] + lay.ownership[k] * pool[inp.pops.province[k] as usize])
            .collect();
        let mut pop_spend = vec![0.0; n_pops * N_GOODS];
        let mut spend = vec![[0.0; 2]; np * N_GOODS];
        let mut x = [0.0; N_GOODS];
        for _ in 0..INCOME_ITERATIONS {
            for k in 0..n_pops {
                let i = inp.pops.province[k] as usize;
                let row: [f64; N_GOODS] = index[i * N_GOODS..(i + 1) * N_GOODS]
                    .try_into()
                    .expect("one row of goods");
                lay.pop_demand(inp, &self.params, k, &row, income[k], &mut x);
                for &g in &traded {
                    pop_spend[k * N_GOODS + g] = if x[g] > 0.0 { x[g] * row[g] } else { 0.0 };
                }
            }
            spend.fill([0.0; 2]);
            for d in 0..np {
                for &g in &traded {
                    let cell = d * N_GOODS + g;
                    for e in lay.enterprises(d) {
                        spend[cell][INTERMEDIATE] += ent_spend[e * N_GOODS + g];
                    }
                    for k in lay.pops(d) {
                        spend[cell][HOUSEHOLD] += pop_spend[k * N_GOODS + g];
                    }
                }
            }
            let (mut rebate, mut market) = (vec![0.0; nc], vec![0.0; np]);
            for &g in &traded {
                let s: Vec<[f64; 2]> = (0..np).map(|d| spend[d * N_GOODS + g]).collect();
                ship(self, g, &s, &mut out);
                for d in 0..np {
                    let cell = d * N_GOODS + g;
                    rebate[inp.country_of(d)] += out.tariff[d];
                    market[d] += price(cell) * (out.shipped[d] - output[cell]);
                }
            }
            let next: Vec<f64> = (0..n_pops)
                .map(|k| {
                    let i = inp.pops.province[k] as usize;
                    let c = inp.country_of(i);
                    wages[k]
                        + lay.ownership[k] * (pool[i] + market[i])
                        + rebate[c] * inp.pops.population[k] / population[c]
                })
                .collect();
            let change = next
                .iter()
                .zip(&income)
                .map(|(a, b)| ((a - b) / b).abs())
                .fold(0.0, f64::max);
            income = next;
            if change <= INCOME_TOLERANCE {
                break;
            }
        }

        // goods in flight: the slot delivered j ticks from now holds every slot of at least j
        // of the last ticks' shipments; enterprise stocks so that orders replace one month
        let mut in_flight = [0.0; N_GOODS];
        let mut lagged = vec![0.0; n_ent * N_GOODS];
        for &g in &traded {
            let s: Vec<[f64; 2]> = (0..np).map(|d| spend[d * N_GOODS + g]).collect();
            ship(self, g, &s, &mut out);
            for d in 0..np {
                let cell = d * N_GOODS + g;
                let buyers = lay
                    .enterprises(d)
                    .map(|e| (n_pops + e, INTERMEDIATE, ent_spend[e * N_GOODS + g]))
                    .chain(
                        lay.pops(d)
                            .map(|k| (k, HOUSEHOLD, pop_spend[k * N_GOODS + g])),
                    );
                for (b, class, value) in buyers.filter(|b| b.2 > 0.0) {
                    let share = value / spend[cell][class];
                    let units: Vec<f64> = out.arrive.iter().map(|a| a[d][class] * share).collect();
                    for j in 1..ARRIVAL_SLOTS {
                        let due: f64 = units[j..].iter().sum();
                        if due > 0.0 {
                            self.pipeline.push(j, b, g, due);
                            in_flight[g] += due;
                        }
                    }
                    if class == INTERMEDIATE {
                        let e = b - n_pops;
                        lagged[e * N_GOODS + g] =
                            (1..ARRIVAL_SLOTS).map(|j| j as f64 * units[j]).sum();
                    }
                }
            }
        }
        for &g in &traded {
            if in_flight[g] > 0.0 {
                self.ledger.create(acc.transit[g], g, in_flight[g]);
            }
        }
        for e in 0..n_ent {
            let i = inp.enterprises.province[e] as usize;
            for k in 0..N_GOODS {
                let held = self.ledger.goods(acc.enterprise[e], k);
                self.ledger.destroy(acc.enterprise[e], k, held);
                let a = lay.coef[e][k];
                if a == 0.0 {
                    continue;
                }
                // traded: E3 sees M + arrivals - use in stock and the rest on order; ordering
                // u = T - stock - on order, T = (1 + COVER + L) a Y, replaces use when
                // M = T + a Y - u - sum_s s a_s. Local: E2 draws the use from this tick's
                // output, so the stock stays at T and E3 orders nothing
                let use_k = a * self.output[e];
                let cell = i * N_GOODS + k;
                let target = (1.0 + COVER + self.scratch.lead[cell]) * use_k;
                let stock = if TRADED[k] {
                    let ordered = ent_spend[e * N_GOODS + k] / index[cell];
                    (target + use_k - ordered - lagged[e * N_GOODS + k]).max(0.0)
                } else {
                    target
                };
                if stock > 0.0 {
                    self.ledger.create(acc.enterprise[e], k, stock);
                }
            }
        }
        for (cell, &y) in output.iter().enumerate() {
            let (i, g) = (cell / N_GOODS, cell % N_GOODS);
            let held = self.ledger.goods(acc.market[i], g);
            self.ledger.destroy(acc.market[i], g, held);
            let stock = if TRADED[g] && self.markets.active[cell] {
                self.trade.coverage[g] * y
            } else {
                0.0
            };
            self.markets.inventory[cell] = stock;
            if stock > 0.0 {
                self.ledger.create(acc.market[i], g, stock);
            }
        }

        // pops hold income less this tick's wages
        for k in 0..n_pops {
            let have = self.ledger.money(acc.pop[k]);
            let target = to_minor(income[k] - wages[k]);
            self.ledger
                .transfer(MINT, acc.mint, acc.pop[k], target - have);
        }
    }
}
