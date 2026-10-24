//! Starting a run from the GE reference equilibrium (`python -m ceteris.reference solve`):
//! the solution's prices, wages, employment and output, with every stock and pop balance at
//! its steady-state value, so that a frictionless run starting there stays there.

use std::fs::{self, File};
use std::path::Path;

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::Float64Type;
use arrow_ipc::reader::FileReader;
use ceteris_trade::goods::TRADED;
use ceteris_trade::{N_GOODS, Routed, TradeParams, route};

use crate::export::ExportError;
use crate::inputs::EconInputs;
use crate::params::EconParams;
use crate::world::{EconWorld, MINT, static_digest, to_minor};

/// A reference solution as written by `python -m ceteris.reference solve --out DIR`.
#[derive(Clone, Debug, PartialEq)]
pub struct Equilibrium {
    /// `[province * N_GOODS + g]`: posted price of active cells, netback of inactive traded
    /// cells, 0 elsewhere.
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

impl EconWorld {
    /// A world at the steady state of `solution`: its prices (inactive local cells keep
    /// price 1), wages, employment and output; traded inventories at target coverage of
    /// steady-state demand (D = Q, home flow plus arrivals of every transit lag at these
    /// prices and outputs); the transit wheel holding the shipments of the last ticks; input
    /// stocks at two months of use (the steady state of the order rule before use); and each
    /// pop holding last tick's dividends and rebates, so that with this tick's wages it
    /// spends exactly its steady-state income.
    ///
    /// The solution must come from the export of these inputs with these structural
    /// parameters (adjustment speeds and the frictionless switch are free); panics
    /// otherwise.
    pub fn from_equilibrium(
        inputs: std::sync::Arc<EconInputs>,
        params: EconParams,
        trade: TradeParams,
        solution: &Equilibrium,
    ) -> EconWorld {
        let structural = static_digest(
            &inputs,
            &EconParams {
                sigma_demand: params.sigma_demand,
                sigma_fl: params.sigma_fl,
                edu_eff: params.edu_eff,
                ..EconParams::default()
            },
            &TradeParams {
                theta: trade.theta,
                coverage: trade.coverage,
                border_nu: trade.border_nu,
                time_cost: trade.time_cost,
                ..TradeParams::default()
            },
        );
        assert_eq!(
            solution.static_digest,
            hex(&structural),
            "the solution belongs to other inputs or parameters"
        );
        let mut w = EconWorld::new(inputs, params, trade);
        let inp = std::sync::Arc::clone(&w.inputs);
        let lay = std::sync::Arc::clone(&w.layout);
        let (inp, lay) = (&*inp, &*lay);
        let acc = &lay.accounts;
        let (np, n_ent) = (inp.n_provinces(), inp.n_enterprises());
        assert!(
            solution.price.len() == np * N_GOODS
                && solution.wage.len() == np
                && solution.employment.len() == n_ent
                && solution.output.len() == n_ent,
            "solution sizes do not match the inputs"
        );

        for (cell, &p) in solution.price.iter().enumerate() {
            if w.markets.active[cell] || (TRADED[cell % N_GOODS] && p > 0.0) {
                w.markets.price[cell] = p;
            }
        }
        w.wage.copy_from_slice(&solution.wage);
        w.employment.copy_from_slice(&solution.employment);
        w.output.copy_from_slice(&solution.output);
        for i in 0..np {
            let rate =
                lay.enterprises(i).map(|e| w.employment[e]).sum::<f64>() / lay.labour_supply[i];
            for k in lay.pops(i) {
                w.employed[k] = inp.pops.labour_force[k] * rate;
            }
        }

        // input stocks before use at two months of use; inputs used per cell
        let mut used = vec![0.0; np * N_GOODS];
        let mut supply = vec![0.0; np * N_GOODS];
        for e in 0..n_ent {
            let i = inp.enterprises.province[e] as usize;
            let (c, g) = (inp.country_of(i), usize::from(inp.enterprises.good[e]));
            let y = w.output[e];
            supply[i * N_GOODS + g] += y;
            for k in 0..N_GOODS {
                let a = inp.a(c, k, g);
                let held = w.ledger.goods(acc.enterprise[e], k);
                w.ledger.destroy(acc.enterprise[e], k, held);
                if a > 0.0 {
                    w.ledger.create(acc.enterprise[e], k, 2.0 * a * y);
                    used[i * N_GOODS + k] += a * y;
                }
            }
        }

        // steady-state supply Q: local output, or home flow plus arrivals of every lag
        let mut q = supply.clone();
        let mut tariff = vec![0.0; np];
        let mut routed = Routed::new();
        let (mut s, mut p, mut active) = (vec![0.0; np], vec![0.0; np], vec![false; np]);
        for (g, &transit) in acc.transit.iter().enumerate() {
            for o in 0..np {
                let cell = o * N_GOODS + g;
                (s[o], p[o], active[o]) =
                    (supply[cell], w.markets.price[cell], w.markets.active[cell]);
            }
            route(&w.kernel, g, &s, &p, &active, &mut routed);
            for d in 0..np {
                q[d * N_GOODS + g] =
                    routed.home[d] + routed.arrive.iter().map(|a| a[d]).sum::<f64>();
                tariff[d] += routed.tariff[d];
            }
            // the slot delivered j ticks from now holds every lag of at least j
            for j in 0..routed.arrive.len() {
                let due: Vec<f64> = (0..np)
                    .map(|d| routed.arrive[j..].iter().map(|a| a[d]).sum())
                    .collect();
                w.ledger.create(transit, g, due.iter().sum());
                w.transit.push(j, g, &due);
            }
        }
        for (cell, &q) in q.iter().enumerate() {
            let (i, g) = (cell / N_GOODS, cell % N_GOODS);
            let held = w.ledger.goods(acc.market[i], g);
            w.ledger.destroy(acc.market[i], g, held);
            let stock = if TRADED[g] && w.markets.active[cell] {
                w.trade.coverage[g] * q
            } else {
                0.0
            };
            w.markets.inventory[cell] = stock;
            if stock > 0.0 {
                w.ledger.create(acc.market[i], g, stock);
            }
        }

        // pops hold income less this tick's wages: rebates by population over the country,
        // dividends by ownership; income is household spending, p (Q - inputs used)
        let mut rebate = vec![0.0; inp.n_countries()];
        for d in 0..np {
            rebate[inp.country_of(d)] += tariff[d];
        }
        for (c, pops) in lay.country_pops.iter().enumerate() {
            let population: f64 = pops.iter().map(|&k| inp.pops.population[k as usize]).sum();
            for &k in pops {
                let k = k as usize;
                let target = rebate[c] * inp.pops.population[k] / population;
                let have = w.ledger.money(acc.pop[k]);
                w.ledger
                    .transfer(MINT, acc.mint, acc.pop[k], to_minor(target) - have);
            }
        }
        for i in 0..np {
            let spending: f64 = (i * N_GOODS..(i + 1) * N_GOODS)
                .filter(|&cell| w.markets.active[cell])
                .map(|cell| w.markets.price[cell] * (q[cell] - used[cell]))
                .sum();
            let wages: f64 = lay
                .enterprises(i)
                .map(|e| w.wage[i] * w.employment[e])
                .sum();
            let rebates: f64 = lay
                .pops(i)
                .map(|k| crate::world::to_numeraire(w.ledger.money(acc.pop[k])))
                .sum();
            let pool = spending - wages - rebates;
            for k in lay.pops(i) {
                w.ledger.transfer(
                    MINT,
                    acc.mint,
                    acc.pop[k],
                    to_minor(lay.ownership[k] * pool),
                );
            }
        }
        w
    }
}
