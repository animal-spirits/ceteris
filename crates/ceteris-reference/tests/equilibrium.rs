//! The reference model and its solve on the fixture export and on closed-form worlds.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt64Type};
use arrow_cast::cast;
use arrow_ipc::reader::FileReader;
use arrow_schema::DataType;
use proptest::prelude::*;

use ceteris_reference::demand::Pigl;
use ceteris_reference::io::{self, Crossings, Inputs, Legs, Mat};
use ceteris_reference::model::{self, Model};
use ceteris_reference::ppml;
use ceteris_reference::sinkhorn;
use ceteris_reference::solve::{self, Equilibrium, Limits, SolveError, TOL};

fn fixture() -> PathBuf {
    std::env::var_os("CETERIS_FIXTURE_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture"),
        PathBuf::from,
    )
}

fn inputs() -> &'static Inputs {
    static INPUTS: OnceLock<Inputs> = OnceLock::new();
    INPUTS.get_or_init(|| io::load(fixture()).unwrap())
}

fn solved() -> &'static Equilibrium {
    static EQ: OnceLock<Equilibrium> = OnceLock::new();
    EQ.get_or_init(|| {
        let m = Model::new(inputs()).unwrap();
        solve::equilibrium(&m, None, TOL, None).unwrap()
    })
}

