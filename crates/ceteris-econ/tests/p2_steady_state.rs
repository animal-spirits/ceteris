//! ADR 0089: the calibrated data state is the model's steady state. From the calibration
//! tables (`ceteris calibrate`), a run with every friction on stays at the calibrated state: 480
//! ticks on the fixture (`tests/golden/p2-fixture/calibration`) and, when the pack holds its
//! calibration, 120 on the full 2000 pack. Run with `--release`; the pack run takes
//! about 20 s.
//!
//! Acceptance (owner decision 2026-10-25, a resolution-aware test; controller ruling on its
//! form, ADR 0089 As built): on the fixture every quantity holds to 1e-9 relative. On the full
//! pack money is integer minor units (1e-4 USD), and national dividends and domestic trade
//! carry each pop's rounding through its country, so prices and flows hold to 2e-6 relative,
//! money (pop income and balances) to 2e-6 of the pop's steady monthly income or 4 minor units,
//! whichever is larger (P156), and world aggregates (price index, income, unemployment) to
//! 1e-9. A balance's error accumulates from one tick's flows through it, wage, dividends and
//! rebates in and spending out, each carrying the relative error eps of the real drift:
//! |dm| <= eps (W + Div + Reb + S) ~ 2 eps y, y the monthly income. A tolerance relative to the
//! balance itself reads that error against the non-wage share of income only (0.22 in Albania,
//! 0.32 in Pitcairn), so it scales with income y_k; the factor 2 is not taken, so 2e-6 y_k is
//! the tighter of the two bounds (owner approval 2026-10-30). Every quantity's
//! drift stays bounded: its largest value over the second half of the run, scaled by its
//! tolerance, is at most twice that over ticks 5 to the half (ticks 1 to 4, where prices and
//! wages settle onto the rounding lattice, left out). The fixture's rounding drift peaks near
//! tick 105 and falls after, so a 120-tick window would read that peak as growth.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::world::{MINOR_PER_UNIT, to_numeraire};
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_trade::TradeParams;

/// Ticks of the fixture's and the pack's runs; the boundedness check splits each at its half.
const FIXTURE_TICKS: u32 = 480;
const PACK_TICKS: u32 = 120;
const SETTLE: u32 = 4;
/// Relative drift below which a quantity is at f64 rounding (about 450 ulps): it wanders
/// rather than grows, so the boundedness check does not apply.
const ROUNDING: f64 = 1e-13;
/// The relative tolerance of ADR 0089, and of world aggregates on the pack.
const EXACT: f64 = 1e-9;
/// The pack's relative tolerance of prices, flows and money.
const PACK_RELATIVE: f64 = 2e-6;
/// The pack's absolute tolerance of money per pop, in minor units.
const PACK_MINOR: f64 = 4.0;

fn fixture_dir() -> PathBuf {
    std::env::var_os("CETERIS_FIXTURE_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture"),
        PathBuf::from,
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    /// Prices and flows in units.
    Real,
    /// Money per pop, numeraire.
    Money,
    /// One world number.
    World,
}

/// The tolerances of a world: relative for real quantities; for money the larger of a
/// relative bound and an absolute one in minor units; relative for world aggregates.
#[derive(Clone, Copy, Debug)]
struct Tolerance {
    real: f64,
    money: f64,
    minor: f64,
    world: f64,
    /// Money relative to the pop's steady monthly income (P156) instead of the quantity itself.
    money_of_income: bool,
}

/// Every quantity at 1e-9 relative (the fixture).
const FIXTURE: Tolerance = Tolerance {
    real: EXACT,
    money: EXACT,
    minor: 0.0,
    world: EXACT,
    money_of_income: false,
};
const PACK: Tolerance = Tolerance {
    real: PACK_RELATIVE,
    money: PACK_RELATIVE,
    minor: PACK_MINOR,
    world: EXACT,
    money_of_income: true,
};

/// A drift over its tolerance; no drift is 0 even where the tolerance is 0 (a zero balance
/// on the fixture).
fn over(drift: f64, tolerance: f64) -> f64 {
    if drift == 0.0 { 0.0 } else { drift / tolerance }
}

