//! P2 fixture determinism: identical hashes across two runs and across 1 vs 8 threads,
//! and restore-and-replay from every snapshot; V1, the frictionless run from the data state
//! against the GE reference committed under `tests/golden/p2-fixture/reference`. On the full
//! pack with its calibration (P2 gate): 100 years of the default run without violation, and 1
//! vs 8 threads. Slow in debug builds; run with `--release`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::{SNAPSHOT_EVERY, Sim};
use ceteris_econ::export::{write_inputs, write_run};
use ceteris_econ::fixture::small_world;
use ceteris_econ::world::{DIVIDEND, WRITE_OFF, to_numeraire};
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::{f64s, read_table, u8s, u32s};
use ceteris_trade::goods::TRADED;
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
/// world price index over consumer prices (`EconWorld::world_price_index`, the index the
/// expectations of ADR 0019 track; the reference is normalized to an index of 1);
/// flows are the mean of the last 12 ticks, joined with the reference on (exporter, importer,
/// good). The 1% tolerances are V1's tolerance mode (ADR 0022): the uniqueness bound
/// (`python/ceteris/reference/uniqueness.py`, Theorem 1 of Allen, Arkolakis and Li 2024 in the
/// special case the SPEC states) is 0.922 at the fixture's theta and value-added shares, at most
/// 1, so one equilibrium is presumed; `test_trade_only_is_unique` checks it on the same export.
#[test]
fn v1_fixture_frictionless_matches_reference() {
    const TOL: f64 = 0.01;
    let started = std::time::Instant::now();
    let reference = fixture_dir().join("reference");
    let solution = ceteris_econ::equilibrium::Equilibrium::read(&reference).unwrap();

    let mut sim = Sim::new(
        EconWorld::new(
            Arc::new(small_world()),
            EconParams::frictionless(),
            TradeParams::default(),
        ),
        ceteris_econ::systems::schedule(),
        ceteris_econ::systems::SEED,
    );
    sim.run_until(TICKS).unwrap_or_else(|v| panic!("{v}"));
    let w = sim.world();

    let index = w.world_price_index();

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

/// `set_network` with the network the world already has, between ticks, leaves the run
/// bit for bit as it was (the fixture; the pack test relies on it for its control run).
#[test]
fn set_network_to_the_same_network_changes_nothing() {
    let sim = || {
        EconWorld::sim(
            Arc::new(small_world()),
            EconParams::default(),
            TradeParams::default(),
        )
    };
    let (mut a, mut b) = (sim(), sim());
    a.run_until(12).unwrap();
    b.run_until(12).unwrap();
    let net = b.world().inputs().network.clone();
    b.world_mut().set_network(net);
    a.run_until(48).unwrap();
    b.run_until(48).unwrap();
    assert_eq!(a.snapshot_hashes(), b.snapshot_hashes());
}

/// The built pack and its directory, or None (with the notice of `ceteris-geo`'s pack tests)
/// where it has not been built, as in CI.
fn built_pack() -> Option<(ceteris_geo::pack::Pack, PathBuf)> {
    let dir = ceteris_geo::pack::default_dir();
    if !dir.join("provinces.arrow").exists() {
        eprintln!(
            "skipping: no pack at {}; build it with `uv run python -m ceteris.packs`",
            dir.display()
        );
        return None;
    }
    let pack =
        ceteris_geo::pack::Pack::load(&dir, &TransportParams::default()).expect("pack loads");
    Some((pack, dir))
}

/// The full-pack world loads: every province has a country whose final demand shares sum to
/// one and at least one enterprise, every enterprise has workers and a positive finite A (1
/// without the calibration tables, which also bring the steady state to start from), every pop
/// belongs to a province with value added, and the inputs pass `validate`.
#[test]
fn pack_inputs_load() {
    let Some((pack, dir)) = built_pack() else {
        return;
    };
    let started = std::time::Instant::now();
    let inputs = ceteris_econ::EconInputs::from_pack(&pack, &dir).unwrap();
    let load = started.elapsed();
    inputs.validate();
    for i in 0..inputs.n_provinces() {
        let omega = &inputs.countries.omega[inputs.country_of(i)];
        assert!(
            (omega.iter().sum::<f64>() - 1.0).abs() < 1e-12,
            "province {i}: final demand shares {omega:?}"
        );
    }
    let calibrated = ceteris_econ::EconInputs::has_calibration(&dir);
    assert_eq!(inputs.steady.is_some(), calibrated);
    let mut firms = vec![0usize; inputs.n_provinces()];
    for e in 0..inputs.n_enterprises() {
        let ent = &inputs.enterprises;
        firms[ent.province[e] as usize] += 1;
        let a_ok = if calibrated {
            ent.a[e].is_finite() && ent.a[e] > 0.0
        } else {
            ent.a[e] == 1.0
        };
        assert!(
            ent.l0[e] > 0.0 && ent.va0[e] > 0.0 && a_ok,
            "enterprise {e}: L0 {}, VA0 {}, A {}",
            ent.l0[e],
            ent.va0[e],
            ent.a[e]
        );
    }
    assert!(firms.iter().all(|&n| n > 0), "a province has no enterprise");
    let supply = inputs.labour_supply(&EconParams::default().edu_eff);
    assert!(supply.iter().all(|&s| s > 0.0));
    let n = pack.provinces.len();
    let left_out = n - inputs.n_provinces();
    let net = &inputs.network;
    println!(
        "pack world: {} of {n} provinces ({left_out} without value added or labour), {} pops, \
         {} enterprises, {} countries, {} gateways; legs: {} domestic, {} access, {} crossings; \
         population {:.4e}; from_pack {load:?}",
        inputs.n_provinces(),
        inputs.n_pops(),
        inputs.n_enterprises(),
        inputs.n_countries(),
        net.n_gateways(),
        net.domestic.len(),
        net.access.len(),
        net.crossings.len(),
        inputs.pops.population.iter().sum::<f64>(),
    );
    assert!(left_out < n / 10);
}

/// Share of the country pairs of `pairs` in the international flows of the last 12 ticks, by
/// value and by units, all goods. Shares compare runs whose price levels differ.
fn pair_shares(w: &EconWorld, pairs: &[(usize, usize)]) -> (f64, f64) {
    let nc = w.inputs().n_countries();
    let mean = w.scratch.flows.mean();
    let cells = |ex: usize, im: usize| (0..N_GOODS).map(move |g| (ex * nc + im) * N_GOODS + g);
    let sum = |x: &[f64], keys: &mut dyn Iterator<Item = usize>| keys.map(|k| x[k]).sum::<f64>();
    let foreign = || {
        (0..nc)
            .flat_map(move |ex| (0..nc).filter(move |&im| im != ex).map(move |im| (ex, im)))
            .flat_map(move |(ex, im)| cells(ex, im))
    };
    let chosen = || pairs.iter().flat_map(move |&(ex, im)| cells(ex, im));
    (
        sum(&mean.pair_value, &mut chosen()) / sum(&mean.pair_value, &mut foreign()),
        sum(&mean.pair_units, &mut chosen()) / sum(&mean.pair_units, &mut foreign()),
    )
}

/// The cheapest crossing cost per (exporter, importer) country pair.
fn cheapest_crossings(net: &ceteris_trade::Network) -> BTreeMap<(usize, usize), f64> {
    let mut out = BTreeMap::new();
    for l in &net.crossings {
        let key = (
            usize::from(net.gateway_country[l.from as usize]),
            usize::from(net.gateway_country[l.to as usize]),
        );
        let c = out.entry(key).or_insert(f64::INFINITY);
        *c = l.cost_per_t.min(*c);
    }
    out
}

fn assert_finite(w: &EconWorld, when: &str) {
    let m = &w.markets;
    let bad_price =
        (0..m.price.len()).find(|&c| m.active[c] && !(m.price[c].is_finite() && m.price[c] > 0.0));
    assert!(bad_price.is_none(), "{when}: price at cell {bad_price:?}");
    assert!(
        w.wage.iter().all(|&x| x.is_finite() && x > 0.0),
        "{when}: a wage is not finite and positive"
    );
    assert!(
        w.world_price_index().is_finite(),
        "{when}: world price index"
    );
}

/// Review Focus 3 on the full pack: 12 ticks from the data state, then the Suez Canal closes,
/// the network is rebuilt from the graph (`network_from_graph`, `set_network`) and the run
/// goes on for 24 ticks with every invariant checked. Against a control run without the
/// closure, the country pairs whose cheapest crossing went through Suez (more than 1% dearer
/// without it) carry a smaller share of international trade, by value and by units.
#[test]
fn kernel_rebuilds_after_chokepoint_closure() {
    const BEFORE: u32 = 12;
    const AFTER: u32 = 24;
    let Some((mut pack, dir)) = built_pack() else {
        return;
    };
    let gateways = ceteris_econ::pack::Gateways::read(&dir).unwrap();
    let inputs = Arc::new(ceteris_econ::EconInputs::from_pack(&pack, &dir).unwrap());
    let sim = |inputs: &Arc<ceteris_econ::EconInputs>| {
        EconWorld::sim(
            Arc::clone(inputs),
            EconParams::default(),
            TradeParams::default(),
        )
    };
    let (mut shocked, mut control) = (sim(&inputs), sim(&inputs));
    let started = std::time::Instant::now();
    for s in [&mut shocked, &mut control] {
        s.run_until(BEFORE).unwrap_or_else(|v| panic!("{v}"));
    }
    let per_tick = started.elapsed() / (2 * BEFORE);
    assert_finite(shocked.world(), "tick 12");
    println!(
        "full pack, 12 ticks from the data state: no violation, prices and wages finite, \
         world price index {}; {per_tick:?} per tick",
        shocked.world().world_price_index()
    );

    let suez = pack.chokepoint("Suez Canal").expect("Suez in the pack");
    pack.graph
        .set_chokepoint_closed(suez, true, &TransportParams::default());
    let closed = inputs.network_from_graph(&pack, &gateways);
    let open = &inputs.network;
    let legs = |n: &ceteris_trade::Network| (n.domestic.len(), n.access.len(), n.crossings.len());
    assert_eq!(
        legs(&closed),
        legs(open),
        "every leg stays reachable without Suez"
    );
    let (open_cost, closed_cost) = (cheapest_crossings(open), cheapest_crossings(&closed));
    let dependent: Vec<(usize, usize)> = open_cost
        .iter()
        .filter(|&(k, &c)| closed_cost[k] > 1.01 * c)
        .map(|(&k, _)| k)
        .collect();
    assert!(!dependent.is_empty());
    let before = pair_shares(control.world(), &dependent);
    shocked.world_mut().set_network(closed);
    assert_ne!(shocked.world().digest(), control.world().digest());

    for s in [&mut shocked, &mut control] {
        s.run_until(BEFORE + AFTER)
            .unwrap_or_else(|v| panic!("{v}"));
    }
    assert_finite(shocked.world(), "after the closure");
    let (with, without) = (
        pair_shares(shocked.world(), &dependent),
        pair_shares(control.world(), &dependent),
    );
    println!(
        "Suez closed at tick {BEFORE}: {} of {} country pairs depend on it; their share of \
         international trade (value, units): ticks 1..12 {before:.4?}, ticks {}..{} with the \
         closure {with:.4?}, without {without:.4?}; world price index {} with, {} without",
        dependent.len(),
        open_cost.len(),
        BEFORE + AFTER - 11,
        BEFORE + AFTER,
        shocked.world().world_price_index(),
        control.world().world_price_index(),
    );
    assert!(
        with.0 < without.0 && with.1 < without.1,
        "Suez-dependent shares {with:?} did not fall below {without:?}"
    );
}

/// The full pack's calibrated inputs, or None (with a notice) where the default pack has not
/// been built or calibrated, as in CI; a pack named by `CETERIS_PACK` must be calibrated.
fn calibrated_pack() -> Option<Arc<EconInputs>> {
    let dir = ceteris_geo::pack::default_dir();
    if !EconInputs::has_calibration(&dir) {
        let hint = format!(
            "no calibrated pack at {}; build it with `uv run python -m ceteris.packs` and \
             install its calibration from `python -m ceteris.reference calibrate --pack`",
            dir.display()
        );
        assert!(std::env::var_os("CETERIS_PACK").is_none(), "{hint}");
        eprintln!("skipping: {hint}");
        return None;
    }
    let pack =
        ceteris_geo::pack::Pack::load(&dir, &TransportParams::default()).expect("pack loads");
    Some(Arc::new(EconInputs::from_pack(&pack, &dir).unwrap()))
}

/// What the 100-year run records each tick besides the invariants: pops holding negative
/// money (allowed: every account may run a negative balance, SPEC Money) and with negative
/// income, countries whose profit pool (what their enterprises and markets paid in at E4,
/// read from the tick's journal) was negative, enterprises carrying retained losses (limited
/// liability), and traded origin markets that ran out (orders above stock plus output).
#[derive(Default)]
struct Census {
    negative_money_ticks: u32,
    negative_money_max: usize,
    worst_money: (f64, u32, usize),
    negative_income_pops: usize,
    negative_pool: (u32, usize),
    retained: (u32, usize, f64),
    stockouts: [u32; N_GOODS],
    first_stockout: Option<(u32, usize)>,
}

impl Census {
    fn record(&mut self, w: &EconWorld, tick: u32) {
        let (inp, lay) = (w.inputs(), w.layout());
        let mut negative = 0;
        for (k, &a) in lay.accounts.pop.iter().enumerate() {
            let money = to_numeraire(w.ledger.money(a));
            if money < 0.0 {
                negative += 1;
                if money < self.worst_money.0 {
                    self.worst_money = (money, tick, k);
                }
            }
        }
        if negative > 0 {
            self.negative_money_ticks += 1;
        }
        self.negative_money_max = self.negative_money_max.max(negative);
        self.negative_income_pops = self
            .negative_income_pops
            .max(w.income.iter().filter(|&&x| x < 0.0).count());
        let acc = &lay.accounts;
        let mut pool = vec![0i64; inp.n_countries()];
        for x in w.ledger.journal().iter().filter(|x| x.flow == DIVIDEND) {
            if let Some(c) = acc.dividend.iter().position(|&a| a == x.to) {
                pool[c] += x.amount;
            }
        }
        let losing = pool.iter().filter(|&&x| x < 0).count();
        if losing > 0 {
            self.negative_pool.0 += 1;
            self.negative_pool.1 = self.negative_pool.1.max(losing);
        }
        let carrying: Vec<i64> = acc
            .enterprise
            .iter()
            .map(|&a| w.ledger.money(a))
            .filter(|&x| x < 0)
            .collect();
        if !carrying.is_empty() {
            self.retained.0 += 1;
            self.retained.1 = self.retained.1.max(carrying.len());
            self.retained.2 = self.retained.2.min(to_numeraire(carrying.iter().sum()));
        }
        let m = &w.markets;
        for c in (0..m.price.len()).filter(|&c| m.active[c] && TRADED[c % N_GOODS]) {
            if m.sales[c] < m.demand[c] {
                self.stockouts[c % N_GOODS] += 1;
                self.first_stockout.get_or_insert((tick, c));
            }
        }
    }
}

/// P2 gate, 100 years (econ SPEC, Invariants, Long run): the default run on the full pack
/// from its calibrated steady state, 1,200 ticks with every invariant checked each tick and
/// every active price, every wage and the world price index finite and positive after each
/// tick. Prints the census of negative money, negative dividend pools and stockouts.
#[test]
fn hundred_years_without_violation() {
    let Some(inputs) = calibrated_pack() else {
        return;
    };
    let started = std::time::Instant::now();
    let mut sim = EconWorld::sim(inputs, EconParams::default(), TradeParams::default());
    let mut census = Census::default();
    for tick in 1..=TICKS {
        sim.step().unwrap_or_else(|v| panic!("tick {tick}: {v}"));
        let w = sim.world();
        assert_finite(w, &format!("tick {tick}"));
        census.record(w, tick);
    }
    let w = sim.world();
    let (money, tick, k) = census.worst_money;
    println!(
        "full pack, {TICKS} ticks of the default run: no violation, prices and wages finite and \
         positive, world price index {}; pops with negative money: {} ticks, at most {} pops, \
         worst {money} (pop {k}, province {}, tick {tick}); at most {} pops with negative income; \
         negative dividend pools: {} ticks, at most {} countries; enterprises with retained \
         losses: {} ticks, at most {}, largest total {}; stockouts by good (cell ticks) {:?}, \
         first {:?}; {:?}",
        w.world_price_index(),
        census.negative_money_ticks,
        census.negative_money_max,
        w.inputs().pops.province.get(k).copied().unwrap_or(0),
        census.negative_income_pops,
        census.negative_pool.0,
        census.negative_pool.1,
        census.retained.0,
        census.retained.1,
        census.retained.2,
        census.stockouts,
        census.first_stockout,
        started.elapsed(),
    );
}

/// Determinism on the full pack (V0): 120 ticks of the default run give identical snapshot
/// hashes at 1 and 8 threads.
#[test]
fn full_pack_one_vs_eight_threads() {
    const PACK_TICKS: u32 = 120;
    let Some(inputs) = calibrated_pack() else {
        return;
    };
    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let started = std::time::Instant::now();
        let hashes = pool.install(|| {
            let mut sim = EconWorld::sim(
                Arc::clone(&inputs),
                EconParams::default(),
                TradeParams::default(),
            );
            sim.run_until(PACK_TICKS).unwrap_or_else(|v| panic!("{v}"));
            sim.snapshot_hashes()
        });
        (hashes, started.elapsed())
    };
    let ((one, t1), (eight, t8)) = (run(1), run(8));
    println!(
        "full pack, {PACK_TICKS} ticks: {} snapshot hashes, 1 thread {t1:?}, 8 threads {t8:?}",
        one.len()
    );
    assert_eq!(one.len() as u32, PACK_TICKS / SNAPSHOT_EVERY + 1);
    assert_eq!(one, eight);
}

