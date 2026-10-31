//! Properties of the P2 tick pipeline on the small-world fixture (`specs/econ/SPEC.md`,
//! Invariants and Test plan).

use std::sync::Arc;

use ceteris_core::ledger::AccountId;
use ceteris_core::sim::{Sim, State};
use ceteris_econ::fixture::{ISLAND_USER, UNREACHABLE, small_world, unreachable_island};
use ceteris_econ::inputs::EconInputs;
use ceteris_econ::local::{self, Province};
use ceteris_econ::params::Technology;
use ceteris_econ::production::{labour_for_value_added, marginal_value_added, rho, value_added};
use ceteris_econ::systems::{self, SEED};
use ceteris_econ::world::{
    DIVIDEND, MINOR_PER_UNIT, MINT, PURCHASE, TARIFF, TRADE, WRITE_OFF, active_cells,
    static_digest, to_minor, to_numeraire,
};
use ceteris_econ::{EconParams, EconWorld};
use ceteris_trade::goods::{STORABLE, TRADED};
use ceteris_trade::source::{HOUSEHOLD, INTERMEDIATE};
use ceteris_trade::{N_GOODS, Network, Rationing, Sourced, TradeParams, dispatch, source};
use proptest::prelude::*;

fn world(params: EconParams) -> EconWorld {
    EconWorld::new(Arc::new(small_world()), params, TradeParams::default())
}

fn sim_of(world: EconWorld) -> Sim<EconWorld> {
    Sim::new(world, systems::schedule(), SEED)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(6))]
    #[test]
    fn conservation_over_120_ticks(
        factors in proptest::collection::vec(0.5f64..2.0, 25 * N_GOODS),
        frictionless in any::<bool>(),
    ) {
        let params = if frictionless { EconParams::frictionless() } else { EconParams::default() };
        let mut w = world(params);
        for (p, f) in w.markets.price.iter_mut().zip(&factors) {
            *p *= f;
        }
        let mut sim = sim_of(w);
        let run = sim.run_until(120);
        prop_assert!(run.is_ok(), "{}", run.unwrap_err());
        let w = sim.world();
        prop_assert!(w.markets.price.iter().all(|p| p.is_finite() && *p > 0.0));
        // every profit is back with the pops at the end of every tick; losses stay with
        // their enterprise (limited liability)
        let acc = &w.layout().accounts;
        for a in &acc.enterprise {
            prop_assert!(w.ledger.money(*a) <= 0);
        }
        for a in acc.market.iter().chain(&acc.rebate).chain(&acc.dividend) {
            prop_assert_eq!(w.ledger.money(*a), 0);
        }
    }
}

/// The case `conservation_over_120_ticks` shrank to after the main merge (frictionless, prices
/// scaled 0.5 to 2): province 10's local chain collapses and an origin with no output is clipped
/// up every tick. Before ADR 0266, frictionless E1 moved all of the province's labour to that
/// enterprise at its posted price, and wages paid from debt overflowed the ledger at tick 91.
/// Capped at the labour its inputs allow, it hires none of it: the world price index reaches 1.55
/// over 120 ticks. The bound, 3, is about twice that: it leaves room for the drift of a case
/// that starts with every price scaled by 0.5 to 2, and fails on a return of the spiral (9e3
/// at tick 91 before the cap) well before the nominal scale of ADR 0106 (about 1e4).
#[test]
fn frictionless_labour_spiral_is_bounded() {
    const FACTORS: [f64; 25 * N_GOODS] = [
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        1.1102753300646495,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        0.5,
        1.388208273518147,
        1.3201563717783693,
        1.7861232113644243,
        0.8504036876394233,
        1.2188135120244277,
        0.9111266265641447,
        1.2874240718049614,
        0.6931944748767133,
        0.5278039135654629,
        1.6110341033558178,
        1.1685490612030451,
        1.9648285990210133,
        1.1851358863281902,
        0.928777951695254,
        1.1572772653048788,
        0.8352177580670089,
        0.9462267832648013,
        1.7573931760343753,
        1.9689338418968478,
        1.5356102083360241,
        1.6883201296623858,
        1.903639841680788,
        0.7313401036373354,
        0.5585365251245761,
        1.4773750239137515,
        1.8671690253429238,
        1.7354262642133298,
        0.9515979890502008,
        1.4574856033971357,
        0.7088776778783925,
        1.0994773620347058,
        1.5574513276810222,
        0.855945925551871,
        1.9594591006362299,
        1.5393289458442612,
        1.5532512123981719,
        1.9173285017099102,
        1.9330294547339106,
        1.9732071818251975,
        1.9063142509808297,
        0.7196713795872864,
        1.996753338632928,
        1.5037278987022085,
        0.5507358766462466,
        1.96381643046714,
        1.8075353299533117,
        0.546294786327373,
        1.9614106318584439,
        0.5889969875141584,
        1.2678979853136514,
        0.5663049623342112,
        0.5073142685823064,
        1.9931393566679225,
        0.561955124105046,
        0.9339548071007574,
        1.5434236172631386,
        0.8563076941958816,
        1.9528607002866676,
        1.1333208111547248,
        1.6392622701457409,
        1.8624839954734922,
        0.930317202044606,
        0.7048589726667379,
        0.6617161580795654,
        1.7966492769430331,
        1.3335379268262404,
        0.9792748317390749,
        1.6751457523186621,
        1.8337322567920784,
        1.688704617213163,
        1.351025074413139,
        0.7892920875378866,
        0.8373820013434974,
        0.5505596779714563,
        0.6643158704902137,
        0.7870679251353713,
        0.6890035840537763,
        1.4403018607076574,
        0.5058832766299562,
        1.492076076578027,
        1.3114768930679432,
        1.8139205394622462,
        1.2617559486532237,
        1.5219777644065127,
        0.7801480502930529,
        0.6219686157042539,
        1.0213172552853544,
        0.8066355816047946,
        1.4875537284385776,
        1.1647854208498722,
        0.7826811529835346,
        0.6315825154191911,
        1.3964583665865784,
        0.9444784068551231,
        1.2231746329408977,
        0.8402107202464197,
        1.969392270901102,
        1.9146494092323794,
        0.6485509624228885,
        1.2911715748885282,
        1.1814904993617878,
        1.4989097939051603,
        1.210709112463523,
        1.9476515506016083,
        0.5570046314909293,
        1.5236107866513593,
        0.9837679208167274,
        1.181881060878456,
        0.504563264133658,
        0.6693315950774626,
        1.655884527856955,
        0.721898973059271,
        0.7555088405278344,
        1.1562177270080982,
        1.4493445989370934,
        0.9723536286131166,
        1.5492208049272027,
        0.789083366948727,
        1.6996799138740437,
        1.4164379691576714,
        1.8979921655253227,
        0.9934315051240867,
        0.6720785441685422,
        1.549720997710603,
        1.750810338674199,
        1.7633539103169786,
        1.3220192306701952,
        1.665764678615127,
        1.0169670010388436,
        1.0333436708849362,
        1.8890040201324323,
        1.9810512120472403,
        1.2851941977543233,
        1.1291216083648534,
        1.3362916374255471,
        1.9691658220461075,
        1.3126633191815829,
        0.8827828117804166,
        0.9194437809672961,
        1.6410925523291742,
        1.500956232574782,
        1.3713715364754349,
        1.7977550422881583,
        1.5765143869200455,
        0.5294540175961284,
        0.9069083505042848,
        0.9769895516558061,
        0.8272763840456754,
        1.6845837268153705,
        0.6921390284449319,
        1.7579532764900325,
        1.293018789542887,
        1.94296824746252,
        1.4227055704513045,
        1.2475923355450285,
        0.9936819065710009,
        1.6949452679089045,
        1.0816191541774507,
        0.6077681722150597,
        0.6619534011159649,
        1.1951616899605044,
        0.7842408422801076,
        1.1330954069016814,
        1.9919331516389198,
        1.7569428377736171,
        1.302222609365242,
        1.0133154915507474,
        0.8809659895107226,
        1.13057812108468,
        1.0316477391757122,
        1.3587563728404712,
        0.8646577660174143,
        1.4653426537681546,
        1.2887281686865966,
        1.053265579928777,
        1.5997314305831363,
        1.8169520159997614,
        1.5585161125481464,
        1.4812947390889315,
        1.0912367450993878,
        1.251681451295731,
        1.945193897340138,
        0.9549231065304616,
        1.3026260918901225,
        0.6050450616758412,
        1.4999902397956841,
        1.5403187069765238,
        1.2309133702594235,
        0.5379389136182858,
        1.1143441013064028,
        0.648276578929874,
        1.584853951985719,
        1.9355982144811736,
        1.2670491617693127,
        1.3745811544973057,
        1.0483058859024432,
        1.4039954748151522,
        1.3618239269462107,
        1.5063251831539541,
        1.0412505676884356,
        1.4889277437528408,
        1.9728352286431647,
        1.09775229205037,
        0.88689624066285,
        0.9607914951972547,
        1.151548353777408,
        0.8610612496250799,
        1.8882086368655853,
        1.3323991029423559,
        1.1859577303234392,
        1.579558804595454,
        1.514628165045147,
        1.7706730557641224,
        0.7871433066292199,
        1.4316500732690716,
        1.9182516606124131,
        1.7517694521311356,
        1.753274287264913,
        1.4241924752768578,
        1.767870175654058,
        1.5171033198155428,
        1.4925229952042438,
        1.3111409139688275,
        1.559055213285625,
        1.63958085532296,
        1.5963074479837749,
        0.8832099023909025,
    ];
    let mut w = world(EconParams::frictionless());
    for (p, f) in w.markets.price.iter_mut().zip(&FACTORS) {
        *p *= f;
    }
    let mut sim = sim_of(w);
    let mut highest = 0.0f64;
    for _ in 0..120 {
        let step = sim.step();
        assert!(step.is_ok(), "{}", step.unwrap_err());
        highest = highest.max(sim.world().world_price_index());
    }
    assert!(highest < 3.0, "world price index reached {highest}");
    let w = sim.world();
    assert!(w.markets.price.iter().all(|p| p.is_finite() && *p > 0.0));
    let acc = &w.layout().accounts;
    for a in &acc.enterprise {
        assert!(w.ledger.money(*a) <= 0);
    }
    for a in acc.market.iter().chain(&acc.rebate).chain(&acc.dividend) {
        assert_eq!(w.ledger.money(*a), 0);
    }
}

/// ADR 0266: 40 frictionless runs of 120 ticks from prices scaled 0.5 to 2 per cell (seed 0)
/// neither panic nor leave the nominal scale of ADR 0106 (about 1e4): the largest world price
/// index is 324; before the input cap it was 2.6e3 (120 with the cap but the own good left out
/// of it and E3 ordering every input on capacity output). Over seeds 0 to 9 (400 runs) none
/// panics and the highest is 1.9e3, against three panics before the cap and one (seed 7, case
/// 31, the write-off's i64 bound) before the split order rule of ADR 0266.
#[test]
fn frictionless_price_sweep_stays_bounded() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::{Config, RngSeed, TestRunner};
    let mut runner = TestRunner::new(Config {
        rng_seed: RngSeed::Fixed(0),
        ..Config::default()
    });
    let mut highest = 0.0f64;
    for case in 0..40 {
        let factors = proptest::collection::vec(0.5f64..2.0, 25 * N_GOODS)
            .new_tree(&mut runner)
            .unwrap()
            .current();
        let mut w = world(EconParams::frictionless());
        for (p, f) in w.markets.price.iter_mut().zip(&factors) {
            *p *= f;
        }
        let mut sim = sim_of(w);
        for _ in 0..120 {
            sim.step().unwrap_or_else(|v| panic!("case {case}: {v}"));
            highest = highest.max(sim.world().world_price_index());
        }
    }
    assert!(highest < 1e4, "world price index reached {highest}");
}

#[test]
fn sim_runs_the_schedule_from_the_data_state() {
    let mut sim = EconWorld::sim(
        Arc::new(small_world()),
        EconParams::default(),
        TradeParams::default(),
    );
    sim.run_until(3).unwrap();
    assert_eq!(sim.tick(), 3);
    assert!(sim.world().output.iter().any(|&y| y > 0.0));
}

#[test]
fn employment_within_bounds() {
    for params in [EconParams::default(), EconParams::frictionless()] {
        let mut sim = sim_of(world(params));
        let mut moved = false;
        for _ in 0..120 {
            sim.step().unwrap();
            let w = sim.world();
            let (inp, lay) = (w.inputs(), w.layout());
            for k in 0..inp.n_pops() {
                let (e, lf) = (w.employed[k], inp.pops.labour_force[k]);
                assert!(e >= 0.0 && e <= lf * (1.0 + 1e-12), "pop {k}: {e} of {lf}");
            }
            for i in 0..inp.n_provinces() {
                let l: f64 = lay.enterprises(i).map(|e| w.employment[e]).sum();
                assert!(lay.enterprises(i).all(|e| w.employment[e] >= 0.0));
                assert!(l <= lay.labour_supply[i] * (1.0 + 1e-12), "province {i}");
                // frictionless provinces stay at (1 - u*) L^s = sum L0, their enterprises move
                moved |= lay
                    .enterprises(i)
                    .any(|e| (w.employment[e] / inp.enterprises.l0[e] - 1.0).abs() > 1e-6);
            }
            assert!(w.wage.iter().all(|x| x.is_finite() && *x > 0.0));
        }
        assert!(moved, "employment never left the base point");
    }
}

/// Multiplies prices, the price index, the last world price level, wages and every pop's
/// money by `scale`; panics on ledger overflow. Expected inflation is a log difference and
/// stays.
fn scaled(mut w: EconWorld, scale: i64) -> EconWorld {
    let s = scale as f64;
    w.markets.price.iter_mut().for_each(|p| *p *= s);
    w.price_index.iter_mut().for_each(|p| *p *= s);
    w.price_level *= s;
    w.wage.iter_mut().for_each(|x| *x *= s);
    let acc = w.layout().accounts.clone();
    for &pop in &acc.pop {
        let mut extra = i128::from(w.ledger.money(pop)) * i128::from(scale - 1);
        while extra != 0 {
            let part = extra.clamp(i128::from(i64::MIN), i128::from(i64::MAX));
            w.ledger
                .transfer(MINT, acc.mint, pop, i64::try_from(part).unwrap());
            extra -= part;
        }
    }
    w
}

/// Laspeyres index over active cells, weights Omega_g times province population.
fn price_index(w: &EconWorld) -> f64 {
    let inp = w.inputs();
    let (mut num, mut den) = (0.0, 0.0);
    for i in 0..inp.n_provinces() {
        let omega = &inp.countries.omega[inp.country_of(i)];
        for (g, &share) in omega.iter().enumerate() {
            let weight = share * inp.provinces.population[i];
            num += weight * w.markets.price[i * N_GOODS + g];
            den += weight;
        }
    }
    num / den
}

/// Equal within 1e-9 of `size[i]` (the value itself, or a larger magnitude it is computed
/// from) plus 1e-12 of the column's largest value.
fn assert_same(what: &str, a: &[f64], b: &[f64], size: &[f64]) {
    assert_eq!(a.len(), b.len());
    let scale = a.iter().chain(b).fold(0.0f64, |m, x| m.max(x.abs()));
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        let tol = 1e-9 * x.abs().max(y.abs()).max(size[i]) + 1e-12 * scale;
        assert!((x - y).abs() <= tol, "{what}[{i}]: {x} vs {y}");
    }
}

/// Real quantities, each with the magnitude its comparison is relative to.
fn real_state(w: &EconWorld) -> Vec<(&'static str, Vec<f64>, Vec<f64>)> {
    let acc = &w.layout().accounts;
    let m = &w.markets;
    let index = price_index(w);
    let stocks = acc
        .enterprise
        .iter()
        .flat_map(|&a| (0..N_GOODS).map(move |g| w.ledger.goods(a, g)))
        .collect();
    let in_flight = (0..N_GOODS)
        .filter(|&g| TRADED[g])
        .map(|g| w.pipeline.in_flight(g))
        .collect();
    let columns: Vec<(&'static str, Vec<f64>)> = vec![
        ("employment", w.employment.clone()),
        ("output", w.output.clone()),
        ("input stocks", stocks),
        ("supply", m.supply.clone()),
        ("sales", m.sales.clone()),
        ("in flight", in_flight),
        (
            "relative price",
            m.price.iter().map(|p| p / index).collect(),
        ),
        ("relative wage", w.wage.iter().map(|x| x / index).collect()),
    ];
    let mut out: Vec<_> = columns
        .into_iter()
        .map(|(what, v)| (what, v.clone(), v))
        .collect();
    // S' = S + (Q - D) loses relative precision when the stock is small against its flows
    let turnover = (0..m.inventory.len())
        .map(|c| m.inventory[c] + m.supply[c] + m.demand[c])
        .collect();
    out.push(("inventory", m.inventory.clone(), turnover));
    out
}