/// splitmix64: uniform draws without a dependency.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn normal(&mut self, sd: f64) -> f64 {
        let (u, v) = (self.uniform(f64::EPSILON, 1.0), self.uniform(0.0, 1.0));
        sd * (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// numpy's `assert_allclose`: |got - want| <= atol + rtol |want|, NaN and inf equal to themselves.
fn assert_close(got: &[f64], want: &[f64], rtol: f64, atol: f64, what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    for (i, (&g, &w)) in got.iter().zip(want).enumerate() {
        let ok = g == w || (g.is_nan() && w.is_nan()) || (g - w).abs() <= atol + rtol * w.abs();
        assert!(ok, "{what}[{i}]: {g} vs {w}");
    }
}

/// A (rows, cols) matrix of draws in row-major order, `f` given the column.
fn draw_mat(rows: usize, cols: usize, mut f: impl FnMut(usize) -> f64) -> Mat {
    Mat {
        rows,
        cols,
        data: (0..rows * cols).map(|i| f(i % cols)).collect(),
    }
}

fn column(m: &Mat, g: usize) -> Vec<f64> {
    (0..m.rows).map(|p| m[(p, g)]).collect()
}

fn traded_goods(inp: &Inputs) -> Vec<usize> {
    (0..inp.n_goods()).filter(|&g| inp.traded[g]).collect()
}

/// Origin weights (p / p_min)^-theta_g of the active origins, one p_min per good.
fn weights(inp: &Inputs, g: usize, p: &[f64], act: &[bool]) -> Vec<f64> {
    let p_min = (0..p.len())
        .filter(|&i| act[i])
        .map(|i| p[i])
        .fold(f64::INFINITY, f64::min);
    (0..p.len())
        .map(|i| {
            if act[i] {
                (p[i] / p_min).powf(-inp.theta[g])
            } else {
                0.0
            }
        })
        .collect()
}

fn active_column(inp: &Inputs, g: usize) -> Vec<bool> {
    inp.active.iter().map(|row| row[g]).collect()
}

#[test]
fn residual_zero_at_solution() {
    let inp = inputs();
    let m = Model::new(inp).unwrap();
    let eq = solved();
    assert!(model::max_abs(&model::residual(&eq.x, &m)) <= 1e-10);
    // Walras' law: the labour equation replaced by the price index holds too.
    assert!(model::max_abs(&model::excess(&eq.x, &m)) <= 1e-9);
    assert!((eq.price_index - 1.0).abs() <= 1e-10);
    for p in 0..inp.n_provinces() {
        for g in 0..inp.n_goods() {
            assert!(!inp.active[p][g] || eq.price[(p, g)] > 0.0);
        }
    }
    assert!(eq.wage.iter().all(|&w| w > 0.0) && eq.income.iter().all(|&i| i > 0.0));
}

/// As `fixture::UNREACHABLE` and `fixture::ISLAND_USER`.
const UNREACHABLE: usize = 6;
const ISLAND_USER: usize = 4;

/// The fixture as `fixture::unreachable_island` exports it: machinery without a route to or from
/// the island port (kernel factor 0, nothing arrives), and of the island's sectors only light
/// manufacturing using machinery.
fn unreachable_island(inp: &Inputs) -> Inputs {
    let mut out = inp.clone();
    let island = inp.n_countries() - 1;
    let port: Vec<usize> = (0..inp.n_gateways())
        .filter(|&a| inp.gateway_country[a] == island)
        .collect();
    let cro = &mut out.crossing;
    for j in 0..cro.good.len() {
        if cro.good[j] == UNREACHABLE && (port.contains(&cro.row[j]) || port.contains(&cro.col[j]))
        {
            cro.k[j] = 0.0;
            cro.inv_loss[j] = 0.0;
        }
    }
    for g in (0..inp.n_goods()).filter(|&g| g != ISLAND_USER) {
        out.io[island][(UNREACHABLE, g)] = 0.0;
    }
    out
}

/// At world level, with machinery unreachable at the island its delivered price
/// there is infinite, the world price index leaves that cell out, the island spends nothing on
/// machinery, the island's light manufacturing (its one user) employs and produces nothing, and
/// the reference solves with a finite residual.
#[test]
fn unreachable_destination_solves() {
    let inp = unreachable_island(inputs());
    let m = Model::new(&inp).unwrap();
    let island = inp.n_provinces() - 1;
    let x0 = m.data_state();
    let at_data = model::evaluate(&x0, &m, &inp.ent_a);
    assert!(at_data.consumer_price[(island, UNREACHABLE)].is_infinite());
    assert!(model::residual(&x0, &m).iter().all(|r| r.is_finite()));
    let eq = solve::equilibrium(&m, None, TOL, None).unwrap();
    let r = model::residual(&eq.x, &m);
    assert!(r.iter().all(|r| r.is_finite()) && model::max_abs(&r) <= 1e-10);
    assert!((eq.price_index - 1.0).abs() <= 1e-10);
    assert_eq!(eq.consumer_price[(island, UNREACHABLE)], f64::INFINITY);
    assert_eq!(eq.expenditure[(island, UNREACHABLE)], 0.0);
    let cut = island * inp.n_goods() + UNREACHABLE;
    assert!(
        (0..eq.consumer_price.data.len())
            .filter(|&i| i != cut)
            .all(|i| eq.consumer_price.data[i].is_finite())
    );
    for e in (0..inp.ent_province.len()).filter(|&e| inp.ent_province[e] == island) {
        if inp.ent_good[e] == ISLAND_USER {
            assert!(eq.employment[e] == 0.0 && eq.output[e] == 0.0);
        } else {
            assert!(eq.output[e] > 0.0);
        }
    }
    for v in [
        &eq.price.data,
        &eq.wage,
        &eq.income,
        &eq.flows_by_country_pair_good,
        &eq.units_by_country_pair_good,
    ] {
        assert!(v.iter().all(|x| x.is_finite()));
    }
    assert!(eq.wage.iter().all(|&w| w > 0.0) && eq.income.iter().all(|&i| i > 0.0));
}

#[test]
fn scale_invariance() {
    let inp = inputs();
    let m = Model::new(inp).unwrap();
    let mut rng = Draws(7);
    let x: Vec<f64> = m.data_state().iter().map(|x| x + rng.normal(0.2)).collect();
    let base = model::residual(&x, &m);
    let shift = 3.7_f64.ln();
    let scaled = model::residual(&x.iter().map(|x| x + shift).collect::<Vec<_>>(), &m);
    let row = m.index_row();
    for i in (0..x.len()).filter(|&i| i != row) {
        assert_close(&[scaled[i]], &[base[i]], 1e-10, 1e-13, "homogeneous rows");
    }
    assert!((scaled[row] - base[row] - shift).abs() <= 1e-12);
}

/// S02 (ADR 0016): balanced Sinkhorn on the dense kernel, with the equilibrium's origin supplies
/// (value at origin prices) and destination expenditures (net of tariff) as marginals, reproduces
/// the sourcing flows w_o K_od E_d / Phi_d; the dense flows aggregate to the model's low-rank
/// country-pair value and units.
#[test]
fn sinkhorn_and_dense_kernel_reproduce_equilibrium_gravity() {
    let inp = inputs();
    let eq = solved();
    let (n_p, n_c, n_g) = (inp.n_provinces(), inp.n_countries(), inp.n_goods());
    let country = &inp.province_country;
    for g in traded_goods(inp) {
        let (k, k_net) = (
            sinkhorn::dense_kernel(inp, g, false),
            sinkhorn::dense_kernel(inp, g, true),
        );
        let p = column(&eq.price, g);
        let w = weights(inp, g, &p, &active_column(inp, g));
        let phi: Vec<f64> = (0..n_p)
            .map(|d| (0..n_p).map(|o| w[o] * k[(o, d)]).sum())
            .collect();
        let flows = Mat::from_fn(n_p, n_p, |o, d| {
            w[o] * k_net[(o, d)] * (eq.expenditure[(d, g)] / phi[d])
        });
        let supply: Vec<f64> = (0..n_p).map(|o| p[o] * eq.supply[(o, g)]).collect();
        let total_supply: f64 = supply.iter().sum();
        // The origin markets clear: what destinations send them is their output at their price.
        let rows = flows.row_sums();
        assert_close(
            &rows,
            &supply,
            1e-8,
            1e-12 * total_supply,
            "origin clearing",
        );
        // Balanced on the flows' own row sums: the solve's tolerance leaves supply inconsistent
        // with the column sums at about 1e-11, beyond what balancing to Sinkhorn's 1e-14 absorbs.
        let cols: Vec<f64> = (0..n_p)
            .map(|d| (0..n_p).map(|o| flows[(o, d)]).sum())
            .collect();
        let solved = sinkhorn::balanced(&k_net, &rows, &cols).unwrap();
        let total: f64 = flows.data.iter().sum();
        for (s, f) in solved.data.iter().zip(&flows.data) {
            if *f > 1e-12 * total {
                assert_close(&[*s], &[*f], 1e-8, 0.0, "balanced flows");
            } else {
                assert!(s.abs() <= 1e-10 * total);
            }
        }
        for (got, units) in [
            (&eq.flows_by_country_pair_good, false),
            (&eq.units_by_country_pair_good, true),
        ] {
            let mut pairs = vec![0.0; n_c * n_c];
            for o in 0..n_p {
                for d in (0..n_p).filter(|&d| d != o) {
                    let per = if units && p[o] > 0.0 { p[o] } else { 1.0 };
                    pairs[country[o] * n_c + country[d]] += flows[(o, d)] / per;
                }
            }
            let at_g: Vec<f64> = (0..n_c * n_c).map(|i| got[i * n_g + g]).collect();
            let scale: f64 = pairs.iter().sum();
            assert_close(&at_g, &pairs, 1e-9, 1e-9 * scale, "country pairs");
        }
    }
}

/// Every route's money enumerated from the exported legs.
struct Oracle {
    orders: Mat,
    home: Mat,
    arrive: Mat,
    paid: Mat,
    received: Mat,
    tariff: Mat,
    index: Mat,
    pair_value: Vec<f64>,
    pair_units: Vec<f64>,
}

/// Every route enumerated from the exported legs: home, o -> d inside a country, and
/// o -> a -> b -> d over every gateway pair, with its own K, iceberg and tariff. Destination d
/// spends E_d over routes by K w_o / Phi_d; the buyer pays p_o (1 + t) per shipped unit,
/// t / (1 + t) of it is tariff, the origin receives p_o per unit. With `gamma` (ADR 0200) d
/// pays q_d = Gamma_d p_min Phi_d^(-1/theta) and receives what it paid over q_d; without, it
/// receives the units over the route's tau and q_d is what it pays per arriving unit.
fn dense_route_oracle(inp: &Inputs, price: &Mat, spend: &Mat, gamma: Option<&Mat>) -> Oracle {
    let (n_p, n_g, n_c) = (inp.n_provinces(), inp.n_goods(), inp.n_countries());
    let zeros = || Mat::zeros(n_p, n_g);
    let mut out = Oracle {
        orders: zeros(),
        home: zeros(),
        arrive: zeros(),
        paid: zeros(),
        received: zeros(),
        tariff: zeros(),
        index: zeros(),
        pair_value: vec![0.0; n_c * n_c * n_g],
        pair_units: vec![0.0; n_c * n_c * n_g],
    };
    let country = &inp.province_country;
    let (dom, acc, cro) = (&inp.domestic, &inp.access, &inp.crossing);
    for g in traded_goods(inp) {
        let p = column(price, g);
        let w = weights(inp, g, &p, &active_column(inp, g));
        // (o, d, k, 1 / tau, 1 / (1 + t))
        let mut routes: Vec<(usize, usize, f64, f64, f64)> =
            (0..n_p).map(|o| (o, o, 1.0, 1.0, 1.0)).collect();
        for j in (0..dom.good.len()).filter(|&j| dom.good[j] == g) {
            routes.push((dom.row[j], dom.col[j], dom.k[j], dom.inv_tau[j], 1.0));
        }
        let mut gate_in: Vec<Vec<(usize, f64, f64)>> = vec![Vec::new(); inp.n_gateways()];
        let acc_g: Vec<usize> = (0..acc.good.len()).filter(|&j| acc.good[j] == g).collect();
        for &j in &acc_g {
            gate_in[acc.col[j]].push((acc.row[j], acc.k[j], acc.inv_tau[j]));
        }
        for &ja in &acc_g {
            let (o, a) = (acc.row[ja], acc.col[ja]);
            for jc in (0..cro.good.len()).filter(|&jc| cro.good[jc] == g && cro.row[jc] == a) {
                for &(d, k_bd, inv_bd) in &gate_in[cro.col[jc]] {
                    let k = acc.k[ja] * cro.k[jc] * k_bd;
                    let arrives = acc.inv_tau[ja] * cro.inv_loss[jc] * inv_bd;
                    routes.push((o, d, k, arrives, cro.inv_duty[jc]));
                }
            }
        }
        let mut phi = vec![0.0; n_p];
        for &(o, d, k, _, _) in &routes {
            phi[d] += k * w[o];
        }
        let active = active_column(inp, g);
        let p_min = (0..n_p)
            .filter(|&i| active[i])
            .map(|i| p[i])
            .fold(f64::INFINITY, f64::min);
        let buyer =
            |d: usize| gamma.map(|gm| gm[(d, g)] * p_min * phi[d].powf(-1.0 / inp.theta[g]));
        let mut alpha = vec![0.0; n_p];
        for &(o, d, k, arrives, net) in &routes {
            if w[o] == 0.0 {
                continue;
            }
            let x = spend[(d, g)] * k * w[o] / phi[d];
            let units = x * net / p[o];
            alpha[d] += k * w[o] / phi[d] * net / p[o] * arrives;
            out.orders[(o, g)] += units;
            out.arrive[(d, g)] += match buyer(d) {
                Some(q) => x / q,
                None => units * arrives,
            };
            out.paid[(d, g)] += x;
            out.tariff[(d, g)] += x * (1.0 - net);
            out.received[(o, g)] += x * net;
            if o == d {
                out.home[(d, g)] += units;
            } else {
                let pair = (country[o] * n_c + country[d]) * n_g + g;
                out.pair_value[pair] += x * net;
                out.pair_units[pair] += units;
            }
        }
        for (d, a) in alpha.iter().enumerate() {
            out.index[(d, g)] = buyer(d).unwrap_or(1.0 / a);
        }
    }
    out
}

/// ADR 0018: the sourcing weights use each good's theta.
#[test]
fn sourcing_weights_use_theta_per_good() {
    let inp = inputs();
    let m = Model::new(inp).unwrap();
    let mut rng = Draws(3);
    let price = draw_mat(inp.n_provinces(), inp.n_goods(), |_| {
        rng.uniform(-0.5, 0.5).exp()
    });
    let w = m.sourcing(&price).w;
    let n_p = inp.n_provinces();
    for g in traded_goods(inp) {
        let want = weights(inp, g, &column(&price, g), &active_column(inp, g));
        assert_close(&w[g * n_p..(g + 1) * n_p], &want, 1e-14, 0.0, "weights");
    }
}

/// Both buyer prices: Gamma P_d with the exported Gamma (ADR 0200), and the delivered unit
/// price of the physical arrivals ([`Model::physical`]).
#[test]
fn trade_money_matches_dense_route_oracle() {
    let inp = inputs();
    assert!(inp.crossing.inv_duty.iter().any(|&d| d < 1.0));
    let gamma = inp.gamma.as_ref().unwrap();
    assert!(gamma.data.iter().any(|&g| (g - 1.0).abs() > 1e-3));
    for (m, gamma) in [
        (Model::new(inp).unwrap(), Some(gamma)),
        (Model::physical(inp).unwrap(), None),
    ] {
        oracle_case(inp, &m, gamma);
    }
}

fn oracle_case(inp: &Inputs, m: &Model, gamma: Option<&Mat>) {
    let mut rng = Draws(11);
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    let price = draw_mat(n_p, n_g, |_| rng.uniform(-2.3, 2.3).exp());
    let spend = draw_mat(n_p, n_g, |g| {
        let s = rng.uniform(0.0, 1e6);
        if inp.traded[g] { s } else { 0.0 }
    });
    let want = dense_route_oracle(inp, &price, &spend, gamma);
    let src = m.sourcing(&price);
    let got = m.flows(&price, &spend, &src);
    let traded = traded_goods(inp);
    let pairs = |v: &[f64]| -> Vec<f64> {
        (0..v.len())
            .filter(|i| traded.contains(&(i % n_g)))
            .map(|i| v[i])
            .collect()
    };
    let cells = |mat: &Mat| -> Vec<f64> { traded.iter().flat_map(|&g| column(mat, g)).collect() };
    for (key, got, want) in [
        ("orders", cells(&got.orders), cells(&want.orders)),
        ("home", cells(&got.home), cells(&want.home)),
        ("arrive", cells(&got.arrive), cells(&want.arrive)),
        ("paid", cells(&got.paid), cells(&want.paid)),
        ("received", cells(&got.received), cells(&want.received)),
        ("tariff", cells(&got.tariff), cells(&want.tariff)),
        ("index", cells(&src.index), cells(&want.index)),
        (
            "pair_value",
            pairs(&got.pair_value),
            pairs(&want.pair_value),
        ),
        (
            "pair_units",
            pairs(&got.pair_units),
            pairs(&want.pair_units),
        ),
    ] {
        let scale = model::max_abs(&want);
        assert_close(&got, &want, 1e-10, 1e-13 * scale, key);
    }
}

/// Owner decision 2026-10-25: coefficients are ICIO cost shares at base delivered prices. The
/// exported base prices are the reference's own buyer prices at prices 1, both with the exported
/// Gamma (Gamma P_d, ADR 0200) and as the delivered unit price of the physical arrivals, so at
/// the data state every enterprise's input cost share is its sector's a_kg.
#[test]
fn base_prices_are_the_reference_sourcing_at_prices_one() {
    let inp = inputs();
    let m = Model::new(inp).unwrap();
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    let ones = Mat::filled(n_p, n_g, 1.0);
    let q = m.sourcing(&ones).index;
    let physical = Model::physical(inp).unwrap().sourcing(&ones).index;
    for g in 0..n_g {
        if inp.traded[g] {
            for (what, q) in [("base price", &q), ("physical base price", &physical)] {
                assert_close(&column(&inp.base_price, g), &column(q, g), 1e-13, 0.0, what);
            }
        } else {
            assert!(column(&inp.base_price, g).iter().all(|&p| p == 1.0));
        }
    }
    assert!(inp.base_price.data.iter().any(|&p| p > 1.0));
    let country = inp.ent_country();
    for (e, &c) in country.iter().enumerate() {
        let a: Vec<f64> = (0..n_g).map(|k| inp.io[c][(k, inp.ent_good[e])]).collect();
        let cost: Vec<f64> = (0..n_g)
            .map(|k| m.ent_inputs[(e, k)] * inp.base_price[(inp.ent_province[e], k)])
            .collect();
        assert_close(&cost, &a, 1e-15, 0.0, "input cost shares");
        assert_close(
            &[m.ent_a_va[e]],
            &[1.0 - io::pairwise_sum(&a)],
            1e-14,
            0.0,
            "a_VA",
        );
    }
}

fn read_table(dir: &Path, name: &str) -> RecordBatch {
    let file = File::open(dir.join(format!("{name}.arrow"))).unwrap();
    let mut batches: Vec<RecordBatch> = FileReader::try_new(file, None)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(batches.len(), 1, "one batch per reference table");
    batches.pop().unwrap()
}

fn f64s(t: &RecordBatch, col: &str) -> Vec<f64> {
    t.column_by_name(col)
        .unwrap()
        .as_primitive::<Float64Type>()
        .values()
        .to_vec()
}

fn indices(t: &RecordBatch, col: &str) -> Vec<usize> {
    cast(t.column_by_name(col).unwrap(), &DataType::UInt64)
        .unwrap()
        .as_primitive::<UInt64Type>()
        .values()
        .iter()
        .map(|&v| v as usize)
        .collect()
}

/// `tests/golden/p2-fixture/reference` (read by Rust: runs started from the equilibrium and the
/// V1 check) is the current solve of the fixture. Regenerate with
/// `ceteris reference solve --inputs tests/golden/p2-fixture --out tests/golden/p2-fixture/reference`.
#[test]
fn fixture_reference_is_current() {
    let inp = inputs();
    let eq = solved();
    let dir = fixture().join("reference");
    let (n_c, n_g) = (inp.n_countries(), inp.n_goods());
    let prices = read_table(&dir, "prices");
    assert_close(&f64s(&prices, "price"), &eq.price.data, 1e-8, 0.0, "price");
    assert_close(
        &f64s(&read_table(&dir, "wages"), "wage"),
        &eq.wage,
        1e-8,
        0.0,
        "wage",
    );
    let ent = read_table(&dir, "enterprises");
    assert_close(
        &f64s(&ent, "employment"),
        &eq.employment,
        1e-8,
        0.0,
        "employment",
    );
    assert_close(&f64s(&ent, "output"), &eq.output, 1e-8, 0.0, "output");
    let flows = read_table(&dir, "flows");
    let pairs: Vec<usize> = (0..n_c * n_c * n_g)
        .filter(|&i| eq.flows_by_country_pair_good[i] > 0.0)
        .collect();
    let (ex, im, g) = (
        indices(&flows, "exporter_country"),
        indices(&flows, "importer_country"),
        indices(&flows, "good"),
    );
    let at: Vec<usize> = (0..ex.len())
        .map(|i| (ex[i] * n_c + im[i]) * n_g + g[i])
        .collect();
    assert_eq!(at, pairs);
    let scale = eq
        .flows_by_country_pair_good
        .iter()
        .copied()
        .fold(0.0, f64::max);
    let pick = |v: &[f64]| -> Vec<f64> { pairs.iter().map(|&i| v[i]).collect() };
    assert_close(
        &f64s(&flows, "value"),
        &pick(&eq.flows_by_country_pair_good),
        1e-8,
        1e-8 * scale,
        "flow value",
    );
    assert_close(
        &f64s(&flows, "units"),
        &pick(&eq.units_by_country_pair_good),
        1e-8,
        1e-8 * scale,
        "flow units",
    );
    let traded: Vec<usize> = (0..inp.n_provinces() * n_g)
        .filter(|i| inp.traded[i % n_g])
        .collect();
    let home = read_table(&dir, "home");
    for (col, m) in [("value", &eq.home_value), ("units", &eq.home_units)] {
        let want: Vec<f64> = traded.iter().map(|&i| m.data[i]).collect();
        assert_close(&f64s(&home, col), &want, 1e-8, 1e-8 * scale, col);
    }
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("solution.json")).unwrap()).unwrap();
    assert_eq!(meta["static_digest"], inp.static_digest.as_str());
    assert!((meta["price_index"].as_f64().unwrap() - eq.price_index).abs() <= 1e-8);
}

