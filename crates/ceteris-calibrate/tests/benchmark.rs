//! The benchmark harness (ADR 0200, ADR 0207): shocks reach the kernel as written, welfare and
//! ACR are exact on the two-country toy and, with `CETERIS_PACK` set to a calibrated pack,
//! ADR 0200's benchmark cases (Japan under costlier borders, the EU under Brexit) have the
//! literature's sign. Run the pack cases with `--release`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ceteris_calibrate::benchmark::{self, Outcome};
use ceteris_calibrate::border;
use ceteris_calibrate::shock::{self, Shock};
use std::sync::Arc;

use ceteris_core::rows::Rows;
use ceteris_core::sim::Sim;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconParams, EconWorld};
use ceteris_trade::goods::p2::{N as N_GOODS, TRADED as N_TRADED};
use ceteris_trade::kernel::Kernel;
use ceteris_trade::network::{Leg, Network, Wedges};
use ceteris_trade::{Sourced, TradeParams, dispatch, source};

const TRADED: [bool; N_GOODS] = {
    let mut t = [false; N_GOODS];
    let mut g = 0;
    while g < N_TRADED {
        t[g] = true;
        g += 1;
    }
    t
};

/// Ticks of every pack run, as the 28 October benchmark.
const TICKS: u32 = 1_200;

/// The EU's 27 members after Brexit.
const EU27: [&str; 27] = [
    "AUT", "BEL", "BGR", "HRV", "CYP", "CZE", "DNK", "EST", "FIN", "FRA", "DEU", "GRC", "HUN",
    "IRL", "ITA", "LVA", "LTU", "LUX", "MLT", "NLD", "POL", "PRT", "ROU", "SVK", "SVN", "ESP",
    "SWE",
];

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/benchmark")
        .join(name)
}

/// Every factor of the kernel's legs, as bits, good by good.
fn kernel_bits(k: &Kernel) -> Vec<u64> {
    let mut out = Vec::new();
    for g in (0..N_GOODS).filter(|&g| TRADED[g]) {
        let gk = k.good(g);
        for e in gk.domestic.entries.iter().chain(&gk.access.entries) {
            out.extend([u64::from(e.col), e.k.to_bits(), e.inv_tau.to_bits()]);
        }
        for e in &gk.crossings.entries {
            out.extend([
                u64::from(e.col),
                e.k.to_bits(),
                e.inv_duty.to_bits(),
                e.inv_loss.to_bits(),
                u64::from(e.lag),
            ]);
        }
    }
    out
}

fn shocked_kernel(shocks: &[Shock]) -> Kernel {
    let mut inputs = small_world();
    let trade = TradeParams::default();
    shock::apply(&mut inputs, shocks, &trade);
    let trade = shock::trade_params(&inputs, trade);
    Kernel::build(&inputs.network, &inputs.weight, &inputs.wedges, &trade)
}

#[test]
fn empty_shock_reproduces_the_default_kernel() {
    let inputs = small_world();
    assert!(inputs.network.bulk.is_some(), "the fixture has bulk legs");
    let default = Kernel::build(
        &inputs.network,
        &inputs.weight,
        &inputs.wedges,
        &TradeParams::default(),
    );
    assert_eq!(kernel_bits(&shocked_kernel(&[])), kernel_bits(&default));
    // an iceberg shock of 1 folds the bulk legs into route taus and changes nothing
    let one = Shock::Border {
        factor: 1.0,
        good: None,
    };
    assert_eq!(kernel_bits(&shocked_kernel(&[one])), kernel_bits(&default));
}

/// A border shock of one good scales that good's crossings and leaves the rest bit for bit.
#[test]
fn border_shock_scales_only_its_good() {
    let base = shocked_kernel(&[]);
    let shocked = shocked_kernel(&[Shock::Border {
        factor: 1.1,
        good: Some(3),
    }]);
    let theta = TradeParams::default().theta;
    for g in (0..N_GOODS).filter(|&g| TRADED[g]) {
        let (a, b) = (base.good(g), shocked.good(g));
        assert_eq!(a.domestic.entries, b.domestic.entries, "good {g}");
        assert_eq!(a.access.entries, b.access.entries, "good {g}");
        for (x, y) in a.crossings.entries.iter().zip(&b.crossings.entries) {
            if g == 3 {
                let ratio = y.k / x.k;
                let want = 1.1f64.powf(-theta[g]);
                assert!((ratio / want - 1.0).abs() < 1e-12, "{ratio} vs {want}");
                assert!((x.inv_loss / y.inv_loss / 1.1 - 1.0).abs() < 1e-15);
            } else {
                assert_eq!(x, y, "good {g}");
            }
        }
    }
}

