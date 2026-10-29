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
//!
//! In memory only the values are kept, at native width (f64, u32, bool); the tick, province
//! and good keys follow from the sampled ticks and the row order and are materialised when
//! written, so a market row is 49 bytes. Files are Arrow IPC with zstd buffer compression, or
//! CSV with the same tables and column names. Both are lossless: f64 values round-trip by bits.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Float64Array, UInt8Array, UInt32Array};

use ceteris_core::invariant::Violation;
use ceteris_core::sim::Sim;
use ceteris_trade::N_GOODS;

use crate::export::{Column, ExportError, write_table_zstd};
use crate::world::{EconWorld, to_numeraire_wide};

/// File format of [`History::write`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Arrow,
    Csv,
}

/// A column at write time: keys are built from the row layout, values are moved out of the
/// history so that no table is held twice.
enum Col {
    Bool(Vec<bool>),
    U8(Vec<u8>),
    U32(Vec<u32>),
    F64(Vec<f64>),
}

/// `N` named f64 columns filled a row at a time.
#[derive(Clone)]
struct F64Cols<const N: usize> {
    names: [&'static str; N],
    data: [Vec<f64>; N],
}

impl<const N: usize> F64Cols<N> {
    fn new(names: [&'static str; N]) -> Self {
        Self {
            names,
            data: std::array::from_fn(|_| Vec::new()),
        }
    }

    fn push(&mut self, row: [f64; N]) {
        for (col, x) in self.data.iter_mut().zip(row) {
            col.push(x);
        }
    }

    fn into_cols(self) -> impl Iterator<Item = (&'static str, Col)> {
        self.names
            .into_iter()
            .zip(self.data.into_iter().map(Col::F64))
    }

    fn bytes(&self) -> usize {
        self.data.iter().map(|v| v.len() * size_of::<f64>()).sum()
    }
}

/// The sampled series of a run.
#[derive(Clone)]
pub struct History {
    sampled: Vec<u32>,
    n_provinces: usize,
    world: F64Cols<7>,
    pops_negative_money: Vec<u32>,
    provinces: F64Cols<6>,
    markets: F64Cols<6>,
    active: Vec<bool>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self {
            sampled: Vec::new(),
            n_provinces: 0,
            world: F64Cols::new([
                "world_price_index",
                "expected_inflation",
                "output_value",
                "employment",
                "labour_supply",
                "unemployment",
                "pop_money",
            ]),
            pops_negative_money: Vec::new(),
            provinces: F64Cols::new([
                "wage",
                "employment",
                "labour_supply",
                "unemployment",
                "pop_money",
                "income",
            ]),
            markets: F64Cols::new([
                "price",
                "price_index",
                "supply",
                "demand",
                "inventory",
                "sales",
            ]),
            active: Vec::new(),
        }
    }

    /// Ticks recorded so far.
    pub fn ticks(&self) -> &[u32] {
        &self.sampled
    }

    /// Bytes held for the market table.
    pub fn market_bytes(&self) -> usize {
        self.markets.bytes() + self.active.len() * size_of::<bool>()
    }

    /// Rows of the market table.
    pub fn market_rows(&self) -> usize {
        self.active.len()
    }

    /// Appends the state of `world` at `tick`.
    pub fn record(&mut self, tick: u32, world: &EconWorld) {
        let (inp, lay, m) = (world.inputs(), world.layout(), &world.markets);
        self.n_provinces = inp.n_provinces();

        let (mut employment, mut supply) = (0.0, 0.0);
        let (mut money, mut negative) = (0_i128, 0_u32);
        for i in 0..inp.n_provinces() {
            let emp: f64 = lay.enterprises(i).map(|e| world.employment[e]).sum();
            let (mut minor, mut income) = (0_i128, 0.0);
            for k in lay.pops(i) {
                let balance = world.ledger.money(lay.accounts.pop[k]);
                minor += i128::from(balance);
                negative += u32::from(balance < 0);
                income += world.income[k];
            }
            let ls = lay.labour_supply[i];
            self.provinces.push([
                world.wage[i],
                emp,
                ls,
                1.0 - emp / ls,
                to_numeraire_wide(minor),
                income,
            ]);
            employment += emp;
            supply += ls;
            money += minor;

            for g in 0..N_GOODS {
                let c = i * N_GOODS + g;
                self.markets.push([
                    m.price[c],
                    world.price_index[c],
                    m.supply[c],
                    m.demand[c],
                    m.inventory[c],
                    m.sales[c],
                ]);
                self.active.push(m.active[c]);
            }
        }

        let e = &inp.enterprises;
        let output_value: f64 = (0..inp.n_enterprises())
            .map(|k| {
                world.output[k] * m.price[e.province[k] as usize * N_GOODS + usize::from(e.good[k])]
            })
            .sum();
        self.world.push([
            world.world_price_index(),
            world.expected_inflation,
            output_value,
            employment,
            supply,
            1.0 - employment / supply,
            to_numeraire_wide(money),
        ]);
        self.pops_negative_money.push(negative);
        self.sampled.push(tick);
    }