/// With the loss write-off off: its cap is M months of base sales in base numeraire (ADR 0112),
/// so scaling every nominal value moves losses past the cap in one run and not the other.
///
/// The frictionless run is compared at tick 6. From the data state its coal shortage holds
/// most mainland enterprises at their coal caps from tick 3 (`frictionless_unemployment_is_the_
/// natural_rate`), and its clearing wage puts some exactly there, target = cap, where ADR 0266's
/// split order rule is discontinuous: whether the cap cut the target, and so whether E3 orders
/// coal on notional or capped output, is decided by rounding. At tick 7 the two runs decide it
/// otherwise for enterprises 8 and 53 (their stocks agree to every printed digit), and from tick
/// 8 they differ by 16% in employment, 0.5% at tick 120. Before the data state's merchants
/// held coverage times their own base output (B8e) the run reached no such tie in 120 ticks.
#[test]
fn homogeneity_of_degree_zero() {
    for (params, ticks) in [
        (EconParams::default(), 120),
        (EconParams::frictionless(), 6),
    ] {
        let params = EconParams {
            write_off_months: f64::INFINITY,
            ..params
        };
        let mut base = sim_of(world(params.clone()));
        let mut big = sim_of(scaled(world(params), 10_000));
        // 120 ticks: the fixture's dynamics are stable and the rounding gap between
        // the two runs is about 1e-11, except that under PIGL demand (MIGRATION Task 7) the
        // default run passes a transient between ticks 40 and 90 where it peaks at 7.5e-9
        // (the island's sales and input stocks), back to 2e-10 by tick 100
        base.run_until(ticks).unwrap();
        big.run_until(ticks).unwrap();
        // 1e-9, not bit for bit: payments round half to even to minor units and 1e4 is not
        // a power of two, so nominal roundings differ between the two runs
        for ((what, a, size), (_, b, _)) in real_state(base.world())
            .into_iter()
            .zip(real_state(big.world()))
        {
            assert_same(what, &a, &b, &size);
        }
    }
}

/// pi^e <- pi^e + g (ln(I_t / I_{t-1}) - pi^e) after T4, I the world price index at the
/// buyer prices of the tick (ADR 0019).
#[test]
fn expected_inflation_follows_the_world_index() {
    let mut sim = sim_of(world(EconParams::default()));
    let g = sim.world().trade().expectation_gain;
    assert!(g > 0.0 && g < 1.0);
    let mut level = sim.world().world_price_index();
    assert_eq!(sim.world().price_level, level);
    assert_eq!(sim.world().expected_inflation, 0.0);
    let mut pi = 0.0;
    for tick in 0..36 {
        sim.step().unwrap();
        let w = sim.world();
        let now = w.world_price_index();
        pi += g * (libm::log(now / level) - pi);
        assert_eq!(w.price_level, now, "tick {tick}");
        assert!((w.expected_inflation - pi).abs() <= 1e-15, "tick {tick}");
        level = now;
    }
    assert!(pi.abs() > 1e-6, "the data state never moved the index");
}

/// Expected inflation far above c_max passes through the clip (ADR 0106): every active price
/// and every rule-set wage moves by pi^e within its clip, which bounds only the gap terms.
#[test]
fn world_counts_clip_bindings() {
    let pi = 10.0;
    let mut w = world(EconParams::default());
    w.expected_inflation = pi;
    let (price, wage) = (w.markets.price.clone(), w.wage.clone());
    let trade = w.trade().clone();
    let mut sim = sim_of(w);
    sim.step().unwrap();
    let w = sim.world();
    for (c, &active) in w.markets.active.iter().enumerate() {
        let step = libm::log(w.markets.price[c] / price[c]);
        if active {
            let c_max = trade.price_clip(c % N_GOODS);
            assert!(
                (step - pi).abs() <= c_max * (1.0 + 1e-12),
                "cell {c}: {step}"
            );
        } else {
            assert_eq!(step, 0.0, "cell {c}");
        }
    }
    for (i, (&now, &before)) in w.wage.iter().zip(&wage).enumerate() {
        let step = libm::log(now / before);
        assert!(
            (step - pi).abs() <= trade.c_max * (1.0 + 1e-12),
            "province {i}: {step}"
        );
    }
    let mut sim = sim_of(world(EconParams::default()));
    sim.run_until(120).unwrap();
    let w = sim.world();
    let counts = w.markets.clip_ticks.iter().chain(&w.wage_clip_ticks);
    assert!(counts.clone().all(|&n| n <= 120));
    assert!(counts.clone().any(|&n| n > 0), "the data state never clips");
}

/// The full pack's mint holds -1.15e16 minor units at scale 1, beyond i64 from a scale of about
/// 802 (ADR 0089, Evidence). The fixture, scaled until its mint holds what the pack's would at
/// scale 1,000, runs a year on the wide mint and pools (ADR 0106), and its real state matches the
/// unscaled run.
#[test]
fn mint_survives_scale_1000() {
    let params = EconParams {
        write_off_months: f64::INFINITY,
        ..EconParams::default()
    };
    let w = world(params.clone());
    let mint = w.layout().accounts.mint;
    let issued = -w.ledger.money_wide(mint);
    let scale = i64::try_from(1000 * 11_500_000_000_000_000 / issued).unwrap();
    let mut big = sim_of(scaled(world(params.clone()), scale));
    assert!(big.world().ledger.money_wide(mint) < i128::from(i64::MIN));
    let mut base = sim_of(world(params));
    base.run_until(12).unwrap();
    big.run_until(12).unwrap();
    assert!(big.world().ledger.money_wide(mint) < i128::from(i64::MIN));
    let (a, b) = (base.world(), big.world());
    assert_same("output", &a.output, &b.output, &a.output);
    assert_same("employment", &a.employment, &b.employment, &a.employment);
}

/// Pops and single payments stay i64: scaled until the richest pop's money sits just below
/// i64::MAX, the run (not the scaling) stops loudly. On the fixture the largest payment of the
/// tick exceeds the richest pop's money, so the payment's entry, not a balance, fails first.
#[test]
fn nominal_scale_beyond_the_ledger_fails_loudly() {
    let w = world(EconParams::default());
    let pops = w.layout().accounts.pop.clone();
    let richest = |w: &EconWorld| pops.iter().map(|&a| w.ledger.money(a)).max().unwrap();
    let largest = richest(&w);
    let w = scaled(w, i64::MAX / largest);
    assert!(richest(&w) > i64::MAX - largest);
    let mut sim = sim_of(w);
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sim.run_until(1)))
        .expect_err("the run overflows");
    let message = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap_or_default();
    assert!(message.contains("does not fit the ledger"), "{message}");
}

/// Runs T1 to T3 of a tick: everything up to the dispatch of the filled orders.
fn up_to_dispatch(w: &mut EconWorld) {
    w.begin_tick();
    systems::deliver(w);
    systems::labour(w);
    systems::produce(w);
    systems::demand(w);
    systems::source(w);
    systems::clear(w);
}

/// Runs T2b to E4, then checks the invariants.
fn from_dispatch(w: &mut EconWorld, tick: u32) {
    systems::dispatch(w);
    systems::reprice(w);
    systems::distribute(w);
    w.end_tick(tick).unwrap();
}

/// ADR 0091, with storage on at the approved values: storage purchases and releases pass
/// through the ledger and spoilage is tallied.
/// At T3 each stored good's origin loses exactly delta S of the stock it carried in, destroyed
/// from its market account (the ledger's goods arithmetic, bit for bit), the ledger's tally of
/// destroyed goods rises by the sum of those losses and by nothing else at T3, and nothing else
/// of that good leaves the account before dispatch. At tick 0 every stored cell expects a large
/// fall (pi^e -10 for its order alone): where the unfloored order delta S + psi D x would sell
/// more than the stock left after spoilage, the order sits exactly at its floor -(1 - delta) S,
/// elsewhere above it, and at least one cell is floored. Over 120 ticks from a disturbed data
/// state every tick's invariants hold (goods identity, the market account holding S, money) and
/// the markets end each tick with no money.
#[test]
fn storage_conserves_goods_and_money() {
    let trade = TradeParams::default().with_storage();
    let mut w = EconWorld::new(
        Arc::new(small_world()),
        EconParams::default(),
        trade.clone(),
    );
    for (i, p) in w.markets.price.iter_mut().enumerate() {
        *p *= 1.0 + 0.3 * ((i * 7 % 11) as f64 / 10.0 - 0.5);
    }
    let acc = w.layout().accounts.clone();
    let (mut stored, mut floored) = (false, 0);
    for tick in 0..120 {
        w.begin_tick();
        systems::deliver(&mut w);
        systems::labour(&mut w);
        systems::produce(&mut w);
        systems::demand(&mut w);
        systems::source(&mut w);
        let cells: Vec<(usize, f64, f64)> = (0..w.markets.price.len())
            .filter(|&c| STORABLE[c % N_GOODS] && w.markets.active[c])
            .map(|c| {
                let held = w.ledger.goods(acc.market[c / N_GOODS], c % N_GOODS);
                (c, w.markets.inventory[c], held)
            })
            .collect();
        let expected = w.markets.expectation.clone();
        if tick == 0 {
            for &(c, _, _) in &cells {
                w.markets.expectation[c] = -10.0;
            }
        }
        let tally: Vec<f64> = (0..N_GOODS).map(|g| w.ledger.destroyed(g)).collect();
        systems::clear(&mut w);
        w.markets.expectation = expected;
        let mut want = tally;
        for &(c, s, held) in &cells {
            let (i, g) = (c / N_GOODS, c % N_GOODS);
            let loss = trade.storage_loss[g] * s;
            if loss > 0.0 {
                want[g] += loss;
            }
            assert_eq!(
                w.ledger.goods(acc.market[i], g),
                held - loss,
                "tick {tick} cell {c}"
            );
            let order = w.markets.storage[c];
            assert!(order >= -(s - loss), "tick {tick} cell {c}: {order}");
            let d = w.markets.demand[c];
            if tick == 0 && s > 0.0 && d > 0.0 {
                // the unfloored order at this cover; oil's convenience yield (zeta 13) still
                // holds stock where cover is short
                let x = trade.storage_return(g, s / (trade.coverage[g] * d), -10.0);
                let below = loss + trade.speculation[g] * d * x < -(s - loss);
                assert_eq!(order == -(s - loss), below, "tick 0 cell {c}: {order}");
                floored += usize::from(below);
            }
            stored |= order != loss;
        }
        for g in (0..N_GOODS).filter(|&g| STORABLE[g]) {
            assert_eq!(w.ledger.destroyed(g), want[g], "tick {tick} good {g}");
        }
        from_dispatch(&mut w, tick);
        for a in &acc.market {
            assert_eq!(w.ledger.money(*a), 0, "tick {tick}");
        }
        if tick == 0 {
            assert!(floored > 0, "tick 0: no order at its floor");
        }
    }
    assert!(stored, "no storage order beyond spoilage");
}

/// A traded cell has a market only where an enterprise of the province produces the good:
/// nobody orders there, nothing is stocked or sold there, and its price never moves, while
/// the province's own buyers still buy the good from other origins.
#[test]
fn origins_without_capacity_have_no_market() {
    let mut w = world(EconParams::default());
    let inp = w.inputs().clone();
    let np = inp.n_provinces();
    let produces = |i: usize, g: usize| {
        (0..inp.n_enterprises()).any(|e| {
            inp.enterprises.province[e] as usize == i
                && usize::from(inp.enterprises.good[e]) == g
                && inp.enterprises.a[e] * inp.enterprises.va0[e] > 0.0
        })
    };
    let mut without: Vec<usize> = vec![];
    for i in 0..np {
        for g in (0..N_GOODS).filter(|&g| TRADED[g]) {
            let cell = i * N_GOODS + g;
            assert_eq!(w.markets.active[cell], produces(i, g), "cell {cell}");
            assert_eq!(active_cells(&inp)[cell], w.markets.active[cell]);
            if !produces(i, g) {
                without.push(cell);
            }
        }
    }
    let buyers_without_market = without
        .iter()
        .filter(|&&c| inp.countries.omega[inp.country_of(c / N_GOODS)][c % N_GOODS] > 0.0)
        .count();
    assert!(
        buyers_without_market > 0,
        "no province buys a good it does not produce"
    );

    let before = w.markets.price.clone();
    let acc = w.layout().accounts.clone();
    for tick in 0..6 {
        up_to_dispatch(&mut w);
        let mut bought = 0.0;
        for &cell in &without {
            let (i, g) = (cell / N_GOODS, cell % N_GOODS);
            let m = &w.markets;
            assert_eq!(
                (m.demand_int[cell], m.demand_hh[cell]),
                (0.0, 0.0),
                "cell {cell}"
            );
            assert_eq!(
                (m.sales[cell], m.inventory[cell]),
                (0.0, 0.0),
                "cell {cell}"
            );
            assert_eq!(w.ledger.goods(acc.market[i], g), 0.0);
            bought += w.scratch.class_spend[cell][HOUSEHOLD];
        }
        assert!(
            bought > 0.0,
            "tick {tick}: buyers without a local market spend nothing"
        );
        from_dispatch(&mut w, tick);
        for &cell in &without {
            assert_eq!(w.markets.price[cell].to_bits(), before[cell].to_bits());
        }
    }
}

/// T2b in minor units: each buyer pays its destination market, per good, its share of its
/// class's filled spending, origin price times (1 + t) per shipped unit, rounded half to
/// even; markets pass the payments on to origins, gateways and rebate accounts, and gateway
/// accounts net to zero.
#[test]
fn buyers_pay_origin_price_at_dispatch_in_minor_units() {
    for params in [EconParams::default(), EconParams::frictionless()] {
        let mut w = world(params);
        for tick in 0..4 {
            up_to_dispatch(&mut w);
            let inp = w.inputs().clone();
            let (lay, kernel) = (w.layout().clone(), w.kernel().clone());
            let (np, n_pops) = (inp.n_provinces(), inp.n_pops());
            let start = w.ledger.journal().len();

            // the filled value of every buyer, from its own sourcing of the class spending
            let mut want = vec![0i64; n_pops + inp.n_enterprises()];
            let (mut home_value, mut tariff_due, mut sourced) = (0.0, 0.0, Sourced::new());
            for g in (0..N_GOODS).filter(|&g| TRADED[g]) {
                let cells = || (0..np).map(|d| d * N_GOODS + g);
                let price: Vec<f64> = cells().map(|c| w.markets.price[c]).collect();
                let active: Vec<bool> = cells().map(|c| w.markets.active[c]).collect();
                let spend: Vec<[f64; 2]> = cells().map(|c| w.scratch.class_spend[c]).collect();
                let fill: Vec<[f64; 2]> = cells().map(|c| w.scratch.fill[c]).collect();
                source(
                    &kernel,
                    g,
                    &price,
                    &active,
                    &lay.gamma[g],
                    &spend,
                    &mut sourced,
                );
                dispatch(&kernel, g, &price, &fill, &mut sourced);
                for d in 0..np {
                    for c in [INTERMEDIATE, HOUSEHOLD] {
                        assert!(sourced.paid_by_class[d][c] <= spend[d][c] * (1.0 + 1e-12));
                    }
                    home_value += price[d] * (sourced.home[d][0] + sourced.home[d][1]);
                    let row = |e: usize| w.scratch.ent_spend[e * N_GOODS + g];
                    let paid = |c: usize, x: f64| {
                        if x > 0.0 {
                            to_minor(sourced.paid_by_class[d][c] * (x / spend[d][c]))
                        } else {
                            0
                        }
                    };
                    for e in lay.enterprises(d) {
                        want[n_pops + e] += paid(INTERMEDIATE, row(e));
                    }
                    for k in lay.pops(d) {
                        want[k] += paid(HOUSEHOLD, w.scratch.pop_spend[k * N_GOODS + g]);
                    }
                }
                let gk = kernel.good(g);
                for a in 0..kernel.n_gateways() {
                    let lo = gk.crossings.offsets[a] as usize;
                    for (j, e) in gk.crossings.row(a).iter().enumerate() {
                        tariff_due += (1.0 / e.inv_duty - 1.0) * sourced.crossing_value[lo + j];
                    }
                }
            }

            systems::dispatch(&mut w);
            let acc = &lay.accounts;
            let buyer = |a| acc.pop.iter().chain(&acc.enterprise).position(|&b| b == a);
            let market = |a| acc.market.iter().position(|&m| m == a);
            let mut got = vec![0i64; want.len()];
            let (mut passed, mut tariff, mut received) = (0i64, 0i64, 0i64);
            for e in &w.ledger.journal()[start..] {
                if e.flow == PURCHASE {
                    got[buyer(e.from).expect("buyers pay")] += e.amount;
                    assert!(market(e.to).is_some(), "buyers pay their market");
                } else if e.flow == TRADE {
                    if market(e.from).is_some() {
                        passed += e.amount;
                    }
                    if market(e.to).is_some() {
                        received += e.amount;
                    }
                } else if e.flow == TARIFF {
                    assert!(market(e.from).is_some(), "markets pay tariffs");
                    tariff += e.amount;
                }
            }
            assert_eq!(got, want, "tick {tick}: buyer payments");
            for &g in &acc.gateway {
                assert_eq!(w.ledger.money(g), 0, "tick {tick}: gateway nets to zero");
            }
            // markets pass on what their buyers paid, less home purchases; origins receive
            // it net of tariff; at most half a minor unit per payment and leg apart
            let total: i64 = got.iter().sum();
            let legs = (w.ledger.journal().len() - start) as f64;
            let near = |a: f64, b: f64| (a - b).abs() <= 0.5 * legs + 1e-9 * b.abs();
            let home = home_value * MINOR_PER_UNIT;
            assert!(
                near((passed + tariff) as f64, total as f64 - home),
                "tick {tick}"
            );
            assert!(near(received as f64, total as f64 - home - tariff as f64));
            assert!(
                near(tariff as f64, tariff_due * MINOR_PER_UNIT),
                "tick {tick}: tariff"
            );

            systems::reprice(&mut w);
            systems::distribute(&mut w);
            w.end_tick(tick).unwrap();
        }
    }
}

