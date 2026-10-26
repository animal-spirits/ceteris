//! Per-tick history of a run, for charting its dynamics. Recording reads the world after a
//! tick completes and never writes to it, so a run with history has the state hashes of one
//! without. Tick 0 is the state before the first step; the final tick is always sampled.
//!
//! | file | columns |
//! |---|---|
//! | `run_history_world` | tick u32, world_price_index f64 (V1 index, [`EconWorld::world_price_index`]), expected_inflation f64, output_value f64 (sum over enterprises of output x price at the origin market), employment f64, labour_supply f64, unemployment f64 (1 - employment / labour_supply), pop_money f64, pops_negative_money u32 |
//! | `run_history_provinces` | tick u32, province u32, wage f64, employment f64, labour_supply f64, unemployment f64, pop_money f64, income f64 (the pops' income of the last tick) |
//! | `run_history_markets` | tick u32, province u32, good u8, price f64, price_index f64 (q_d, as in `run_prices`), supply f64 (Q), demand f64 (D), inventory f64 (S), sales f64, active bool |
//!
//! Effective labour units, numeraire values; pop money is the ledger balance of the pops in
//! numeraire (minor units summed exactly, then converted). Tick 0 has no sales, supply or
//! income yet.

use std::path::Path;

use ceteris_core::invariant::Violation;
use ceteris_core::sim::Sim;
use ceteris_trade::N_GOODS;

use crate::export::{Column, ExportError, bools, f64s, u8s, u32s, write_table};
use crate::world::{EconWorld, to_numeraire};

#[derive(Clone, Copy)]
enum Kind {
    Bool,
    U8,
    U32,
    F64,
}