/// The fixture with a tariff per country pair and good, 0 to 10%: its own tariffs vary by
/// importer and good only, which importer effects absorb.
fn pair_tariffs(inp: &Inputs) -> Inputs {
    let mut out = inp.clone();
    let cro = &mut out.crossing;
    for j in 0..cro.k.len() {
        let (e, m, g) = (
            inp.gateway_country[cro.row[j]],
            inp.gateway_country[cro.col[j]],
            cro.good[j],
        );
        let inv_duty = 1.0 / (1.0 + 0.01 * ((7 * e + 3 * m * m + 5 * e * m + g) % 11) as f64);
        cro.k[j] *= (inv_duty / cro.inv_duty[j]).powf(inp.theta[g]);
        cro.inv_duty[j] = inv_duty;
    }
    out
}

/// H2 per theta class (ADRs 0092 and 0264): with the model's own country-pair kernel net of
/// tariffs as the cost control, the tariff regression of each class's equilibrium flows recovers
/// the class's theta, on the fixture with tariffs per pair. Weighted by the sourcing weights w_o
/// and E_d / Phi_d the kernel is exact: theta to the fit's tolerance, the kernel's coefficient 1.
#[test]
fn kernel_controlled_tariff_regression_recovers_theta_per_class() {
    let inp = &pair_tariffs(inputs());
    let m = Model::new(inp).unwrap();
    let eq = solve::equilibrium(&m, None, TOL, None).unwrap();
    let (n_p, n_c, n_g) = (inp.n_provinces(), inp.n_countries(), inp.n_goods());
    let src = m.sourcing(&eq.price);
    let phi = model::cells(&src.phi, n_p, n_g);
    let destination = Mat::from_fn(n_p, n_g, |p, g| {
        if phi[(p, g)] > 0.0 {
            eq.expenditure[(p, g)] / phi[(p, g)]
        } else {
            0.0
        }
    });
    let ln_k = model::ln_country_kernel(inp, &model::cells(&src.w, n_p, n_g), &destination);
    let mut tariff = vec![f64::NAN; n_c * n_c * n_g];
    let cro = &inp.crossing;
    for j in 0..cro.k.len() {
        let (e, m) = (
            inp.gateway_country[cro.row[j]],
            inp.gateway_country[cro.col[j]],
        );
        tariff[(e * n_c + m) * n_g + cro.good[j]] = 1.0 / cro.inv_duty[j] - 1.0;
    }
    let goods = traded_goods(inp);
    let n_classes = goods.iter().map(|&g| inp.theta_class[g]).max().unwrap() + 1;
    for c in 0..n_classes {
        let rows: Vec<(usize, usize, usize)> = (0..n_c)
            .flat_map(|e| (0..n_c).filter(move |&m| m != e).map(move |m| (e, m)))
            .flat_map(|(e, m)| {
                goods
                    .iter()
                    .filter(move |&&g| inp.theta_class[g] == c)
                    .map(move |&g| (e, m, g))
            })
            .filter(|&(e, m, g)| ln_k[(e * n_c + m) * n_g + g].is_finite())
            .collect();
        let at = |&(e, m, g): &(usize, usize, usize)| (e * n_c + m) * n_g + g;
        let t: Vec<f64> = rows.iter().map(|r| tariff[at(r)]).collect();
        let paid: Vec<f64> = rows
            .iter()
            .zip(&t)
            .map(|(r, t)| eq.flows_by_country_pair_good[at(r)] * (1.0 + t))
            .collect();
        let control: Vec<f64> = rows.iter().map(|r| ln_k[at(r)]).collect();
        let fit =
            ppml::class_tariff_fit(&rows, &paid, &t, &[("ln_kernel", &control)], true).unwrap();
        let theta = -fit.coef("ln_tariff");
        let theta_c = inp.theta[rows[0].2];
        println!(
            "class {c}: theta {theta:.4} (pair-clustered s.e. {:.4}), ln kernel {:.4}, theta_c \
             {theta_c}, n {}",
            fit.se("ln_tariff"),
            fit.coef("ln_kernel"),
            fit.n
        );
        assert!(
            (theta - theta_c).abs() <= 1e-6 * theta_c,
            "class {c}: theta {theta} against {theta_c}"
        );
        assert!((fit.coef("ln_kernel") - 1.0).abs() <= 1e-6);
    }
}