/// National profit ownership (owner, 2026-10-25; SPEC Income) with limited liability (owner,
/// 2026-10-26): at E4 every enterprise's and province market's positive balance joins one pool
/// per country, paid to the country's pops by population share N_k / N_c, the cumulative shares
/// rounded half to even, with every province market's signed cash flow. Money is conserved
/// exactly: each country's pops receive, in minor units, exactly what joined its pool; each
/// pop's part is within one minor unit of N_k / N_c of the pool; enterprises keep a negative
/// balance, less any write-off (ADR 0112), and end at zero otherwise; markets and rebate
/// accounts end at zero.
#[test]
fn national_profit_pool_conserves_money_exactly() {
    for params in [EconParams::default(), EconParams::frictionless()] {
        let mut w = world(params);
        for tick in 0..6 {
            up_to_dispatch(&mut w);
            systems::dispatch(&mut w);
            systems::reprice(&mut w);
            let inp = w.inputs().clone();
            let lay = w.layout().clone();
            let acc = &lay.accounts;
            let nc = inp.n_countries();
            let mut pool = vec![0i64; nc];
            for i in 0..inp.n_provinces() {
                let c = inp.country_of(i);
                pool[c] += w.ledger.money(acc.market[i]);
                pool[c] += lay
                    .enterprises(i)
                    .map(|e| w.ledger.money(acc.enterprise[e]).max(0))
                    .sum::<i64>();
            }
            let held: Vec<i64> = acc.enterprise.iter().map(|&a| w.ledger.money(a)).collect();
            let start = w.ledger.journal().len();
            systems::distribute(&mut w);
            let pop_of = |a| acc.pop.iter().position(|&p| p == a);
            let mut dividend = vec![0i64; inp.n_pops()];
            for e in &w.ledger.journal()[start..] {
                if e.flow == DIVIDEND
                    && let Some(k) = pop_of(e.to)
                {
                    dividend[k] += e.amount;
                }
            }
            let mut people = vec![0.0; nc];
            for k in 0..inp.n_pops() {
                people[inp.country_of(inp.pops.province[k] as usize)] += inp.pops.population[k];
            }
            let mut paid = vec![0i64; nc];
            for (k, &part) in dividend.iter().enumerate() {
                let c = inp.country_of(inp.pops.province[k] as usize);
                paid[c] += part;
                let share = pool[c] as f64 * inp.pops.population[k] / people[c];
                assert!(
                    (part as f64 - share).abs() <= 1.0,
                    "tick {tick} pop {k}: dividend {part} of share {share}"
                );
            }
            assert_eq!(paid, pool, "tick {tick}: pools paid out exactly");
            for (a, &before) in acc.enterprise.iter().zip(&held) {
                let written = written(&w, start, *a);
                assert_eq!(w.ledger.money(*a), before.min(0) + written, "tick {tick}");
            }
            for a in acc.market.iter().chain(&acc.rebate).chain(&acc.dividend) {
                assert_eq!(w.ledger.money(*a), 0, "tick {tick}");
            }
            w.end_tick(tick).unwrap();
        }
    }
}

/// Limited liability of enterprises (owner, 2026-10-26; SPEC Income): an enterprise whose
/// balance is negative at E4 keeps it as retained losses, pays no dividend and takes none from
/// its country's pool; while enterprises earn, no pop is paid a negative dividend; later profit
/// first repays the loss and only the rest joins the pool. Money is conserved exactly.
#[test]
fn limited_liability_keeps_losses_and_repays_them_first() {
    let calibration = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/p2-fixture/calibration");
    let mut inputs = small_world();
    inputs.calibrate(&calibration).unwrap();
    let mut w = EconWorld::new(
        Arc::new(inputs),
        EconParams::default(),
        TradeParams::default(),
    );
    let lay = w.layout().clone();
    let acc = &lay.accounts;
    // DIVIDEND entries from account `from`, or else to pops
    let dividends = |w: &EconWorld, start: usize, from: Option<usize>| -> Vec<i64> {
        w.ledger.journal()[start..]
            .iter()
            .filter(|x| x.flow == DIVIDEND)
            .filter(|x| from.map_or(acc.pop.contains(&x.to), |a| x.from.0 as usize == a))
            .map(|x| x.amount)
            .collect()
    };

    // tick 0: the most profitable enterprise ends with a loss of half its profit (the other
    // half taken by the mint)
    up_to_dispatch(&mut w);
    systems::dispatch(&mut w);
    systems::reprice(&mut w);
    let e = (0..acc.enterprise.len())
        .max_by_key(|&e| w.ledger.money(acc.enterprise[e]))
        .unwrap();
    let profit = w.ledger.money(acc.enterprise[e]);
    assert!(profit > 0, "the steady state is profitable");
    let loss = profit / 2;
    w.ledger
        .transfer(MINT, acc.enterprise[e], acc.mint, profit + loss);
    let start = w.ledger.journal().len();
    systems::distribute(&mut w);
    assert_eq!(w.ledger.money(acc.enterprise[e]), -loss, "the loss stays");
    let enterprise_account = acc.enterprise[e].0 as usize;
    assert!(dividends(&w, start, Some(enterprise_account)).is_empty());
    assert!(
        dividends(&w, start, None).iter().all(|&d| d >= 0),
        "a negative dividend"
    );
    w.end_tick(0).unwrap();

    // tick 1: the profit repays the loss before anything joins the pool
    up_to_dispatch(&mut w);
    systems::dispatch(&mut w);
    systems::reprice(&mut w);
    let balance = w.ledger.money(acc.enterprise[e]);
    let start = w.ledger.journal().len();
    systems::distribute(&mut w);
    let paid: i64 = dividends(&w, start, Some(enterprise_account)).iter().sum();
    assert_eq!(
        paid,
        balance.max(0),
        "pays out only what is left after the loss"
    );
    assert_eq!(w.ledger.money(acc.enterprise[e]), balance.min(0));
    assert!(
        dividends(&w, start, None).iter().all(|&d| d >= 0),
        "a negative dividend"
    );
    w.end_tick(1).unwrap();
}

/// ADR 0089: the frictionless wage clears labour at the natural rate, sum_g L*_ig(w) =
/// (1 - u*_i) L^s_i, so unemployment is u* in every province at every tick unless enterprises
/// held to the labour their inputs allow (ADR 0266) leave more unemployed; it is never below u*.
/// At tick 0 every enterprise is at its cap (one month of stocks is base output), which clears
/// at u* exactly. On the fixture more are unemployed at 87 province ticks, all in ticks 1 to 11:
///
/// - Province 24 at tick 1, whose one month of input stocks the first tick used up, with its
///   orders still in transit: every cap is 0, no enterprise would hire, demand is 0 at every
///   wage, and the wage halves (the no-root bound of the frictionless search).
/// - The other 86, in 20 mainland provinces and in province 24 at ticks 8 to 10, where every
///   enterprise that would hire is held by its stock of coal (good 2), under a month of use, and
///   the caps sum below (1 - u*) L^s: no wage clears, and the rationed rule sets min(w_{t-1},
///   w_top), w_top the lowest p_VA dVA/dL at the caps; up to 0.81 above u* (province 20, tick
///   6). The data state's merchants hold coverage times their own base output (econ SPEC,
///   Initial state, B8e), and the fixture's world orders about twice its base coal output at
///   prices 1 (the coal-led shortage of the data state), so its coal stocks are about half what
///   coverage times those orders gave. Under that rule only province 20 at ticks 8 and 9 was
///   held, once the cap counted each enterprise's own good and E3 ordered the short input on
///   notional output (ADR 0266).
#[test]
fn frictionless_unemployment_is_the_natural_rate() {
    let mut sim = sim_of(world(EconParams::frictionless()));
    let mut rationed = Vec::new();
    for tick in 0..24 {
        let mut probe = sim.world().clone();
        probe.begin_tick();
        systems::deliver(&mut probe);
        let p_va: Vec<f64> = {
            let (inp, lay) = (probe.inputs(), probe.layout());
            (0..inp.n_enterprises())
                .map(|e| {
                    let i = inp.enterprises.province[e] as usize;
                    let row = i * N_GOODS..(i + 1) * N_GOODS;
                    let g = usize::from(inp.enterprises.good[e]);
                    let p = probe.markets.price[row.start + g];
                    let index = &probe.price_index[row];
                    systems::value_added_price(p, |k| lay.coef[e][k], index, lay.a_va[e])
                })
                .collect()
        };
        let caps: Vec<f64> = {
            let (inp, lay) = (probe.inputs(), probe.layout());
            let r = rho(probe.params().sigma_fl);
            (0..inp.n_enterprises())
                .map(|e| {
                    let ent = &inp.enterprises;
                    let s_l = inp.countries.s_l[inp.country_of(ent.province[e] as usize)];
                    let va = lay.a_va[e] * systems::input_output(&probe, e);
                    labour_for_value_added(va, ent.a[e], ent.va0[e], ent.l0[e], s_l, r)
                })
                .collect()
        };
        sim.step().unwrap();
        let w = sim.world();
        let (inp, lay) = (w.inputs(), w.layout());
        for i in 0..inp.n_provinces() {
            let l: f64 = lay.enterprises(i).map(|e| w.employment[e]).sum();
            let u = 1.0 - l / lay.labour_supply[i];
            let u_star = inp.countries.u_star[inp.country_of(i)];
            assert!(
                u >= u_star - 1e-10,
                "tick {tick} province {i}: {u} below u* {u_star}"
            );
            if u > u_star + 1e-10 {
                assert!(
                    lay.enterprises(i).any(|e| w.employment[e] >= caps[e]),
                    "tick {tick} province {i}: unemployment {u}, u* {u_star}, no one held"
                );
                rationed.push((tick, i));
                let ent = &inp.enterprises;
                let s_l = inp.countries.s_l[inp.country_of(i)];
                let r = rho(w.params().sigma_fl);
                let top = lay
                    .enterprises(i)
                    .filter(|&e| p_va[e] > 0.0 && caps[e] > 0.0)
                    .map(|e| {
                        p_va[e]
                            * marginal_value_added(ent.a[e], ent.va0[e], ent.l0[e], s_l, r, caps[e])
                    })
                    .reduce(f64::min);
                let (before, after) = (probe.wage[i], w.wage[i]);
                let want = top.map_or(0.5 * before, |top| top.min(before));
                assert!(
                    (after / want - 1.0).abs() <= 1e-12,
                    "tick {tick} province {i}: wage {after}, want {want} (last {before})"
                );
            }
        }
    }
    #[rustfmt::skip]
    let want = [
        (1, 13), (1, 18), (1, 24), (3, 0), (3, 16), (3, 18), (3, 22), (4, 0), (4, 2), (4, 4),
        (4, 16), (4, 18), (4, 19), (4, 20), (4, 22), (5, 0), (5, 2), (5, 4), (5, 6), (5, 16),
        (5, 17), (5, 18), (5, 19), (5, 20), (5, 22), (5, 23), (6, 0), (6, 1), (6, 2), (6, 4),
        (6, 6), (6, 7), (6, 16), (6, 17), (6, 18), (6, 19), (6, 20), (6, 21), (6, 22), (6, 23),
        (7, 0), (7, 1), (7, 2), (7, 3), (7, 4), (7, 5), (7, 6), (7, 7), (7, 16), (7, 17),
        (7, 18), (7, 19), (7, 20), (7, 21), (7, 22), (7, 23), (8, 0), (8, 1), (8, 2), (8, 3),
        (8, 4), (8, 5), (8, 6), (8, 7), (8, 11), (8, 15), (8, 18), (8, 19), (8, 22), (8, 23),
        (8, 24), (9, 0), (9, 1), (9, 2), (9, 4), (9, 5), (9, 6), (9, 11), (9, 15), (9, 24),
        (10, 2), (10, 6), (10, 9), (10, 13), (10, 24), (11, 9), (11, 13),
    ];
    assert_eq!(rationed, want);
}

/// Capacity output VA(L) / a_VA of enterprise `e` at its current employment.
fn capacity(w: &EconWorld, e: usize) -> f64 {
    let (inp, lay) = (w.inputs(), w.layout());
    let ent = &inp.enterprises;
    let c = inp.country_of(ent.province[e] as usize);
    let va = value_added(
        ent.a[e],
        ent.va0[e],
        ent.l0[e],
        inp.countries.s_l[c],
        rho(w.params().sigma_fl),
        w.employment[e],
    );
    va / lay.a_va[e]
}

/// Output VA(L) / a_VA of enterprise `e` at employment `l`.
fn output_at(w: &EconWorld, e: usize, l: f64) -> f64 {
    let (inp, lay) = (w.inputs(), w.layout());
    let ent = &inp.enterprises;
    let c = inp.country_of(ent.province[e] as usize);
    let s_l = inp.countries.s_l[c];
    value_added(
        ent.a[e],
        ent.va0[e],
        ent.l0[e],
        s_l,
        rho(w.params().sigma_fl),
        l,
    ) / lay.a_va[e]
}

/// The order rule of E3 for every enterprise and input: up to an inventory position of
/// (1 + COVER + L) coef_k Y_plan, L the expected months in transit to the province,
/// (target - stock - on order) at least zero, where Y_plan is the output at E1's notional
/// employment for a traded input whose cap cut the enterprise's labour and capacity output
/// VA(L) / a_VA otherwise (ADR 0266); returns the orders that bind at zero only because of
/// goods on order.
fn assert_order_rule(w: &EconWorld) -> usize {
    let (inp, lay) = (w.inputs(), w.layout());
    let mut held_back = 0;
    for e in 0..inp.n_enterprises() {
        let i = inp.enterprises.province[e] as usize;
        for (k, &traded) in TRADED.iter().enumerate() {
            let a = lay.coef[e][k];
            let stock = w.ledger.goods(lay.accounts.enterprise[e], k);
            let on_order = w.pipeline.on_order(inp.n_pops() + e, k);
            let lead = w.scratch.lead[i * N_GOODS + k];
            assert!(traded || lead == 0.0);
            let plan = if w.scratch.short[e] & 1 << k != 0 {
                output_at(w, e, w.scratch.notional[e])
            } else {
                capacity(w, e)
            };
            let gap = (1.0 + systems::COVER + lead) * a * plan - stock;
            let want = if a > 0.0 {
                (gap - on_order).max(0.0)
            } else {
                0.0
            };
            let got = w.scratch.ent_order[e * N_GOODS + k];
            assert!(
                (got - want).abs() <= 1e-12 * want.max(1.0),
                "enterprise {e} good {k}: order {got}, want {want}"
            );
            held_back += usize::from(a > 0.0 && want == 0.0 && gap > 0.0);
        }
    }
    held_back
}

/// An enterprise whose pipeline already holds its whole target position of an input orders
/// none of it; with half a month on order it orders the rest of the gap.
#[test]
fn input_orders_count_goods_on_order() {
    let mut w = world(EconParams::default());
    w.begin_tick();
    systems::deliver(&mut w);
    systems::labour(&mut w);
    systems::produce(&mut w);
    let (inp, lay) = (w.inputs().clone(), w.layout().clone());
    let acc = &lay.accounts;
    let (full, half) = (0, 1);
    let mut checked = 0;
    let mut position = [0.0; N_GOODS];
    // the lead E3 will use: the prices do not move between here and E3
    w.refresh_price_index();
    for (e, months) in [(full, None), (half, Some(0.5))] {
        let i = inp.enterprises.province[e] as usize;
        for k in (0..N_GOODS).filter(|&k| TRADED[k] && lay.coef[e][k] > 0.0) {
            let target = 1.0 + systems::COVER + w.scratch.lead[i * N_GOODS + k];
            let units = months.unwrap_or(target) * lay.coef[e][k] * capacity(&w, e);
            if e == full {
                position[k] = units;
            }
            // in flight over three slots; the goods sit in the transit account
            for slot in 2..=4 {
                w.pipeline.push(slot, inp.n_pops() + e, k, units / 3.0);
            }
            w.ledger.create(acc.transit[k], k, units);
            checked += 1;
        }
    }
    assert!(checked >= 2, "enterprises 0 and 1 use no traded input");
    systems::demand(&mut w);
    assert!(
        assert_order_rule(&w) > 0,
        "no order held back by goods on order"
    );
    // nothing beyond the rounding of the three slots that hold the target position
    let i = inp.enterprises.province[full] as usize;
    for k in (0..N_GOODS).filter(|&k| TRADED[k] && lay.coef[full][k] > 0.0) {
        let q = w.price_index[i * N_GOODS + k];
        assert!(w.scratch.ent_order[full * N_GOODS + k] <= 1e-12 * position[k]);
        assert!(w.scratch.ent_spend[full * N_GOODS + k] <= 1e-12 * position[k] * q);
    }
}

/// Input orders plan on output before a short input cuts it, less the goods already on order:
/// not on realized output (owner decision, 2026-10-24). The traded input whose cap cut the
/// enterprise's labour at E1 is ordered on the output of its notional employment, every other
/// input on capacity output VA(L) / a_VA at this tick's employment (effective demand, Benassy
/// 1975; owner decision 2026-10-30T22:43Z, ADR 0266). Starved of every input, enterprise 0 is
/// cut by its scarcest traded inputs, and only those are planned on notional output.
#[test]
fn input_orders_plan_on_notional_output_for_the_short_input() {
    let mut w = world(EconParams::default());
    // starve enterprise 0 of every input, so its output is cut far below capacity
    let account = w.layout().accounts.enterprise[0];
    for k in 0..N_GOODS {
        let held = w.ledger.goods(account, k);
        if held > 0.0 {
            w.ledger.destroy(account, k, 0.5 * held);
        }
    }
    for tick in 0..3 {
        w.begin_tick();
        systems::deliver(&mut w);
        systems::labour(&mut w);
        let n = w.inputs().n_enterprises();
        for e in (0..n).filter(|&e| w.scratch.short[e] != 0) {
            assert_eq!(
                w.scratch.short[e],
                systems::scarcest_inputs(&w, e),
                "enterprise {e}"
            );
        }
        systems::produce(&mut w);
        systems::demand(&mut w);
        let binding = (0..n)
            .filter(|&e| w.output[e] < 0.9 * capacity(&w, e))
            .count();
        assert!(tick > 0 || binding > 0, "no enterprise was input-bound");
        assert!(
            tick > 0 || w.scratch.short[0] != 0,
            "enterprise 0 was not held by an input"
        );
        let all = (0..N_GOODS).filter(|&k| TRADED[k] && w.layout().coef[0][k] > 0.0);
        assert!(tick > 0 || all.fold(0u16, |b, k| b | 1 << k) == w.scratch.short[0]);
        assert_order_rule(&w);
        systems::source(&mut w);
        systems::clear(&mut w);
        from_dispatch(&mut w, tick);
    }
}