    /// Tick of every row, `per_tick` rows per sampled tick.
    fn tick_keys(&self, per_tick: usize) -> Vec<u32> {
        self.sampled
            .iter()
            .flat_map(|&t| std::iter::repeat_n(t, per_tick))
            .collect()
    }

    fn province_keys(&self, per_province: usize) -> Vec<u32> {
        let rows = self.sampled.len() * self.n_provinces;
        (0..rows)
            .flat_map(|r| std::iter::repeat_n((r % self.n_provinces) as u32, per_province))
            .collect()
    }

    fn tables(self) -> [(&'static str, Vec<(&'static str, Col)>); 3] {
        let np = self.n_provinces;
        let (province_ticks, market_ticks) = (self.tick_keys(np), self.tick_keys(np * N_GOODS));
        let (province_ids, market_ids) = (self.province_keys(1), self.province_keys(N_GOODS));
        let mut world = vec![("tick", Col::U32(self.sampled))];
        world.extend(self.world.into_cols());
        world.push(("pops_negative_money", Col::U32(self.pops_negative_money)));

        let mut provinces = vec![
            ("tick", Col::U32(province_ticks)),
            ("province", Col::U32(province_ids)),
        ];
        provinces.extend(self.provinces.into_cols());

        let mut markets = vec![
            ("tick", Col::U32(market_ticks)),
            ("province", Col::U32(market_ids)),
            (
                "good",
                Col::U8(
                    (0..self.active.len())
                        .map(|r| (r % N_GOODS) as u8)
                        .collect(),
                ),
            ),
        ];
        markets.extend(self.markets.into_cols());
        markets.push(("active", Col::Bool(self.active)));

        [
            ("run_history_world", world),
            ("run_history_provinces", provinces),
            ("run_history_markets", markets),
        ]
    }

    /// Writes the three history files into `dir`.
    pub fn write(self, dir: &Path, format: Format) -> Result<(), ExportError> {
        std::fs::create_dir_all(dir)?;
        for (name, cols) in self.tables() {
            match format {
                Format::Arrow => write_table_zstd(dir, name, arrow_columns(cols))?,
                Format::Csv => write_csv(&dir.join(format!("{name}.csv")), &cols)?,
            }
        }
        Ok(())
    }
}

fn arrow_columns(cols: Vec<(&str, Col)>) -> Vec<Column> {
    cols.into_iter()
        .map(|(n, c)| {
            let array: ArrayRef = match c {
                Col::Bool(v) => Arc::new(BooleanArray::from(v)),
                Col::U8(v) => Arc::new(UInt8Array::from(v)),
                Col::U32(v) => Arc::new(UInt32Array::from(v)),
                Col::F64(v) => Arc::new(Float64Array::from(v)),
            };
            (n.to_owned(), array)
        })
        .collect()
}

