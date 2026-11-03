//! ADRs 0307, 0308: sector labour shares and dividends by wealth. The GE reference reads the
//! labour share of every enterprise and the dividend share of every pop the simulator exports,
//! and the calibrated steady state of a fixture with both is the run's.

use std::sync::{Arc, OnceLock};

use ceteris_calibrate::{steady, write};
use ceteris_core::rows::Rows;
use ceteris_core::sim::Sim;
use ceteris_econ::export::write_inputs;
use ceteris_econ::fixture::small_world;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_reference::io::{self, Inputs};
use ceteris_trade::{Kernel, TradeParams};
use tempfile::TempDir;

/// The fixture with a labour share per good and WID-like wealth shares in every country.
fn factored() -> &'static EconInputs {
    static INPUTS: OnceLock<EconInputs> = OnceLock::new();
    INPUTS.get_or_init(|| {
        let mut inputs = small_world();
        let (nc, ng) = (inputs.n_countries(), inputs.n_goods());
        inputs.countries.s_l_good = Rows::from_flat(
            (0..nc * ng)
                .map(|x| 0.25 + 0.5 * ((x * 7) % ng) as f64 / ng as f64)
                .collect(),
            ng,
        );
        inputs.countries.wealth = Rows::from_flat(
            (0..nc)
                .flat_map(|c| [0.04 + 0.01 * c as f64, 0.36, 0.6 - 0.01 * c as f64])
                .collect(),
            3,
        );
        let params = EconParams::default();
        inputs.set_base_point(&params.edu_eff);
        inputs.set_dividend_shares(&params);
        inputs.validate();
        inputs
    })
}

fn exported() -> &'static Inputs {
    static INPUTS: OnceLock<Inputs> = OnceLock::new();
    INPUTS.get_or_init(|| {
        let dir = TempDir::new().unwrap();
        let (params, trade) = (EconParams::default(), TradeParams::default());
        let inputs = factored();
        let kernel = Kernel::build(&inputs.network, &inputs.weight, &inputs.wedges, &trade);
        write_inputs(dir.path(), inputs, &kernel, &params, &trade).unwrap();
        io::load(dir.path()).unwrap()
    })
}

fn rel(got: f64, want: f64) -> f64 {
    (got / want - 1.0).abs()
}

#[test]
fn reference_reads_the_factor_shares() {
    let (econ, inp) = (factored(), exported());
    let s_l = inp.ent_s_l();
    for (e, &s) in s_l.iter().enumerate() {
        assert_eq!(s, econ.ent_s_l(e), "enterprise {e}");
    }
    assert_eq!(inp.pop_dividend, econ.pops.dividend);
    let params = EconParams::default();
    let (want, got) = (
        ceteris_econ::world::pop_base_income(econ, &params),
        inp.pop_base_income(),
    );
    for k in 0..want.len() {
        assert!(
            rel(got[k], want[k]) <= 1e-14,
            "pop {k}: {} vs {}",
            got[k],
            want[k]
        );
    }
}

/// From the calibration tables, 120 ticks with every friction on keep every active price and
/// wage at its calibrated value to 1e-9.
#[test]
fn calibrated_factor_shares_world_is_stationary() {
    let inp = exported();
    let s = steady::calibrate(inp, &inp.u_star.clone(), &mut |_: &str| {}).unwrap();
    let dir = TempDir::new().unwrap();
    let provinces: Vec<usize> = (0..inp.n_provinces()).collect();
    write::write(dir.path(), inp, &s, &provinces).unwrap();
    let mut econ = factored().clone();
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
    }
    for (i, &wage) in w.wage.iter().enumerate() {
        worst = worst.max(rel(wage, start_wage[i]));
    }
    println!("largest relative move of a price or wage over 120 ticks: {worst:e}");
    assert!(worst <= 1e-9, "{worst}");
}