/// Buyers pay the CES index (ADR 0200): a traded good's buyer price is q_d = Gamma_d P_d, and
/// spending q_d at any destination, sourced at the current prices and fully filled, delivers
/// one unit whatever the origins; the lead is the mean arrival slot of those units. Local goods
/// keep their posted price.
#[test]
fn buyers_pay_the_ces_index() {
    let mut sim = sim_of(world(EconParams::default()));
    sim.run_until(5).unwrap();
    let mut w = sim.world().clone();
    w.refresh_price_index();
    let (np, kernel) = (w.inputs().n_provinces(), w.kernel().clone());
    let mut sourced = Sourced::new();
    let mut leads = 0;
    for (g, &traded) in TRADED.iter().enumerate() {
        if !traded {
            for d in 0..np {
                assert_eq!(
                    w.price_index[d * N_GOODS + g],
                    w.markets.price[d * N_GOODS + g]
                );
            }
            continue;
        }
        let cells = || (0..np).map(|o| o * N_GOODS + g);
        let price: Vec<f64> = cells().map(|c| w.markets.price[c]).collect();
        let active: Vec<bool> = cells().map(|c| w.markets.active[c]).collect();
        for d in 0..np {
            let q = w.price_index[d * N_GOODS + g];
            assert!(q.is_finite() && q > 0.0);
            let mut spend = vec![[0.0; 2]; np];
            spend[d] = [0.0, q];
            let gamma = &w.layout().gamma[g];
            source(&kernel, g, &price, &active, gamma, &spend, &mut sourced);
            dispatch(&kernel, g, &price, &vec![[1.0; 2]; np], &mut sourced);
            assert_eq!(q, gamma[d] * sourced.price_index[d]);
            let units: Vec<f64> = sourced.arrive.iter().map(|a| a[d][HOUSEHOLD]).collect();
            let total: f64 = units.iter().sum();
            assert!(
                (total - 1.0).abs() < 1e-12,
                "cell {}: {total}",
                d * N_GOODS + g
            );
            let lead: f64 = units.iter().enumerate().map(|(s, x)| s as f64 * x).sum();
            assert!((lead - w.scratch.lead[d * N_GOODS + g]).abs() < 1e-12);
            leads += usize::from(lead > 1.0);
        }
    }
    assert!(
        leads > 0,
        "no destination waits more than a month on average"
    );
}

/// ADR 0200: over 120 ticks of the fixture from the data state, every tick, what T2b created
/// less what it destroyed is its selection term less its melt (the invariant `end_tick`
/// checks), and selection is not idle. Net selection per destination is zero where Gamma is
/// fixed (q_d = 1 / alpha_d), so the net term only moves with the prices; per arrival slot it
/// is not zero there, since spending is split over slots by value: from tick 1 some slot's
/// arrivals differ from its surviving units by more than 1e-3 of the destination's arrivals.
#[test]
fn goods_invariant_holds_with_selection() {
    let mut sim = sim_of(world(EconParams::default()));
    let mut largest = 0.0f64;
    let mut per_slot = 0.0f64;
    for tick in 1..=120 {
        sim.step().unwrap_or_else(|v| panic!("{v}"));
        let sc = &sim.world().scratch;
        for (g, r) in sc.sourced.iter().enumerate() {
            let (made, due, scale) = (
                sc.dispatch_made[g],
                sc.dispatch_due[g],
                sc.dispatch_scale[g],
            );
            assert!(
                (made - due).abs() <= 1e-9 * scale.max(1.0),
                "tick {tick} good {g}: made {made}, due {due}"
            );
            assert_eq!(due, r.selected - r.lost);
            largest = largest.max(r.selected.abs() / scale.max(1.0));
            if tick == 1 {
                for d in 0..r.paid.len() {
                    let total: f64 = r.arrive.iter().map(|a| a[d][0] + a[d][1]).sum();
                    for (a, s) in r.arrive.iter().zip(&r.survived) {
                        let gap = (a[d][0] + a[d][1] - s[d]).abs();
                        per_slot = per_slot.max(gap / total.max(f64::MIN_POSITIVE));
                    }
                }
            }
        }
    }
    assert!(
        largest > 1e-6,
        "selection never moved more than {largest} of the units"
    );
    assert!(
        per_slot > 1e-3,
        "no slot's arrivals differ from its surviving units by more than {per_slot}"
    );
}

/// ADR 0200: in autarky (no legs: every province buys only at home) every route's selection
/// multiplier is 1, so buyers pay their own market's price and selection adds nothing, every
/// tick. The pre-ADR 0200 build's state hash is not reproduced bit for bit: Gamma P_d is
/// p_min exp(-ln Phi / theta), which differs from 1 / (1 / p_d) in the last bits (at tick 120
/// the states agree to 3.4e-12 relative, money rounding to minor units included). PLAN B2a
/// names this test `selection_off_reproduces_the_status_quo`.
#[test]
fn autarky_has_no_selection() {
    let mut inputs = small_world();
    inputs.network = Network {
        province_country: inputs.network.province_country.clone(),
        ..Network::default()
    };
    // no legs, so no bulk legs: the bulk freight rate is off (ADR 0247)
    let trade = TradeParams::default().bulk_off();
    inputs.set_base_prices(&trade);
    inputs.set_coefficients();
    inputs.set_base_point(&EconParams::default().edu_eff);
    let gamma = inputs.provinces.gamma.clone();
    let mut sim = sim_of(EconWorld::new(
        Arc::new(inputs),
        EconParams::default(),
        trade,
    ));
    for tick in 1..=120 {
        sim.step().unwrap_or_else(|v| panic!("{v}"));
        // buyer prices at this tick's posted prices (the world refreshes them at E3)
        let mut now = sim.world().clone();
        now.refresh_price_index();
        let w = sim.world();
        for (cell, &q) in now.price_index.iter().enumerate() {
            let (d, g) = (cell / N_GOODS, cell % N_GOODS);
            if !TRADED[g] || !q.is_finite() {
                continue;
            }
            assert!(
                w.markets.active[cell],
                "tick {tick} cell {cell}: buys abroad"
            );
            assert!(
                (gamma[d][g] - 1.0).abs() <= 4.0 * f64::EPSILON,
                "{}",
                gamma[d][g]
            );
            let p = w.markets.price[cell];
            assert!(
                (q - p).abs() <= 4.0 * f64::EPSILON * p,
                "tick {tick} cell {cell}: {q} vs {p}"
            );
        }
        let sc = &w.scratch;
        for (g, r) in sc.sourced.iter().enumerate() {
            assert_eq!(r.lost, 0.0);
            let scale = sc.dispatch_scale[g].max(1.0);
            assert!(
                r.selected.abs() <= 1e-14 * scale,
                "tick {tick} good {g}: {}",
                r.selected
            );
        }
    }
}

const COAL: usize = 2;
const CONSTRUCTION: usize = 8;
const SERVICES: usize = 9;
const LOCAL: [usize; 3] = [7, 8, 9];

/// The fixture with input coefficients `(k, g, a_kg)` set in every country, one vintage.
fn fixture_with(coefficients: &[(usize, usize, f64)]) -> EconInputs {
    let mut inputs = small_world();
    for io in &mut inputs.countries.io {
        for &(k, g, a) in coefficients {
            io[k * N_GOODS + g] = a;
        }
    }
    inputs.set_coefficients();
    inputs.validate();
    inputs
}

/// The enterprise of province `i` producing `g`.
fn producer(w: &EconWorld, i: usize, g: usize) -> usize {
    let inp = w.inputs();
    w.layout()
        .enterprises(i)
        .find(|&e| usize::from(inp.enterprises.good[e]) == g)
        .expect("a producer")
}

/// One tick through the systems, with `hook` run right after E1 labour.
fn step_with(w: &mut EconWorld, tick: u32, hook: impl Fn(&mut EconWorld)) {
    w.begin_tick();
    systems::deliver(w);
    systems::labour(w);
    hook(w);
    systems::produce(w);
    systems::demand(w);
    systems::source(w);
    systems::clear(w);
    from_dispatch(w, tick);
}

/// Destroys the local input stocks (the chain's buffers) of every enterprise of province `i`.
fn drain_local_buffers(w: &mut EconWorld, i: usize) {
    let lay = w.layout().clone();
    for e in lay.enterprises(i) {
        for k in LOCAL {
            let held = w.ledger.goods(lay.accounts.enterprise[e], k);
            if held > 0.0 {
                w.ledger.destroy(lay.accounts.enterprise[e], k, held);
            }
        }
    }
}

/// Services feeding every sector at 0.3, so that intermediate users take most of services
/// output, and the first tick from the data state in two runs, both with the local buffers
/// of province 0 drained: control, and cut (half of its services labour). The cut is at tick
/// 0, not 10: at tick 10 the data state's traded shortage caps services output by its
/// traded input stocks, so its labour does not bind.
fn services_shortage() -> (EconWorld, EconWorld) {
    let coefficients: Vec<_> = (0..N_GOODS).map(|g| (SERVICES, g, 0.3)).collect();
    let w = EconWorld::new(
        Arc::new(fixture_with(&coefficients)),
        EconParams::default(),
        TradeParams::default(),
    );
    let services = producer(&w, 0, SERVICES);
    let mut control = w.clone();
    step_with(&mut control, 0, |w| drain_local_buffers(w, 0));
    let mut cut = w;
    step_with(&mut cut, 0, |w| {
        drain_local_buffers(w, 0);
        w.employment[services] *= 0.5;
    });
    (control, cut)
}

/// ADR 0020: a shortage of a local good reaches its users' output within the tick.
#[test]
fn local_shortage_reaches_downstream_same_tick() {
    let (control, cut) = services_shortage();
    let (services, construction) = (
        producer(&control, 0, SERVICES),
        producer(&control, 0, CONSTRUCTION),
    );
    assert!(cut.output[services] < 0.7 * control.output[services]);
    // without the cut, services covers its users from this tick's output
    assert_eq!(control.scratch.chain_iterations[0], 1);
    assert!(cut.scratch.chain_iterations[0] > 1);
    let (was, now) = (control.output[construction], cut.output[construction]);
    assert!(now < 0.95 * was, "construction {now} against {was}");
}

/// ADR 0093, stock floor (amendment of 2026-10-30): a user whose own stock covers its local
/// needs is not cut by the chain's fill. Services feed every sector at 0.3; in province 0 the
/// local buffers are drained, services hires no one (no output), and one enterprise gets two
/// months of every local input at its capped output. It runs at its cap; the other users of
/// services, without stock, are cut to nothing; the invariants (Leontief, conservation) hold.
#[test]
fn a_stocked_user_is_not_cut_by_the_fill() {
    let coefficients: Vec<_> = (0..N_GOODS).map(|g| (SERVICES, g, 0.3)).collect();
    let mut w = EconWorld::new(
        Arc::new(fixture_with(&coefficients)),
        EconParams::default(),
        TradeParams::default(),
    );
    let services = producer(&w, 0, SERVICES);
    let lay = w.layout().clone();
    let stocked = lay
        .enterprises(0)
        .find(|&e| !LOCAL.contains(&usize::from(w.inputs().enterprises.good[e])))
        .expect("a user making a traded good");
    let cut = |w: &mut EconWorld| {
        drain_local_buffers(w, 0);
        w.employment[services] = 0.0;
    };
    let mut probe = w.clone();
    probe.begin_tick();
    systems::deliver(&mut probe);
    systems::labour(&mut probe);
    cut(&mut probe);
    systems::cap_output(&mut probe);
    let cap = probe.scratch.produced[stocked];
    assert!(cap > 0.0);
    step_with(&mut w, 0, |w| {
        cut(w);
        for k in LOCAL {
            let need = w.layout().coef[stocked][k] * cap;
            if need > 0.0 {
                w.ledger
                    .create(lay.accounts.enterprise[stocked], k, 2.0 * need);
            }
        }
    });
    assert_eq!(w.output[services], 0.0);
    assert_eq!(w.output[stocked], cap, "the stocked user runs at its cap");
    let others: Vec<usize> = lay
        .enterprises(0)
        .filter(|&e| e != stocked && e != services && lay.coef[e][SERVICES] > 0.0)
        .collect();
    assert!(!others.is_empty());
    for &e in &others {
        assert_eq!(w.output[e], 0.0, "enterprise {e} without stock");
    }
}

/// Intermediate users of a local good are served before households: in the shortage they
/// draw all of services output and households get none; without it households buy the rest.
/// At origin markets the default Priority fills intermediate orders before households.
#[test]
fn local_chain_respects_intermediate_priority() {
    let (control, cut) = services_shortage();
    let cell = SERVICES;
    let households = |w: &EconWorld| -> f64 {
        w.layout()
            .pops(0)
            .map(|k| w.scratch.pop_demand[k * N_GOODS + SERVICES])
            .sum()
    };
    for w in [&control, &cut] {
        assert!(households(w) > 0.0);
        assert!(w.scratch.drawn[cell] <= w.markets.supply[cell] * (1.0 + 1e-12));
    }
    let to_households = |w: &EconWorld| w.markets.sales[cell] - w.scratch.drawn[cell];
    assert!(to_households(&control) > 0.0);
    let supply = cut.markets.supply[cell];
    assert!((cut.scratch.drawn[cell] - supply).abs() <= 1e-12 * supply);
    assert!(to_households(&cut) <= 1e-12 * supply);

    let mut sim = sim_of(world(EconParams::default()));
    let mut short = 0;
    for _ in 0..36 {
        sim.step().unwrap();
        let w = sim.world();
        for (c, f) in w.scratch.fill.iter().enumerate() {
            if TRADED[c % N_GOODS] && w.markets.active[c] && f[0] < 1.0 {
                let none = if w.markets.demand_hh[c] > 0.0 {
                    0.0
                } else {
                    1.0
                };
                assert_eq!(f[1], none, "cell {c}: {f:?}");
                short += 1;
            }
        }
    }
    assert!(short > 0, "no intermediate shortage at an origin");
}

/// A traded input reaches its users after transit: cutting coal output leaves downstream
/// output unchanged in the tick of the cut, and lowers it once input stocks run out.
#[test]
fn traded_shortage_still_lags() {
    let mut sim = sim_of(world(EconParams::default()));
    sim.run_until(10).unwrap();
    let (mut control, mut cut) = (sim.world().clone(), sim.world().clone());
    let inp = control.inputs().clone();
    let coal: Vec<usize> = (0..inp.n_enterprises())
        .filter(|&e| usize::from(inp.enterprises.good[e]) == COAL)
        .collect();
    let users: Vec<usize> = (0..inp.n_enterprises())
        .filter(|&e| !coal.contains(&e) && control.layout().coef[e][COAL] > 0.0)
        .collect();
    let mut lower = None;
    for tick in 10..48 {
        step_with(&mut control, tick, |_| {});
        step_with(&mut cut, tick, |w| {
            coal.iter().for_each(|&e| w.employment[e] = 0.0)
        });
        if tick == 10 {
            assert!(coal.iter().all(|&e| cut.output[e] == 0.0));
            for &e in &users {
                assert_eq!(cut.output[e].to_bits(), control.output[e].to_bits());
            }
        } else if lower.is_none()
            && users
                .iter()
                .any(|&e| cut.output[e] < 0.99 * control.output[e])
        {
            lower = Some(tick);
        }
    }
    assert!(lower.is_some(), "downstream output never fell");
}

/// Leontief, local goods (ADR 0093, amendment of 2026-10-30): the allowance for where the chain's
/// passes stop short is relative to each user's own need, and only the market drain's rounding is
/// relative to the province's draws D. A user whose need is 1e-12 of D and which gets none of it
/// breaks the invariant (a D-relative 2e-12 would let it pass).
#[test]
fn a_tiny_local_user_left_without_its_input_breaks_leontief() {
    let mut w = world(EconParams::default());
    up_to_dispatch(&mut w);
    from_dispatch(&mut w, 0);
    let (e, k) = (0..w.inputs().n_enterprises())
        .flat_map(|e| LOCAL.map(|k| (e, k)))
        .find(|&(e, k)| w.layout().coef[e][k] * w.scratch.produced[e] > 0.0)
        .expect("a user of a local good");
    let want = w.layout().coef[e][k] * w.scratch.produced[e];
    let cell = w.inputs().enterprises.province[e] as usize * N_GOODS + k;
    w.scratch.drawn[cell] = 1e12 * want;
    w.scratch.input_used[e * N_GOODS + k] = 0.0;
    w.scratch.input_before[e * N_GOODS + k] = 0.0;
    let v = w.end_tick(0).expect_err("a user short of its whole need");
    assert_eq!(v.invariant, "leontief", "{v}");
}