#[test]
fn shock_files_name_countries_by_code() {
    let codes: Vec<String> = ["GBR", "FRA", "JPN"].map(String::from).to_vec();
    let text = "# comment\ntariff FRA GBR -1 0.05\nntb GBR FRA 1.0277 # soft\n\
                border 1.1\nborder 10 4\nprod JPN 3 1.5\n";
    assert_eq!(
        shock::parse(text, &codes, N_GOODS).unwrap(),
        vec![
            Shock::Tariff {
                importer: 1,
                exporter: 0,
                group: None,
                add: 0.05
            },
            Shock::Ntb {
                exporter: 0,
                importer: 1,
                factor: 1.0277
            },
            Shock::Border {
                factor: 1.1,
                good: None
            },
            Shock::Border {
                factor: 10.0,
                good: Some(4)
            },
            Shock::Prod {
                country: 2,
                good: 3,
                factor: 1.5
            },
        ]
    );
    let err = shock::parse("ntb GBR XXX 1.1", &codes, N_GOODS).unwrap_err();
    assert!(err.contains("XXX"), "{err}");
    for bad in [
        "ntb GBR FRA",
        "ntb GBR FRA -1",
        "border 1.1 10",
        "tariff FRA GBR 2 0.1",
        "quota GBR 1",
    ] {
        assert!(shock::parse(bad, &codes, N_GOODS).is_err(), "{bad}");
    }
    for name in [
        "border-1.1.shocks",
        "border-10.shocks",
        "brexit-soft.shocks",
        "brexit-hard.shocks",
    ] {
        let text = std::fs::read_to_string(data(name)).unwrap();
        let mut codes: Vec<String> = EU27.map(String::from).to_vec();
        codes.push("GBR".into());
        assert!(
            !shock::parse(&text, &codes, N_GOODS).unwrap().is_empty(),
            "{name}"
        );
    }
}

/// ADR 0200's two-country toy (`ceteris_trade::source`): one province per country, prices
/// fixed, Gamma held, two traded goods with their own theta and Omega and one local good. Under
/// costlier borders each traded good's buyer price moves by exactly 1/theta d ln lambda, so
/// `benchmark::welfare` equals the hand-computed d ln I - Omega_L d ln q_L - sum Omega_g /
/// theta_g d ln lambda_g to 1e-12, and `benchmark::acr` the lambda term alone: a missing or
/// inverted income term, a wrong Omega weighting, or the local good in `acr` each break it.
#[test]
fn welfare_and_acr_match_the_hand_computed_toy() {
    const GA: usize = 4;
    const GB: usize = 5;
    const LOCAL: usize = 8;
    const OMEGA: [(usize, f64); 3] = [(GA, 0.3), (GB, 0.5), (LOCAL, 0.2)];
    const INCOME: f64 = 1.1;
    const LOCAL_PRICE: f64 = 1.2;
    let weight = [0.5, 0.8, 0.9, 0.3, 0.002, 0.004, 0.001, 0.0, 0.0, 0.0];
    let leg = |from, to, cost_per_t| Leg {
        from,
        to,
        cost_per_t,
        hours: 10.0,
    };
    let world = |border: f64| Network {
        province_country: vec![0, 1],
        gateway_country: vec![0, 1],
        gateway_mass: vec![1.0, 1.0],
        access: vec![leg(0, 0, 20.0), leg(1, 1, 30.0)],
        crossings: vec![leg(0, 1, 60.0 * border), leg(1, 0, 80.0 * border)],
        ..Network::default()
    };
    let mut theta = [5.0; N_GOODS];
    theta[GB] = 3.0;
    let trade = TradeParams {
        theta: theta.to_vec(),
        ..TradeParams::default().bulk_off()
    };
    let mut wedges = Wedges::none(2);
    wedges.tariff.fill(0.05);
    let price = [1.0, 0.8];
    let spend = [[0.0, 100.0], [0.0, 0.0]];
    let gamma = [1.17, 0.93];
    let at = |border: f64, income: f64, local_price: f64, local_home: f64| {
        let k = Kernel::build(&world(border), &weight, &wedges, &trade);
        let mut omega = [0.0; N_GOODS];
        let mut q = [1.0; N_GOODS];
        let mut home = [f64::NAN; N_GOODS];
        let mut absorption = [0.0; N_GOODS];
        for (g, o) in OMEGA {
            omega[g] = o;
        }
        for g in [GA, GB] {
            let mut out = Sourced::new();
            source(&k, g, &price, &[true; 2], &gamma, &spend, &mut out);
            dispatch(&k, g, &price, &[[1.0; 2]; 2], &mut out);
            q[g] = out.buyer_price[0];
            home[g] = out.home[0][1] * price[0] / spend[0][1];
            absorption[g] = spend[0][1];
        }
        q[LOCAL] = local_price;
        // a real outcome has NaN here; a finite, moving share shows a local good leaking into acr
        home[LOCAL] = local_home;
        Outcome {
            traded: N_TRADED,
            income: vec![income * price[0]],
            omega: Rows::from_rows(N_GOODS, [omega]),
            buyer_price: Rows::from_rows(N_GOODS, [q]),
            home_share: Rows::from_rows(N_GOODS, [home]),
            absorption: Rows::from_rows(N_GOODS, [absorption]),
            province_country: vec![0],
            province_income: vec![income * price[0]],
            province_buyer_price: Rows::from_rows(N_GOODS, [q]),
            province_home_share: Rows::from_rows(N_GOODS, [home]),
            max_dlnp_last: 0.0,
            ticks: 0,
        }
    };
    let base = at(1.0, 1.0, 1.0, 0.5);
    for g in [GA, GB] {
        assert!(
            base.home_share[0][g] > 0.05 && base.home_share[0][g] < 0.95,
            "{g}: {}",
            base.home_share[0][g]
        );
    }
    assert_ne!(base.home_share[0][GA], base.home_share[0][GB]);
    for border in [1.1, 2.0, 10.0] {
        let shocked = at(border, INCOME, LOCAL_PRICE, 0.4);
        let dln = |g: usize| (shocked.home_share[0][g] / base.home_share[0][g]).ln();
        let want_acr = -(0.3 / 5.0 * dln(GA) + 0.5 / 3.0 * dln(GB));
        let want_welfare = INCOME.ln() - 0.2 * LOCAL_PRICE.ln() + want_acr;
        let (w, a) = (
            benchmark::welfare(&base, &shocked)[0],
            benchmark::acr(&base, &shocked, &theta)[0],
        );
        assert!(
            (a - want_acr).abs() < 1e-15,
            "x{border}: acr {a} vs {want_acr}"
        );
        assert!(
            (w - want_welfare).abs() < 1e-12,
            "x{border}: welfare {w} vs {want_welfare}"
        );
        assert!(a < 0.0, "x{border}: {a}");
        // with no input-output linkages the input-output form is the plain one
        let io = benchmark::acr_io(
            &base,
            &shocked,
            &theta,
            &Rows::filled(1, N_GOODS * N_GOODS, 0.0),
        )[0];
        assert_eq!(io.to_bits(), a.to_bits(), "x{border}");
        // one province: the province forms are the country forms
        let io_p = benchmark::acr_io_province(
            &base,
            &shocked,
            &theta,
            &Rows::filled(1, N_GOODS * N_GOODS, 0.0),
        )[0];
        assert_eq!(io_p.to_bits(), io.to_bits(), "x{border}");
        let w_p = benchmark::welfare_province(&base, &shocked)[0];
        assert_eq!(w_p.to_bits(), w.to_bits(), "x{border}");
    }
    let f = benchmark::foreign_share(&base, &[GA]);
    assert!((f - (1.0 - base.home_share[0][GA])).abs() < 1e-15);
}

