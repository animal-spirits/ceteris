//! P2 fixture determinism: identical hashes across two runs and across 1 vs 8 threads,
//! and restore-and-replay from every snapshot; V1, the frictionless run from the data state
//! against the GE reference committed under `tests/golden/p2-fixture/reference`. Slow in debug
//! builds; run with `--release`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::{SNAPSHOT_EVERY, Sim};
use ceteris_econ::export::{write_inputs, write_run};
use ceteris_econ::fixture::small_world;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::pack::{f64s, read_table, u8s, u32s};
use ceteris_trade::{Kernel, N_GOODS, TradeParams};

const TICKS: u32 = 1200;

fn run(threads: usize, ticks: u32) -> Sim<EconWorld> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    pool.install(|| {
        let mut sim = EconWorld::sim(
            Arc::new(small_world()),
            EconParams::default(),
            TradeParams::default(),
        );
        sim.run_until(ticks).unwrap_or_else(|v| panic!("{v}"));
        sim
    })
}

#[test]
fn two_runs_give_identical_hashes_at_every_snapshot() {
    let (a, b) = (run(4, TICKS), run(4, TICKS));
    assert_eq!(a.snapshot_hashes().len() as u32, TICKS / SNAPSHOT_EVERY + 1);
    assert_eq!(a.snapshot_hashes(), b.snapshot_hashes());
}

#[test]
fn one_vs_eight_threads_identical() {
    assert_eq!(
        run(1, TICKS).snapshot_hashes(),
        run(8, TICKS).snapshot_hashes()
    );
}

/// Replay from every snapshot on a world with every crossing at lag 3, so goods sit in all
/// four pipeline slots at every snapshot after the first (Review Focus 3): domestic
/// shipments in the next slot, crossings dispatched over the last three ticks in the others.
#[test]
fn replay_from_every_snapshot() {
    let mut world = EconWorld::new(
        Arc::new(small_world()),
        EconParams::default(),
        TradeParams::default(),
    );
    world.set_crossing_lags(3);
    let mut sim = Sim::new(
        world,
        ceteris_econ::systems::schedule(),
        ceteris_econ::systems::SEED,
    );
    sim.run_until(TICKS).unwrap_or_else(|v| panic!("{v}"));
    let reference = sim.snapshot_hashes();
    let final_hash = sim.state_hash();
    for &(t, _) in &reference {
        sim.seek(t).unwrap();
        if t > 0 {
            let p = &sim.world().pipeline;
            for slot in 1..=4 {
                assert!(
                    p.scheduled(slot).iter().any(|&x| x > 0.0),
                    "tick {t}: pipeline slot {slot} empty"
                );
            }
        }
        sim.run_until((t + SNAPSHOT_EVERY).min(TICKS)).unwrap();
        assert_eq!(sim.snapshot_hashes(), reference, "replay from tick {t}");
    }
    for t in [0, 600, 1188] {
        sim.seek(t).unwrap();
        sim.run_until(TICKS).unwrap();
        assert_eq!(sim.state_hash(), final_hash, "full replay from tick {t}");
    }
}

/// Fixture export directory; `CETERIS_FIXTURE_DIR` overrides the committed one.
fn fixture_dir() -> PathBuf {
    std::env::var_os("CETERIS_FIXTURE_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture"),
        PathBuf::from,
    )
}

fn files_of(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.file_type().unwrap().is_file())
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (name, fs::read(e.path()).unwrap())
        })
        .collect()
}

#[test]
#[ignore = "re-blessed in MIGRATION Task 10 (ADRs 0015-0022)"]
fn fixture_export_is_current() {
    let out = std::env::temp_dir().join(format!("ceteris-p2-export-{}", std::process::id()));
    let _ = fs::remove_dir_all(&out);
    let inputs = small_world();
    let (params, trade) = (EconParams::default(), TradeParams::default());
    let kernel = Kernel::build(&inputs.network, &inputs.weight, &inputs.wedges, &trade);
    write_inputs(&out, &inputs, &kernel, &params, &trade).unwrap();
    let (fresh, committed) = (files_of(&out), files_of(&fixture_dir()));
    fs::remove_dir_all(&out).unwrap();

    let stale: Vec<&String> = fresh
        .keys()
        .chain(committed.keys())
        .filter(|name| fresh.get(*name) != committed.get(*name))
        .collect();
    assert!(
        stale.is_empty(),
        "tests/golden/p2-fixture differs from the fixture export in {stale:?}; regenerate with \
         `cargo run -p ceteris-econ --bin ceteris -- export --fixture --out tests/golden/p2-fixture` \
         and, because the inputs changed, re-solve the reference with `uv run --project python \
         python -m ceteris.reference solve --inputs tests/golden/p2-fixture --out \
         tests/golden/p2-fixture/reference`"
    );
}

/// Base household consumption quantities at prices 1, as `ceteris.reference.io.Inputs.index_weights`:
/// the final demand shares of the country times the province's base income (wages of the
/// employed plus the profit pool), `[province * N_GOODS + g]`.
fn index_weights(inp: &EconInputs, params: &EconParams) -> Vec<f64> {
    let supply = inp.labour_supply(&params.edu_eff);
    let mut pool = vec![0.0; inp.n_provinces()];
    for e in 0..inp.n_enterprises() {
        let i = inp.enterprises.province[e] as usize;
        let s_l = inp.countries.s_l[inp.country_of(i)];
        pool[i] += (1.0 - s_l) * inp.enterprises.va0[e];
    }
    let mut out = vec![0.0; inp.n_provinces() * N_GOODS];
    for i in 0..inp.n_provinces() {
        let c = inp.country_of(i);
        let income =
            inp.provinces.w0[i] * (1.0 - inp.countries.unemployment[c]) * supply[i] + pool[i];
        let omega = &inp.countries.omega[c];
        let total: f64 = omega.iter().sum();
        for g in 0..N_GOODS {
            out[i * N_GOODS + g] = omega[g] / total * income;
        }
    }
    out
}