/// Measurement for the P2 gate's V1 from the data state (ADR 0112): the frictionless run of
/// the calibrated pack from the data state of its inputs, `TICKS_CENSUS` ticks (default
/// 4,800), printing at checkpoints the world price index, the enterprises carrying retained
/// losses and their total, pops with negative money, and the write-offs so far by country.
#[test]
#[ignore = "measurement, about 3 minutes"]
fn data_state_write_off_census() {
    let Some(inputs) = calibrated_pack() else {
        return;
    };
    let mut inputs = (*inputs).clone();
    inputs.steady = None;
    let ticks: u32 = std::env::var("TICKS_CENSUS").map_or(4800, |v| v.parse().unwrap());
    let mut sim = EconWorld::sim(
        Arc::new(inputs),
        EconParams::frictionless(),
        TradeParams::default(),
    );
    let mut by_country: BTreeMap<usize, i128> = BTreeMap::new();
    let mut most_negative = (0usize, 0u32);
    let checkpoints = [
        12, 24, 36, 48, 60, 120, 240, 360, 480, 600, 840, 1200, 1800, 2400, 3600,
    ];
    for t in 1..=ticks {
        sim.step().unwrap_or_else(|v| panic!("tick {t}: {v}"));
        let w = sim.world();
        let acc = &w.layout().accounts;
        for x in w.ledger.journal().iter().filter(|x| x.flow == WRITE_OFF) {
            if let Some(c) = acc.dividend.iter().position(|&a| a == x.from) {
                *by_country.entry(c).or_default() += i128::from(x.amount);
            }
        }
        let negative = acc.pop.iter().filter(|&&a| w.ledger.money(a) < 0).count();
        if negative > most_negative.0 {
            most_negative = (negative, t);
        }
        if checkpoints.contains(&t) || t == ticks {
            let retained: Vec<i64> = acc
                .enterprise
                .iter()
                .map(|&a| w.ledger.money(a))
                .filter(|&x| x < 0)
                .collect();
            let mut top: Vec<(usize, i128)> = by_country.iter().map(|(&c, &v)| (c, v)).collect();
            top.sort_by_key(|x| -x.1);
            let numeraire = |minor: i128| minor as f64 / ceteris_econ::world::MINOR_PER_UNIT;
            let top: Vec<String> = top
                .iter()
                .take(8)
                .map(|&(c, v)| format!("{c}:{:.3e}", numeraire(v)))
                .collect();
            println!(
                "tick {t}: index {:.4}; {} enterprises with retained losses, total {:.4e}; {negative} \
                 pops with negative money; written off {:.4e} in {} countries, largest {}",
                w.world_price_index(),
                retained.len(),
                to_numeraire(retained.iter().sum()),
                numeraire(by_country.values().sum()),
                by_country.len(),
                top.join(" ")
            );
        }
    }
    println!(
        "most pops with negative money {} at tick {}",
        most_negative.0, most_negative.1
    );
}