/// An outcome of one country, `omega` and `home_share` per good at the base and shocked runs.
fn outcomes(omega: &[(usize, f64)], home: &[(usize, f64, f64)]) -> (Outcome, Outcome) {
    let mut o = [0.0; N_GOODS];
    for &(g, w) in omega {
        o[g] = w;
    }
    let at = |shocked: bool| {
        let mut h = [f64::NAN; N_GOODS];
        for &(g, l0, l1) in home {
            h[g] = if shocked { l1 } else { l0 };
        }
        Outcome {
            traded: N_TRADED,
            income: vec![1.0],
            omega: Rows::from_rows(N_GOODS, [o]),
            buyer_price: Rows::filled(1, N_GOODS, 1.0),
            home_share: Rows::from_rows(N_GOODS, [h]),
            absorption: Rows::filled(1, N_GOODS, 1.0),
            province_country: vec![0],
            province_income: vec![1.0],
            province_buyer_price: Rows::filled(1, N_GOODS, 1.0),
            province_home_share: Rows::from_rows(N_GOODS, [h]),
            max_dlnp_last: 0.0,
            ticks: 0,
        }
    };
    (at(false), at(true))
}

/// Costinot and Rodriguez-Clare (2014, fn. 20 of NBER WP 18896): one good using itself as an
/// input with cost share a amplifies the ACR change by 1/(1 - a); a local good that buys a
/// traded input passes its change on with the input's cost share, though Omega of the traded
/// good is 0.
#[test]
fn input_output_acr_matches_its_closed_forms() {
    const G: usize = 4;
    const LOCAL: usize = 9;
    let theta = [5.0; N_GOODS];
    let (base, shocked) = outcomes(&[(G, 1.0)], &[(G, 0.6, 0.5)]);
    let plain = -(0.5f64 / 0.6).ln() / 5.0;
    assert!((benchmark::acr(&base, &shocked, &theta)[0] - plain).abs() < 1e-15);
    for share in [0.0, 0.4, 0.55, 0.9] {
        let mut a = [0.0; N_GOODS * N_GOODS];
        a[G * N_GOODS + G] = share;
        let io = benchmark::acr_io(
            &base,
            &shocked,
            &theta,
            &Rows::from_rows(N_GOODS * N_GOODS, [a]),
        )[0];
        let want = plain / (1.0 - share);
        assert!((io / want - 1.0).abs() < 1e-14, "a {share}: {io} vs {want}");
    }

    let (base, shocked) = outcomes(&[(LOCAL, 1.0)], &[(G, 0.6, 0.5)]);
    assert_eq!(benchmark::acr(&base, &shocked, &theta)[0], 0.0);
    let mut a = [0.0; N_GOODS * N_GOODS];
    a[G * N_GOODS + LOCAL] = 0.3;
    a[LOCAL * N_GOODS + LOCAL] = 0.2;
    a[G * N_GOODS + G] = 0.4;
    let io = benchmark::acr_io(
        &base,
        &shocked,
        &theta,
        &Rows::from_rows(N_GOODS * N_GOODS, [a]),
    )[0];
    // v_L = 1 / (1 - 0.2), v_G = 0.3 v_L / (1 - 0.4)
    let want = 0.3 / 0.8 / 0.6 * plain;
    assert!((io / want - 1.0).abs() < 1e-14, "{io} vs {want}");
}

/// A good bought at home in one run only makes the country's ACR NaN in both forms; one bought
/// at home in neither adds nothing.
#[test]
fn acr_is_nan_where_a_home_share_vanishes_in_one_run() {
    let theta = [5.0; N_GOODS];
    let a = [[0.0; N_GOODS * N_GOODS]];
    for (l0, l1) in [(0.5, 0.0), (0.0, 0.5), (f64::NAN, 0.5), (0.5, f64::NAN)] {
        let (base, shocked) = outcomes(&[(2, 0.5), (3, 0.5)], &[(2, 0.6, 0.5), (3, l0, l1)]);
        assert!(
            benchmark::acr(&base, &shocked, &theta)[0].is_nan(),
            "{l0} {l1}"
        );
        assert!(
            benchmark::acr_io(
                &base,
                &shocked,
                &theta,
                &Rows::from_rows(N_GOODS * N_GOODS, a)
            )[0]
            .is_nan(),
            "{l0} {l1}"
        );
    }
    let (base, shocked) = outcomes(&[(2, 0.5), (3, 0.5)], &[(2, 0.6, 0.5), (3, 0.0, f64::NAN)]);
    let want = -0.5 / 5.0 * (0.5f64 / 0.6).ln();
    assert!((benchmark::acr(&base, &shocked, &theta)[0] - want).abs() < 1e-15);
    assert!(
        (benchmark::acr_io(
            &base,
            &shocked,
            &theta,
            &Rows::from_rows(N_GOODS * N_GOODS, a)
        )[0] - want)
            .abs()
            < 1e-15
    );
}