/// The passes [`local::fills`] takes on `p` where every jump applies (a resolvable fall per
/// cycle, on pieces that repeat within [`local::PERIOD`] passes), from the kinks the fall can
/// cross. A jump from a cycle on its pieces ends at the limit or at the last whole cycle before
/// the pieces change, so the passes are: one from F = 1, which cuts only the goods short there;
/// at most two per kink, the pass that crosses it and one more where that pass straddles the
/// kink, so that its map does not repeat; and at the limit up to `PERIOD` confirming passes and
/// the pass that changes nothing. The kinks are a user's floor of a good inside (0, 1)
/// (0 < b < coef cap; under the stock floor, ADR 0093, amendment of 2026-10-30, the user's stock
/// covers it from where its smallest other level falls to that floor, crossed at most once since
/// the fills only fall; before it, the stock threshold where its own fill fell to b / (coef cap)),
/// each local good beyond the first that starts to fall, and each pair of local goods some user
/// takes both of (a switch of the users' binding level, taken to happen once per pair: an
/// assumption, not derived; under the stock floor each user of both goods switches on its own
/// line, s_j + (1 - s_j) F_j = s_k + (1 - s_k) F_k, so a fall may cross one switch per such user,
/// and the others are not counted).
fn kink_bound(p: &Province) -> usize {
    let n = p.cap.len();
    let uses = |u: usize, j: usize| p.cap[u] > 0.0 && p.coef[u][j] > 0.0;
    let floors = (0..n)
        .flat_map(|u| (0..3).map(move |j| (u, j)))
        .filter(|&(u, j)| uses(u, j) && p.stock[u][j] > 0.0)
        .filter(|&(u, j)| p.stock[u][j] < p.coef[u][j] * p.cap[u])
        .count();
    let goods = (0..3).filter(|&j| (0..n).any(|u| uses(u, j))).count();
    let pairs = [(0, 1), (0, 2), (1, 2)]
        .iter()
        .filter(|&&(j, k)| (0..n).any(|u| uses(u, j) && uses(u, k)))
        .count();
    let kinks = floors + goods.saturating_sub(1) + pairs;
    1 + 2 * kinks + local::PERIOD + 1
}

/// Local-good chains: services feeding services, and construction and services feeding each
/// other. From the data state the chain binds at tick 1 in every mainland province but 13 and
/// 18, whose enterprises all produce at 0.59 and 0.60 of base output, held by their coal (good
/// 2): the data state's merchants hold coverage times their own base output (econ SPEC, Initial
/// state, B8e), about half the coal the world orders at prices 1; over 120
/// ticks every province tick ends within the passes its kinks allow ([`kink_bound`]) without a
/// stall, and every local input used equals coef Y at every tick, to 1e-12 of it plus the
/// rounding of the market draws, 2 (N_GOODS + 1) eps of the units drawn at the province (the
/// Leontief invariant's drain term, spec Invariants): at tick 50 enterprise 69 needs 3.8e-79 of
/// good 8, a residue of a collapse, and misses it by 2.1e-12 of that.
///
/// The most is 2 passes (2,972 binding province ticks). Before the data state's merchants held
/// coverage times their own base output (B8e) it was 14 passes against its bound of 25 (482
/// binding province ticks, the smallest margin 11 at that tick, the smallest bound 15):
/// province 10 at tick 76, where utilities
/// and construction collapse to 0 and the fall crosses the floors of users holding residues of
/// earlier collapses (stocks of 1e-52 to 1e-12 of their needs), each a kink under the stock floor
/// (ADR 0093, amendment of 2026-10-30), where the user's stock comes to cover it. Before the floor
/// the most was 4 passes (483 binding province ticks), the residues then thresholds below the
/// fall's end. Before ADR 0266's split order rule it was 6 passes (province 5 at tick 79, where the kink bound allows 21): a collapse of utilities and
/// construction, inputs of each other with no stocks but residues of earlier collapses, at a
/// loop gain of 0.7 to 0.9 per pass, the fall to 0 crossing the residue thresholds. Before the jump found the last whole cycle on the pieces (it returned the last
/// point the doubling checked), a jump could stop short of a kink and cost passes no kink
/// accounts for: 11 passes there at 13410ce. This bound does not tell that defect apart (the old
/// jump also stays within it here), nor does [`chain_passes_within_their_kinks`]; the guard is
/// the unit test `a_jump_ahead_ends_after_the_last_whole_cycle_on_the_pieces`.
#[test]
fn local_chain_terminates_on_cycles() {
    let coefficients = [
        (SERVICES, SERVICES, 0.3),
        (SERVICES, CONSTRUCTION, 0.2),
        (CONSTRUCTION, SERVICES, 0.2),
    ];
    let inputs = Arc::new(fixture_with(&coefficients));
    let mut sim = EconWorld::sim(inputs, EconParams::default(), TradeParams::default());
    let (mut binding, mut most) = (0, 0);
    for tick in 0..120 {
        let mut probe = sim.world().clone();
        probe.begin_tick();
        systems::deliver(&mut probe);
        systems::labour(&mut probe);
        systems::cap_output(&mut probe);
        let (inp, lay) = (probe.inputs().clone(), probe.layout().clone());
        let bounds: Vec<usize> = (0..inp.n_provinces())
            .map(|i| {
                let cap = &probe.scratch.produced[lay.enterprises(i)];
                kink_bound(&Province::new(&inp, &lay, &probe.ledger, i, cap))
            })
            .collect();
        sim.step().unwrap_or_else(|v| panic!("{v}"));
        let w = sim.world();
        let passes = &w.scratch.chain_iterations;
        for (i, (&n, &bound)) in passes.iter().zip(&bounds).enumerate() {
            assert!(
                (1..=bound).contains(&(n as usize)),
                "tick {tick} province {i}: {n} passes, kink bound {bound}"
            );
        }
        assert!(
            !w.scratch.chain_stalled.iter().any(|&s| s),
            "tick {tick}: stalled"
        );
        if tick == 1 {
            let bound: Vec<usize> = (0..24).filter(|&i| passes[i] > 1).collect();
            let held = [13, 18];
            assert_eq!(
                bound,
                (0..24).filter(|i| !held.contains(i)).collect::<Vec<_>>()
            );
        }
        binding += passes.iter().filter(|&&n| n > 1).count();
        most = most.max(*passes.iter().max().unwrap());
        for e in 0..w.inputs().n_enterprises() {
            for k in LOCAL {
                let want = w.layout().coef[e][k] * w.scratch.produced[e];
                let used = w.scratch.input_used[e * N_GOODS + k];
                let i = w.inputs().enterprises.province[e] as usize;
                let drain = 2.0 * (N_GOODS as f64 + 1.0) * f64::EPSILON;
                assert!(
                    (used - want).abs() <= 1e-12 * want + drain * w.scratch.drawn[i * N_GOODS + k],
                    "tick {tick} enterprise {e} good {k}"
                );
            }
        }
    }
    println!("chain binds in {binding} province ticks, at most {most} passes");
    assert!(binding > 0);
}

/// The random province families of the chain bound: (extra enterprises, largest coefficient,
/// probability of no stock) for [`random_province_with`]: the default, higher coefficients, no
/// stocks, up to 10 enterprises, scarce stocks.
const FAMILIES: [(usize, f64, f64); 5] = [
    (5, 0.6, 0.5),
    (5, 0.9, 0.5),
    (5, 0.6, 1.0),
    (9, 0.6, 0.5),
    (5, 0.6, 0.9),
];

/// The passes of [`local::fills`] on `p` are at most [`kink_bound`] plus the passes outside its
/// accounting ([`local::Fills::uncovered`]).
fn assert_within_kinks(p: &Province, what: &str) {
    let out = local::fills(p);
    let bound = kink_bound(p);
    assert!(
        out.passes <= bound + out.uncovered,
        "{what}: {out:?}, kink bound {bound}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]
    /// ADR 0093, jump ahead to the last whole cycle: on random small provinces of every family the
    /// passes are within the kink bound, plus the passes no jump covered.
    #[test]
    fn chain_passes_within_their_kinks(seed in any::<u64>()) {
        for (i, &(extra, coef_max, no_stock)) in FAMILIES.iter().enumerate() {
            let p = random_province_with(seed, extra, coef_max, no_stock);
            assert_within_kinks(&p, &format!("family {i} seed {seed}"));
        }
    }
}

/// ADR 0093, jump ahead to the last whole cycle: the 13 provinces of 1,000,000 (200,000 of each
/// family, seeds k x 0x9E3779B97F4A7C15 for k = 1 to 200,000; k in the comments) whose passes
/// exceed [`kink_bound`], each within it once the passes outside its accounting are added:
/// - the stall rule (12): pieces that repeat over 4 or more passes or binding fills that switch
///   every pass (99418: goods 0 and 1 switch 204 times), so no jump of up to 3 passes applies and
///   the stall rule ends the fall at 257 passes (73799 stalls, 792 passes, and is within the
///   bound only by the charge for every pass after a stall);
/// - a root rounded onto a kink (1, 170254: 14 against 11): good 0's step lands on good 2's
///   fill every other pass, so the cycle's map is undefined and no jump applies there.
///
/// Before the stock floor (ADR 0093, amendment of 2026-10-30) there were 22: the 10 more cut a
/// good by rounding alone every other pass, sitting at its limit with U = R where users' stocks
/// covered their needs at a threshold (175527: 88 passes against 25). Under the floor a user
/// bound by a good draws (1 - s) F of its need, which no stock zeroes along the fall, and none of
/// the 10 exceeds the bound.
#[test]
fn chain_passes_beyond_their_kinks_are_uncovered() {
    for (family, seed) in [
        (1, 14587390709483492012u64), // 95996
        (2, 1532048138609151479),     // 2267
        (2, 10068621952487064433),    // 38125
        (2, 1643254794265822703),     // 50931
        (2, 16419583651343627261),    // 51785
        (2, 12969786537767039842),    // 99418
        (2, 13995925185394726438),    // 170254
        (2, 16771139658170358534),    // 189038
        (3, 4922913425195756184),     // 175672
        (4, 1532048138609151479),     // 2267
        (4, 16419583651343627261),    // 51785
        (4, 5355749340075788755),     // 73799
        (4, 12969786537767039842),    // 99418
    ] {
        let (extra, coef_max, no_stock) = FAMILIES[family];
        let p = random_province_with(seed, extra, coef_max, no_stock);
        let out = local::fills(&p);
        assert!(
            out.passes > kink_bound(&p),
            "family {family} seed {seed}: {out:?}"
        );
        assert_within_kinks(&p, &format!("family {family} seed {seed}"));
    }
}

/// ADR 0093, jump ahead to the last whole cycle: (passes, uncovered) pinned for a few provinces,
/// so that a solver change that adds passes the counter charges is seen, though the property
/// [`chain_passes_within_their_kinks`] still holds. Exceeding their kink bound: 170254 (bound
/// 11), 99418 (15, stall rule), 73799 (15, stalled), 95996 (21, stall rule, stocks drawn).
/// Within it, no pass uncovered: k = 6, 320, 419 and 461 (4 to 11 passes against bounds of 15
/// to 29).
/// The stocked provinces 175527 and 3779 were pinned at 88 and 68 passes before the stock floor
/// (ADR 0093, amendment of 2026-10-30), and 320, 419 and 461 at 7; they now take 3, 4, 5, 4 and
/// 11.
#[test]
fn chain_passes_and_uncovered_are_pinned() {
    for (family, seed, passes, uncovered) in [
        (2, 13995925185394726438u64, 14, 3),
        (2, 12969786537767039842, 257, 256),
        (4, 5355749340075788755, 792, 791),
        (1, 14587390709483492012, 257, 256),
        (1, 12026231257191621299, 3, 0),
        (3, 10153890110564051455, 4, 0),
        (2, 13064056694810536062, 6, 0),
        (1, 14220159662641846848, 5, 0),
        (3, 17639538279355848287, 4, 0),
        (3, 16854214774481842641, 11, 0),
    ] {
        let (extra, coef_max, no_stock) = FAMILIES[family];
        let out = local::fills(&random_province_with(seed, extra, coef_max, no_stock));
        assert_eq!(
            (out.passes, out.uncovered),
            (passes, uncovered),
            "family {family} seed {seed}: {out:?}"
        );
    }
}

/// Outputs at fills `f`, as ADR 0093's Amendment defines them (each user's stock a floor under
/// the fill, amendment of 2026-10-30), written out here independently: Y = cap min_j G_j(F_j),
/// G = s + (1 - s) F with s the share of the need at capacity the stock covers, 1 at F = 1.
fn outputs_at(p: &Province, f: &[f64; 3]) -> Vec<f64> {
    (0..p.cap.len())
        .map(|u| {
            let least = (0..3).filter(|&j| p.coef[u][j] > 0.0).fold(1.0f64, |m, j| {
                let (need, b) = (p.coef[u][j] * p.cap[u], p.stock[u][j]);
                let level = if f[j] >= 1.0 || b >= need {
                    1.0
                } else {
                    // rounded down until the stock alone covers the need at cap s
                    let mut s = b / need;
                    while s > 0.0 && p.coef[u][j] * (p.cap[u] * s) > b {
                        s = s.next_down();
                    }
                    (s + (1.0 - s) * f[j]).min(1.0)
                };
                m.min(level)
            });
            p.cap[u] * least
        })
        .collect()
}

/// What the users' stocks of good j cannot cover at fills `f` (ADR 0093, amendment of
/// 2026-10-30), written out independently: each user's coef cap max(min_k (G_k - s_j), 0) with
/// G_k - s_j as (1 - s_j) F_j for j itself and (s_k - s_j) + (1 - s_k) F_k for its other goods,
/// so that the stock's share does not cancel, at most its need coef Y; a user without stock has
/// its whole need uncovered.
fn uncovered_at(p: &Province, j: usize, f: &[f64; 3]) -> f64 {
    let y = outputs_at(p, f);
    (0..p.cap.len())
        .map(|u| {
            let a = p.coef[u][j] * p.cap[u];
            let need = p.coef[u][j] * y[u];
            if need <= 0.0 {
                return 0.0;
            }
            if p.stock[u][j] == 0.0 {
                return need;
            }
            let floor = |k: usize| {
                let (need, b) = (p.coef[u][k] * p.cap[u], p.stock[u][k]);
                if b >= need {
                    return 1.0;
                }
                let mut s = b / need;
                while s > 0.0 && p.coef[u][k] * (p.cap[u] * s) > b {
                    s = s.next_down();
                }
                s
            };
            let sj = floor(j);
            let d = (0..3)
                .filter(|&k| p.coef[u][k] > 0.0)
                .map(|k| {
                    let sk = floor(k);
                    if k == j {
                        (1.0 - sj) * f[j].min(1.0)
                    } else if f[k] >= 1.0 || sk >= 1.0 {
                        1.0 - sj
                    } else {
                        ((sk - sj) + (1.0 - sk) * f[k]).min(1.0 - sj)
                    }
                })
                .fold(1.0 - sj, f64::min);
            (a * d.max(0.0)).min(need)
        })
        .sum()
}

fn fits_at(p: &Province, j: usize, f: &[f64; 3]) -> bool {
    let y = outputs_at(p, f);
    let rest = p.producer[j].map_or(0.0, |q| (1.0 - p.own[j]) * y[q]);
    uncovered_at(p, j, f) <= rest
}

/// Whether good j fits at fills f up to rounding (ADR 0093, bounded work): where the passes stop
/// at what f64 resolves, a good may be short by rounding, at most 1e-12 of its flows, which the
/// split shares pro rata within the Leontief invariant's 1e-12.
fn fits_up_to_rounding(p: &Province, j: usize, f: &[f64; 3]) -> bool {
    let y = outputs_at(p, f);
    let rest = p.producer[j].map_or(0.0, |q| (1.0 - p.own[j]) * y[q]);
    let uncovered = uncovered_at(p, j, f);
    uncovered <= rest + 1e-12 * (uncovered + rest)
}

/// The fill iteration run until a pass changes nothing, each step by bisection: the reference
/// the exact solve must reproduce. None where that takes more than a million passes (a loop
/// gain near 1) or a fill falls towards 0 below 1e-200.
fn converged_fills(p: &Province) -> Option<[f64; 3]> {
    bisection_iteration(p, false)
}

/// The fill iteration by bisection until a pass changes nothing (at most a million passes). A
/// fill falling below 1e-200 is None, or 0 with `flush`.
fn bisection_iteration(p: &Province, flush: bool) -> Option<[f64; 3]> {
    let mut f = [1.0; 3];
    for _ in 0..1_000_000 {
        let before = f;
        for j in 0..3 {
            if fits_at(p, j, &f) {
                continue;
            }
            let (mut lo, mut hi) = (0.0, f[j]);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if mid <= lo || mid >= hi {
                    break;
                }
                let mut g = f;
                g[j] = mid;
                if fits_at(p, j, &g) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            // near the bottom of f64 the iteration underflows good by good, which is no limit
            if lo > 0.0 && lo < 1e-200 {
                if !flush {
                    return None;
                }
                lo = 0.0;
            }
            f[j] = lo;
        }
        if f == before {
            return Some(f);
        }
    }
    None
}

/// Checks that every good fits at the exact fills and, where the fill iteration converges,
/// that they are its limit. Returns the passes of the exact solve and whether it compared.
fn fills_match(p: &Province, what: &str) -> (usize, bool) {
    let local::Fills {
        fills: exact,
        passes,
        ..
    } = local::fills(p);
    for j in 0..3 {
        assert!(
            fits_up_to_rounding(p, j, &exact),
            "{what}: good {j} short at {exact:?}"
        );
    }
    let Some(iterated) = converged_fills(p) else {
        return (passes, false);
    };
    for j in 0..3 {
        assert!(
            (exact[j] - iterated[j]).abs() <= 1e-12,
            "{what}: good {j}: exact {exact:?}, iterated {iterated:?}, {p:?}"
        );
    }
    // fills agree to 1e-12, so outputs to 1e-12 of capacity
    let (a, b) = (outputs_at(p, &exact), outputs_at(p, &iterated));
    for (u, (x, y)) in a.iter().zip(&b).enumerate() {
        assert!(
            (x - y).abs() <= 1e-12 * p.cap[u],
            "{what}: enterprise {u}: {x} vs {y}"
        );
    }
    (passes, true)
}

/// A small random province from `seed` (xorshift): two to six enterprises, each local good made
/// by at most one of them, coefficients up to 0.6, stocks often 0 and otherwise up to about
/// the need at capacity.
fn random_province(seed: u64) -> Province {
    random_province_with(seed, 5, 0.6, 0.5)
}

/// [`random_province`] with 2 to `extra` + 1 enterprises, coefficients up to `coef_max`, and a
/// stock with probability 1 - `no_stock`.
fn random_province_with(seed: u64, extra: usize, coef_max: f64, no_stock: f64) -> Province {
    let mut s = seed | 1;
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let n = 2 + (next() * extra as f64) as usize;
    let mut producer = [None; 3];
    for j in 0..3 {
        let u = (next() * n as f64) as usize;
        if next() < 0.85 && !producer.contains(&Some(u)) {
            producer[j] = Some(u);
        }
    }
    let cap: Vec<f64> = (0..n).map(|_| 0.1 + 10.0 * next()).collect();
    let coef: Vec<[f64; 3]> = (0..n)
        .map(|u| {
            std::array::from_fn(|j| {
                if producer[j] == Some(u) || next() < 0.4 {
                    0.0
                } else {
                    coef_max * next()
                }
            })
        })
        .collect();
    let stock = (0..n)
        .map(|u| {
            std::array::from_fn(|j| {
                if next() < no_stock {
                    0.0
                } else {
                    1.2 * next() * coef[u][j] * cap[u]
                }
            })
        })
        .collect();
    let own = std::array::from_fn(|_| 0.3 * next());
    Province::from_parts(cap, coef, stock, producer, own)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]
    /// ADR 0093, Amendment: on random small provinces every good fits at the exact fills, and
    /// they are the converged iteration's (1e-12) wherever it converges (about 98% of draws;
    /// the rest fall towards 0 below 1e-200, where bisection underflows good by good).
    #[test]
    fn exact_fill_equals_converged_iteration_on_random_provinces(seed in any::<u64>()) {
        fills_match(&random_province(seed), &format!("seed {seed}"));
    }
}