/// A quantity: name, kind and entries.
type Quantity = (&'static str, Kind, Vec<f64>);

/// The quantities the fixed point holds: prices of active cells, buyer prices that are
/// finite, wages, employment, output and sales per enterprise, Q, D, sales and merchant stock
/// per active cell; each pop's income and money balance; the world price index, world
/// household income and world unemployment.
fn quantities(w: &EconWorld) -> Vec<Quantity> {
    let m = &w.markets;
    let active = |x: &[f64]| -> Vec<f64> {
        x.iter()
            .zip(&m.active)
            .filter(|&(_, &on)| on)
            .map(|(&v, _)| v)
            .collect()
    };
    let finite = w
        .price_index
        .iter()
        .map(|&p| if p.is_finite() { p } else { 0.0 })
        .collect();
    let lay = w.layout();
    let money = lay
        .accounts
        .pop
        .iter()
        .map(|&a| to_numeraire(w.ledger.money(a)))
        .collect();
    let employed: f64 = w.employment.iter().sum();
    let supply: f64 = lay.labour_supply.iter().sum();
    vec![
        ("price", Kind::Real, active(&m.price)),
        ("buyer price", Kind::Real, finite),
        ("wage", Kind::Real, w.wage.clone()),
        ("employment", Kind::Real, w.employment.clone()),
        ("output", Kind::Real, w.output.clone()),
        ("enterprise sales", Kind::Real, w.sales.clone()),
        ("supply Q", Kind::Real, active(&m.supply)),
        ("demand D", Kind::Real, active(&m.demand)),
        ("market sales", Kind::Real, active(&m.sales)),
        ("merchant stock", Kind::Real, active(&m.inventory)),
        ("pop income", Kind::Money, w.income.clone()),
        ("pop money", Kind::Money, money),
        (
            "world price index",
            Kind::World,
            vec![w.world_price_index()],
        ),
        ("world income", Kind::World, vec![w.income.iter().sum()]),
        (
            "world unemployment",
            Kind::World,
            vec![1.0 - employed / supply],
        ),
    ]
}

/// One quantity's record over the run.
#[derive(Debug)]
struct Record {
    name: &'static str,
    kind: Kind,
    /// Largest relative drift (entries at 0 measured against the largest entry), its tick and
    /// entry.
    relative: f64,
    tick: u32,
    entry: usize,
    /// Largest absolute drift in minor units, its tick and entry.
    minor: f64,
    minor_tick: u32,
    minor_entry: usize,
    /// Largest drift over its tolerance, over the run and over ticks SETTLE + 1 to the half and
    /// the half + 1 to the end.
    over: f64,
    early: f64,
    late: f64,
    /// Largest relative drift over the second half.
    late_relative: f64,
}

/// Runs `ticks` ticks from the steady state of `inputs` and returns the record of every
/// quantity under `tol`; prints them, the world price index path and the time per tick.
fn run(inputs: EconInputs, ticks: u32, tol: Tolerance, what: &str) -> Vec<Record> {
    let split = ticks / 2;
    assert!(inputs.steady.is_some(), "{what}: inputs are not calibrated");
    let trade = inputs.trade_params();
    let world = EconWorld::new(Arc::new(inputs), EconParams::default(), trade);
    let steady = world.inputs().steady.clone().unwrap();
    let mut base = quantities(&world);
    // the calibrated state's last tick, which the first tick must reproduce
    for (name, _, values) in &mut base {
        match *name {
            "output" => values.clone_from(&steady.output),
            "enterprise sales" => values.clone_from(&steady.ent_sales),
            "pop income" => values.clone_from(&steady.income),
            "pop money" => values.clone_from(&steady.money),
            "world income" => *values = vec![steady.income.iter().sum()],
            _ => {}
        }
    }
    let mut records: Vec<Record> = base
        .iter()
        .map(|&(name, kind, _)| Record {
            name,
            kind,
            relative: 0.0,
            tick: 0,
            entry: 0,
            minor: 0.0,
            minor_tick: 0,
            minor_entry: 0,
            over: 0.0,
            early: 0.0,
            late: 0.0,
            late_relative: 0.0,
        })
        .collect();
    let mut sim = Sim::new(world, schedule(), SEED);
    let mut index = Vec::new();
    let started = std::time::Instant::now();
    for tick in 1..=ticks {
        sim.step().unwrap_or_else(|v| panic!("{what}: {v}"));
        let w = sim.world();
        index.push(w.world_price_index());
        for (r, ((_, kind, b), (_, _, now))) in
            records.iter_mut().zip(base.iter().zip(quantities(w)))
        {
            let scale = b.iter().fold(0.0f64, |a, &x| a.max(x.abs()));
            for (entry, (&b, &n)) in b.iter().zip(&now).enumerate() {
                let gap = (n - b).abs();
                let relative = gap / if b != 0.0 { b.abs() } else { scale };
                let minor = gap * MINOR_PER_UNIT;
                let scaled = match kind {
                    Kind::Real => over(relative, tol.real),
                    Kind::World => over(relative, tol.world),
                    Kind::Money => {
                        let of = if tol.money_of_income {
                            steady.income[entry]
                        } else {
                            b
                        };
                        over(minor, tol.minor.max(tol.money * of.abs() * MINOR_PER_UNIT))
                    }
                };
                if relative > r.relative {
                    (r.relative, r.tick, r.entry) = (relative, tick, entry);
                }
                if minor > r.minor {
                    (r.minor, r.minor_tick, r.minor_entry) = (minor, tick, entry);
                }
                r.over = r.over.max(scaled);
                if tick > SETTLE && tick <= split {
                    r.early = r.early.max(scaled);
                } else if tick > split {
                    r.late = r.late.max(scaled);
                    r.late_relative = r.late_relative.max(relative);
                }
            }
        }
    }
    let per_tick = started.elapsed() / ticks;
    println!("{what}: {ticks} ticks from the calibrated state, {per_tick:?} per tick");
    for r in &records {
        println!(
            "  {:>18} ({:?}): max relative drift {:.3e} (tick {}, entry {}), max absolute \
             {:.3} minor units (tick {}, entry {}); max drift / tolerance {:.3e}, ticks \
             {}-{split} {:.3e}, {}-{ticks} {:.3e}",
            r.name,
            r.kind,
            r.relative,
            r.tick,
            r.entry,
            r.minor,
            r.minor_tick,
            r.minor_entry,
            r.over,
            SETTLE + 1,
            r.early,
            split + 1,
            r.late
        );
    }
    let (lo, hi) = index
        .iter()
        .fold((f64::INFINITY, 0.0f64), |a, &x| (a.0.min(x), a.1.max(x)));
    println!(
        "  world price index: tick 1 {:.15}, tick {ticks} {:.15}, range [{lo:.15}, {hi:.15}]",
        index[0],
        index[ticks as usize - 1]
    );
    records
}

fn assert_fixed(records: &[Record], ticks: u32, what: &str) {
    let split = ticks / 2;
    for r in records {
        assert!(
            r.over <= 1.0,
            "{what}: {} reaches {:.3} times its tolerance (relative {:.3e} at tick {}, {:.3} \
             minor units at tick {})",
            r.name,
            r.over,
            r.relative,
            r.tick,
            r.minor,
            r.minor_tick
        );
        assert!(
            r.late <= 2.0 * r.early || r.late_relative <= ROUNDING,
            "{what}: {} is not bounded: ticks {}-{ticks} reach {:.3e} of the tolerance, ticks \
             {}-{split} {:.3e}",
            r.name,
            split + 1,
            r.late,
            SETTLE + 1,
            r.early
        );
    }
}

#[test]
fn fixture_calibrated_state_holds_to_1e_9() {
    let mut inputs = small_world();
    inputs
        .calibrate(&fixture_dir().join("calibration"))
        .unwrap();
    let records = run(inputs, FIXTURE_TICKS, FIXTURE, "fixture");
    assert_fixed(&records, FIXTURE_TICKS, "fixture");
}

/// Calibration tables of other inputs do not load: the static digest guards them.
#[test]
fn calibration_of_other_inputs_does_not_load() {
    let mut inputs = small_world();
    inputs.pops.labour_force[0] *= 1.01;
    let err = inputs
        .calibrate(&fixture_dir().join("calibration"))
        .unwrap_err();
    assert!(err.0.contains("other inputs"), "{err}");
}

fn calibrated_fixture() -> Arc<EconInputs> {
    let mut inputs = small_world();
    inputs
        .calibrate(&fixture_dir().join("calibration"))
        .unwrap();
    Arc::new(inputs)
}

/// The calibrated start belongs to the structural parameters it was built under.
#[test]
#[should_panic(expected = "calibrated steady state")]
fn calibrated_start_rejects_other_theta() {
    let mut trade = TradeParams::default();
    trade.theta[0] *= 1.1;
    EconWorld::new(calibrated_fixture(), EconParams::default(), trade);
}

/// A calibration solved at other border frictions than the defaults (ADR 0092).
fn fixture_at_other_nu() -> Arc<EconInputs> {
    let mut inputs = small_world();
    inputs
        .calibrate(&fixture_dir().join("calibration"))
        .unwrap();
    inputs.border_nu = Some([0.2, 0.4, 0.6, 0.8]);
    Arc::new(inputs)
}

/// The calibrated border friction is part of the parameters the steady state holds under: the
/// world starts under `EconInputs::trade_params`, which carries it.
#[test]
fn calibrated_start_takes_the_calibrated_border_nu() {
    let inputs = fixture_at_other_nu();
    let trade = inputs.trade_params();
    assert_eq!(trade.border_nu, [0.2, 0.4, 0.6, 0.8]);
    assert_eq!(
        TradeParams {
            border_nu: TradeParams::default().border_nu,
            ..trade.clone()
        },
        TradeParams::default()
    );
    EconWorld::new(inputs, EconParams::default(), trade);
}

/// ... and the default border friction is then a structural change.
#[test]
#[should_panic(expected = "calibrated steady state")]
fn calibrated_start_rejects_the_default_border_nu() {
    EconWorld::new(
        fixture_at_other_nu(),
        EconParams::default(),
        TradeParams::default(),
    );
}

/// The adjustment speeds do not move the steady state, so they load.
#[test]
fn calibrated_start_accepts_other_speeds() {
    let trade = TradeParams {
        kappa1: 0.02,
        expectation_gain: 0.01,
        ..TradeParams::default()
    };
    let params = EconParams {
        kappa_w: 0.2,
        ..EconParams::default()
    };
    EconWorld::new(calibrated_fixture(), params, trade);
}

/// ADR 0263: the calibrated fixture carries each market's elasticities; at the default eta no
/// market's J reaches J_ref = 20, so every gain is kappa1 and the default dynamics are those
/// before the cap. Frictionless (eta 1) J is reported with the cells it caps.
#[test]
fn no_cell_capped_in_the_default_mode() {
    let inputs = calibrated_fixture();
    let m = inputs
        .market_elasticity
        .as_ref()
        .expect("calibrated elasticities");
    let trade = inputs.trade_params();
    let active: Vec<usize> = (0..m.demand.len())
        .filter(|&i| m.demand[i] > 0.0 || m.supply[i] > 0.0)
        .collect();
    assert!(!active.is_empty());
    for (params, what) in [
        (EconParams::default(), "default"),
        (EconParams::frictionless(), "frictionless"),
    ] {
        let w = EconWorld::new(Arc::clone(&inputs), params.clone(), trade.clone());
        let (top, cell) = active
            .iter()
            .map(|&i| (m.j(i, params.eta, &trade), i))
            .fold((0.0, 0), |a, b| if b.0 > a.0 { b } else { a });
        let capped = w
            .layout()
            .price_gain
            .iter()
            .filter(|&&k| k != trade.kappa1)
            .count();
        eprintln!("{what}: largest J {top:.3} at cell {cell}, {capped} cells capped");
        if what == "default" {
            assert!(
                top < 2.0 * trade.gain_share / trade.kappa1,
                "J {top} at cell {cell}"
            );
            assert_eq!(capped, 0);
        }
    }
}

#[test]
fn pack_calibrated_state_holds_to_2e_6_and_4_minor_units() {
    let dir = std::env::var_os("CETERIS_PACK").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/packs/2000"),
        PathBuf::from,
    );
    if !EconInputs::has_calibration(&dir) {
        let hint = format!(
            "no calibration tables in {}; install them from `ceteris calibrate --pack {}`",
            dir.display(),
            dir.display()
        );
        // a pack named explicitly must be testable; only the absent default pack skips (CI)
        assert!(std::env::var_os("CETERIS_PACK").is_none(), "{hint}");
        eprintln!("skipping: {hint}");
        return;
    }
    let pack =
        ceteris_geo::pack::Pack::load(&dir, &TransportParams::default()).expect("pack loads");
    let inputs = EconInputs::from_pack(&pack, &dir).unwrap();
    let records = run(inputs, PACK_TICKS, PACK, "full pack");
    assert_fixed(&records, PACK_TICKS, "full pack");
}