/// One country of provinces with base and shocked `(income, buyer price, home share)` each,
/// every good at the same buyer price and home share; the country rows are the province sums
/// for income and population-weighted means (equal populations) for the buyer price.
fn provinces(omega: &[(usize, f64)], rows: &[[(f64, f64, f64); 2]]) -> (Outcome, Outcome) {
    let mut o = [0.0; N_GOODS];
    for &(g, w) in omega {
        o[g] = w;
    }
    let at = |run: usize| {
        let n = rows.len() as f64;
        let income: Vec<f64> = rows.iter().map(|r| r[run].0).collect();
        let q = Rows::from_rows(N_GOODS, rows.iter().map(|r| [r[run].1; N_GOODS]));
        let home =
            Rows::from_rows(
                N_GOODS,
                rows.iter().map(|r| {
                    std::array::from_fn::<f64, N_GOODS, _>(|g| {
                        if TRADED[g] { r[run].2 } else { f64::NAN }
                    })
                }),
            );
        let mean = rows.iter().map(|r| r[run].1).sum::<f64>() / n;
        Outcome {
            traded: N_TRADED,
            income: vec![income.iter().sum()],
            omega: Rows::from_rows(N_GOODS, [o]),
            buyer_price: Rows::filled(1, N_GOODS, mean),
            home_share: Rows::filled(1, N_GOODS, f64::NAN),
            absorption: Rows::filled(1, N_GOODS, 1.0),
            province_country: vec![0; rows.len()],
            province_income: income,
            province_buyer_price: q,
            province_home_share: home,
            max_dlnp_last: 0.0,
            ticks: 0,
        }
    };
    (at(0), at(1))
}

/// ADR 0268, point 1 and 2: each province's input-output ACR and real income change, on its
/// own home share and buyer prices, weighted by base income share; a province with no base
/// income adds nothing.
#[test]
fn province_forms_weight_provinces_by_base_income() {
    const G: usize = 4;
    const LOCAL: usize = 9;
    let theta = [5.0; N_GOODS];
    let mut a = [0.0; N_GOODS * N_GOODS];
    a[G * N_GOODS + G] = 0.4;
    let a = Rows::from_rows(N_GOODS * N_GOODS, [a]);
    let omega = [(G, 0.6), (LOCAL, 0.4)];
    let (base, shocked) = provinces(
        &omega,
        &[
            [(3.0, 1.0, 0.8), (3.3, 1.02, 0.7)],
            [(1.0, 1.0, 0.5), (0.9, 1.05, 0.45)],
            [(0.0, 1.0, 0.9), (0.0, 1.0, 0.1)],
        ],
    );
    let dw = |l0: f64, l1: f64| -0.6 / 0.6 / 5.0 * (l1 / l0).ln();
    let want = 0.75 * dw(0.8, 0.7) + 0.25 * dw(0.5, 0.45);
    let got = benchmark::acr_io_province(&base, &shocked, &theta, &a)[0];
    assert!((got / want - 1.0).abs() < 1e-14, "{got} vs {want}");

    let real = |i0: f64, i1: f64, q0: f64, q1: f64| (i1 / i0).ln() - (q1 / q0).ln();
    let want = 0.75 * real(3.0, 3.3, 1.0, 1.02) + 0.25 * real(1.0, 0.9, 1.0, 1.05);
    let got = benchmark::welfare_province(&base, &shocked)[0];
    assert!((got / want - 1.0).abs() < 1e-14, "{got} vs {want}");

    // a province with no absorption of G in either run adds nothing for G, its weight kept
    let (base, shocked) = provinces(
        &omega,
        &[
            [(3.0, 1.0, 0.8), (3.3, 1.02, 0.7)],
            [(1.0, 1.0, f64::NAN), (0.9, 1.05, f64::NAN)],
        ],
    );
    let want = 0.75 * dw(0.8, 0.7);
    let got = benchmark::acr_io_province(&base, &shocked, &theta, &a)[0];
    assert!((got / want - 1.0).abs() < 1e-14, "{got} vs {want}");

    // a province that buys at home in one run only makes the country NaN
    let (base, shocked) = provinces(
        &omega,
        &[
            [(3.0, 1.0, 0.8), (3.3, 1.02, 0.7)],
            [(1.0, 1.0, 0.5), (0.9, 1.05, 0.0)],
        ],
    );
    assert!(benchmark::acr_io_province(&base, &shocked, &theta, &a)[0].is_nan());
}