fn worst(deviations: impl Iterator<Item = (String, f64)>) -> (String, f64) {
    deviations.fold(
        (String::from("none"), 0.0),
        |a, b| if b.1 > a.1 { b } else { a },
    )
}

fn rel(got: f64, want: f64) -> f64 {
    (got / want - 1.0).abs()
}

type FlowKey = (u32, u32, u8);

fn flow_table(dir: &Path, name: &str) -> BTreeMap<FlowKey, (f64, f64)> {
    let b = read_table(&dir.join(name)).unwrap();
    let (ex, im, g) = (
        u32s(&b, "exporter_country").unwrap(),
        u32s(&b, "importer_country").unwrap(),
        u8s(&b, "good").unwrap(),
    );
    let (value, units) = (f64s(&b, "value").unwrap(), f64s(&b, "units").unwrap());
    (0..ex.len())
        .map(|r| ((ex[r], im[r], g[r]), (value[r], units[r])))
        .collect()
}

/// V1 (specs/econ/SPEC.md, Equilibrium nesting): with frictionless settings the run from the
/// data state reaches the GE reference within 1,200 ticks. Prices and wages are divided by the
/// world price index over consumer prices (the reference is normalized to an index of 1);
/// flows are the mean of the last 12 ticks, joined with the reference on (exporter, importer,
/// good).
#[test]
fn v1_fixture_frictionless_matches_reference() {
    const TOL: f64 = 0.01;
    let started = std::time::Instant::now();
    let reference = fixture_dir().join("reference");
    let solution = ceteris_econ::equilibrium::Equilibrium::read(&reference).unwrap();

    let params = EconParams::frictionless();
    let inputs = small_world();
    let weights = index_weights(&inputs, &params);
    let mut sim = Sim::new(
        EconWorld::new(Arc::new(inputs), params.clone(), TradeParams::default()),
        ceteris_econ::systems::schedule(),
        ceteris_econ::systems::SEED,
    );
    sim.run_until(TICKS).unwrap_or_else(|v| panic!("{v}"));
    let w = sim.world();

    // consumer prices: the delivered price index of traded goods, the local price of local goods
    let index = weights
        .iter()
        .zip(&w.price_index)
        .map(|(a, p)| a * p)
        .sum::<f64>()
        / weights.iter().sum::<f64>();

    let (price_cell, price_dev) = worst(
        (0..w.markets.price.len())
            .filter(|&c| w.markets.active[c])
            .map(|c| {
                (
                    format!("province {} good {}", c / N_GOODS, c % N_GOODS),
                    rel(w.markets.price[c] / index, solution.price[c]),
                )
            }),
    );
    let (wage_province, wage_dev) = worst((0..w.wage.len()).map(|i| {
        (
            format!("province {i}"),
            rel(w.wage[i] / index, solution.wage[i]),
        )
    }));

    // The window holds the last 12 ticks, as run_flows writes it.
    let out = std::env::temp_dir().join(format!("ceteris-p2-v1-{}", std::process::id()));
    let _ = fs::remove_dir_all(&out);
    write_run(&out, w).unwrap();
    let got = flow_table(&out, "run_flows.arrow");
    fs::remove_dir_all(&out).unwrap();
    let want = flow_table(&reference, "flows.arrow");

    let world_trade: f64 = want.values().map(|v| v.0).sum();
    let floor = 1e-6 * world_trade;
    let (mut compared, mut missing) = (0usize, Vec::new());
    let (mut unit_dev, mut value_dev) = ((String::new(), 0.0), (String::new(), 0.0));
    for (key, &(value, units)) in &want {
        if value <= floor {
            continue;
        }
        let Some(&(got_value, got_units)) = got.get(key) else {
            missing.push(*key);
            continue;
        };
        compared += 1;
        let label = format!("exporter {} importer {} good {}", key.0, key.1, key.2);
        for (best, d) in [
            (&mut unit_dev, rel(got_units, units)),
            (&mut value_dev, rel(got_value / index, value)),
        ] {
            if d > best.1 {
                *best = (label.clone(), d);
            }
        }
    }

    let summary = format!(
        "V1 max deviations: price {price_dev:e} ({price_cell}), wage {wage_dev:e} ({wage_province}), \
         flow units {:e} ({}), flow value {:e} ({}); {} active prices, {} wages, {compared} of {} \
         reference flows above 1e-6 of world trade {world_trade:e}; price index {index}; {:?}",
        unit_dev.1,
        unit_dev.0,
        value_dev.1,
        value_dev.0,
        (0..w.markets.price.len())
            .filter(|&c| w.markets.active[c])
            .count(),
        w.wage.len(),
        want.len(),
        started.elapsed(),
    );
    println!("{summary}");
    assert!(
        missing.is_empty(),
        "reference flows missing from the run: {missing:?}; {summary}"
    );
    assert!(compared > 0, "no flow compared; {summary}");
    for (what, d) in [
        ("price", price_dev),
        ("wage", wage_dev),
        ("flow units", unit_dev.1),
        ("flow value", value_dev.1),
    ] {
        assert!(d < TOL, "{what} deviates by {d:e}; {summary}");
    }
}
