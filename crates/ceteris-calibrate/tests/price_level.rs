//! ADR 0098, local base price p_c: the GE reference reads the price level the simulator exports,
//! agrees with the run from the data state of a fixture with p_c != 1 (V1 style), and the
//! calibration keeps local goods at p_c with the normalization over traded goods only.

use std::sync::{Arc, OnceLock};

use ceteris_calibrate::{steady, write};
use ceteris_core::sim::Sim;
use ceteris_econ::export::write_inputs;
use ceteris_econ::fixture::small_world;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_reference::io::{self, Inputs};
use ceteris_reference::model::Model;
use ceteris_reference::solve::{self, TOL};
use ceteris_trade::{Kernel, TradeParams};
use tempfile::TempDir;

/// The country whose price level is lifted, and its level.
const LIFTED: (usize, f64) = (0, 1.3);
/// V1's horizons (`p2_acceptance.rs`).
const TICKS: u32 = 1200;
const STORAGE_TICKS: u32 = 6000;
/// V1's tolerance mode (ADR 0022).
const V1_TOL: f64 = 0.01;

/// The fixture with country `LIFTED.0` at price level `LIFTED.1`, the rest at 1.
fn lifted() -> &'static EconInputs {
    static INPUTS: OnceLock<EconInputs> = OnceLock::new();
    INPUTS.get_or_init(|| {
        let mut inputs = small_world();
        let mut levels = vec![1.0; inputs.n_countries()];
        levels[LIFTED.0] = LIFTED.1;
        inputs.set_price_levels(levels, &TradeParams::default());
        inputs.set_base_point(&EconParams::default().edu_eff);
        inputs.validate();
        inputs
    })
}

/// The reference's inputs from the simulator's export of [`lifted`].
fn exported() -> &'static Inputs {
    static INPUTS: OnceLock<Inputs> = OnceLock::new();
    INPUTS.get_or_init(|| {
        let dir = TempDir::new().unwrap();
        let (params, trade) = (EconParams::default(), TradeParams::default());
        let inputs = lifted();
        let kernel = Kernel::build(&inputs.network, &inputs.weight, &inputs.wedges, &trade);
        write_inputs(dir.path(), inputs, &kernel, &params, &trade).unwrap();
        io::load(dir.path()).unwrap()
    })
}

fn rel(got: f64, want: f64) -> f64 {
    (got / want - 1.0).abs()
}

/// The reference reads p_c as the base price of local goods: its data-state prices, input
/// coefficients, a_VA and PIGL weights are the simulator's.
#[test]
fn reference_reads_the_price_level() {
    let (econ, inp) = (lifted(), exported());
    let (c, level) = LIFTED;
    let price = inp.data_state_price();
    for p in 0..inp.n_provinces() {
        for g in 0..inp.n_goods() {
            let want = econ.data_state_price(p, g);
            assert_eq!(price[(p, g)], want, "province {p} good {g}");
            if !inp.traded[g] {
                let lifted = inp.province_country[p] == c;
                assert_eq!(want, if lifted { level } else { 1.0 });
            }
        }
    }
    let (coef, a_va) = (inp.ent_inputs(), inp.ent_a_va());
    for e in 0..inp.ent_good.len() {
        assert_eq!(inp.ent_a[e], econ.enterprises.a[e], "enterprise {e}: A");
        assert_eq!(a_va[e], econ.enterprises.a_va[e], "enterprise {e}: a_VA");
        for k in 0..inp.n_goods() {
            assert_eq!(
                coef[(e, k)],
                econ.enterprises.coef[e][k],
                "enterprise {e} input {k}"
            );
        }
    }
    let w = EconWorld::new(
        Arc::new(econ.clone()),
        EconParams::default(),
        TradeParams::default(),
    );
    let weights = inp.pigl_weights().unwrap();
    for (k, rust) in w.layout().pigl.iter().enumerate() {
        assert!(
            rel(weights.scale[k], rust.scale) <= 1e-12,
            "country {k}: scale"
        );
        for (j, &o) in rust.omega.iter().enumerate() {
            assert!(
                (weights.omega[(k, j)] - o).abs() <= 1e-12,
                "country {k}: omega {j}"
            );
        }
        for (g, &a) in rust.within.iter().enumerate() {
            assert!(
                (weights.within[(k, g)] - a).abs() <= 1e-12,
                "country {k}: within {g}"
            );
        }
    }
}