/// Provinces reduce to their own country with that country's income: two countries, the
/// second with one province between two of the first.
#[test]
fn province_forms_reduce_to_each_province_country() {
    const G: usize = 4;
    let theta = [5.0; N_GOODS];
    let a = Rows::filled(2, N_GOODS * N_GOODS, 0.0);
    let rows = [
        [(3.0, 1.0, 0.8), (3.3, 1.02, 0.7)],
        [(2.0, 1.0, 0.6), (2.2, 1.01, 0.5)],
        [(1.0, 1.0, 0.5), (0.9, 1.05, 0.45)],
    ];
    let split = |o: Outcome| {
        let y = &o.province_income;
        let two = |r: &Rows<f64>| Rows::from_rows(N_GOODS, [0, 1].map(|_| r[0].to_vec()));
        Outcome {
            income: vec![y[0] + y[2], y[1]],
            omega: two(&o.omega),
            buyer_price: two(&o.buyer_price),
            home_share: two(&o.home_share),
            absorption: two(&o.absorption),
            province_country: vec![0, 1, 0],
            ..o
        }
    };
    let (base, shocked) = provinces(&[(G, 1.0)], &rows);
    let (base, shocked) = (split(base), split(shocked));
    let dw = |l0: f64, l1: f64| -(l1 / l0).ln() / 5.0;
    let got = benchmark::acr_io_province(&base, &shocked, &theta, &a);
    let want = [0.75 * dw(0.8, 0.7) + 0.25 * dw(0.5, 0.45), dw(0.6, 0.5)];
    for c in 0..2 {
        assert!(
            (got[c] / want[c] - 1.0).abs() < 1e-14,
            "{c}: {got:?} vs {want:?}"
        );
    }
    let real = |i0: f64, i1: f64, q1: f64| (i1 / i0).ln() - q1.ln();
    let got = benchmark::welfare_province(&base, &shocked);
    let want = [
        0.75 * real(3.0, 3.3, 1.02) + 0.25 * real(1.0, 0.9, 1.05),
        real(2.0, 2.2, 1.01),
    ];
    for c in 0..2 {
        assert!(
            (got[c] / want[c] - 1.0).abs() < 1e-14,
            "{c}: {got:?} vs {want:?}"
        );
    }
}

/// ADR 0268, point 2: the income-weighted welfare less the population-weighted one is, to first
/// order, -sum_g Omega_g sum_p (s_p - pi_p) d ln q_pg, the covariance of income per head with
/// price changes inside the country; the remainder is second order in d ln q.
#[test]
fn income_weights_differ_from_population_weights_by_the_price_covariance() {
    const G: usize = 4;
    let omega = [(G, 1.0)];
    let d: [f64; 2] = [1e-3, -2e-3];
    let (base, shocked) = provinces(
        &omega,
        &[
            [(3.0, 1.0, 0.8), (3.0 * 1.001, d[0].exp(), 0.8)],
            [(1.0, 1.0, 0.5), (1.0 * 1.001, d[1].exp(), 0.5)],
        ],
    );
    let gap =
        benchmark::welfare_province(&base, &shocked)[0] - benchmark::welfare(&base, &shocked)[0];
    let want = -((0.75 - 0.5) * d[0] + (0.25 - 0.5) * d[1]);
    assert!((gap - want).abs() < d[1] * d[1], "{gap} vs {want}");
}

/// `benchmark::outcome` on the small world run frictionless: lambda in [0, 1] for the traded
/// goods with absorption and NaN for the local ones; absorption the importer's sum of the flow
/// window's pair values and home purchases; the foreign share the exporter-not-importer pair
/// values over absorption.
#[test]
fn outcome_aggregates_the_flow_window() {
    const TICKS: u32 = 15;
    let world = EconWorld::new(
        Arc::new(small_world()),
        EconParams::frictionless(),
        TradeParams::default(),
    );
    let mut sim = Sim::new(world, schedule(), SEED);
    sim.run_until(TICKS).unwrap();
    let w = sim.world();
    let o = benchmark::outcome(w, 0.0, sim.tick());
    let inp = w.inputs();
    let (nc, np) = (inp.n_countries(), inp.n_provinces());
    let mean = w.scratch.flows.mean();
    let traded: Vec<usize> = (0..N_GOODS).filter(|&g| TRADED[g]).collect();
    let (mut abroad, mut total, mut with_absorption) = (0.0, 0.0, 0);
    for c in 0..nc {
        for (g, &is_traded) in TRADED.iter().enumerate() {
            let l = o.home_share[c][g];
            if !is_traded {
                assert!(l.is_nan(), "country {c} good {g}: {l}");
                assert_eq!(o.absorption[c][g], 0.0);
                continue;
            }
            let pairs: f64 = (0..nc)
                .map(|ex| mean.pair_value[(ex * nc + c) * N_GOODS + g])
                .sum();
            let home: f64 = (0..np)
                .filter(|&i| inp.country_of(i) == c)
                .map(|i| mean.home_value[i * N_GOODS + g])
                .sum();
            let want = pairs + home;
            assert!(
                (o.absorption[c][g] - want).abs() <= 1e-12 * want,
                "country {c} good {g}: {} vs {want}",
                o.absorption[c][g]
            );
            if want > 0.0 {
                with_absorption += 1;
                assert!((0.0..=1.0).contains(&l), "country {c} good {g}: {l}");
                abroad += pairs - mean.pair_value[(c * nc + c) * N_GOODS + g];
                total += want;
            } else {
                assert!(l.is_nan(), "country {c} good {g}: {l}");
            }
        }
    }
    assert!(
        with_absorption > nc,
        "{with_absorption} cells with absorption"
    );
    // provinces: income adds up to the country's, lambda is home purchases over absorption
    for c in 0..nc {
        let sum: f64 = (0..np)
            .filter(|&i| o.province_country[i] == c)
            .map(|i| o.province_income[i])
            .sum();
        assert!((sum - o.income[c]).abs() <= 1e-12 * o.income[c], "{c}");
    }
    let mut with_home = 0;
    for i in 0..np {
        assert_eq!(o.province_country[i], inp.country_of(i));
        for (g, &traded) in TRADED.iter().enumerate() {
            let l = o.province_home_share[i][g];
            let a = mean.absorption[i * N_GOODS + g];
            if traded && a > 0.0 {
                assert_eq!(l, mean.home_value[i * N_GOODS + g] / a, "{i} {g}");
                assert!((0.0..=1.0).contains(&l), "province {i} good {g}: {l}");
                with_home += usize::from(l < 1.0);
            } else {
                assert!(l.is_nan(), "province {i} good {g}: {l}");
            }
            assert_eq!(
                o.province_buyer_price[i][g].to_bits(),
                w.price_index[i * N_GOODS + g].to_bits()
            );
        }
    }
    assert!(
        with_home > np,
        "{with_home} province cells buying elsewhere"
    );
    let f = benchmark::foreign_share(&o, &traded);
    assert!(f > 0.0 && f < 1.0, "{f}");
    assert!(
        (f - abroad / total).abs() < 1e-12,
        "{f} vs {}",
        abroad / total
    );
}