/// The exact fills of `p` against the fill iteration run to convergence by bisection, which must
/// converge: 1e-12 on the fills, every good fitting, no subnormal fill. Returns the fills and the
/// jumps' work.
fn assert_iteration_limit(p: &Province, what: &str) -> ([f64; 3], usize) {
    let out = local::fills(p);
    let iterated = converged_fills(p).unwrap_or_else(|| panic!("{what}: no reference limit"));
    for j in 0..3 {
        assert!(
            fits_up_to_rounding(p, j, &out.fills),
            "{what}: good {j} short at {out:?}"
        );
        assert!(
            out.fills[j] == 0.0 || out.fills[j] >= f64::MIN_POSITIVE,
            "{what}: {out:?}"
        );
        assert!(
            (out.fills[j] - iterated[j]).abs() <= 1e-12,
            "{what}: good {j}: {out:?} against {iterated:?}"
        );
    }
    assert!(!out.stalled, "{what}: {out:?}");
    (out.fills, out.work)
}

/// A province in the calibrated form: producers of the three local goods (enterprises 0 to 2)
/// and 1 to 4 others, every enterprise using every local good but its own, no stocks, the caps
/// solved so that every local market balances exactly at F = 1; then one enterprise's cap cut to
/// 30 to 99%. None where the caps come out nonpositive.
fn balanced_province(seed: u64) -> Option<Province> {
    let mut s = seed | 1;
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let m = 1 + (next() * 4.0) as usize;
    let n = 3 + m;
    let coef: Vec<[f64; 3]> = (0..n)
        .map(|u| std::array::from_fn(|j| if u == j { 0.0 } else { 0.02 + 0.2 * next() }))
        .collect();
    let own: [f64; 3] = std::array::from_fn(|_| 0.2 * next());
    let mut cap = vec![0.0; n];
    for c in cap.iter_mut().skip(3) {
        *c = 0.5 + 10.0 * next();
    }
    // (1 - own_k) cap_k - sum over the other producers e of coef_ek cap_e = the others' use
    let mut a = [[0.0f64; 4]; 3];
    for k in 0..3 {
        for e in 0..3 {
            a[k][e] = if e == k { 1.0 - own[k] } else { -coef[e][k] };
        }
        a[k][3] = (3..n).map(|e| coef[e][k] * cap[e]).sum();
    }
    for c in 0..3 {
        let pivot = (c..3)
            .max_by(|&x, &y| a[x][c].abs().total_cmp(&a[y][c].abs()))
            .unwrap();
        a.swap(c, pivot);
        for r in (0..3).filter(|&r| r != c) {
            let ratio = a[r][c] / a[c][c];
            let top = a[c];
            for (x, t) in a[r].iter_mut().zip(&top) {
                *x -= ratio * t;
            }
        }
    }
    for k in 0..3 {
        cap[k] = a[k][3] / a[k][k];
        if cap[k].is_nan() || cap[k] <= 0.0 {
            return None;
        }
    }
    let who = (next() * n as f64) as usize;
    cap[who] *= 0.3 + 0.69 * next();
    Some(Province::from_parts(
        cap,
        coef,
        vec![[0.0; 3]; n],
        [Some(0), Some(1), Some(2)],
        own,
    ))
}

/// The original of the translated collapses from `seed`: [`random_province`] with the producers'
/// stocks removed and every other stock raised to cover its user's whole need (1.01 coef cap), as
/// [`translate`] has them. Under the stock floor a partial stock gives the fall a constant, and a
/// producer covered on its inputs is not cut, so its good does not fall: the province must be free
/// of both to collapse along the same map as its translation (a quarter of the draws before the
/// floor, about 1% with the stocks as drawn).
fn translated_original(seed: u64) -> Province {
    let mut p = random_province(seed);
    for u in 0..p.cap.len() {
        let producer = p.producer.contains(&Some(u));
        for k in 0..3 {
            if p.stock[u][k] > 0.0 {
                p.stock[u][k] = if producer {
                    0.0
                } else {
                    1.01 * p.coef[u][k] * p.cap[u]
                };
            }
        }
    }
    p.rebuilt()
}

/// Whether [`translate`] applies to `p`: every local good with a user has a producer, and where
/// that producer uses local goods without stock, its users without stock need at least what it
/// supplies at capacity (A >= R in [`translate`]). Under the stock floor (ADR 0093, amendment of
/// 2026-10-30) a good without a producer has fill 0 whatever the stocks (each user bound by it
/// draws (1 - s) F of its need, and nothing supplies that), so there is no collapse to move. Where
/// A < R, balancing the good at t takes an extra user, which makes U - R = R ((F_k - t) - (F_p -
/// t)) where the users bind on the good's own fill and the producer on another: a direction of
/// gain exactly 1, a line of fixed points, of which the stop may take any on the pieces
/// (`near_singular_cycle_keeps_its_limit` is the same rule at gain 1 - 1e-15). Seed
/// 1997589725841679792 at 0.1 ended at [0.093, 0.013, 0.013] that way.
fn translatable(p: &Province) -> bool {
    let n = p.cap.len();
    let uncovered = |u: usize, k: usize| p.coef[u][k] > 0.0 && p.stock[u][k] == 0.0;
    (0..3).all(|k| {
        let used = p.coef.iter().any(|c| c[k] > 0.0);
        match p.producer[k] {
            None => !used,
            Some(pp) => {
                let total: f64 = (0..n)
                    .filter(|&u| uncovered(u, k))
                    .map(|u| p.coef[u][k] * p.cap[u])
                    .sum();
                let share = (1.0 - p.own[k]) * p.cap[pp];
                !(0..3).any(|j| uncovered(pp, j)) || total == 0.0 || total >= share
            }
        }
    })
}

/// The collapse of `p` towards 0 moved to a limit at fills t, under the stock floor (ADR 0093,
/// amendment of 2026-10-30): users with stock fully covered; then per good k, A the needs at
/// capacity of its users without stock and R its producer's share (1 - own) cap, so that U - R is
/// homogeneous in F - t with the original's slopes. A producer that uses local goods without
/// stock gets a floor s = r / (1 + r) under them, r = t (A - R) / R, and capacity (1 + r) cap: its
/// needs beyond its stocks stay cap F, and its output adds the constant R r to good k's supply
/// (A >= R: [`translatable`]). A producer that uses none (its output does not depend on the
/// fills) gets the capacity at which it supplies A t.
fn translate(p: &Province, t: f64) -> Province {
    let mut q = p.clone();
    let n = p.cap.len();
    let uncovered = |u: usize, k: usize| p.coef[u][k] > 0.0 && p.stock[u][k] == 0.0;
    for k in 0..3 {
        let total: f64 = (0..n)
            .filter(|&u| uncovered(u, k))
            .map(|u| p.coef[u][k] * p.cap[u])
            .sum();
        let Some(pp) = p.producer[k] else {
            continue;
        };
        if total == 0.0 {
            continue;
        }
        let share = (1.0 - p.own[k]) * p.cap[pp];
        if !(0..3).any(|j| uncovered(pp, j)) {
            q.cap[pp] = total * t / (1.0 - p.own[k]);
            continue;
        }
        let r = t * (total - share) / share;
        assert!(r >= 0.0, "translatable: A >= R");
        q.cap[pp] = (1.0 + r) * p.cap[pp];
        for j in (0..3).filter(|&j| uncovered(pp, j)) {
            q.stock[pp][j] = r * p.coef[pp][j] * p.cap[pp];
        }
    }
    for u in 0..n {
        for k in 0..3 {
            if p.coef[u][k] > 0.0 && p.stock[u][k] > 0.0 {
                q.stock[u][k] = 1.01 * p.coef[u][k] * q.cap[u];
            }
        }
    }
    q.rebuilt()
}

/// A two-good cycle with loop gain 1 - eps: enterprise 0 makes good 7 from good 8, enterprise 1
/// good 8 from good 7, enterprise 2 uses good 7. The fill iteration's limit is F_8 = 0.6, F_7 =
/// 0.6 (1 - eps) / 2, F_9 = 1.
fn near_singular(eps: f64) -> (Province, [f64; 3]) {
    let d = 1.0 - eps;
    let s = 2.0 / d;
    let p = Province::from_parts(
        vec![1.0, 1.0, 3.0],
        vec![
            [0.0, 0.5, 0.0],
            [0.5, 0.0, 0.0],
            [(s - 0.5) / 3.0, 0.0, 0.0],
        ],
        vec![[0.0, 0.3 * eps, 0.0], [0.0; 3], [0.0; 3]],
        [Some(0), Some(1), None],
        [0.0; 3],
    );
    (p, [0.6 / s, 0.6, 1.0])
}

/// ADR 0093, bounded work: provinces whose fills fall towards 0 settle in bounded passes and
/// work, at fills exactly 0 (never subnormal), every good fitting. Without stocks (family 2),
/// seed 13622680876955865696 falls in 4 passes and 16771139658170358534 ends at the stall rule
/// (257 passes); with stocks, 709302389291714235 in 5 and 14587390709483492012 at the stall
/// rule. Before the stock floor (ADR 0093, amendment of 2026-10-30) the first and third, drawn
/// with stocks, were the pinned cases: 13622680876955865696 fell at a rate near 1 on one pass map
/// (781 passes when every cycle was followed), 709302389291714235 switched irregularly between two
/// maps (15,678 passes before the stall rule); under the floor the first no longer collapses.
#[test]
fn collapsing_provinces_settle_in_bounded_passes() {
    let stall = 4 * local::STALL_PASSES + 16;
    for (family, seed, bound) in [
        (2, 13622680876955865696u64, 16),
        (2, 16771139658170358534, stall),
        (0, 709302389291714235, 16),
        (1, 14587390709483492012, stall),
    ] {
        let (extra, coef_max, no_stock) = FAMILIES[family];
        let p = random_province_with(seed, extra, coef_max, no_stock);
        let out = local::fills(&p);
        assert!(out.passes <= bound, "seed {seed}: {out:?}");
        assert!(out.work <= local::WORK_BOUND, "seed {seed}: {out:?}");
        for j in 0..3 {
            assert!(
                fits_up_to_rounding(&p, j, &out.fills),
                "seed {seed}: good {j} short at {out:?}"
            );
        }
        assert_eq!(out.fills, [0.0; 3], "seed {seed}");
    }
}

/// ADR 0093, bounded work: markets balanced exactly along a ray of fills (the calibrated form
/// after a capacity cut with stocks used up) drift by a unit in the last place per pass; no jump
/// extrapolates that drift, and the fills are the iteration's limit (1e-12).
#[test]
fn balanced_markets_keep_their_fills() {
    for seed in [
        12609463204538519967u64,
        8100737891477092383,
        3786356886628797132,
        11555781272310548465,
        18077030379050124424,
    ] {
        let p = balanced_province(seed).expect("positive caps");
        let (_, work) = assert_iteration_limit(&p, &format!("balanced seed {seed}"));
        assert!(work <= local::WORK_BOUND, "seed {seed}: work {work}");
    }
}

/// The exact limit of `translate(p, t)` where it is degenerate, a heuristic read from the
/// construction, not a logical consequence (ADR 0093, bounded work). `translate` makes U - R of
/// every good homogeneous in F - t, but the province is not `p` shifted: producers get a stock
/// floor and more capacity, producers without local inputs the capacity that balances. The goods
/// whose fill the bisection iteration on `p` takes to 0 (a fill below 1e-200 taken as 0) are
/// taken to fall to t, where supply equals need and the plain iteration's own result depends on
/// rounding; the goods it never cuts (fill 1) are taken to stay at 1, though rounding at the
/// others' limit may cut them in the plain iteration. This holds on the sampled provinces. The
/// 1e-200 threshold leaks: in seed 2172124891379086981 good 1 ends at 1.02e-200 in the original,
/// a collapse, and is classified None. None goods get only the loose check of lying in [t, 1].
/// `original` is the bisection iteration's limit on `p`, fills below 1e-200 flushed to 0.
fn degenerate_limit(original: &[f64; 3], t: f64) -> [Option<f64>; 3] {
    original.map(|v| {
        if v == 0.0 {
            Some(t)
        } else if v == 1.0 {
            Some(1.0)
        } else {
            None
        }
    })
}

/// How close to its exact limit a degenerate good is held (ADR 0093, bounded work). At the limit,
/// U - R is a difference of terms of the size of the gross flows that vanishes linearly in F - t,
/// so its rounding decides the fit near t. The cap is empirical: under the stock floor the worst
/// observed over 3,000,000 draws of the translated collapses is 1.08e-13 on the goods the oracle
/// holds, 1.72e-12 on any good within 1e-9 of t (seed 705654478377484459 at 0.3, good 2, which
/// it loosens); before the floor, 1.81e-12. The per-case eps (U + R) / |slope of U - R| has no
/// bound where a good's U - R is about 0 along the ray.
const RESOLUTION: f64 = 1e-11;

/// The fills of `translate(translated_original(seed), t)`, which must be held to the exact limits.
/// Where the province is degenerate (some good of the original falls to 0, by
/// [`degenerate_limit`]), each good whose heuristic limit the plain iteration on the translated
/// province confirms (within [`RESOLUTION`]) and resolves ([`loosened`]) is within
/// [`RESOLUTION`] of it, and each other good lies in [t, 1] to that resolution (the fills only
/// fall, and no good can fall below t: with the others at t or above, a good at t fits by the
/// construction of the stocks). The heuristic is not a logical consequence, so a good where the
/// two disagree, or where the plain iteration does not converge, gets the loose check only. Which
/// goods are loosened is decided at the plain iteration's limit, never at the fills under test.
/// Otherwise every good is at the plain iteration's limit (1e-12). Every good fits to 1e-12 of
/// its flows. On top of that oracle, every good with a heuristic limit that the oracle does not
/// loosen is held to it here.
fn assert_translated_limit(seed: u64, t: f64) -> local::Fills {
    let original = translated_original(seed);
    assert!(translatable(&original), "seed {seed}");
    let limit = bisection_iteration(&original, true).expect("the original iteration converges");
    let out = assert_translated_limit_of(seed, &original, &limit, t);
    let p = translate(&original, t);
    let loose = loosened(&p, converged_fills(&p), t);
    for (j, want) in degenerate_limit(&limit, t).into_iter().enumerate() {
        if let Some(want) = want.filter(|_| !loose[j]) {
            assert!(
                (out.fills[j] - want).abs() <= RESOLUTION,
                "seed {seed} t {t}: good {j}: {out:?} against {want}"
            );
        }
    }
    out
}

/// [`assert_translated_limit`] with the original province and its flushed bisection limit given,
/// without holding the goods the oracle loosens to their heuristic limits.
fn assert_translated_limit_of(
    seed: u64,
    original: &Province,
    limit: &[f64; 3],
    t: f64,
) -> local::Fills {
    let p = translate(original, t);
    let heuristic = degenerate_limit(limit, t);
    let plain = converged_fills(&p);
    let out = local::fills(&p);
    translated_verdict(&p, &heuristic, plain, &out.fills, t)
        .unwrap_or_else(|e| panic!("seed {seed} t {t}: {e}: {out:?}"));
    out
}

/// The oracle of [`assert_translated_limit`] on fills `f` of the translated province `p`, given
/// the heuristic limits and the plain iteration's limit, decided without `f` but for the checks
/// themselves. Ok(whether every good with a heuristic limit was held to it), or what failed.
fn translated_verdict(
    p: &Province,
    heuristic: &[Option<f64>; 3],
    plain: Option<[f64; 3]>,
    f: &[f64; 3],
    t: f64,
) -> Result<bool, String> {
    let degenerate = heuristic.contains(&Some(t));
    let loose = loosened(p, plain, t);
    // the heuristic limits the plain iteration confirms, at the goods whose limit it resolves
    let exact: [Option<f64>; 3] = std::array::from_fn(|j| {
        heuristic[j]
            .filter(|&want| !loose[j] && plain.is_some_and(|it| (it[j] - want).abs() <= RESOLUTION))
    });
    for (j, &x) in f.iter().enumerate() {
        if !fits_up_to_rounding(p, j, f) {
            return Err(format!("good {j} short"));
        }
        if !degenerate {
            let want = plain.ok_or("no reference")?[j];
            if (x - want).abs() > 1e-12 {
                return Err(format!("good {j} against {want}"));
            }
            continue;
        }
        match exact[j] {
            Some(want) if (x - want).abs() > RESOLUTION => {
                return Err(format!("good {j} against {want}"));
            }
            None if x < t - RESOLUTION || x > 1.0 => {
                return Err(format!("good {j} outside [t, 1]"));
            }
            _ => {}
        }
    }
    Ok((0..3).all(|j| heuristic[j].is_none() || exact[j].is_some()))
}

