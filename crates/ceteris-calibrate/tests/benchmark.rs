//! The benchmark harness (ADR 0200, ADR 0207): shocks reach the kernel as written, welfare and
//! ACR are exact on the two-country toy and, with `CETERIS_PACK` set to a calibrated pack,
//! ADR 0200's benchmark cases (Japan under costlier borders, the EU under Brexit) have the
//! literature's sign. Run the pack cases with `--release`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ceteris_calibrate::benchmark::{self, Outcome};
use ceteris_calibrate::border;
use ceteris_calibrate::shock::{self, Shock};
use ceteris_econ::fixture::small_world;
use ceteris_trade::goods::{N_GOODS, TRADED};
use ceteris_trade::kernel::Kernel;
use ceteris_trade::network::{Leg, Network, Wedges};
use ceteris_trade::{Sourced, TradeParams, dispatch, source};

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
        shock::parse(text, &codes).unwrap(),
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
    let err = shock::parse("ntb GBR XXX 1.1", &codes).unwrap_err();
    assert!(err.contains("XXX"), "{err}");
    for bad in [
        "ntb GBR FRA",
        "ntb GBR FRA -1",
        "border 1.1 10",
        "tariff FRA GBR 2 0.1",
        "quota GBR 1",
    ] {
        assert!(shock::parse(bad, &codes).is_err(), "{bad}");
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
        assert!(!shock::parse(&text, &codes).unwrap().is_empty(), "{name}");
    }
}

/// ADR 0200's two-country toy (`ceteris_trade::source`): one province per country, prices
/// fixed, Gamma held: under costlier borders `benchmark::welfare` equals `benchmark::acr` to
/// 1e-12.
#[test]
fn acr_is_exact_on_the_two_country_toy() {
    const THETA: f64 = 5.0;
    const G: usize = 4;
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
    let trade = TradeParams {
        theta: [THETA; N_GOODS],
        ..TradeParams::default().bulk_off()
    };
    let mut wedges = Wedges::none(2);
    wedges.tariff.fill(0.05);
    let price = [1.0, 0.8];
    let spend = [[0.0, 100.0], [0.0, 0.0]];
    let gamma = [1.17, 0.93];
    let at = |border: f64| {
        let k = Kernel::build(&world(border), &weight, &wedges, &trade);
        let mut out = Sourced::new();
        source(&k, G, &price, &[true; 2], &gamma, &spend, &mut out);
        dispatch(&k, G, &price, &[[1.0; 2]; 2], &mut out);
        let lambda = out.home[0][1] * price[0] / spend[0][1];
        let mut omega = [0.0; N_GOODS];
        omega[G] = 1.0;
        let mut q = [1.0; N_GOODS];
        q[G] = out.buyer_price[0];
        let mut home = [f64::NAN; N_GOODS];
        home[G] = lambda;
        let mut absorption = [0.0; N_GOODS];
        absorption[G] = spend[0][1];
        Outcome {
            income: vec![price[0]],
            omega: vec![omega],
            buyer_price: vec![q],
            home_share: vec![home],
            absorption: vec![absorption],
            max_dlnp_last: 0.0,
            ticks: 0,
        }
    };
    let base = at(1.0);
    assert!(base.home_share[0][G] > 0.05 && base.home_share[0][G] < 0.95);
    for border in [1.1, 2.0, 10.0] {
        let shocked = at(border);
        let (w, a) = (
            benchmark::welfare(&base, &shocked)[0],
            benchmark::acr(&base, &shocked, &[THETA; N_GOODS])[0],
        );
        assert!((w - a).abs() < 1e-12, "x{border}: {w} vs {a}");
        assert!(w < 0.0, "x{border}: {w}");
    }
    let f = benchmark::foreign_share(&base, &[G]);
    assert!((f - (1.0 - base.home_share[0][G])).abs() < 1e-15);
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
    benchmark::run(pack, &shock::parse(&text, codes(pack))?, TICKS)
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

/// ADR 0207's three criteria and the hard Brexit case on the pack, printed for the record (C7
/// asserts them): openness per theta class against ICIO 2000, ACR under border costs x1.1, and
/// convergence under x10.
#[test]
#[ignore = "report only; about 4 minutes on the pack in release"]
fn benchmark_cases_are_reported() {
    let Some(pack) = pack() else { return };
    let base = base(&pack);
    let trade = TradeParams::default();
    let class: Vec<usize> = trade.theta_class.iter().map(|&c| usize::from(c)).collect();
    let (target, icio) = border::target_shares(&pack, &class).unwrap();
    for (c, t) in target.iter().enumerate() {
        let goods: Vec<usize> = (0..N_GOODS)
            .filter(|&g| TRADED[g] && class[g] == c)
            .collect();
        let model = benchmark::foreign_share(base, &goods);
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
    let theta = trade.theta.map(|t| t * trade.theta_scale);
    let (w, a) = (
        benchmark::welfare(base, &shocked),
        benchmark::acr(base, &shocked, &theta),
    );
    let pairs: Vec<(f64, f64)> = (0..w.len())
        .filter(|&c| icio[c] && a[c].abs() > 1e-4)
        .map(|c| (w[c], a[c]))
        .collect();
    let wrong = pairs.iter().filter(|(w, a)| w * a <= 0.0).count();
    let mut ratio: Vec<f64> = pairs.iter().map(|(w, a)| w / a).collect();
    ratio.sort_by(f64::total_cmp);
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
    eprintln!(
        "ACR x1.1: {} ICIO countries with |ACR| > 1e-4, {wrong} of the wrong sign, median ratio \
         {:.3}, correlation {:.3}; max |dlnp| {:e}",
        pairs.len(),
        ratio[ratio.len() / 2],
        cov / (vw * va).sqrt(),
        shocked.max_dlnp_last
    );

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