fn pack() -> Option<PathBuf> {
    let p = std::env::var_os("CETERIS_PACK").map(PathBuf::from);
    if p.is_none() {
        eprintln!("skipping: CETERIS_PACK unset (a calibrated pack)");
    }
    p
}

fn codes(pack: &Path) -> &'static [String] {
    static CODES: OnceLock<Vec<String>> = OnceLock::new();
    CODES.get_or_init(|| benchmark::codes(pack).unwrap())
}

fn base(pack: &Path) -> &'static Outcome {
    static BASE: OnceLock<Outcome> = OnceLock::new();
    BASE.get_or_init(|| benchmark::run(pack, &[], TICKS).unwrap())
}

fn run(pack: &Path, file: &str) -> Result<Outcome, String> {
    let text = std::fs::read_to_string(data(file)).unwrap();
    benchmark::run(
        pack,
        &shock::parse(&text, codes(pack), benchmark::n_goods(pack)?)?,
        TICKS,
    )
}

fn country(pack: &Path, code: &str) -> usize {
    codes(pack).iter().position(|c| c == code).unwrap()
}

/// Base-income-weighted mean of `w` over the EU27 members in the pack.
fn eu27(pack: &Path, base: &Outcome, w: &[f64]) -> f64 {
    let members: Vec<usize> = EU27
        .iter()
        .filter_map(|m| codes(pack).iter().position(|c| c == m))
        .collect();
    let total: f64 = members.iter().map(|&c| base.income[c]).sum();
    members.iter().map(|&c| base.income[c] * w[c]).sum::<f64>() / total
}

#[test]
#[ignore = "pack-gated solver run; nightly job"]
fn japan_loses_from_costlier_borders() {
    let Some(pack) = pack() else { return };
    let base = base(&pack);
    let shocked = run(&pack, "border-1.1.shocks").unwrap();
    let w = benchmark::welfare(base, &shocked);
    let jpn = w[country(&pack, "JPN")];
    eprintln!(
        "border x1.1: JPN {jpn:e}, GBR {:e}, EU27 {:e}; base max |dlnp| {:e}, shocked {:e}",
        w[country(&pack, "GBR")],
        eu27(&pack, base, &w),
        base.max_dlnp_last,
        shocked.max_dlnp_last
    );
    assert!(jpn < 0.0, "JPN welfare {jpn}");
}

#[test]
#[ignore = "pack-gated solver run; nightly job"]
fn eu_loses_from_soft_brexit() {
    let Some(pack) = pack() else { return };
    let base = base(&pack);
    let shocked = run(&pack, "brexit-soft.shocks").unwrap();
    let w = benchmark::welfare(base, &shocked);
    let (gbr, eu) = (w[country(&pack, "GBR")], eu27(&pack, base, &w));
    eprintln!(
        "soft Brexit: GBR {gbr:e}, EU27 {eu:e}, JPN {:e}; shocked max |dlnp| {:e}",
        w[country(&pack, "JPN")],
        shocked.max_dlnp_last
    );
    assert!(eu <= 0.0, "EU27 welfare {eu}");
    assert!(gbr < 0.0, "GBR welfare {gbr}");
}

/// ICIO countries with |ACR| above 1e-4 and finite welfare and ACR, as (welfare, ACR) pairs.
fn graded(icio: &[bool], w: &[f64], a: &[f64]) -> Vec<(f64, f64)> {
    (0..w.len())
        .filter(|&c| icio[c] && w[c].is_finite() && a[c].is_finite() && a[c].abs() > 1e-4)
        .map(|c| (w[c], a[c]))
        .collect()
}