/// V1 on the fixture with a price level (econ SPEC, Equilibrium nesting): the frictionless run
/// from its data state reaches the GE reference solved from its export, prices and wages over
/// the world price index within V1's 1%.
#[test]
fn v1_price_level_matches_reference() {
    let inp = exported();
    let eq = solve::equilibrium(&Model::new(inp).unwrap(), None, TOL, None).unwrap();
    let trade = TradeParams::default();
    let ng = inp.n_goods();
    let ticks = if (0..ng).any(|g| trade.stores(g)) {
        STORAGE_TICKS
    } else {
        TICKS
    };
    let mut sim = Sim::new(
        EconWorld::new(
            Arc::new(lifted().clone()),
            EconParams::frictionless(),
            trade,
        ),
        ceteris_econ::systems::schedule(),
        ceteris_econ::systems::SEED,
    );
    sim.run_until(ticks).unwrap_or_else(|v| panic!("{v}"));
    let w = sim.world();
    let index = w.world_price_index();
    let (mut price_dev, mut price_cell) = (0.0, 0);
    for c in (0..w.markets.price.len()).filter(|&c| w.markets.active[c]) {
        let d = rel(w.markets.price[c] / index, eq.price.data[c]);
        if d > price_dev {
            (price_dev, price_cell) = (d, c);
        }
    }
    let (mut wage_dev, mut wage_province) = (0.0, 0);
    for (i, &wage) in w.wage.iter().enumerate() {
        let d = rel(wage / index, eq.wage[i]);
        if d > wage_dev {
            (wage_dev, wage_province) = (d, i);
        }
    }
    // the lifted country's local prices relative to the others', run and reference
    let (c, level) = LIFTED;
    let local = (0..inp.n_provinces())
        .flat_map(|p| (0..ng).map(move |g| (p, g)))
        .filter(|&(p, g)| !inp.traded[g] && inp.province_country[p] == c && inp.active[p][g])
        .map(|(p, g)| eq.price[(p, g)])
        .fold(0.0, f64::max);
    println!(
        "V1 with p_c {level} in country {c}: price {price_dev:e} (province {} good {}), wage \
         {wage_dev:e} (province {wage_province}), price index {index}, largest local price of \
         country {c} in the reference {local}",
        price_cell / ng,
        price_cell % ng,
    );
    assert!(price_dev <= V1_TOL && wage_dev <= V1_TOL);
}

/// Calibration (econ SPEC, Calibration, stages 2 and 3): local goods stay at p_c, so their A is
/// 1 / p_c, their output Y = V / p_c and their coefficients a_kg p_c / q, while each traded good's
/// prices are normalized as without a price level.
#[test]
fn calibration_keeps_local_goods_at_the_price_level() {
    let inp = exported();
    let s = steady::calibrate(inp, &inp.u_star.clone(), &mut |_: &str| {}).unwrap();
    let level = inp.data_state_price();
    let mut lifted = 0;
    for p in 0..inp.n_provinces() {
        for g in (0..inp.n_goods()).filter(|&g| !inp.traded[g] && inp.active[p][g]) {
            assert_eq!(s.price[(p, g)], level[(p, g)], "province {p} good {g}");
            assert_eq!(s.consumer[(p, g)], level[(p, g)], "province {p} good {g}");
            lifted += usize::from(level[(p, g)] != 1.0);
        }
    }
    assert!(lifted > 0);
    for (e, &country) in inp.ent_country().iter().enumerate() {
        let (p, g) = (inp.ent_province[e], inp.ent_good[e]);
        let own = s.price[(p, g)];
        assert_eq!(s.a[e], 1.0 / own, "enterprise {e}: A");
        assert!(
            rel(s.output[e] * own, s.value[e]) <= 1e-15,
            "enterprise {e}: Y"
        );
        for k in (0..inp.n_goods()).filter(|&k| s.coef[(e, k)] > 0.0) {
            let share = s.coef[(e, k)] * s.consumer[(p, k)] / own;
            let a = inp.io[country][(k, g)];
            assert!(rel(share, a) <= 1e-15, "enterprise {e} input {k}");
        }
    }
}

/// The calibrated steady state of the fixture with a price level is the run's (econ SPEC,
/// Calibration, Checks): from the calibration tables, 120 ticks with every friction on keep every
/// active price and wage at its calibrated value to 1e-9, the lifted country's local prices at
/// p_c.
#[test]
fn calibrated_price_level_world_is_stationary() {
    let inp = exported();
    let s = steady::calibrate(inp, &inp.u_star.clone(), &mut |_: &str| {}).unwrap();
    let dir = TempDir::new().unwrap();
    let provinces: Vec<usize> = (0..inp.n_provinces()).collect();
    write::write(dir.path(), inp, &s, &provinces).unwrap();
    let mut econ = lifted().clone();
    econ.calibrate(dir.path()).unwrap();
    let trade = econ.trade_params();
    let mut sim = Sim::new(
        EconWorld::new(Arc::new(econ), EconParams::default(), trade),
        ceteris_econ::systems::schedule(),
        ceteris_econ::systems::SEED,
    );
    let (start_price, start_wage) = (sim.world().markets.price.clone(), sim.world().wage.clone());
    sim.run_until(120).unwrap_or_else(|v| panic!("{v}"));
    let w = sim.world();
    let mut worst: f64 = 0.0;
    for c in (0..start_price.len()).filter(|&c| w.markets.active[c]) {
        worst = worst.max(rel(w.markets.price[c], start_price[c]));
        if !inp.traded[c % inp.n_goods()] {
            assert_eq!(start_price[c], s.price.data[c], "cell {c}");
        }
    }
    for (i, &wage) in w.wage.iter().enumerate() {
        worst = worst.max(rel(wage, start_wage[i]));
    }
    println!("largest relative move of a price or wage over 120 ticks: {worst:e}");
    assert!(worst <= 1e-9, "{worst}");
}