/// Newton-Krylov that hits its step cap falls back to tatonnement, then
/// Newton-Krylov from there, and the solution reports it. At a wage of e^-800 (0) the residual is
/// not finite: Newton-Krylov falls back at once, tatonnement does not settle within its cap, and
/// the solve errs instead of panicking.
///
/// The cap `NEWTON_CAP` is chosen for the fixture as it stands (gateway masses, ADR 0092): from
/// the perturbed start Newton-Krylov needs more than 6 steps (residuals 2.9, 1.9, 0.91, 0.12,
/// 0.013, 0.0038, 2.2e-5), while from tatonnement's point it needs 2 (4.7e-6, 1.2e-10, 3.2e-15).
/// At 3 the first run fails and the second converges; at 2 the second fails too (3.2e-15 is
/// reached only at the third check).
#[test]
fn nonconverging_solve_reports_and_falls_back() {
    let inp = inputs();
    let m = Model::new(inp).unwrap();
    let perturbed = |sd: f64| -> Vec<f64> {
        let mut rng = Draws(5);
        m.data_state().iter().map(|x| x + rng.normal(sd)).collect()
    };
    let solve_from = |x0: &[f64], limits: Limits| {
        let mut lines = Vec::new();
        let eq = solve::equilibrium_with(&m, Some(x0), TOL, None, limits, &mut |l| {
            lines.push(l.to_owned());
        });
        (eq, lines)
    };

    const NEWTON_CAP: usize = 3;
    let limits = Limits {
        max_newton: NEWTON_CAP,
        ..Limits::default()
    };
    let (eq, lines) = solve_from(&perturbed(0.1), limits);
    let eq = eq.unwrap();
    let failed = lines
        .iter()
        .find_map(|l| {
            l.strip_prefix("newton_krylov failed (residual ")?
                .strip_suffix(&format!(" after {NEWTON_CAP} Newton steps): tatonnement"))
        })
        .unwrap_or_else(|| panic!("no capped Newton-Krylov run: {lines:?}"));
    // the first run really did not converge within the cap
    assert!(failed.parse::<f64>().unwrap() > TOL, "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("tatonnement 0 step")));
    assert_eq!(eq.info.method, "tatonnement, newton_krylov");
    assert!(eq.info.iterations < NEWTON_CAP && eq.info.residual <= TOL);
    assert!(model::max_abs(&model::residual(&eq.x, &m)) <= TOL);

    let limits = Limits {
        max_tatonnement: 20,
        ..Limits::default()
    };
    let mut x0 = m.data_state();
    *x0.last_mut().unwrap() = -800.0;
    let (err, lines) = solve_from(&x0, limits);
    assert!(
        lines.contains(
            &"newton_krylov failed (residual not finite after 0 Newton steps): tatonnement".into()
        ),
        "{lines:?}"
    );
    assert!(
        matches!(&err, Err(SolveError::NoConvergence(s)) if s.starts_with("tatonnement step ") && s.ends_with(" after 20 iterations")),
        "{lines:?}"
    );
}

/// Past the deadline the solve stops with an error.
#[test]
fn spent_deadline_is_no_convergence() {
    let m = Model::new(inputs()).unwrap();
    let err = solve::equilibrium(&m, None, TOL, Some(std::time::Instant::now())).unwrap_err();
    assert!(
        matches!(&err, SolveError::NoConvergence(s) if s.ends_with("wall-clock budget spent")),
        "{err:?}"
    );
}

fn empty_legs() -> Legs {
    Legs {
        good: vec![],
        row: vec![],
        col: vec![],
        k: vec![],
        inv_tau: vec![],
    }
}

/// One traded good, one country, two identical provinces joined by a domestic leg, with the Rust
/// base point: L0 = (1 - u) L^s, w0 = s_L VA0 / L0. Each province has two half-size enterprises:
/// with a single one, the target's cap at L^s leaves labour demand at or below supply for every
/// low wage, so no wage clears uniquely. The good is the only one of PIGL category S, so the
/// inversion gives z = 0 (no category A demand) and demand is homothetic: the province spends
/// its whole income on it, whatever the pops' incomes.
#[allow(clippy::too_many_arguments)]
fn two_provinces(
    theta: f64,
    tau: f64,
    sigma_demand: f64,
    sigma_fl: f64,
    s_l: f64,
    a: f64,
    labour: f64,
    u: f64,
) -> Inputs {
    let k = tau.powf(-theta);
    let l0 = (1.0 - u) * labour / 2.0;
    Inputs {
        theta: vec![theta],
        theta_class: vec![0],
        border_nu: vec![0.3],
        sigma_demand,
        pigl: Pigl {
            sigma_b: 0.26,
            phi: 0.0,
            gamma: 0.71,
            theta: [0.634, 0.082, 0.284],
            eps: 0.71,
        },
        category: vec![2],
        sigma_fl,
        traded: vec![true],
        province_country: vec![0, 0],
        gateway_country: vec![],
        gateway_mass: vec![],
        labour_supply: vec![labour; 2],
        population: vec![1e6; 2],
        w0: vec![s_l * 50.0 / l0; 2],
        base_price: Mat::filled(2, 1, 1.0),
        gamma: None,
        s_l: vec![s_l],
        unemployment: vec![u],
        u_star: vec![0.0],
        gini: vec![0.35],
        omega: Mat::filled(1, 1, 1.0),
        io: vec![Mat::zeros(1, 1)],
        active: vec![vec![true]; 2],
        ent_province: vec![0, 0, 1, 1],
        ent_good: vec![0; 4],
        ent_a: vec![a; 4],
        ent_va0: vec![50.0; 4],
        ent_l0: vec![l0; 4],
        pop_province: vec![0, 0, 1, 1],
        pop_effective: [0.3, 0.7, 0.3, 0.7].iter().map(|s| s * labour).collect(),
        pop_population: vec![4e5, 6e5, 4e5, 6e5],
        domestic: Legs {
            good: vec![0, 0],
            row: vec![0, 1],
            col: vec![1, 0],
            k: vec![k; 2],
            inv_tau: vec![1.0 / tau; 2],
        },
        access: empty_legs(),
        crossing: Crossings {
            good: vec![],
            row: vec![],
            col: vec![],
            k: vec![],
            inv_loss: vec![],
            inv_duty: vec![],
            lag: vec![],
        },
        ent_va_target: None,
        coverage: None,
        storage_loss: vec![0.0],
        storage: Default::default(),
        pop_edu: None,
        static_digest: String::new(),
        calibrated_coef: None,
        calibrated_a_va: None,
        calibrated_weights: None,
    }
}

fn close(got: f64, want: f64, rel: f64) -> bool {
    (got - want).abs() <= rel * want.abs()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(25))]

    #[test]
    fn two_province_closed_form(
        theta in 4.0..8.0f64,
        tau in 1.01..2.0f64,
        sigma_demand in 0.3..0.9f64,
        sigma_fl in 0.5..1.2f64,
        s_l in 0.4..0.7f64,
        a in 0.5..2.0f64,
        labour in 10.0..500.0f64,
        u in 0.0..0.15f64,
    ) {
        let inp = two_provinces(theta, tau, sigma_demand, sigma_fl, s_l, a, labour, u);
        let m = Model::new(&inp).unwrap();
        let eq = solve::equilibrium(&m, None, TOL, None).unwrap();
        // Without Gamma (the toy's inputs carry none) buyers pay the delivered unit price of
        // the physical arrivals, q = 1 / alpha: a unit of spending buys home at p
        // and the other province's good at p tau, with sourcing shares 1 / (1 + k) and
        // k / (1 + k); the world index is q, normalized to 1.
        let k = tau.powf(-theta);
        let p = (1.0 + k / tau) / (1.0 + k);
        for i in 0..2 {
            prop_assert!(close(eq.price[(i, 0)], p, 1e-9));
            prop_assert!(close(eq.consumer_price[(i, 0)], 1.0, 1e-9));
        }
        // Full employment at the marginal product of labour; per province VA0 = 100,
        // L0 = (1 - u) L^s.
        let rho = (sigma_fl - 1.0) / sigma_fl;
        let l0 = (1.0 - u) * labour;
        let x = labour / l0;
        let (y, w) = if rho.abs() < 1e-9 {
            (
                a * 100.0 * x.powf(s_l),
                a * 100.0 * s_l * x.powf(s_l - 1.0) / l0,
            )
        } else {
            let bracket = (1.0 - s_l) + s_l * x.powf(rho);
            (
                a * 100.0 * bracket.powf(1.0 / rho),
                a * 100.0 * bracket.powf(1.0 / rho - 1.0) * s_l * x.powf(rho - 1.0) / l0,
            )
        };
        prop_assert!(eq.output.iter().all(|&o| close(o, y / 2.0, 1e-9)));
        prop_assert!(eq.wage.iter().all(|&v| close(v, p * w, 1e-9)));
        // Each province spends its income p y over the two origins by the sourcing shares.
        for i in 0..2 {
            prop_assert!(close(eq.home_units[(i, 0)], y / (1.0 + k), 1e-9));
        }
        prop_assert!(close(eq.units_by_country_pair_good[0], 2.0 * y * k / (1.0 + k), 1e-9));
        // value is the origin value of the shipped units
        prop_assert!(close(eq.flows_by_country_pair_good[0], 2.0 * p * y * k / (1.0 + k), 1e-9));
    }
}