/// Countries of the wrong sign, the model / ACR ratio at quantiles p10, p50 and p90, and the
/// correlation of `pairs`.
fn acr_stats(pairs: &[(f64, f64)]) -> (usize, [f64; 3], f64) {
    let wrong = pairs.iter().filter(|(w, a)| w * a <= 0.0).count();
    let mut ratio: Vec<f64> = pairs.iter().map(|(w, a)| w / a).collect();
    ratio.sort_by(f64::total_cmp);
    let at = |q: f64| ratio[((ratio.len() - 1) as f64 * q).round() as usize];
    let n = pairs.len() as f64;
    let (mw, ma) = (
        pairs.iter().map(|p| p.0).sum::<f64>() / n,
        pairs.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let cov: f64 = pairs.iter().map(|(w, a)| (w - mw) * (a - ma)).sum();
    let (vw, va): (f64, f64) = (
        pairs.iter().map(|(w, _)| (w - mw).powi(2)).sum(),
        pairs.iter().map(|(_, a)| (a - ma).powi(2)).sum(),
    );
    (wrong, [at(0.1), at(0.5), at(0.9)], cov / (vw * va).sqrt())
}

/// The model's theta per good, the theta classes of the traded goods grouped, and the ICIO
/// foreign-share target of each group.
fn openness_groups(pack: &Path) -> (Vec<Vec<usize>>, Vec<f64>, Vec<bool>) {
    let world = benchmark::world(pack, &[]).unwrap();
    let class: Vec<usize> = world
        .trade()
        .theta_class
        .iter()
        .map(|&c| usize::from(c))
        .collect();
    let n_traded = (0..N_GOODS).filter(|&g| TRADED[g]).count();
    let groups = border::class_groups(&class[..n_traded]);
    let (target, icio) = border::target_shares(pack, &groups).unwrap();
    (groups, target, icio)
}

/// ADR 0207 criterion 1, frozen for C7 (specs/pre-p3/PLAN.md, ADR 0257): per theta class and for
/// all traded goods together, the model's absorption-weighted foreign share 1 - lambda is within
/// [1/1.5, 1.5] times ICIO 2000's. The class targets are what nu is fitted to (ADR 0092); the
/// all-goods one is not fitted and is the free check.
#[test]
fn openness_within_1_5_of_icio() {
    let Some(pack) = pack() else { return };
    let base = base(&pack);
    let (groups, target, _) = openness_groups(&pack);
    let traded: Vec<usize> = (0..N_GOODS).filter(|&g| TRADED[g]).collect();
    let (world_target, _) = border::target_shares(&pack, std::slice::from_ref(&traded)).unwrap();
    let mut rows: Vec<(String, f64, f64)> = groups
        .iter()
        .zip(&target)
        .map(|(goods, &t)| {
            (
                format!("class group {goods:?}"),
                benchmark::foreign_share(base, goods),
                t,
            )
        })
        .collect();
    rows.push((
        "all traded goods".into(),
        benchmark::foreign_share(base, &traded),
        world_target[0],
    ));
    let mut misses = Vec::new();
    for (name, model, icio) in &rows {
        let r = model / icio;
        eprintln!("openness {name}: model {model:.4}, ICIO {icio:.4}, ratio {r:.3}");
        if !(1.0 / 1.5..=1.5).contains(&r) {
            misses.push(name.clone());
        }
    }
    assert!(
        misses.is_empty(),
        "openness outside [1/1.5, 1.5] of ICIO: {misses:?}"
    );
}

/// ADR 0207 criterion 2 as ADR 0268 grades it (frozen for C7): under every crossing tau x 1.1,
/// the model's province-weighted welfare against the province-graded input-output ACR over the
/// ICIO countries with |ACR| above 1e-4: every sign agrees, the median ratio is in [0.8, 1.25]
/// and the correlation is at least 0.9.
#[test]
fn acr_formula_holds_under_costlier_borders() {
    let Some(pack) = pack() else { return };
    let base = base(&pack);
    let (_, _, icio) = openness_groups(&pack);
    let world = benchmark::world(&pack, &[]).unwrap();
    let theta: [f64; N_GOODS] = std::array::from_fn(|g| world.kernel().theta(g));
    let io = &world.inputs().countries.io;
    let shocked = run(&pack, "border-1.1.shocks").unwrap();
    let pairs = graded(
        &icio,
        &benchmark::welfare_province(base, &shocked),
        &benchmark::acr_io_province(base, &shocked, &theta, io),
    );
    let (wrong, [p10, median, p90], corr) = acr_stats(&pairs);
    eprintln!(
        "ACR x1.1 province: {} countries, {wrong} of the wrong sign, median {median:.3} \
         (p10 {p10:.3}, p90 {p90:.3}), correlation {corr:.3}",
        pairs.len()
    );
    assert_eq!(wrong, 0, "countries of the wrong sign");
    assert!((0.8..=1.25).contains(&median), "median {median}");
    assert!(corr >= 0.9, "correlation {corr}");
}

/// ADR 0207's three criteria and the hard Brexit case on the pack, printed for the record (C7
/// asserts them): openness per theta class against ICIO 2000, ACR under border costs x1.1 in
/// three forms (ADR 0268 grades the province one against the province-weighted welfare; the
/// country forms of ADR 0265 and ACR 2012 against the country welfare) with each country's gap
/// between the input-output country and province forms, and convergence under x10.
#[test]
#[ignore = "report only; about 4 minutes on the pack in release"]
fn benchmark_cases_are_reported() {
    let Some(pack) = pack() else { return };
    let base = base(&pack);
    // theta, classes and A of the run itself
    let world = benchmark::world(&pack, &[]).unwrap();
    let theta: [f64; N_GOODS] = std::array::from_fn(|g| world.kernel().theta(g));
    let io = &world.inputs().countries.io;
    let class: Vec<usize> = world
        .trade()
        .theta_class
        .iter()
        .map(|&c| usize::from(c))
        .collect();
    let n_traded = (0..N_GOODS).filter(|&g| TRADED[g]).count();
    let groups = border::class_groups(&class[..n_traded]);
    let (target, icio) = border::target_shares(&pack, &groups).unwrap();
    for (goods, t) in groups.iter().zip(&target) {
        let c = class[goods[0]];
        let model = benchmark::foreign_share(base, goods);
        eprintln!(
            "openness class {c}: model {model:.4}, ICIO {t:.4}, ratio {:.3}",
            model / t
        );
    }
    let traded: Vec<usize> = (0..N_GOODS).filter(|&g| TRADED[g]).collect();
    eprintln!(
        "openness traded: model {:.4}",
        benchmark::foreign_share(base, &traded)
    );
    eprintln!(
        "base max |dlnp| over the last {} ticks {:e}",
        benchmark::LAST,
        base.max_dlnp_last
    );

    let shocked = run(&pack, "border-1.1.shocks").unwrap();
    let w = benchmark::welfare(base, &shocked);
    let w_province = benchmark::welfare_province(base, &shocked);
    let io_country = benchmark::acr_io(base, &shocked, &theta, io);
    let io_province = benchmark::acr_io_province(base, &shocked, &theta, io);
    eprintln!("ACR x1.1: max |dlnp| {:e}", shocked.max_dlnp_last);
    for (name, w, a) in [
        ("plain", &w, benchmark::acr(base, &shocked, &theta)),
        ("input-output", &w, io_country.clone()),
        ("province", &w_province, io_province.clone()),
    ] {
        // a province with no buyer price for a good it demands makes its country's welfare NaN
        let nan = (0..w.len())
            .filter(|&c| icio[c] && (a[c].is_nan() || w[c].is_nan()))
            .count();
        let pairs = graded(&icio, w, &a);
        let (wrong, [p10, median, p90], corr) = acr_stats(&pairs);
        if name == "province" {
            // ADR 0268's criterion, asserted by C7 at B8 on a settled baseline, reported here
            eprintln!(
                "ACR x1.1 criterion (ADR 0268): signs {}, median in [0.8, 1.25] {}, \
                 correlation >= 0.9 {}",
                wrong == 0,
                (0.8..=1.25).contains(&median),
                corr >= 0.9
            );
        }
        eprintln!(
            "ACR x1.1 {name}: {} ICIO countries with |ACR| > 1e-4 ({nan} NaN in either), {wrong} of the \
             wrong sign, model/ACR median {:.3}, p10 {:.3}, p90 {:.3}, correlation {:.3}",
            pairs.len(),
            median,
            p10,
            p90,
            corr,
        );
    }
    eprintln!("ACR x1.1 per country: welfare, province welfare, input-output, province, gap");
    for c in (0..w.len()).filter(|&c| icio[c]) {
        eprintln!(
            "  {} {:+.3e} {:+.3e} {:+.3e} {:+.3e} {:+.3e}",
            codes(&pack)[c],
            w[c],
            w_province[c],
            io_country[c],
            io_province[c],
            io_country[c] - io_province[c]
        );
    }

    let hard = run(&pack, "brexit-hard.shocks").unwrap();
    let w = benchmark::welfare(base, &hard);
    eprintln!(
        "hard Brexit: GBR {:e}, EU27 {:e}, IRL {:e}; max |dlnp| {:e}",
        w[country(&pack, "GBR")],
        eu27(&pack, base, &w),
        w[country(&pack, "IRL")],
        hard.max_dlnp_last
    );

    // the run may stop on a panic in the model rather than an invariant violation
    let x10 = std::panic::catch_unwind(|| run(&pack, "border-10.shocks"));
    match x10 {
        Err(_) => eprintln!("border x10: the run panicked (above)"),
        Ok(Ok(o)) => eprintln!(
            "border x10: {} ticks, max |dlnp| over the last {} {:e}",
            o.ticks,
            benchmark::LAST,
            o.max_dlnp_last
        ),
        Ok(Err(e)) => eprintln!("border x10: {e}"),
    }
}

/// The US-China 2018 tariff case (`us-china-2018.shocks`) on the pack, printed for the record:
/// the literature reads near-complete pass-through of the tariffs to US buyer prices (Amiti,
/// Redding and Weinstein 2019; Fajgelbaum et al. 2020) and a US welfare loss of about 0.04% of
/// GDP. Pass-through is proxied by 1 + d ln q_CHN / d ln (1 + t), q_CHN the Chinese
/// population-weighted buyer price of the traded goods the tariff covers: the Chinese price is
/// the origin price of the goods the US buys, up to China's own tariff-free local price index.
#[test]
#[ignore = "report only; about 2 minutes on the pack in release"]
fn us_china_2018_is_reported() {
    let Some(pack) = pack() else { return };
    let base = base(&pack);
    let shocked = run(&pack, "us-china-2018.shocks").unwrap();
    let w = benchmark::welfare(base, &shocked);
    let (usa, chn) = (country(&pack, "USA"), country(&pack, "CHN"));
    let traded: Vec<usize> = (0..N_GOODS).filter(|&g| TRADED[g]).collect();
    let mean_dlnq = |c: usize| {
        let (mut num, mut den) = (0.0, 0.0);
        for &g in &traded {
            let (a, b) = (base.buyer_price[c][g], shocked.buyer_price[c][g]);
            let wt = base.absorption[c][g];
            if a > 0.0 && b > 0.0 && wt > 0.0 {
                num += wt * (b / a).ln();
                den += wt;
            }
        }
        num / den
    };
    let (d_chn, d_usa) = (mean_dlnq(chn), mean_dlnq(usa));
    eprintln!(
        "US-China 2018: USA welfare {:e}, CHN welfare {:e}; traded buyer price d ln q USA {d_usa:e}, \
         CHN {d_chn:e}; pass-through proxy {:.3}; max |dlnp| {:e}",
        w[usa],
        w[chn],
        1.0 + d_chn / 1.12_f64.ln(),
        shocked.max_dlnp_last
    );
}