/// The goods of `p` whose limit the plain iteration's limit `plain` does not resolve: those whose
/// balance there is rounding whatever their own fill ([`shortage_at_one`] within [`RESOLUTION`]
/// of 0), and those whose choice between staying at 1 and being cut turns on where such goods
/// end: with each of them at t or at 1, the others at `plain`, the good's [`shortage_at_one`]
/// is within [`RESOLUTION`] of 0 or changes sign. Every good where the plain iteration does not
/// converge.
fn loosened(p: &Province, plain: Option<[f64; 3]>, t: f64) -> [bool; 3] {
    let Some(at) = plain else {
        return [true; 3];
    };
    let unresolved: [bool; 3] =
        std::array::from_fn(|k| shortage_at_one(p, k, &at).abs() <= RESOLUTION);
    std::array::from_fn(|j| {
        if unresolved[j] {
            return true;
        }
        let side = shortage_at_one(p, j, &at) > 0.0;
        (0..8u8).any(|ends| {
            let mut g = at;
            for k in (0..3).filter(|&k| unresolved[k]) {
                g[k] = if ends >> k & 1 == 1 { 1.0 } else { t };
            }
            let s = shortage_at_one(p, j, &g);
            s.abs() <= RESOLUTION || (s > 0.0) != side
        })
    })
}

/// What good j of `p` lacks at fills `f` with its own raised to 1, U - R, relative to its flows
/// at F = 1. Where this is within [`RESOLUTION`] of 0 at the plain iteration's limit, the good's
/// users bind on the other goods at t and its producer falls with them: its balance is rounding
/// whatever its own fill, so whether it falls to t, stays at 1 or stops between is decided by the
/// rounding of the other fills, and its limit is not resolved; nor are the limits of goods whose
/// choice between 1 and a cut turns on where it ends ([`loosened`]). Each of these is held by
/// that rule, none by name. The examples measured before the stock floor (seeds
/// 1097310483361641164, 7641237549142102837, 13946236107313884973 at 0.1) are not translatable
/// under it: each has a local good without a producer.
fn shortage_at_one(p: &Province, j: usize, f: &[f64; 3]) -> f64 {
    let mut g = *f;
    g[j] = 1.0;
    let y = outputs_at(p, &g);
    let rest = p.producer[j].map_or(0.0, |q| (1.0 - p.own[j]) * y[q]);
    let uncovered = uncovered_at(p, j, &g);
    let flows: f64 = (0..p.cap.len())
        .map(|u| p.coef[u][j] * p.cap[u])
        .sum::<f64>()
        + p.producer[j].map_or(0.0, |q| (1.0 - p.own[j]) * p.cap[q]);
    (uncovered - rest) / flows
}

/// ADR 0093, bounded work: collapses moved to a limit at t, every good with a heuristic limit the
/// oracle does not loosen held to it. The cases pinned before the stock floor (ADR 0093, amendment
/// of 2026-10-30; jumps that overshot to 0, a root rounded onto a piece's end, stops 1e-12 above
/// t) are not translatable under it, but for 1122501757669556625 at 0.3, kept. The others are the
/// longest of 3,000,000 draws (9 passes: 5311499786894156349 at 0.1, 11108278234816594936 at
/// 0.3) and the one whose fill ends furthest from t (705654478377484459 at 0.3, good 2 1.72e-12
/// above it, loosened).
#[test]
fn translated_collapses_stop_at_their_limit() {
    for (seed, t) in [
        (1122501757669556625u64, 0.3),
        (5311499786894156349, 0.1),
        (11108278234816594936, 0.3),
        (705654478377484459, 0.3),
    ] {
        assert_translated_limit(seed, t);
    }
}

/// What [`translated_sample`] counts.
#[derive(Debug, Default)]
struct Sample {
    /// Cases checked.
    checked: usize,
    /// Cases held to every heuristic limit, none to the loose check only.
    held: usize,
    /// Mutants rejected: a good the plain iteration confirms at its heuristic limit 1 and the
    /// fills keep at 1, lowered to t (fitting: lowering a fill only helps fit).
    rejected: usize,
    /// Mutants accepted: the good's own balance is rounding at the plain limit
    /// ([`shortage_at_one`]), where the fills may end at t, at 1 or between.
    unresolved: usize,
    /// The largest distance from t of a fill whose heuristic limit is t, which the plain iteration
    /// confirms there and resolves ([`loosened`]).
    worst: f64,
}