/// Header and one line per row; f64 as the shortest text that parses back to the same bits.
fn write_csv(path: &Path, cols: &[(&str, Col)]) -> Result<(), ExportError> {
    let len = |c: &Col| match c {
        Col::Bool(v) => v.len(),
        Col::U8(v) => v.len(),
        Col::U32(v) => v.len(),
        Col::F64(v) => v.len(),
    };
    let rows = len(&cols[0].1);
    let mut out = BufWriter::new(File::create(path)?);
    let header: Vec<&str> = cols.iter().map(|(n, _)| *n).collect();
    writeln!(out, "{}", header.join(","))?;
    for r in 0..rows {
        for (k, (_, c)) in cols.iter().enumerate() {
            if k > 0 {
                out.write_all(b",")?;
            }
            match c {
                Col::Bool(v) => write!(out, "{}", v[r])?,
                Col::U8(v) => write!(out, "{}", v[r])?,
                Col::U32(v) => write!(out, "{}", v[r])?,
                Col::F64(v) => write!(out, "{}", v[r])?,
            }
        }
        out.write_all(b"\n")?;
    }
    out.flush()?;
    Ok(())
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
            history.clone().write(&dir, Format::Arrow).unwrap();
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
        history.clone().write(&dir, Format::Arrow).unwrap();
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

    fn bits(v: &[f64]) -> Vec<u64> {
        v.iter().map(|x| x.to_bits()).collect()
    }

    #[test]
    fn history_round_trips_losslessly() {
        let history = run_recorded(&mut sim(), 24, 1).unwrap();
        let dir = scratch_dir("round-trip");
        history.clone().write(&dir, Format::Arrow).unwrap();
        let markets = read_table(&dir.join("run_history_markets.arrow")).unwrap();
        for (name, kept) in history.markets.names.iter().zip(&history.markets.data) {
            assert_eq!(bits(&f64s(&markets, name).unwrap()), bits(kept), "{name}");
        }
        let provinces = read_table(&dir.join("run_history_provinces.arrow")).unwrap();
        for (name, kept) in history.provinces.names.iter().zip(&history.provinces.data) {
            assert_eq!(bits(&f64s(&provinces, name).unwrap()), bits(kept), "{name}");
        }
        let world = read_table(&dir.join("run_history_world.arrow")).unwrap();
        for (name, kept) in history.world.names.iter().zip(&history.world.data) {
            assert_eq!(bits(&f64s(&world, name).unwrap()), bits(kept), "{name}");
        }
        let keys = u32s(&markets, "province").unwrap();
        assert_eq!(keys[N_GOODS], 1);
        assert_eq!(keys[NP * N_GOODS], 0);
        assert_eq!(keys[N_GOODS * 3], 3);
        assert_eq!(u8s(&markets, "good").unwrap()[N_GOODS + 2], 2);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn history_memory_per_market_row_at_most_49_bytes() {
        let history = run_recorded(&mut sim(), 24, 1).unwrap();
        assert_eq!(history.market_rows(), 25 * NP * N_GOODS);
        assert!(history.market_bytes() <= 49 * history.market_rows());
    }

    #[test]
    fn history_csv_matches_arrow() {
        let history = run_recorded(&mut sim(), 24, 5).unwrap();
        let dir = scratch_dir("csv");
        history.clone().write(&dir, Format::Arrow).unwrap();
        history.clone().write(&dir, Format::Csv).unwrap();
        for name in [
            "run_history_world",
            "run_history_provinces",
            "run_history_markets",
        ] {
            let table = read_table(&dir.join(format!("{name}.arrow"))).unwrap();
            let text = fs::read_to_string(dir.join(format!("{name}.csv"))).unwrap();
            let mut lines = text.lines();
            let header: Vec<&str> = lines.next().unwrap().split(',').collect();
            let rows: Vec<Vec<&str>> = lines.map(|l| l.split(',').collect()).collect();
            assert_eq!(rows.len(), table.num_rows(), "{name}");
            assert_eq!(header.len(), table.num_columns(), "{name}");
            for (k, col) in header.iter().enumerate() {
                let field = table.schema().field_with_name(col).unwrap().clone();
                assert_eq!(table.schema().index_of(col).unwrap(), k, "{name} {col}");
                let cells = rows.iter().map(|r| r[k]);
                match field.data_type() {
                    arrow_schema::DataType::Float64 => {
                        let kept = f64s(&table, col).unwrap();
                        let parsed: Vec<u64> =
                            cells.map(|c| c.parse::<f64>().unwrap().to_bits()).collect();
                        assert_eq!(parsed, bits(&kept), "{name} {col}");
                    }
                    arrow_schema::DataType::UInt32 => {
                        let kept: Vec<String> = u32s(&table, col)
                            .unwrap()
                            .iter()
                            .map(u32::to_string)
                            .collect();
                        assert_eq!(cells.map(str::to_owned).collect::<Vec<_>>(), kept);
                    }
                    arrow_schema::DataType::UInt8 => {
                        let kept: Vec<String> = u8s(&table, col)
                            .unwrap()
                            .iter()
                            .map(u8::to_string)
                            .collect();
                        assert_eq!(cells.map(str::to_owned).collect::<Vec<_>>(), kept);
                    }
                    _ => assert_eq!(*col, "active"),
                }
            }
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn history_is_deterministic() {
        let write = |name: &str| {
            let dir = scratch_dir(name);
            run_recorded(&mut sim(), 24, 1)
                .unwrap()
                .write(&dir, Format::Arrow)
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