/// A table under construction: columns of f64, converted to their type when written (u8 and
/// u32 values are exact in f64).
struct Table {
    cols: Vec<(&'static str, Kind, Vec<f64>)>,
}

impl Table {
    fn new(spec: &[(&'static str, Kind)]) -> Self {
        Table {
            cols: spec.iter().map(|&(n, k)| (n, k, Vec::new())).collect(),
        }
    }

    fn push(&mut self, row: &[f64]) {
        debug_assert_eq!(row.len(), self.cols.len());
        for (col, &x) in self.cols.iter_mut().zip(row) {
            col.2.push(x);
        }
    }

    fn write(&self, dir: &Path, name: &str) -> Result<(), ExportError> {
        let columns: Vec<Column> = self
            .cols
            .iter()
            .map(|(n, kind, v)| match kind {
                Kind::Bool => bools(n, v.iter().map(|&x| x != 0.0)),
                Kind::U8 => u8s(n, v.iter().map(|&x| x as u8)),
                Kind::U32 => u32s(n, v.iter().map(|&x| x as u32)),
                Kind::F64 => f64s(n, v.iter().copied()),
            })
            .collect();
        write_table(dir, name, columns)
    }
}

/// The sampled series of a run.
pub struct History {
    world: Table,
    provinces: Table,
    markets: Table,
    sampled: Vec<u32>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        use Kind::{Bool, F64, U8, U32};
        Self {
            world: Table::new(&[
                ("tick", U32),
                ("world_price_index", F64),
                ("expected_inflation", F64),
                ("output_value", F64),
                ("employment", F64),
                ("labour_supply", F64),
                ("unemployment", F64),
                ("pop_money", F64),
                ("pops_negative_money", U32),
            ]),
            provinces: Table::new(&[
                ("tick", U32),
                ("province", U32),
                ("wage", F64),
                ("employment", F64),
                ("labour_supply", F64),
                ("unemployment", F64),
                ("pop_money", F64),
                ("income", F64),
            ]),
            markets: Table::new(&[
                ("tick", U32),
                ("province", U32),
                ("good", U8),
                ("price", F64),
                ("price_index", F64),
                ("supply", F64),
                ("demand", F64),
                ("inventory", F64),
                ("sales", F64),
                ("active", Bool),
            ]),
            sampled: Vec::new(),
        }
    }

    /// Ticks recorded so far.
    pub fn ticks(&self) -> &[u32] {
        &self.sampled
    }

    /// Appends the state of `world` at `tick`.
    pub fn record(&mut self, tick: u32, world: &EconWorld) {
        let (inp, lay, m) = (world.inputs(), world.layout(), &world.markets);
        let t = f64::from(tick);

        let (mut employment, mut supply) = (0.0, 0.0);
        let (mut money, mut negative) = (0_i64, 0_u32);
        for i in 0..inp.n_provinces() {
            let emp: f64 = lay.enterprises(i).map(|e| world.employment[e]).sum();
            let (mut minor, mut income) = (0_i64, 0.0);
            for k in lay.pops(i) {
                let balance = world.ledger.money(lay.accounts.pop[k]);
                minor += balance;
                negative += u32::from(balance < 0);
                income += world.income[k];
            }
            let ls = lay.labour_supply[i];
            self.provinces.push(&[
                t,
                i as f64,
                world.wage[i],
                emp,
                ls,
                1.0 - emp / ls,
                to_numeraire(minor),
                income,
            ]);
            employment += emp;
            supply += ls;
            money += minor;

            for g in 0..N_GOODS {
                let c = i * N_GOODS + g;
                self.markets.push(&[
                    t,
                    i as f64,
                    g as f64,
                    m.price[c],
                    world.price_index[c],
                    m.supply[c],
                    m.demand[c],
                    m.inventory[c],
                    m.sales[c],
                    f64::from(u8::from(m.active[c])),
                ]);
            }
        }

        let e = &inp.enterprises;
        let output_value: f64 = (0..inp.n_enterprises())
            .map(|k| {
                world.output[k] * m.price[e.province[k] as usize * N_GOODS + usize::from(e.good[k])]
            })
            .sum();
        self.world.push(&[
            t,
            world.world_price_index(),
            world.expected_inflation,
            output_value,
            employment,
            supply,
            1.0 - employment / supply,
            to_numeraire(money),
            f64::from(negative),
        ]);
        self.sampled.push(tick);
    }

    /// Writes the three history files into `dir`.
    pub fn write(&self, dir: &Path) -> Result<(), ExportError> {
        std::fs::create_dir_all(dir)?;
        self.world.write(dir, "run_history_world")?;
        self.provinces.write(dir, "run_history_provinces")?;
        self.markets.write(dir, "run_history_markets")
    }
}

/// Runs `sim` to `ticks`, recording tick 0, every `every`-th tick and the final tick.
pub fn run_recorded(
    sim: &mut Sim<EconWorld>,
    ticks: u32,
    every: u32,
) -> Result<History, Violation> {
    assert!(every > 0, "history sampling interval must be positive");
    let mut history = History::new();
    history.record(sim.tick(), sim.world());
    while sim.tick() < ticks {
        sim.step()?;
        let t = sim.tick();
        if t.is_multiple_of(every) || t == ticks {
            history.record(t, sim.world());
        }
    }
    Ok(history)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    use ceteris_geo::pack::{f64s, read_table, u8s, u32s};
    use ceteris_trade::TradeParams;

    use crate::export::write_run;
    use crate::fixture::small_world;
    use crate::params::EconParams;

    const NP: usize = 25;

    fn sim() -> Sim<EconWorld> {
        EconWorld::sim(
            Arc::new(small_world()),
            EconParams::default(),
            TradeParams::default(),
        )
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ceteris-history-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn rows(dir: &Path, name: &str) -> usize {
        read_table(&dir.join(format!("{name}.arrow")))
            .unwrap()
            .num_rows()
    }

    #[test]
    fn row_counts_follow_the_sampling_interval() {
        for (every, ticks) in [(1, 25), (5, 6)] {
            let history = run_recorded(&mut sim(), 24, every).unwrap();
            let dir = scratch_dir(&format!("counts-{every}"));
            history.write(&dir).unwrap();
            assert_eq!(rows(&dir, "run_history_world"), ticks);
            assert_eq!(rows(&dir, "run_history_provinces"), ticks * NP);
            assert_eq!(rows(&dir, "run_history_markets"), ticks * NP * N_GOODS);
            fs::remove_dir_all(dir).unwrap();
        }
        let history = run_recorded(&mut sim(), 24, 5).unwrap();
        assert_eq!(history.ticks(), [0, 5, 10, 15, 20, 24]);
        let history = run_recorded(&mut sim(), 0, 3).unwrap();
        assert_eq!(history.ticks(), [0]);
    }

    #[test]
    fn recording_leaves_the_run_unchanged() {
        let mut plain = sim();
        plain.run_until(24).unwrap();
        let mut recorded = sim();
        run_recorded(&mut recorded, 24, 1).unwrap();
        assert_eq!(plain.state_hash(), recorded.state_hash());
    }

    #[test]
    fn last_row_is_the_end_state_of_the_run_files() {
        let mut s = sim();
        let history = run_recorded(&mut s, 24, 5).unwrap();
        let dir = scratch_dir("last");
        write_run(&dir, s.world()).unwrap();
        history.write(&dir).unwrap();
        let table = |name: &str| read_table(&dir.join(format!("{name}.arrow"))).unwrap();
        let w = s.world();

        let prices = table("run_prices");
        let markets = table("run_history_markets");
        let tail = markets.num_rows() - NP * N_GOODS;
        let last = |v: Vec<f64>| v[tail..].to_vec();
        assert_eq!(last(f64s(&markets, "price").unwrap()), w.markets.price);
        assert_eq!(f64s(&prices, "price").unwrap(), w.markets.price);
        assert_eq!(
            last(f64s(&markets, "price_index").unwrap()),
            f64s(&prices, "price_index").unwrap()
        );
        assert_eq!(
            last(f64s(&markets, "inventory").unwrap()),
            w.markets.inventory
        );
        assert_eq!(u32s(&markets, "tick").unwrap()[tail..], [24; NP * N_GOODS]);
        assert_eq!(
            u8s(&markets, "good").unwrap()[tail..],
            u8s(&prices, "good").unwrap()[..]
        );

        let wages = table("run_wages");
        let provinces = table("run_history_provinces");
        let tail = provinces.num_rows() - NP;
        assert_eq!(
            f64s(&provinces, "wage").unwrap()[tail..],
            f64s(&wages, "wage").unwrap()[..]
        );

        let world = table("run_history_world");
        let at = world.num_rows() - 1;
        assert_eq!(u32s(&world, "tick").unwrap()[at], 24);
        assert_eq!(
            f64s(&world, "world_price_index").unwrap()[at],
            w.world_price_index()
        );
        assert_eq!(
            f64s(&world, "expected_inflation").unwrap()[at],
            w.expected_inflation
        );
        let ents = table("run_enterprises");
        let employment: f64 = f64s(&ents, "employment").unwrap().iter().sum();
        let total = f64s(&world, "employment").unwrap()[at];
        assert!(
            (total - employment).abs() <= 1e-9 * employment,
            "{total} {employment}"
        );
        let u = f64s(&world, "unemployment").unwrap()[at];
        assert!((0.0..1.0).contains(&u), "{u}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn history_is_deterministic() {
        let write = |name: &str| {
            let dir = scratch_dir(name);
            run_recorded(&mut sim(), 24, 1)
                .unwrap()
                .write(&dir)
                .unwrap();
            let files: Vec<_> = ["world", "provinces", "markets"]
                .map(|t| fs::read(dir.join(format!("run_history_{t}.arrow"))).unwrap())
                .into();
            fs::remove_dir_all(dir).unwrap();
            files
        };
        assert_eq!(write("det-a"), write("det-b"));
    }
}