/// The collapsing provinces among `n` random draws from `offset` (SplitMix64 seeds), each moved
/// to limits at 0.1, 0.3 and 0.5, every one held to [`assert_translated_limit`]. A collapse is
/// chosen without the code under test: a [`translatable`] draw ([`translated_original`]) whose
/// plain fill iteration ([`bisection_iteration`], fills below 1e-200 flushed to 0) ends with at
/// least two fills of 0, about 0.4% of the draws (about 23% before the stock floor, which drops
/// the goods without a producer and those whose producer supplies more than they need). The
/// oracle is tested too: each good the plain iteration confirms at 1 and the fills keep there is
/// lowered to t, and [`translated_verdict`] must reject that, unless the good's own limit is not
/// resolved at the plain limit.
fn translated_sample(n: u64, offset: u64) -> Sample {
    let mix = |i: u64| {
        let mut z = i.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let mut sample = Sample::default();
    for i in 0..n {
        let seed = mix(i + offset);
        let original = translated_original(seed);
        if !translatable(&original) {
            continue;
        }
        let Some(limit) = bisection_iteration(&original, true) else {
            continue;
        };
        if limit.iter().filter(|&&x| x == 0.0).count() < 2 {
            continue;
        }
        for t in [0.1, 0.3, 0.5] {
            let p = translate(&original, t);
            let heuristic = degenerate_limit(&limit, t);
            let plain = converged_fills(&p);
            let out = local::fills(&p);
            let held = translated_verdict(&p, &heuristic, plain, &out.fills, t)
                .unwrap_or_else(|e| panic!("seed {seed} t {t}: {e}: {out:?}"));
            sample.checked += 1;
            sample.held += usize::from(held);
            let Some(at) = plain else {
                continue;
            };
            let loose = loosened(&p, plain, t);
            for j in 0..3 {
                if heuristic[j] == Some(t) && !loose[j] && (at[j] - t).abs() <= RESOLUTION {
                    sample.worst = sample.worst.max((out.fills[j] - t).abs());
                }
            }
            for j in 0..3 {
                if heuristic[j] != Some(1.0) || (at[j] - 1.0).abs() > RESOLUTION {
                    continue;
                }
                if out.fills[j] != 1.0 {
                    continue;
                }
                let mut wrong = out.fills;
                wrong[j] = t;
                if translated_verdict(&p, &heuristic, plain, &wrong, t).is_err() {
                    sample.rejected += 1;
                } else {
                    assert!(
                        shortage_at_one(&p, j, &at).abs() <= RESOLUTION,
                        "seed {seed} t {t}: good {j} lowered to t accepted: {wrong:?}"
                    );
                    sample.unresolved += 1;
                }
            }
        }
    }
    sample
}

/// ADR 0093, bounded work: a sample of the translated collapses (75,000 draws: 969 cases, 546
/// held to every heuristic limit; 393 mutants rejected, 10 at goods whose own limit is rounding;
/// before the stock floor 5,000 draws gave 3,552 cases). The minimums are these counts. The
/// selection alone fixes the cases, so a case fails rather than drops out; the held count turns
/// on the plain iteration and the heuristic only.
#[test]
fn translated_collapses_sample() {
    let s = translated_sample(75_000, 12 << 40);
    println!("{s:?}");
    assert!(
        s.checked >= 969 && s.held >= 546 && s.rejected >= 393,
        "{s:?}"
    );
}

/// ADR 0093, bounded work: the translated collapses of 3,000,000 draws (38,187 cases, 19,947 held
/// to every heuristic limit; 14,277 mutants rejected, 553 at goods whose own limit is rounding;
/// minimums as in [`translated_collapses_sample`]; about 56 s in release); slow, so run on
/// request.
#[test]
#[ignore = "slow: 3,000,000 draws; run on request"]
fn translated_collapses_full() {
    let s = translated_sample(3_000_000, 12 << 40);
    println!("{s:?}");
    assert!(
        s.checked >= 38_187 && s.held >= 19_947 && s.rejected >= 14_277,
        "{s:?}"
    );
}

/// ADR 0093, bounded work: the oracle's loosening is decided without the fills under test.
/// While it read the fills, lowering a good at 1 to t in these exact limits was accepted (a
/// review's mutants, 3,898 of 6,418 accepted, on seeds 9896657790763124343 and
/// 17141039903868238212, which have a local good without a producer and are not translatable
/// under the stock floor). Read at the plain limit, the good at 1 fits by a margin wherever the
/// unresolved goods end, so it is held to 1 and the wrong fills are rejected: here on the first
/// two seeds of the sample with a rejected mutant.
#[test]
fn translated_oracle_rejects_a_good_lowered_from_one() {
    for (seed, t, j) in [
        (10705095954986202167u64, 0.1, 0),
        (10705095954986202167, 0.3, 0),
        (10705095954986202167, 0.5, 0),
        (7502845947401349490, 0.1, 2),
        (7502845947401349490, 0.3, 2),
    ] {
        let original = translated_original(seed);
        let limit = bisection_iteration(&original, true).expect("the original iteration converges");
        let out = assert_translated_limit_of(seed, &original, &limit, t);
        assert_eq!(out.fills[j], 1.0, "seed {seed} t {t}: {out:?}");
        let p = translate(&original, t);
        let mut wrong = out.fills;
        wrong[j] = t;
        assert!((0..3).all(|k| fits_up_to_rounding(&p, k, &wrong)));
        let verdict = translated_verdict(
            &p,
            &degenerate_limit(&limit, t),
            converged_fills(&p),
            &wrong,
            t,
        );
        assert!(verdict.is_err(), "seed {seed} t {t}: {wrong:?} accepted");
    }
}

/// ADR 0093, bounded work: seed 1997589725841679792 at 0.1, where good 1's users need less than
/// its producer supplies, is not [`translatable`]: balanced at t by an extra user, it has a line of
/// fixed points along which the stop took [0.093, 0.013, 0.013], below t.
#[test]
fn translated_family_leaves_out_a_neutral_good() {
    let p = translated_original(1997589725841679792);
    assert!(!translatable(&p));
}

/// ADR 0093, bounded work: the neutral good of [`translated_family_leaves_out_a_neutral_good`]
/// balanced at t = 0.1 by an extra user without stock that takes its producer's surplus over its
/// users' needs (coef 0.5, the capacity at which it needs that surplus). The province has a
/// direction of gain exactly 1, a line of fixed points through t, and the fills are not unique
/// there: the stop takes the last pass's fixed point on its pieces, [0.093, 0.013, 0.013], every
/// good fitting, so a fixed point of the passes, though the plain iteration from 1 ends at t to
/// rounding. Below t every user bound by the goods runs less than at the limit the SPEC defines:
/// an over-cut, deterministic, with every invariant holding.
#[test]
fn neutral_good_stop_takes_a_fixed_point_on_its_line() {
    let t = 0.1;
    let mut p = translated_original(1997589725841679792);
    let n = p.cap.len();
    for k in 0..3 {
        let Some(pp) = p.producer[k] else { continue };
        let total: f64 = (0..n)
            .filter(|&u| p.coef[u][k] > 0.0 && p.stock[u][k] == 0.0)
            .map(|u| p.coef[u][k] * p.cap[u])
            .sum();
        let share = (1.0 - p.own[k]) * p.cap[pp];
        let uncovered = (0..3).any(|j| p.coef[pp][j] > 0.0 && p.stock[pp][j] == 0.0);
        if uncovered && total > 0.0 && total < share {
            let mut c = [0.0; 3];
            c[k] = 0.5;
            p.cap.push((share - total) / 0.5);
            p.coef.push(c);
            p.stock.push([0.0; 3]);
        }
    }
    let q = translate(&p.rebuilt(), t);
    let out = local::fills(&q);
    let plain = converged_fills(&q).expect("the plain iteration converges");
    assert_eq!(q.cap.len(), n + 2, "two goods balanced by an extra user");
    // a fixed point of the passes: every good fits there, so no step moves it
    for j in 0..3 {
        assert!(fits_at(&q, j, &out.fills), "good {j} short at {out:?}");
    }
    let want = [0.0933514012111659, 0.0129778015608787, 0.0129778015608787];
    for j in 0..3 {
        assert!((plain[j] - t).abs() <= 1e-12, "{plain:?}");
        assert!((out.fills[j] - want[j]).abs() <= 1e-12, "{out:?}");
    }
    // the over-cut: no user runs above its output at t, and some run below
    let (at, limit) = (outputs_at(&q, &out.fills), outputs_at(&q, &plain));
    assert!(
        at.iter().zip(&limit).all(|(a, l)| a <= l),
        "{at:?} {limit:?}"
    );
    assert!(
        at.iter().zip(&limit).any(|(a, l)| a < &(0.5 * l)),
        "{at:?} {limit:?}"
    );
}

/// ADR 0093, bounded work: provinces whose collapse outlasts the stall rule end within
/// `local::PASS_CAP` passes, every good fitting to 1e-12 of its flows, the stall reported. Seeds
/// 15714615364009314852 and 17705091116894527464 (no stocks) took 818 to 1,518 passes with no
/// bound after the stall was reported; 5355749340075788755 (scarce stocks) stalls at 792. Seeds
/// 17368639463330687291 (random) and 14824132916395666595 (up to 10 enterprises) stalled before
/// the stock floor (ADR 0093, amendment of 2026-10-30) and settle in 2 and 8 passes under it. The
/// last case is a two-good cycle of gain 1 - 1e-6 with 200 small users with stock on its path:
/// before the floor their thresholds were 200 kinks, each costing a jump (a review measured
/// 10,600,518 passes on an earlier build); under it each stock is a floor and no kink, and the
/// case is held to the cap only.
#[test]
fn stalled_provinces_end_within_the_pass_cap() {
    let mut cases: Vec<(String, Province)> = vec![
        (
            "15714615364009314852".into(),
            random_province_with(15714615364009314852, 5, 0.6, 1.0),
        ),
        (
            "17705091116894527464".into(),
            random_province_with(17705091116894527464, 5, 0.6, 1.0),
        ),
        (
            "5355749340075788755".into(),
            random_province_with(5355749340075788755, 5, 0.6, 0.9),
        ),
    ];
    let (mut kinks, _) = near_singular(1e-6);
    for i in 0..200 {
        let threshold = 0.49 - 0.18 * (i as f64 + 0.5) / 200.0;
        kinks.cap.push(1.0);
        kinks.coef.push([1e-3, 0.0, 0.0]);
        kinks.stock.push([1e-3 * threshold, 0.0, 0.0]);
    }
    cases.push(("200 kinks".into(), kinks.rebuilt()));
    for (what, p) in &cases {
        let out = local::fills(p);
        println!("{what}: {out:?}");
        assert!(out.passes <= local::PASS_CAP, "{what}: {out:?}");
        assert!(out.work <= local::WORK_BOUND, "{what}: {out:?}");
        for j in 0..3 {
            assert!(
                fits_up_to_rounding(p, j, &out.fills),
                "{what}: good {j} short at {out:?}"
            );
        }
        if what != "200 kinks" {
            assert!(out.stalled, "{what}: {out:?}");
        }
    }
}

/// ADR 0093, bounded work: a province alternating between two pass maps stops at what f64
/// resolves 1.7e-12 above its limit unless the stop takes the two-pass cycle's fixed point.
#[test]
fn stop_takes_the_cycle_fixed_point() {
    let p = random_province(13018842184927700166);
    let (_, work) = assert_iteration_limit(&p, "seed 13018842184927700166");
    assert!(work <= local::WORK_BOUND);
}

/// ADR 0093, bounded work: a cycle with loop gain 1 - 1e-15 ends near its limit 0.6, not at 0:
/// the stall rule moves its goods to the last pass's fixed point, clamped to [0, f]. Within 10%:
/// the gain's rounding (about 1e-16) is a tenth of its distance from 1, which is the problem's
/// own conditioning.
#[test]
fn near_singular_cycle_keeps_its_limit() {
    let (p, limit) = near_singular(1e-15);
    let out = local::fills(&p);
    for j in 0..3 {
        assert!(
            fits_up_to_rounding(&p, j, &out.fills),
            "good {j} short at {out:?}"
        );
        assert!(
            (out.fills[j] - limit[j]).abs() <= 0.1 * limit[j],
            "{out:?} against {limit:?}"
        );
    }
    assert!(out.passes <= 4 * local::STALL_PASSES + 16, "{out:?}");
}

/// ADR 0093, Amendment: at every province tick of the fixture's default run (60 ticks) and of
/// the cycle fixture (120 ticks), every good fits at the exact fills (to 1e-12 of its flows) and
/// they equal the fill iteration run to convergence (1e-12) wherever it converges. Where a fill
/// collapses below 1e-200 the bisection iteration underflows good by good and gives no limit to
/// compare with, so the bound on the compared ticks is coverage, not model: at least 400 of the
/// binding province ticks: 476 of 482 (480 of 483 before the stock floor of ADR 0093's
/// amendment of 2026-10-30, 534 of 547 before ADR 0266's split order rule, 410 of 415 before its
/// input cap, 411 of 413 before the gateway masses).
#[test]
fn exact_fill_equals_converged_iteration() {
    let cycles = [
        (SERVICES, SERVICES, 0.3),
        (SERVICES, CONSTRUCTION, 0.2),
        (CONSTRUCTION, SERVICES, 0.2),
    ];
    let (mut bound, mut compared_bound) = (0, 0);
    for (inputs, ticks) in [(small_world(), 60), (fixture_with(&cycles), 120)] {
        let mut sim = EconWorld::sim(
            Arc::new(inputs),
            EconParams::default(),
            TradeParams::default(),
        );
        for tick in 0..ticks {
            let mut probe = sim.world().clone();
            probe.begin_tick();
            systems::deliver(&mut probe);
            systems::labour(&mut probe);
            systems::cap_output(&mut probe);
            let (inp, lay) = (probe.inputs().clone(), probe.layout().clone());
            for i in 0..inp.n_provinces() {
                let cap = &probe.scratch.produced[lay.enterprises(i)];
                let p = Province::new(&inp, &lay, &probe.ledger, i, cap);
                let what = format!("tick {tick} province {i}");
                let (passes, compared) = fills_match(&p, &what);
                if passes > 1 {
                    bound += 1;
                    compared_bound += usize::from(compared);
                }
            }
            sim.step().unwrap_or_else(|v| panic!("{v}"));
        }
    }
    println!("chain bound in {bound} province ticks, {compared_bound} compared");
    assert!(compared_bound >= 400, "{compared_bound} of {bound}");
}

/// S11 Proportional: one fill ratio for both classes at every short origin market.
#[test]
fn proportional_rationing_switch() {
    let params = EconParams {
        rationing: Rationing::Proportional,
        ..EconParams::default()
    };
    let mut sim = sim_of(world(params));
    let mut short = 0;
    for _ in 0..36 {
        sim.step().unwrap();
        let w = sim.world();
        for (c, f) in w.scratch.fill.iter().enumerate() {
            if TRADED[c % N_GOODS] && w.markets.active[c] && f != &[1.0, 1.0] {
                assert_eq!(f[0], f[1], "cell {c}");
                short += 1;
            }
        }
    }
    assert!(short > 0, "no shortage at an origin");
}

/// ADR 0020: with one vintage (the 2000 ICIO cost shares at base prices) putty-clay coefficients run exactly as
/// fixed Leontief (S09), down to the state hash; a second vintage would tell them apart.
#[test]
fn putty_clay_one_vintage_equals_fixed_leontief() {
    let with = |technology| EconParams {
        technology,
        ..EconParams::default()
    };
    let (putty, fixed) = (with(Technology::PuttyClay), with(Technology::FixedLeontief));
    let mut a = sim_of(world(putty.clone()));
    let mut b = sim_of(world(fixed.clone()));
    assert_eq!(a.world().digest(), b.world().digest());
    for tick in 0..120 {
        a.step().unwrap();
        b.step().unwrap();
        assert_eq!(a.state_hash(), b.state_hash(), "tick {tick}");
    }

    let mut inputs = small_world();
    inputs.enterprises.coef[0][SERVICES] *= 1.5;
    let t = TradeParams::default();
    assert_ne!(
        static_digest(&inputs, &putty, &t),
        static_digest(&inputs, &fixed, &t)
    );
    let (inputs, e) = (Arc::new(inputs), 0);
    let fixed_world = EconWorld::new(Arc::clone(&inputs), fixed, t.clone());
    let putty_world = EconWorld::new(Arc::clone(&inputs), putty, t);
    assert_eq!(fixed_world.layout().coef[e], inputs.base_column(e));
    assert_eq!(putty_world.layout().coef[e], inputs.enterprises.coef[e]);
}

/// Unreachable origins at world level (`fixture::unreachable_island`): machinery has no reachable
/// origin at the island. Its delivered price there is the only infinite one and the world
/// price index leaves it out; island pops buy none of it and spend the rest of their money;
/// the island's light manufacturing, its one user there, orders none, never restocks, and is
/// capped by the stock it started with until it produces nothing; no price, wage or flow
/// turns NaN, and the invariants hold every tick.
#[test]
fn unreachable_destination_buys_nothing_in_the_world() {
    for params in [EconParams::default(), EconParams::frictionless()] {
        let trade = TradeParams::default().bulk_off();
        let mut w = EconWorld::new(Arc::new(unreachable_island(&trade)), params, trade);
        let (inp, lay) = (w.inputs().clone(), w.layout().clone());
        let island = inp.n_provinces() - 1;
        let cell = island * N_GOODS + UNREACHABLE;
        let user = producer(&w, island, ISLAND_USER);
        let (account, buyer) = (lay.accounts.enterprise[user], inp.n_pops() + user);
        let coef = lay.coef[user][UNREACHABLE];
        let mut stock = w.ledger.goods(account, UNREACHABLE);
        assert!(coef > 0.0 && stock > 0.0);
        assert_eq!(w.price_index[cell], f64::INFINITY);
        let mut spending = 0;
        for tick in 0..240 {
            w.begin_tick();
            systems::deliver(&mut w);
            systems::labour(&mut w);
            systems::produce(&mut w);
            assert!(
                w.output[user] * coef <= stock * (1.0 + 1e-12),
                "tick {tick}"
            );
            let money: Vec<f64> = lay
                .pops(island)
                .map(|k| to_numeraire(w.ledger.money(lay.accounts.pop[k])))
                .collect();
            systems::demand(&mut w);
            for (c, p) in w.price_index.iter().enumerate() {
                assert!(p.is_finite() == (c != cell), "tick {tick} cell {c}: {p}");
            }
            assert!(w.world_price_index().is_finite());
            let prices = &w.price_index[island * N_GOODS..(island + 1) * N_GOODS];
            for (k, money) in lay.pops(island).zip(money) {
                let x = &w.scratch.pop_demand[k * N_GOODS..(k + 1) * N_GOODS];
                assert_eq!(x[UNREACHABLE], 0.0);
                let spent: f64 = (0..N_GOODS)
                    .filter(|&g| x[g] > 0.0)
                    .map(|g| x[g] * prices[g])
                    .sum();
                // a pop whose money a loss-making tick left negative buys nothing, as on the
                // plain fixture
                let want = money.max(0.0);
                assert!(
                    (spent - want).abs() <= 1e-9 * want,
                    "tick {tick} pop {k}: spent {spent} of {money}"
                );
                spending += usize::from(money > 0.0);
            }
            assert_eq!(w.scratch.ent_spend[user * N_GOODS + UNREACHABLE], 0.0);
            systems::source(&mut w);
            assert_eq!(w.scratch.class_spend[cell], [0.0; 2]);
            systems::clear(&mut w);
            from_dispatch(&mut w, tick);
            let held = w.ledger.goods(account, UNREACHABLE);
            assert!(held <= stock, "tick {tick}: restocked {stock} -> {held}");
            stock = held;
            for b in lay.pops(island).chain([buyer]) {
                assert_eq!(w.pipeline.on_order(b, UNREACHABLE), 0.0);
            }
            assert!(w.markets.price.iter().all(|p| p.is_finite() && *p > 0.0));
            assert!(w.wage.iter().all(|x| x.is_finite() && *x > 0.0));
            assert!(w.output.iter().chain(&w.employment).all(|x| x.is_finite()));
            assert!(w.expected_inflation.is_finite());
            let traded = (0..N_GOODS).filter(|&g| TRADED[g]);
            assert!(traded.map(|g| w.pipeline.in_flight(g)).all(f64::is_finite));
            for a in &lay.accounts.enterprise {
                assert!(w.ledger.money(*a) <= 0, "a profit was not paid out");
            }
            for a in &lay.accounts.market {
                assert_eq!(w.ledger.money(*a), 0);
            }
        }
        assert!(spending > 0, "no island pop had money to spend");
        assert_eq!(w.output[user], 0.0);
        let mut others = lay.enterprises(island).filter(|&e| e != user);
        assert!(others.all(|e| w.output[e] > 0.0));
    }
}

/// The calibrated fixture, run up to E4 of its first tick.
fn calibrated_before_distribute() -> EconWorld {
    let calibration = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/p2-fixture/calibration");
    let mut inputs = small_world();
    inputs.calibrate(&calibration).unwrap();
    let mut w = EconWorld::new(
        Arc::new(inputs),
        EconParams::default(),
        TradeParams::default(),
    );
    up_to_dispatch(&mut w);
    systems::dispatch(&mut w);
    systems::reprice(&mut w);
    w
}

/// Sets `account`'s balance to `target` against the mint (no pop involved).
fn set_balance(w: &mut EconWorld, account: AccountId, target: i64) {
    let mint = w.layout().accounts.mint;
    let have = w.ledger.money(account);
    w.ledger.transfer(MINT, account, mint, have - target);
}

/// Sum of WRITE_OFF entries from `start` on into (positive) or out of (negative) `account`.
fn written(w: &EconWorld, start: usize, account: AccountId) -> i64 {
    w.ledger.journal()[start..]
        .iter()
        .filter(|x| x.flow == WRITE_OFF)
        .map(|x| i64::from(x.to == account) * x.amount - i64::from(x.from == account) * x.amount)
        .sum()
}

/// Loss write-off (ADR 0112): an enterprise's retained loss beyond M months of its base sales
/// is written off at E4 against its country's pops in proportion to their positive money,
/// exactly in minor units; no pop pays more than it holds.
#[test]
fn write_off_takes_the_excess_beyond_the_cap_from_the_pops() {
    let mut w = calibrated_before_distribute();
    let lay = w.layout().clone();
    let acc = &lay.accounts;
    let (account, cap) = (acc.enterprise[0], lay.ent_loss_cap[0]);
    let c = w
        .inputs()
        .country_of(w.inputs().enterprises.province[0] as usize);
    assert!(cap > 0);
    let excess = 12_345;
    set_balance(&mut w, account, -cap - excess);
    let start = w.ledger.journal().len();
    systems::distribute(&mut w);
    assert_eq!(w.ledger.money(account), -cap, "only the excess goes");
    assert_eq!(written(&w, start, account), excess);
    let levied: i64 = lay.country_pops[c]
        .iter()
        .map(|&k| -written(&w, start, acc.pop[k as usize]))
        .sum();
    assert_eq!(levied, excess, "the country's pops pay exactly the excess");
    assert!(acc.pop.iter().all(|&a| written(&w, start, a) <= 0));
    assert!(acc.pop.iter().all(|&a| w.ledger.money(a) >= 0));
    assert_eq!(w.ledger.money(acc.dividend[c]), 0);
    w.end_tick(0).unwrap();
}

/// The cap is M months of base sales, the calibrated steady-state sales value, and a loss up to
/// it stays where it is.
#[test]
fn write_off_spares_losses_within_the_cap() {
    let mut w = calibrated_before_distribute();
    let lay = w.layout().clone();
    let acc = &lay.accounts;
    let steady = w.inputs().steady.clone().unwrap();
    for (e, &cap) in lay.ent_loss_cap.iter().enumerate() {
        let want = to_minor(3.0 * steady.ent_sales[e]) as f64;
        assert!(
            (cap as f64 - want).abs() <= 1e-12 * want + 1.0,
            "enterprise {e}"
        );
    }
    let cap = lay.ent_loss_cap[0];
    set_balance(&mut w, acc.enterprise[0], -cap);
    let start = w.ledger.journal().len();
    systems::distribute(&mut w);
    assert!(
        w.ledger.journal()[start..]
            .iter()
            .all(|x| x.flow != WRITE_OFF)
    );
    assert_eq!(w.ledger.money(acc.enterprise[0]), -cap);
    w.end_tick(0).unwrap();
}

/// Where the country's pops hold less positive money than the excess, all of it is taken (no
/// pop below zero, pops already negative untouched), shared over the enterprises by their
/// excess, and the rest stays with them for the next tick.
#[test]
fn write_off_takes_what_the_pops_hold_and_carries_the_rest() {
    let mut w = calibrated_before_distribute();
    let lay = w.layout().clone();
    let acc = &lay.accounts;
    let inp = w.inputs().clone();
    let c = inp.country_of(inp.enterprises.province[0] as usize);
    let e2 = (1..inp.n_enterprises())
        .find(|&e| inp.country_of(inp.enterprises.province[e] as usize) == c)
        .unwrap();
    let (a1, cap1) = (acc.enterprise[0], lay.ent_loss_cap[0]);
    let (a2, cap2) = (acc.enterprise[e2], lay.ent_loss_cap[e2]);
    // nothing else reaches the pops this tick: every holder and rebate account at zero
    for &a in acc.enterprise.iter().chain(&acc.market).chain(&acc.rebate) {
        set_balance(&mut w, a, 0);
    }
    let pops: Vec<usize> = lay.country_pops[c].iter().map(|&k| k as usize).collect();
    for (n, &k) in pops.iter().enumerate() {
        set_balance(&mut w, acc.pop[k], if n == 0 { -500 } else { 7 });
    }
    let room = 7 * (pops.len() as i64 - 1);
    let (x1, x2) = (3 * room, room);
    set_balance(&mut w, a1, -cap1 - x1);
    set_balance(&mut w, a2, -cap2 - x2);
    let start = w.ledger.journal().len();
    systems::distribute(&mut w);
    assert_eq!(written(&w, start, a1) + written(&w, start, a2), room);
    assert!(
        (written(&w, start, a1) - 3 * room / 4).abs() <= 1,
        "shared by excess"
    );
    assert_eq!(
        w.ledger.money(acc.pop[pops[0]]),
        -500,
        "a negative pop pays nothing"
    );
    for &k in &pops[1..] {
        assert_eq!(
            w.ledger.money(acc.pop[k]),
            0,
            "pop {k} paid all it had, no more"
        );
    }
    assert!(
        w.ledger.money(a1) < -cap1 && w.ledger.money(a2) < -cap2,
        "the rest is carried"
    );
    w.end_tick(0).unwrap();

    // next tick the pops hold enough again, and the carried rest is written off down to the caps
    w.begin_tick();
    for &a in acc.market.iter().chain(&acc.rebate) {
        set_balance(&mut w, a, 0);
    }
    for &a in acc.enterprise.iter().filter(|&&a| a != a1 && a != a2) {
        set_balance(&mut w, a, 0);
    }
    for &k in &pops[1..] {
        set_balance(&mut w, acc.pop[k], 10 * room);
    }
    let start = w.ledger.journal().len();
    systems::distribute(&mut w);
    assert_eq!(
        written(&w, start, a1) + written(&w, start, a2),
        x1 + x2 - room
    );
    assert_eq!((w.ledger.money(a1), w.ledger.money(a2)), (-cap1, -cap2));
    assert!(acc.pop.iter().all(|&a| written(&w, start, a) <= 0));
    w.end_tick(1).unwrap();
}

/// ADR 0106: the write-off's country total is an aggregate moved through the wide pool account,
/// so it may exceed i64 where each pop's money and each enterprise's excess fit. Pops holding
/// 1.5 x i64::MAX in all and enterprises with as much excess: the write-off completes and
/// conserves, every pop paying exactly what its enterprise counterparts receive.
#[test]
fn write_off_total_beyond_i64_conserves() {
    let mut w = calibrated_before_distribute();
    let lay = w.layout().clone();
    let acc = &lay.accounts;
    let inp = w.inputs().clone();
    let c = (0..inp.n_countries())
        .find(|&c| lay.country_pops[c].len() >= 3)
        .expect("a country with three pops");
    let pops: Vec<usize> = lay.country_pops[c].iter().map(|&k| k as usize).collect();
    let ents: Vec<usize> = (0..inp.n_enterprises())
        .filter(|&e| inp.country_of(inp.enterprises.province[e] as usize) == c)
        .collect();
    assert!(ents.len() >= 2, "two enterprises in country {c}");
    let each = i64::MAX / 2;
    for (n, &k) in pops.iter().enumerate() {
        set_balance(&mut w, acc.pop[k], 0);
        set_balance(&mut w, acc.pop[k], if n < 3 { each } else { 0 });
    }
    for &e in &ents {
        set_balance(&mut w, acc.enterprise[e], 0);
    }
    for &e in &ents[..2] {
        set_balance(&mut w, acc.enterprise[e], -i64::MAX);
    }
    let held = 3 * i128::from(each);
    let excess: i128 = ents[..2]
        .iter()
        .map(|&e| i128::from(i64::MAX) - i128::from(lay.ent_loss_cap[e]))
        .sum();
    let total = held.min(excess);
    assert!(total > i128::from(i64::MAX));
    let start = w.ledger.journal().len();
    systems::write_off(&mut w);
    let paid: i128 = pops
        .iter()
        .map(|&k| -i128::from(written(&w, start, acc.pop[k])))
        .sum();
    let received: i128 = ents
        .iter()
        .map(|&e| i128::from(written(&w, start, acc.enterprise[e])))
        .sum();
    assert_eq!((paid, received), (total, total));
    assert_eq!(w.ledger.money_wide(acc.dividend[c]), 0);
    assert!(pops.iter().all(|&k| w.ledger.money(acc.pop[k]) >= 0));
    for &e in &ents[..2] {
        assert!(w.ledger.money(acc.enterprise[e]) <= -lay.ent_loss_cap[e]);
    }
}

/// A run with write-offs is identical at 1 and 4 threads.
#[test]
fn write_off_runs_are_thread_independent() {
    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            let mut w = world(EconParams::frictionless());
            for (c, p) in w.markets.price.iter_mut().enumerate() {
                *p *= if c % 3 == 0 { 0.2 } else { 3.0 };
            }
            let mut sim = sim_of(w);
            let mut fired = 0;
            for _ in 0..120 {
                sim.step().unwrap();
                fired += sim
                    .world()
                    .ledger
                    .journal()
                    .iter()
                    .filter(|x| x.flow == WRITE_OFF)
                    .count();
            }
            (sim.snapshot_hashes(), fired)
        })
    };
    let ((one, fired), (four, _)) = (run(1), run(4));
    assert!(fired > 0, "no write-off fired");
    assert_eq!(one, four);
}
