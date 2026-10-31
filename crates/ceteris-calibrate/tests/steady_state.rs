//! ADR 0089: the calibrated data state is the steady state, on the fixture and, with
//! `CETERIS_PACK_EXPORT` set (a `ceteris export --pack` directory of the pack in `CETERIS_PACK`,
//! default `data/packs/2000`), on the full pack.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt8Type, UInt32Type};
use arrow_ipc::reader::FileReader;
use arrow_schema::DataType;
use ceteris_calibrate::steady::{
    self, COVER, CalibrationError, Limits, PASS_TOL, SINKHORN_TOL, SLOTS, Steady,
};
use ceteris_calibrate::write;
use ceteris_calibrate::{block, border, elasticity};
use ceteris_core::entity::{EntityId, Kind};
use ceteris_core::rng::Seed;
use ceteris_core::schedule::SystemId;
use ceteris_pack::compare::{self, Tolerance};
use ceteris_reference::demand;
use ceteris_reference::io::{self, Inputs, Mat};
use ceteris_reference::model::{self, Factors, Model, max_abs};
use ceteris_trade::TradeParams;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture() -> PathBuf {
    std::env::var_os("CETERIS_FIXTURE_DIR")
        .map_or_else(|| root().join("tests/golden/p2-fixture"), PathBuf::from)
}

fn pack() -> PathBuf {
    std::env::var_os("CETERIS_PACK").map_or_else(|| root().join("data/packs/2000"), PathBuf::from)
}

struct World {
    name: &'static str,
    inp: Inputs,
    s: Steady,
    /// The pack province of each province, for a world from a pack.
    pack_province: Option<Vec<usize>>,
}

fn quiet() -> impl FnMut(&str) {
    |_: &str| {}
}

/// The fixture and, when `CETERIS_PACK_EXPORT` is set, the pack, each calibrated once.
fn worlds() -> &'static [World] {
    static WORLDS: OnceLock<Vec<World>> = OnceLock::new();
    WORLDS.get_or_init(|| {
        let inp = io::load(fixture()).unwrap();
        let s = steady::calibrate(&inp, &inp.u_star.clone(), &mut quiet()).unwrap();
        let mut out = vec![World {
            name: "fixture",
            inp,
            s,
            pack_province: None,
        }];
        match std::env::var_os("CETERIS_PACK_EXPORT") {
            Some(dir) => {
                let dir = PathBuf::from(dir);
                let inp = io::load(&dir).unwrap();
                let u_star = write::read_u_star(&pack()).unwrap();
                let s = steady::calibrate(&inp, &u_star, &mut quiet()).unwrap();
                out.push(World {
                    name: "pack",
                    inp,
                    s,
                    pack_province: Some(write::read_pack_provinces(&dir).unwrap()),
                });
            }
            None => eprintln!(
                "skipping the pack: CETERIS_PACK_EXPORT unset (a full-pack export directory)"
            ),
        }
        out
    })
}

/// numpy's `assert_allclose`: |actual - desired| <= atol + rtol |desired|, NaN equal to NaN.
fn allclose(what: &str, actual: &[f64], desired: &[f64], rtol: f64, atol: f64) {
    assert_eq!(actual.len(), desired.len(), "{what}: length");
    for (i, (&a, &d)) in actual.iter().zip(desired).enumerate() {
        let ok = (a.is_nan() && d.is_nan()) || a == d || (a - d).abs() <= atol + rtol * d.abs();
        assert!(ok, "{what}[{i}]: {a} vs {d}");
    }
}

/// SplitMix64 with Box-Muller normals: reproducible draws for the tests.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn uniform(&mut self) -> f64 {
        ((self.next() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    fn normal(&mut self, mean: f64, sd: f64) -> f64 {
        let (u, v) = (self.uniform(), self.uniform());
        mean + sd * (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// Spending-weighted shares (C, G) of the pops of each country: each pop at its own buyer
/// prices `price` (K, G) and income per person, weighted by its spending, at the dispersion the
/// weights carry.
fn aggregate_shares(inp: &Inputs, w: &demand::Weights, price: &Mat, spend: &[f64]) -> Mat {
    let (n_c, n_g) = (inp.n_countries(), inp.n_goods());
    let country = inp.pop_country();
    let per_person: Vec<f64> = spend
        .iter()
        .zip(&inp.pop_population)
        .map(|(e, n)| if *n > 0.0 { e / n } else { 0.0 })
        .collect();
    let sigma: Vec<f64> = country.iter().map(|&c| w.sigma[c]).collect();
    let share = demand::shares(
        w,
        &country,
        price,
        &per_person,
        &sigma,
        &inp.category,
        inp.sigma_demand,
        &inp.pigl,
    );
    let mut spent = Mat::zeros(n_c, n_g);
    for (k, &c) in country.iter().enumerate() {
        for g in 0..n_g {
            if spend[k] > 0.0 {
                spent[(c, g)] += share[(k, g)] * spend[k];
            }
        }
    }
    let total = spent.row_sums();
    Mat::from_fn(n_c, n_g, |c, g| spent[(c, g)] / total[c])
}

fn omega_shares(inp: &Inputs) -> Mat {
    let total = inp.omega.row_sums();
    Mat::from_fn(inp.n_countries(), inp.n_goods(), |c, g| {
        inp.omega[(c, g)] / total[c]
    })
}

/// Stage 4 (ADR 0094): weights inverted jointly over a country's pops, each at its own buyer
/// prices and spending, give the final demand shares as the pops' spending-weighted aggregate
/// exactly. The simulator's own inversion at prices 1 is checked against the reference's by
/// `pigl_matches_rust`.
#[test]
fn inversion_at_given_prices_reproduces_the_shares() {
    let inp = io::load(fixture()).unwrap();
    let n_g = inp.n_goods();
    let country = inp.pop_country();
    let n_k = country.len();
    let mut rng = Rng(4);
    let draws: Vec<f64> = (0..n_k * n_g).map(|_| rng.normal(0.0, 0.3).exp()).collect();
    let price = Mat {
        rows: n_k,
        cols: n_g,
        data: draws,
    };
    let base = inp.pop_base_income();
    let spend: Vec<f64> = (0..n_k)
        .map(|k| base[k] * inp.pop_population[k] * rng.normal(0.0, 0.2).exp())
        .collect();
    let per_person: Vec<f64> = (0..n_k).map(|k| spend[k] / inp.pop_population[k]).collect();
    let pops = demand::Pops {
        country: &country,
        price: &price,
        income: &per_person,
        spend: &spend,
    };
    let sigma = inp.within_sigma();
    let w = demand::invert_pops(
        &inp.omega,
        &pops,
        &sigma,
        &inp.category,
        inp.sigma_demand,
        &inp.pigl,
    )
    .unwrap();
    let got = aggregate_shares(&inp, &w, &price, &spend);
    allclose("shares", &got.data, &omega_shares(&inp).data, 0.0, 1e-12);
}

/// ADR 0094: in the calibrated data state the pops' spending-weighted shares, each pop at its
/// province's buyer prices and its income, are each country's final demand shares Omega.
#[test]
fn aggregate_data_state_shares_match_omega() {
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let price = s.consumer.select_rows(&inp.pop_province);
        let got = aggregate_shares(inp, &s.weights, &price, &s.pop_income);
        allclose(w.name, &got.data, &omega_shares(inp).data, 0.0, 1e-12);
    }
}

/// ADR 0106: the passes reach the same fixed point from eight starts, every traded buyer price
/// of the data state times a factor uniform in [0.5, 1.5] (Philox seeds 0 to 7, one stream per
/// province). A failure means several steady states, not a flake.
#[test]
fn calibration_unique_from_eight_starts() {
    let inp = io::load(fixture()).unwrap();
    let base = steady::calibrate(&inp, &inp.u_star.clone(), &mut quiet()).unwrap();
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    for seed in 0..8u64 {
        let mut start = Mat::filled(n_p, n_g, 1.0);
        for p in 0..n_p {
            let mut draw = Seed(seed).stream(0, SystemId(0), Kind(0), EntityId(p as u32));
            for g in (0..n_g).filter(|&g| inp.traded[g]) {
                start[(p, g)] = inp.base_price[(p, g)] * (0.5 + draw.uniform());
            }
        }
        let s = steady::calibrate_from(
            &inp,
            &inp.u_star.clone(),
            Limits::default(),
            &start,
            &mut quiet(),
        )
        .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let what = format!("seed {seed}");
        allclose(&what, &s.price.data, &base.price.data, 1e-9, 0.0);
        allclose(&what, &s.consumer.data, &base.consumer.data, 1e-9, 0.0);
        allclose(&what, &s.r, &base.r, 1e-9, 0.0);
        allclose(
            &what,
            &s.output,
            &base.output,
            1e-9,
            1e-9 * max_abs(&base.output),
        );
        allclose(&what, &s.w0, &base.w0, 1e-9, 0.0);
        allclose(
            &what,
            &s.rebate,
            &base.rebate,
            1e-9,
            1e-9 * max_abs(&base.rebate),
        );
        allclose(&what, &s.weights.scale, &base.weights.scale, 1e-9, 0.0);
        allclose(
            &what,
            &s.weights.omega.data,
            &base.weights.omega.data,
            1e-9,
            1e-12,
        );
    }
}

/// Power iterations for the second eigenvalue of the Sinkhorn map's Jacobian. On the full pack
/// the estimate for coal and other mining settles within 1,000 iterations on one pack and keeps
/// moving by 2e-4 in 1 - lambda until about 3,000 on another (a second slow block close to the
/// first); 3,000 cost about a minute there.
const POWER_ITERATIONS: usize = 3_000;
/// Trailing iterations whose geometric-mean contraction is the estimate.
const POWER_WINDOW: usize = 500;

/// Per good, the second eigenvalue lambda2_g of M, the Jacobian of -ln reach with respect to ln w
/// over the active origins of traded good g at the scaling `lw` (P, G), by power iteration on the
/// mean-free part: M_oo' = sum_d (K_od / (1 + t_od) b_d / reach_o) (K_o'd w_o' / Phi_d), b_d =
/// E_d / Phi_d, is row-stochastic (M 1 = 1), so deflating the constant leaves its other
/// eigenvalues. 0 for a good with at most one origin and for local goods.
fn slow_rates(m: &Model, spend: &Mat, lw: &Mat) -> Vec<f64> {
    let inp = m.inp;
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    let n = n_p * n_g;
    let origin: Vec<bool> = (0..n)
        .map(|i| inp.traded[i / n_p] && inp.active[i % n_p][i / n_p])
        .collect();
    let lwv = model::good_major(lw);
    let mut top = vec![f64::NEG_INFINITY; n_g];
    for i in (0..n).filter(|&i| origin[i]) {
        top[i / n_p] = top[i / n_p].max(lwv[i]);
    }
    let w: Vec<f64> = (0..n)
        .map(|i| {
            if origin[i] {
                (lwv[i] - top[i / n_p]).exp()
            } else {
                0.0
            }
        })
        .collect();
    let e: Vec<f64> = model::good_major(spend)
        .into_iter()
        .enumerate()
        .map(|(i, x)| if inp.traded[i / n_p] { x } else { 0.0 })
        .collect();
    let net: Vec<f64> = inp
        .crossing
        .k
        .iter()
        .zip(&inp.crossing.inv_duty)
        .map(|(k, d)| k * d)
        .collect();
    let gross = Factors {
        dom: &inp.domestic.k,
        acc: &inp.access.k,
        cro: &inp.crossing.k,
    };
    let net = Factors {
        dom: &inp.domestic.k,
        acc: &inp.access.k,
        cro: &net,
    };
    let phi = m.origin_sums(&w, gross);
    let b: Vec<f64> = (0..n)
        .map(|i| if phi[i] > 0.0 { e[i] / phi[i] } else { 0.0 })
        .collect();
    let reach = m.destination_sums(&b, net);
    let apply = |x: &[f64]| -> Vec<f64> {
        let wx: Vec<f64> = w.iter().zip(x).map(|(w, x)| w * x).collect();
        let y = m.origin_sums(&wx, gross);
        let by: Vec<f64> = (0..n)
            .map(|i| {
                if phi[i] > 0.0 {
                    b[i] * y[i] / phi[i]
                } else {
                    0.0
                }
            })
            .collect();
        let z = m.destination_sums(&by, net);
        (0..n)
            .map(|i| if origin[i] { z[i] / reach[i] } else { 0.0 })
            .collect()
    };
    let cells = |g: usize| (g * n_p..(g + 1) * n_p).filter(|&i| origin[i]);
    // Deflates the constant and scales each good's part to unit max-norm; returns the norms.
    let normalise = |x: &mut [f64]| -> Vec<f64> {
        (0..n_g)
            .map(|g| {
                let k = cells(g).count();
                if k == 0 {
                    return 0.0;
                }
                let mean = cells(g).map(|i| x[i]).sum::<f64>() / k as f64;
                let norm = cells(g).fold(0.0f64, |a, i| a.max((x[i] - mean).abs()));
                for i in cells(g) {
                    x[i] = if norm > 0.0 {
                        (x[i] - mean) / norm
                    } else {
                        0.0
                    };
                }
                norm
            })
            .collect()
    };
    let mut rng = Rng(17);
    let mut x: Vec<f64> = (0..n)
        .map(|i| if origin[i] { rng.normal(0.0, 1.0) } else { 0.0 })
        .collect();
    normalise(&mut x);
    let mut log_rate = vec![0.0; n_g];
    for k in 1..=POWER_ITERATIONS {
        x = apply(&x);
        let norm = normalise(&mut x);
        if k > POWER_ITERATIONS - POWER_WINDOW {
            for (l, nrm) in log_rate.iter_mut().zip(norm) {
                *l += nrm.ln();
            }
        }
    }
    log_rate
        .iter()
        .map(|l| (l / POWER_WINDOW as f64).exp())
        .collect()
}

/// The longest chain of additions in one of `Model::origin_sums` or `Model::destination_sums`
/// for traded good g, the depth D_g of the rounding derivation in
/// [`sinkhorn_is_unique_up_to_scale`]: a cell's sum is its own term plus the domestic legs into
/// it plus the access legs into it of the gateway sums, each of which sums crossings of access
/// sums, so D_g = max(m_dom, m_gate + m_cross + m_prov) + 2, each m the most legs of good g in
/// one bucket of that sum (by origin or by destination, whichever is larger).
fn summation_depth(inp: &Inputs, g: usize) -> f64 {
    let most = |good: &[usize], key: &[usize]| -> usize {
        let mut count = std::collections::BTreeMap::new();
        for (_, &k) in good.iter().zip(key).filter(|&(&h, _)| h == g) {
            *count.entry(k).or_insert(0usize) += 1;
        }
        count.into_values().max().unwrap_or(0)
    };
    let (d, a, c) = (&inp.domestic, &inp.access, &inp.crossing);
    let dom = most(&d.good, &d.row).max(most(&d.good, &d.col));
    let cross = most(&c.good, &c.row).max(most(&c.good, &c.col));
    let chain = most(&a.good, &a.col) + cross + most(&a.good, &a.row);
    (dom.max(chain) + 2) as f64
}

/// Stage 2: from two starts the scaling reaches the same weights up to one scale per good.
///
/// The bound on the spread is derived from the stop rule. Let g(ln w) = ln received - ln target
/// over a good's origins; each solve stops with max |g| <= tau = `SINKHORN_TOL` as computed. Near
/// the fixed point, two solutions differing by d (ln w) have g_1 - g_2 = (I - M) d to first order
/// (r agrees to 1e-16 between them, so the target moves by no constant), M the row-stochastic
/// Jacobian of `slow_rates`. M 1 = 1 carries the free scale; on the mean-free part d = sum_k M^k
/// (g_1 - g_2), whose terms shrink by lambda2_g a step once the slowest mode dominates, so
/// spread(d) <= 2 (tau + delta_g) / (1 - lambda2_g), delta_g the rounding of the computed gap
/// against the exact one at the same stored ln w. A near-closed block of origins sets lambda2_g:
/// on the full pack Brazil's 48 origins of coal and other mining (good 2) keep 0.999 of their
/// Sinkhorn coupling, lambda2 = 0.99908, so the first-order bound is 2.2e-10 where the observed
/// spread is 2.1e-10. The factor 1.5 covers the first-order and asymptotic steps and the
/// estimate of lambda2_g; it is a test margin, not a parameter.
///
/// delta_g, from the operations of the step that computes the gap (u = eps / 2, first order;
/// libm's exp and ln within 1 ulp, 2u relative; L = max |ln w| over the good's origins in either
/// solution, T = max |ln (r v)| over them, D = [`summation_depth`]):
/// - w = exp(ln w - top): the subtraction rounds by u |ln w - top| <= 2uL, exp by 2u, so w has
///   relative error e_w <= 2uL + 2u;
/// - Phi, a sum of positive terms each with relative error e_w, D additions and one product per
///   level deep: e_w + (D + 1)u; b = E / Phi one more u; reach, the same sums over b with the
///   net factors (one more product): e_w + (2D + 3)u; got = w reach: 2 e_w + (2D + 4)u;
/// - ln got rounds by 2u |ln got|, and the target ln (r v) by u + 2u |ln (r v)|, with |ln got|
///   within tau of |ln (r v)| <= T; r itself is common to the good's origins and recomputed from
///   the same got, so its rounding moves no origin against another and is left out;
/// - delta_g <= 4uL + 4uT + (2D + 9)u = eps (2L + 2T + D + 4.5).
///
/// So the bound is 1.5 x 2 (tau + delta_g) / (1 - lambda2_g): the stop-rule term and a rounding
/// term 3 eps (2L + 2T + D + 4.5) / (1 - lambda2_g), whose part in L (the update's subtraction
/// and exp) is k eps L with k = 6 / (1 - lambda2_g). The sums' term is a worst case ((m - 1)u for
/// m positive terms) and dominates on the full pack, where one gateway's access legs make D =
/// 858: there the rounding term is about twice the stop-rule term for every good, and good 2's
/// bound is 1.0e-9 against an observed 2.1e-10. The well-conditioned goods (lambda2 0.39 to
/// 0.44) sit up to 8.3e-14 above the unscaled stop-rule term (3e-13 to 4.3e-13 against 3.3e-13
/// to 3.6e-13); the part in L alone is about 3e-14 there, the targets' about 5e-14.
#[test]
fn sinkhorn_is_unique_up_to_scale() {
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
        let m = Model::new(inp).unwrap();
        let mut v = Mat::zeros(n_p, n_g);
        for (e, (&p, &g)) in inp.ent_province.iter().zip(&inp.ent_good).enumerate() {
            // the stage 2 target is the sales value, V / (1 + delta s) (ADR 0091)
            v[(p, g)] += if inp.traded[g] {
                s.value[e] / s.r[g] / (1.0 + s.spoilage[g])
            } else {
                0.0
            };
        }
        let spend = Mat::from_fn(n_p, n_g, |p, g| s.spend_hh[(p, g)] + s.spend_int[(p, g)]);
        let origin = |p: usize, g: usize| inp.active[p][g] && inp.traded[g];
        let mut rng = Rng(9);
        let mut start = Mat::zeros(n_p, n_g);
        for p in 0..n_p {
            for g in (0..n_g).filter(|&g| origin(p, g)) {
                start[(p, g)] = rng.normal(0.0, 3.0);
            }
        }
        let one = steady::sinkhorn(&m, &spend, &v, None).unwrap();
        let other = steady::sinkhorn(&m, &spend, &v, Some(&start)).unwrap();
        let lambda2 = slow_rates(&m, &spend, &one.log_weight);
        for g in (0..n_g).filter(|&g| inp.traded[g]) {
            let d: Vec<f64> = (0..n_p)
                .filter(|&p| origin(p, g))
                .map(|p| one.log_weight[(p, g)] - other.log_weight[(p, g)])
                .collect();
            let mean = d.iter().sum::<f64>() / d.len() as f64;
            let spread = d.iter().fold(0.0f64, |m, x| m.max((x - mean).abs()));
            let cells = || (0..n_p).filter(|&p| origin(p, g));
            let big = cells().fold(0.0f64, |a, p| {
                a.max(one.log_weight[(p, g)].abs())
                    .max(other.log_weight[(p, g)].abs())
            });
            let target = cells().fold(0.0f64, |a, p| a.max((one.r[g] * v[(p, g)]).ln().abs()));
            let depth = summation_depth(inp, g);
            let stop = 1.5 * 2.0 * SINKHORN_TOL / (1.0 - lambda2[g]);
            let rounding = 1.5 * 2.0 * f64::EPSILON * (2.0 * big + 2.0 * target + depth + 4.5)
                / (1.0 - lambda2[g]);
            let bound = stop + rounding;
            eprintln!(
                "{}: good {g} lambda2 {:.7} spread {spread:.3e} stop {stop:.3e} rounding \
                 {rounding:.3e} (L {big:.2}, T {target:.2}, D {depth}) bound {bound:.3e}",
                w.name, lambda2[g]
            );
            assert!(
                spread <= bound,
                "{}: good {g} spread {spread} above {bound} (lambda2 {})",
                w.name,
                lambda2[g]
            );
        }
        allclose(w.name, &one.r, &other.r, 1e-12, 0.0);
        let traded: Vec<usize> = (0..n_g).filter(|&g| inp.traded[g]).collect();
        let pick = |r: &[f64]| traded.iter().map(|&g| r[g]).collect::<Vec<_>>();
        allclose(w.name, &pick(&one.r), &pick(&s.r), 1e-12, 0.0);
    }
}

/// Stage 3: at the calibrated prices and base wages, with A = 1 / p_VA, coef = a_kg p_o / q_ik
/// and L0 = s_L VA0 / w0, the reference model's residual vanishes (every market clears, labour
/// at (1 - u*) L^s, the world price index 1), and VA, employment and output are the data's.
#[test]
fn closed_forms_zero_the_reference_residual() {
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let reference = steady::reference_inputs(inp, s);
        let x = steady::reference_point(inp, s);
        let m = Model::new(&reference).unwrap();
        let residual = max_abs(&model::residual(&x, &m));
        assert!(residual <= 1e-10, "{}: residual {residual}", w.name);
        let e = model::evaluate(&x, &m, &reference.ent_a);
        allclose(w.name, &e.value_added, &s.va0, 1e-10, 0.0);
        allclose(w.name, &e.employment, &s.l0, 1e-10, 0.0);
        allclose(w.name, &e.output, &s.output, 1e-10, 0.0);
        let target = inp.ent_va_target.as_ref().unwrap();
        let traded: Vec<usize> = (0..inp.ent_good.len())
            .filter(|&e| inp.traded[inp.ent_good[e]])
            .collect();
        let va0: Vec<f64> = traded.iter().map(|&e| s.va0[e]).collect();
        let want: Vec<f64> = traded
            .iter()
            .map(|&e| s.r[inp.ent_good[e]] * target[e])
            .collect();
        allclose(w.name, &va0, &want, 1e-13, 0.0);
        assert!(s.a.iter().all(|a| a.is_finite() && *a > 0.0), "{}", w.name);
        assert!(s.a_va.iter().all(|a| *a > 0.0), "{}", w.name);
    }
}

/// ADR 0200 (Amendment 2026-10-30): with buyers at Gamma P_d, Gamma = q / P_d per cell from
/// the calibration, the calibrated point is the same steady state as with buyers at the
/// delivered unit price of the physical arrivals: buyer prices, spending, incomes, value added,
/// orders and tariffs (stage 1's values) agree to 1e-12.
#[test]
fn steady_state_unchanged_by_selection() {
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let reference = steady::reference_inputs(inp, s);
        let x = steady::reference_point(inp, s);
        let selection = Model::new(&reference).unwrap();
        let physical = Model::physical(&reference).unwrap();
        let on = model::evaluate(&x, &selection, &reference.ent_a);
        let off = model::evaluate(&x, &physical, &reference.ent_a);
        let finite = |m: &Mat| -> Vec<f64> {
            m.data
                .iter()
                .map(|&v| if v.is_finite() { v } else { 0.0 })
                .collect()
        };
        let src = selection.sourcing(&on.price);
        for (i, &g) in s.gamma.data.iter().enumerate() {
            let (q, ces) = (s.consumer.data[i], src.ces.data[i]);
            if q.is_finite() && ces.is_finite() && inp.traded[i % inp.n_goods()] {
                allclose(w.name, &[g * ces], &[q], 1e-12, 0.0);
            }
        }
        allclose(
            w.name,
            &finite(&on.consumer_price),
            &finite(&s.consumer),
            1e-12,
            0.0,
        );
        allclose(
            w.name,
            &finite(&on.consumer_price),
            &finite(&off.consumer_price),
            1e-12,
            0.0,
        );
        allclose(w.name, &on.spend.data, &off.spend.data, 1e-12, 0.0);
        allclose(w.name, &on.income, &off.income, 1e-12, 0.0);
        allclose(w.name, &on.value_added, &off.value_added, 1e-12, 0.0);
        allclose(w.name, &on.output, &off.output, 1e-12, 0.0);
        allclose(
            w.name,
            &on.flows.orders.data,
            &off.flows.orders.data,
            1e-12,
            0.0,
        );
        allclose(
            w.name,
            &on.flows.tariff.data,
            &off.flows.tariff.data,
            1e-12,
            0.0,
        );
        allclose(
            w.name,
            &on.flows.arrive.data,
            &off.flows.arrive.data,
            1e-12,
            0.0,
        );
    }
}

/// Local goods at p = 1: output solves each province's Leontief system against household and
/// intermediate local demand.
#[test]
fn local_output_is_nonnegative_and_clears_locally() {
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
        let (ep, eg) = (&inp.ent_province, &inp.ent_good);
        for e in (0..eg.len()).filter(|&e| !inp.traded[eg[e]]) {
            assert!(s.output[e] >= 0.0, "{}: enterprise {e}", w.name);
        }
        let mut used = Mat::zeros(n_p, n_g);
        let mut out = Mat::zeros(n_p, n_g);
        for e in 0..eg.len() {
            for k in 0..n_g {
                used[(ep[e], k)] += s.coef[(e, k)] * s.output[e];
            }
            out[(ep[e], eg[e])] += s.output[e];
        }
        let (mut got, mut want) = (Vec::new(), Vec::new());
        for p in 0..n_p {
            for g in (0..n_g).filter(|&g| !inp.traded[g]) {
                got.push(out[(p, g)]);
                want.push(s.spend_hh[(p, g)] + used[(p, g)]);
            }
        }
        allclose(w.name, &got, &want, 1e-12, 0.0);
    }
}

/// With national profit ownership every province's households earn dividends, so every local
/// enterprise produces and every province has employment and a positive base wage.
#[test]
fn every_province_produces_and_pays_a_wage() {
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let eg = &inp.ent_good;
        assert!(
            (0..eg.len())
                .filter(|&e| !inp.traded[eg[e]])
                .all(|e| s.output[e] > 0.0),
            "{}",
            w.name
        );
        let mut va = vec![0.0; inp.n_provinces()];
        for (&p, v) in inp.ent_province.iter().zip(&s.va0) {
            va[p] += v;
        }
        assert!(va.iter().all(|&v| v > 0.0), "{}", w.name);
        assert!(s.w0.iter().all(|w| w.is_finite() && *w > 0.0), "{}", w.name);
        assert!(s.l0.iter().all(|l| l.is_finite() && *l > 0.0), "{}", w.name);
    }
}

/// The fixture with competitive storage on at the approved values (ADR 0091), as an export of
/// those parameters would carry them, calibrated once.
fn fixture_with_storage() -> &'static (Inputs, Steady) {
    static WORLD: OnceLock<(Inputs, Steady)> = OnceLock::new();
    WORLD.get_or_init(|| {
        let on = TradeParams::default().with_storage();
        let mut inp = io::load(fixture()).unwrap();
        inp.storage_loss = on.storage_loss.to_vec();
        inp.storage = io::Storage {
            cost: on.storage_cost.to_vec(),
            speculation: on.speculation.to_vec(),
            convenience: on.convenience.to_vec(),
            gain: on.storage_gain,
        };
        let s = steady::calibrate(&inp, &inp.u_star.clone(), &mut quiet()).unwrap();
        (inp, s)
    })
}

/// Stage 5: at the start of a tick each input stock is (1 + COVER) months of use and the pipeline
/// holds, j ticks ahead, one month of shipments of every arrival slot j or later, so the
/// order-up-to rule of E3 orders exactly one month of use; merchant stocks are coverage times the
/// orders, and output covers the orders and the stock's spoilage (ADR 0091); pops hold one month
/// of income less this tick's wages.
#[test]
fn stage5_state_is_stationary_under_the_stock_rules() {
    // the fixture with competitive storage on at the approved values (ADR 0091), beside the
    // default worlds, where it is off
    let (inp, s) = fixture_with_storage();
    assert!(s.spoilage.iter().any(|&x| x > 0.0));
    stage5_holds("fixture with storage", inp, s);
    for w in worlds() {
        stage5_holds(w.name, &w.inp, &w.s);
    }
}

fn stage5_holds(name: &str, inp: &Inputs, s: &Steady) {
    {
        let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
        let n_e = inp.ent_good.len();
        let ep = &inp.ent_province;
        let traded = &inp.traded;
        let used = Mat::from_fn(n_e, n_g, |e, k| s.coef[(e, k)] * s.output[e]);
        let want_stock: Vec<f64> = used.data.iter().map(|u| (1.0 + COVER) * u).collect();
        let stock = s.stock();
        allclose(name, &stock.data, &want_stock, 1e-15, 0.0);
        assert_eq!(s.arrive.len(), SLOTS + 1);
        // arrive[e][g][slot]: units arriving per month in each slot.
        let arrive =
            |e: usize, g: usize, slot: usize| s.arrive[slot][(ep[e], g)] * s.ent_spend[(e, g)];
        let ahead = |e: usize, g: usize| s.ent_pipeline[e * n_g + g];
        let use_max = used.data.iter().fold(0.0f64, |m, &u| m.max(u));
        let (mut got, mut want, mut order, mut month, mut first, mut later) =
            (vec![], vec![], vec![], vec![], vec![], vec![]);
        for e in 0..n_e {
            for g in (0..n_g).filter(|&g| traded[g]) {
                let total: f64 = (0..=SLOTS).map(|j| arrive(e, g, j)).sum();
                got.push(total);
                want.push(used[(e, g)]);
                let target = (1.0 + COVER + s.lead[(ep[e], g)]) * used[(e, g)];
                let held = stock[(e, g)] - arrive(e, g, 0);
                let on_order: f64 = ahead(e, g)[1..].iter().sum();
                order.push(target - held - on_order);
                month.push(used[(e, g)]);
                first.push(ahead(e, g)[0]);
                later.push((1..=SLOTS).map(|j| arrive(e, g, j)).sum());
            }
            for g in (0..n_g).filter(|&g| !traded[g]) {
                assert!(ahead(e, g).iter().all(|&u| u == 0.0), "{}", name);
            }
        }
        allclose(name, &got, &want, 1e-12, 0.0);
        allclose(name, &order, &month, 1e-10, 1e-12 * use_max);
        allclose(name, &first, &later, 1e-12, 0.0);
        // merchant stocks at coverage times the orders, and output covering the orders and the
        // stock's spoilage, Q = D + delta S (ADR 0091): the storage order is delta S at z = 1
        let inventory = s.inventory();
        let (mut stock_got, mut stock_want, mut q_got, mut q_want) =
            (vec![], vec![], vec![], vec![]);
        for p in 0..n_p {
            for g in (0..n_g).filter(|&g| traded[g] && inp.active[p][g]) {
                let d = s.orders[0][(p, g)] + s.orders[1][(p, g)];
                stock_got.push(inventory[(p, g)]);
                stock_want.push(s.coverage[g] * d);
                q_got.push(s.supply[(p, g)]);
                q_want.push(d + inp.storage_loss[g] * inventory[(p, g)]);
            }
        }
        allclose(name, &stock_got, &stock_want, 1e-12, 0.0);
        allclose(name, &q_got, &q_want, 1e-12, 0.0);
        let money: Vec<f64> = (0..s.pop_income.len())
            .map(|k| s.pop_income[k] - s.pop_wage[k])
            .collect();
        allclose(name, &s.pop_money(), &money, 1e-15, 0.0);
        allclose(name, &s.pop_spend.row_sums(), &s.pop_income, 1e-12, 0.0);
    }
}

fn read(dir: &Path, name: &str) -> RecordBatch {
    let file = File::open(dir.join(name)).unwrap();
    let mut batches: Vec<RecordBatch> = FileReader::try_new(file, None)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(batches.len(), 1, "{name}: one batch");
    batches.pop().unwrap()
}

fn f64s(t: &RecordBatch, col: &str) -> Vec<f64> {
    t.column_by_name(col)
        .unwrap()
        .as_primitive::<Float64Type>()
        .values()
        .to_vec()
}

fn u32s(t: &RecordBatch, col: &str) -> Vec<usize> {
    let a = t.column_by_name(col).unwrap();
    assert_eq!(a.data_type(), &DataType::UInt32, "{col}");
    a.as_primitive::<UInt32Type>()
        .values()
        .iter()
        .map(|&v| v as usize)
        .collect()
}

fn u8s(t: &RecordBatch, col: &str) -> Vec<usize> {
    let a = t.column_by_name(col).unwrap();
    assert_eq!(a.data_type(), &DataType::UInt8, "{col}");
    a.as_primitive::<UInt8Type>()
        .values()
        .iter()
        .map(|&v| usize::from(v))
        .collect()
}

#[test]
fn calibration_tables_have_the_documented_layout() {
    let w = &worlds()[0];
    let (inp, s) = (&w.inp, &w.s);
    let n_g = inp.n_goods();
    let tmp = tempfile::tempdir().unwrap();
    let pack: Vec<usize> = (0..inp.n_provinces()).map(|p| 2 * p + 1).collect();
    write::write(tmp.path(), inp, s, &pack).unwrap();

    let ent = read(tmp.path(), "econ_calib.arrow");
    let names: Vec<String> = ent
        .schema()
        .fields()
        .iter()
        .map(|f| f.name().clone())
        .collect();
    assert_eq!(names[..3], ["province", "good", "a"]);
    let key: Vec<(usize, usize)> = u32s(&ent, "province")
        .into_iter()
        .zip(u8s(&ent, "good"))
        .collect();
    let mut sorted = key.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(key, sorted);
    assert_eq!(key.len(), inp.ent_good.len());
    let want: Vec<usize> = inp.ent_province.iter().map(|&p| pack[p]).collect();
    assert_eq!(u32s(&ent, "province"), want);
    assert_eq!(f64s(&ent, "a"), s.a);
    for k in 0..n_g {
        let want: Vec<f64> = (0..s.coef.rows).map(|e| s.coef[(e, k)]).collect();
        assert_eq!(f64s(&ent, &format!("coef_{k}")), want);
    }
    let meta: serde_json::Value =
        serde_json::from_str(&ent.schema().metadata()["calibration"]).unwrap();
    assert_eq!(meta["format"], steady::FORMAT);
    assert_eq!(meta["r"].as_array().unwrap().len(), n_g);

    let cells = read(tmp.path(), "econ_calib_cells.arrow");
    assert_eq!(cells.num_rows(), inp.n_provinces() * n_g);
    assert_eq!(f64s(&cells, "consumer_price"), s.consumer.data);
    let pops = read(tmp.path(), "econ_calib_pops.arrow");
    let key: Vec<(usize, usize)> = u32s(&pops, "province")
        .into_iter()
        .zip(u8s(&pops, "edu"))
        .collect();
    let mut sorted: Vec<(usize, usize)> = inp
        .pop_province
        .iter()
        .map(|&p| pack[p])
        .zip(u8s(&pops, "edu"))
        .collect();
    sorted.sort_unstable();
    assert_eq!(key, sorted);
    let pipe = read(tmp.path(), "econ_calib_pipeline.arrow");
    assert!(u8s(&pipe, "ahead").iter().all(|a| (1..=4).contains(a)));
    let units = f64s(&pipe, "units");
    assert!(units.iter().all(|&u| u > 0.0));
    let total: f64 = s
        .pop_pipeline
        .iter()
        .chain(&s.ent_pipeline)
        .flatten()
        .sum::<f64>();
    let got: f64 = units.iter().sum();
    assert!(
        (got - total).abs() <= 1e-12 * total.abs(),
        "{got} vs {total}"
    );
    let countries = read(tmp.path(), "econ_calib_countries.arrow");
    assert_eq!(f64s(&countries, "u_star"), inp.u_star);
    let provinces = read(tmp.path(), "econ_calib_provinces.arrow");
    assert_eq!(f64s(&provinces, "w0"), s.w0);
    for name in [
        "econ_calib.arrow",
        "econ_calib_cells.arrow",
        "econ_calib_provinces.arrow",
        "econ_calib_pops.arrow",
        "econ_calib_pipeline.arrow",
        "econ_calib_countries.arrow",
    ] {
        let t = read(tmp.path(), name);
        assert_eq!(
            t.schema().metadata()["calibration_format"],
            steady::FORMAT,
            "{name}"
        );
        assert!(
            t.schema().fields().iter().all(|f| f.is_nullable()),
            "{name}"
        );
    }
}

/// The committed fixture calibration (`tests/golden/p2-fixture/calibration`, which the Rust
/// property tests read) is the one this crate writes, within the tolerance of an iterative solve.
#[test]
fn fixture_tables_match_the_committed_calibration() {
    let w = &worlds()[0];
    let tmp = tempfile::tempdir().unwrap();
    let provinces: Vec<usize> = (0..w.inp.n_provinces()).collect();
    write::write(tmp.path(), &w.inp, &w.s, &provinces).unwrap();
    let committed = fixture().join("calibration");
    let report = compare::tables(&committed, tmp.path(), Tolerance::ITERATIVE).unwrap();
    assert!(report.ok, "{:#?}", report.lines());
    let meta = |dir: &Path| -> serde_json::Value {
        let t = read(dir, "econ_calib.arrow");
        serde_json::from_str(&t.schema().metadata()["calibration"]).unwrap()
    };
    let (got, want) = (meta(tmp.path()), meta(&committed));
    for key in ["format", "static_digest"] {
        assert_eq!(got[key], want[key], "{key}");
    }
}

/// The reference read back from written tables is the one built from the calibration, and
/// tables of another format or of other inputs are refused.
#[test]
fn read_reference_round_trips_the_fixture_tables() {
    let w = &worlds()[0];
    let tmp = tempfile::tempdir().unwrap();
    let provinces: Vec<usize> = (0..w.inp.n_provinces()).collect();
    write::write(tmp.path(), &w.inp, &w.s, &provinces).unwrap();
    let (got, x) = write::read_reference(tmp.path(), &w.inp, &provinces).unwrap();
    let want = steady::reference_inputs(&w.inp, &w.s);
    assert_eq!(x, steady::reference_point(&w.inp, &w.s));
    assert_eq!(got.ent_a, want.ent_a);
    assert_eq!(got.ent_va0, want.ent_va0);
    assert_eq!(got.ent_l0, want.ent_l0);
    assert_eq!(got.w0, want.w0);
    assert_eq!(got.u_star, want.u_star);
    assert_eq!(got.calibrated_coef, want.calibrated_coef);
    assert_eq!(got.calibrated_a_va, want.calibrated_a_va);
    assert_eq!(got.calibrated_weights, want.calibrated_weights);

    let mut other = w.inp.clone();
    other.static_digest = "another export".into();
    let err = write::read_reference(tmp.path(), &other, &provinces).unwrap_err();
    assert_eq!(err, "the calibration tables belong to other inputs");

    let path = tmp.path().join("econ_calib_provinces.arrow");
    let t = read(tmp.path(), "econ_calib_provinces.arrow");
    let mut metadata = t.schema().metadata().clone();
    metadata.insert("calibration_format".to_owned(), "0".to_owned());
    let schema = std::sync::Arc::new(t.schema().as_ref().clone().with_metadata(metadata));
    let t = RecordBatch::try_new(schema.clone(), t.columns().to_vec()).unwrap();
    let mut out =
        arrow_ipc::writer::FileWriter::try_new(File::create(&path).unwrap(), &schema).unwrap();
    out.write(&t).unwrap();
    out.finish().unwrap();
    let err = write::read_reference(tmp.path(), &w.inp, &provinces).unwrap_err();
    assert_eq!(
        err,
        format!(
            "econ_calib_provinces.arrow: calibration format is not {}",
            steady::FORMAT
        )
    );
}

/// A calibration that does not settle reports the passes it ran and the last change, and
/// writes no table; so does a scaling or an income fixed point that does not converge.
#[test]
fn nonconverging_calibration_reports_and_writes_nothing() {
    let inp = &worlds()[0].inp;
    let provinces: Vec<usize> = (0..inp.n_provinces()).collect();
    let tmp = tempfile::tempdir().unwrap();
    let run = |limits: Limits| {
        let out = tmp.path().join("out");
        let mut lines = Vec::new();
        let result = ceteris_calibrate::run(
            inp,
            &inp.u_star,
            &provinces,
            &out,
            limits,
            &mut |l: &str| lines.push(l.to_owned()),
        );
        assert!(
            !out.exists(),
            "a failed calibration wrote {}",
            out.display()
        );
        (result.unwrap_err(), lines)
    };
    let (err, lines) = run(Limits {
        max_passes: 3,
        ..Limits::default()
    });
    let CalibrationError::NotSettled { passes, change } = err.clone() else {
        panic!("{err}");
    };
    assert_eq!(passes, 3);
    assert!(change > PASS_TOL);
    assert_eq!(lines.len(), 3);
    assert!(lines[2].starts_with("pass 3: change ") && lines[2].contains(", residual "));
    assert_eq!(
        err.to_string(),
        format!(
            "passes did not settle in 3, last change {}",
            steady::sci(change)
        )
    );
    let (err, _) = run(Limits {
        max_sweeps: 2,
        ..Limits::default()
    });
    assert!(
        err.to_string()
            .starts_with("Sinkhorn did not converge in 2 sweeps, residual "),
        "{err}"
    );
    let (err, _) = run(Limits {
        max_local: 1,
        ..Limits::default()
    });
    assert_eq!(
        err.to_string(),
        "income and local output did not settle in 1 iterations"
    );
}

/// `EconInputs::from_pack` loads the tables this crate writes for the pack: a copy of the pack
/// (its files linked) with the Rust tables in place of its calibration.
#[cfg(unix)]
#[test]
fn pack_world_loads_the_written_tables() {
    let Some(w) = worlds().iter().find(|w| w.name == "pack") else {
        eprintln!("skipping: CETERIS_PACK_EXPORT unset (a full-pack export directory)");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let copy = tmp.path().join("pack");
    std::fs::create_dir(&copy).unwrap();
    let pack = std::fs::canonicalize(pack()).unwrap();
    for entry in std::fs::read_dir(&pack).unwrap() {
        let name = entry.unwrap().file_name();
        if !name.to_string_lossy().starts_with("econ_calib") {
            std::os::unix::fs::symlink(pack.join(&name), copy.join(&name)).unwrap();
        }
    }
    write::write(&copy, &w.inp, &w.s, w.pack_province.as_ref().unwrap()).unwrap();
    let graph =
        ceteris_geo::pack::Pack::load(&copy, &ceteris_geo::graph::TransportParams::default())
            .unwrap();
    let inputs = ceteris_econ::EconInputs::from_pack(&graph, &copy).unwrap();
    let steady = inputs.steady.as_ref().expect("a calibrated steady state");
    assert_eq!(steady.output, w.s.output);
    assert_eq!(
        inputs.enterprises.a, w.s.a,
        "the enterprises' A are the calibrated ones"
    );
}

/// The traded goods' theta classes of the fixture.
fn traded_classes(inp: &Inputs) -> Vec<usize> {
    let n_traded = inp.traded.iter().filter(|&&t| t).count();
    inp.theta_class[..n_traded].to_vec()
}

/// Shares of `groups` at nu `per_group`, stage 2 at the data state, as the fit evaluates them.
fn shares_at(inp: &Inputs, groups: &[Vec<usize>], per_group: &[f64], icio: &[bool]) -> Vec<f64> {
    let nu = border::nu_of_goods(groups, per_group, &inp.border_nu);
    border::data_state_shares(inp, &nu, groups, icio, Limits::default()).unwrap()
}

/// ADR 0276: the friction groups are the theta classes, class 3 split into its goods.
#[test]
fn friction_groups_split_class_3_per_good() {
    let inp = io::load(fixture()).unwrap();
    let class = traded_classes(&inp);
    assert_eq!(
        border::class_groups(&class),
        vec![vec![3, 5], vec![6], vec![0, 4], vec![1, 2]]
    );
    assert_eq!(
        border::friction_groups(&class),
        vec![vec![3, 5], vec![6], vec![0, 4], vec![1], vec![2]]
    );
}

/// ADRs 0092, 0276: on the fixture, the border friction fitted to cross-border shares that the
/// model produces at known nu returns those shares per friction group (1e-6) and those nu, goods
/// 1 and 2 of class 3 each at its own.
#[test]
fn nu_fit_matches_cross_border_share() {
    let inp = io::load(fixture()).unwrap();
    let icio = vec![true; inp.n_countries()];
    let groups = border::friction_groups(&traded_classes(&inp));
    let known = [0.15, 0.6, 0.35, 1.2, 0.5];
    let target = shares_at(&inp, &groups, &known, &icio);
    let fit = border::fit_border_nu(
        &inp,
        &groups,
        &target,
        &icio,
        Limits::default(),
        &mut quiet(),
    )
    .unwrap();
    println!("fit: {fit:?}");
    assert_eq!(fit.clamped, vec![false; groups.len()]);
    for k in 0..groups.len() {
        // relative, not the PLAN's absolute 1e-6: class 3's shares move by about 1e-7 per unit
        // of nu here, so an absolute 1e-6 would accept any nu from 0 to several
        assert!(
            (fit.share[k] / target[k] - 1.0).abs() <= 1e-6,
            "group {k}: share {} vs target {}",
            fit.share[k],
            target[k]
        );
        assert!(
            (fit.nu[k] - known[k]).abs() <= 1e-6 * known[k],
            "group {k}: nu {} vs {}",
            fit.nu[k],
            known[k]
        );
    }
    let per_good = fit.nu_of_goods(&inp.border_nu);
    assert_eq!((per_good[1], per_good[2]), (fit.nu[3], fit.nu[4]));
    assert_eq!((per_good[3], per_good[5]), (fit.nu[0], fit.nu[0]));
}

/// ADR 0276: a class with one good, or whose goods share a nu, fits as it did per class: at
/// targets made with goods 1 and 2 at one nu, the per-good fit gives classes 0 to 2 the class
/// fit's nu (to the root's resolution) and goods 1 and 2 class 3's.
#[test]
fn per_good_fit_keeps_the_class_fit_where_goods_agree() {
    let inp = io::load(fixture()).unwrap();
    let icio = vec![true; inp.n_countries()];
    let class = traded_classes(&inp);
    let (by_class, by_group) = (
        border::class_groups(&class),
        border::friction_groups(&class),
    );
    let known = [0.15, 0.6, 0.35, 1.2];
    let fit_class = border::fit_border_nu(
        &inp,
        &by_class,
        &shares_at(&inp, &by_class, &known, &icio),
        &icio,
        Limits::default(),
        &mut quiet(),
    )
    .unwrap();
    for (c, k) in known.iter().enumerate() {
        assert!(
            (fit_class.nu[c] - k).abs() <= 1e-6 * k,
            "class {c}: {fit_class:?}"
        );
    }
    let same = [0.15, 0.6, 0.35, 1.2, 1.2];
    let fit = border::fit_border_nu(
        &inp,
        &by_group,
        &shares_at(&inp, &by_group, &same, &icio),
        &icio,
        Limits::default(),
        &mut quiet(),
    )
    .unwrap();
    println!("per class {fit_class:?}\nper group {fit:?}");
    for c in 0..3 {
        assert!(
            (fit.nu[c] - fit_class.nu[c]).abs() <= 1e-8 * fit_class.nu[c],
            "class {c}: {} vs {}",
            fit.nu[c],
            fit_class.nu[c]
        );
    }
    for k in [3, 4] {
        assert!(
            (fit.nu[k] - fit_class.nu[3]).abs() <= 1e-6 * fit_class.nu[3],
            "good {:?}: {} vs class 3's {}",
            by_group[k],
            fit.nu[k],
            fit_class.nu[3]
        );
    }
}

/// ADRs 0092 (Amendment 2026-10-30), 0276: a good of class 3 whose own target is above its share
/// at nu 0 clamps at nu = 0 alone; the other good of the class still hits its own target.
#[test]
fn per_good_fit_clamps_one_good_of_class_3() {
    let inp = io::load(fixture()).unwrap();
    let icio = vec![true; inp.n_countries()];
    let groups = border::friction_groups(&traded_classes(&inp));
    let at_zero = shares_at(&inp, &groups, &[0.0; 5], &icio);
    let mut target = shares_at(&inp, &groups, &[0.3; 5], &icio);
    target[4] = 0.5 * (1.0 + at_zero[4]);
    let fit = border::fit_border_nu(
        &inp,
        &groups,
        &target,
        &icio,
        Limits::default(),
        &mut quiet(),
    )
    .unwrap();
    assert_eq!(fit.clamped, vec![false, false, false, false, true]);
    assert_eq!(fit.nu[4], 0.0);
    assert!(
        (fit.share[4] - at_zero[4]).abs() <= 1e-12 * at_zero[4],
        "{fit:?}"
    );
    for (k, t) in target.iter().enumerate().take(4) {
        assert!((fit.share[k] / t - 1.0).abs() <= 1e-6, "group {k}: {fit:?}");
        assert!((fit.nu[k] - 0.3).abs() <= 1e-6 * 0.3, "group {k}: {fit:?}");
    }
}

/// Each group's share falls as its nu rises and does not depend on the other groups' nu; a
/// target outside the shares at nu 0 and `NU_MAX` is an error naming the group.
#[test]
fn nu_fit_is_monotone_and_bracketed() {
    let inp = io::load(fixture()).unwrap();
    let icio = vec![true; inp.n_countries()];
    let groups = border::friction_groups(&traded_classes(&inp));
    let n = groups.len();
    let at = |nu: &[f64]| shares_at(&inp, &groups, nu, &icio);
    let steps = [0.0, 0.3, 1.0, 3.0];
    let shares: Vec<_> = steps.iter().map(|&v| at(&vec![v; n])).collect();
    for k in 0..n {
        for w in shares.windows(2) {
            assert!(w[1][k] < w[0][k], "group {k}: {shares:?}");
        }
    }
    // the groups are independent: moving class 2's nu or good 2's leaves the others' shares
    // unchanged, up to the scaling's tolerance (Anderson mixing runs over all goods at once)
    for moved_group in [2, 4] {
        let mut nu = vec![0.3; n];
        nu[moved_group] = 3.0;
        let moved = at(&nu);
        for k in (0..n).filter(|&k| k != moved_group) {
            assert!(
                (moved[k] - shares[1][k]).abs() <= 1e-12 * shares[1][k],
                "group {k} with {moved_group} moved: {} vs {}",
                moved[k],
                shares[1][k]
            );
        }
    }
    // below the share at the bracket's upper end
    let top = at(&vec![border::NU_MAX; n]);
    let mut target = shares[1].clone();
    target[2] = 0.5 * top[2];
    let err = border::fit_border_nu(
        &inp,
        &groups,
        &target,
        &icio,
        Limits::default(),
        &mut quiet(),
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.starts_with("group 2 (goods [0, 4]):") && err.contains("outside"),
        "{err}"
    );
}
/// ADR 0263: E of each traded origin against the finite difference of its orders in its own
/// price, at the calibrated point with buyers at Gamma P_d who keep their delivered units fixed
/// (Leontief, so each destination's spending moves with its buyer price), by central
/// differences. Where a pair is joined by several routes E leaves out the cross terms between
/// them: the fixture has none that matter, and E holds there to the finite difference's
/// precision (1.4e-10 relative); on the pack the gap is reported.
#[test]
fn own_price_elasticity_matches_finite_difference() {
    const H: f64 = 1e-6;
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let reference = steady::reference_inputs(inp, s);
        let m = Model::new(&reference).unwrap();
        let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
        let spend = Mat::from_fn(n_p, n_g, |p, g| {
            if inp.traded[g] {
                s.spend_int[(p, g)] + s.spend_hh[(p, g)]
            } else {
                0.0
            }
        });
        let orders = |price: &Mat| {
            let src = m.sourcing(price);
            let fixed_units = Mat::from_fn(n_p, n_g, |p, g| {
                let q = src.index[(p, g)];
                if q.is_finite() {
                    spend[(p, g)] * q / s.consumer[(p, g)]
                } else {
                    0.0
                }
            });
            m.flows(price, &fixed_units, &src).orders
        };
        let base = orders(&s.price);
        let (mut worst, mut n) = (0.0f64, 0);
        for p in 0..n_p {
            for g in (0..n_g).filter(|&g| inp.traded[g] && inp.active[p][g]) {
                if base[(p, g)] <= 0.0 {
                    continue;
                }
                let at = |h: f64| {
                    let mut price = s.price.clone();
                    price[(p, g)] *= h.exp();
                    orders(&price)[(p, g)]
                };
                let fd = -(at(H) / at(-H)).ln() / (2.0 * H);
                let e = s.demand_elasticity[(p, g)];
                assert!(
                    e >= 0.0 && e <= inp.theta[g] + 1.0,
                    "{} p{p} g{g}: E {e}",
                    w.name
                );
                worst = worst.max((e - fd).abs() / fd.abs().max(1.0));
                n += 1;
            }
        }
        eprintln!(
            "{}: E against the finite difference, {n} cells, worst {worst:.3e}",
            w.name
        );
        assert!(n > 0, "{}", w.name);
        if w.pack_province.is_none() {
            assert!(worst < 1e-8, "{}: worst {worst:e}", w.name);
        }
    }
}

/// ADR 0263: a traded cell that reaches no buyer with spending has Pi 0, so E = 1 + theta_g, the
/// study's value. With one traded good's spending set to 0 everywhere on the fixture, every
/// active cell of that good takes it and the other goods keep their calibrated E.
#[test]
fn traded_cell_with_no_buyer_has_the_study_elasticity() {
    let w = &worlds()[0];
    let (inp, s) = (&w.inp, &w.s);
    let reference = steady::reference_inputs(inp, s);
    let m = Model::new(&reference).unwrap();
    let src = m.sourcing(&s.price);
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    let none = (0..n_g)
        .find(|&g| inp.traded[g] && (0..n_p).filter(|&p| inp.active[p][g]).count() > 1)
        .unwrap();
    let spend = Mat::from_fn(n_p, n_g, |p, g| {
        if inp.traded[g] && g != none {
            s.spend_int[(p, g)] + s.spend_hh[(p, g)]
        } else {
            0.0
        }
    });
    let e = elasticity::demand(&m, &src, &spend);
    let mut cells = 0;
    for p in 0..n_p {
        for g in (0..n_g).filter(|&g| inp.active[p][g]) {
            if g == none {
                assert_eq!(e[(p, g)], 1.0 + inp.theta[g], "p{p} g{g}");
                cells += 1;
            } else {
                let want = s.demand_elasticity[(p, g)];
                assert!((e[(p, g)] - want).abs() <= 1e-12 * want.abs(), "p{p} g{g}");
            }
        }
    }
    assert!(cells > 1, "{cells} cells of good {none}");
}

/// ADR 0267: the cross-price entries of each block, E_ij = -(1 + theta) w_j T2_ij / t_i for two
/// origins of a good in one country, are the elasticity of the units ordered at i to the price
/// of j, against central finite differences of the orders of buyers that need fixed delivered
/// units. Like E they leave out the cross terms of routes that do not share their last leg: the
/// fixture has none that matter, and the entries hold there to the finite difference's
/// precision; on the pack the gap is reported.
#[test]
fn cross_price_elasticity_matches_finite_difference() {
    const H: f64 = 1e-6;
    let trade = ceteris_trade::TradeParams::default();
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let physical = Model::physical(inp).unwrap();
        let blocks = block::blocks(
            &physical,
            &physical.sourcing(&s.price),
            s,
            &inp.ent_s_l(),
            &ceteris_econ::EconParams::frictionless(),
            &trade,
        );
        let reference = steady::reference_inputs(inp, s);
        let m = Model::new(&reference).unwrap();
        let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
        let spend = Mat::from_fn(n_p, n_g, |p, g| {
            if inp.traded[g] {
                s.spend_int[(p, g)] + s.spend_hh[(p, g)]
            } else {
                0.0
            }
        });
        let orders = |price: &Mat| {
            let src = m.sourcing(price);
            let fixed_units = Mat::from_fn(n_p, n_g, |p, g| {
                let q = src.index[(p, g)];
                if q.is_finite() {
                    spend[(p, g)] * q / s.consumer[(p, g)]
                } else {
                    0.0
                }
            });
            m.flows(price, &fixed_units, &src).orders
        };
        let (mut worst, mut n) = (0.0f64, 0);
        for (cells, b) in &blocks {
            for &(i, j, e) in b.demand.iter().filter(|d| d.0 != d.1) {
                let (ci, cj) = (cells[i], cells[j]);
                let (pi, gi, pj) = (ci % n_p, ci / n_p, cj % n_p);
                assert_eq!(gi, cj / n_p, "{}: a cross entry joins one good", w.name);
                let at = |h: f64| {
                    let mut price = s.price.clone();
                    price[(pj, gi)] *= h.exp();
                    orders(&price)[(pi, gi)]
                };
                // the storage order moves with the orders of a stored good (ADR 0091)
                let fd = (at(H) / at(-H)).ln() / (2.0 * H) * trade.order_response(gi);
                assert!(e <= 0.0, "{} cells {ci} {cj}: E_ij {e}", w.name);
                worst = worst.max((-e - fd).abs() / fd.abs().max(1.0));
                n += 1;
            }
        }
        eprintln!(
            "{}: cross-price E against the finite difference, {n} pairs, worst {worst:.3e}",
            w.name
        );
        assert!(n > 0, "{}", w.name);
        if w.pack_province.is_none() {
            assert!(worst < 1e-8, "{}: worst {worst:e}", w.name);
        }
    }
}

/// ADR 0267: the input orders a block's slots carry, sum over slots of u_is sum_k v_ks, are the
/// share of market i's orders that its own country's enterprises place, against the flows of
/// that country's input spending alone (paths that leave the country and return are left out:
/// none on the fixture).
#[test]
fn input_order_shares_match_the_flows() {
    let trade = ceteris_trade::TradeParams::default();
    for w in worlds().iter().filter(|w| w.pack_province.is_none()) {
        let (inp, s) = (&w.inp, &w.s);
        let m = Model::physical(inp).unwrap();
        let src = m.sourcing(&s.price);
        let params = ceteris_econ::EconParams::frictionless();
        let blocks = block::blocks(&m, &src, s, &inp.ent_s_l(), &params, &trade);
        let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
        let traded = |g: usize, x: f64| if inp.traded[g] { x } else { 0.0 };
        let all = Mat::from_fn(n_p, n_g, |p, g| {
            traded(g, s.spend_int[(p, g)] + s.spend_hh[(p, g)])
        });
        let total = m.flows(&s.price, &all, &src).orders;
        let mut checked = 0;
        for (cells, b) in &blocks {
            let c = inp.province_country[cells[0] % n_p];
            let of_country = Mat::from_fn(n_p, n_g, |p, g| {
                if inp.province_country[p] == c {
                    traded(g, s.spend_int[(p, g)])
                } else {
                    0.0
                }
            });
            let ours = m.flows(&s.price, &of_country, &src).orders;
            let mut slot_v = vec![0.0; b.hold.len()];
            for input in b.inputs.iter().filter(|i| i.place >= cells.len()) {
                slot_v[input.place - cells.len()] += input.v;
            }
            let mut share = vec![0.0; cells.len()];
            for r in &b.routes {
                share[r.cell] += r.u * slot_v[r.slot];
            }
            for (i, &cell) in cells.iter().enumerate() {
                let (p, g) = (cell % n_p, cell / n_p);
                if total[(p, g)] > 0.0 {
                    // with the storage order's factor of a stored good (ADR 0091)
                    let want = ours[(p, g)] / total[(p, g)] * trade.order_response(g);
                    assert!(
                        (share[i] - want).abs() <= 1e-10 * want.max(1.0),
                        "{} cell {cell}: {} vs {want}",
                        w.name,
                        share[i]
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 0);
    }
}

/// ADR 0267: the calibration's block limit gains are those of `block::limits` in each mode,
/// infinite outside every block and for a block within its margin, and b kappa_cell / m with b
/// in (0, 1) elsewhere; on the fixture no block is beyond its margin in either mode. The pack's
/// capped blocks are reported.
#[test]
fn block_limits_of_the_calibration() {
    let trade = ceteris_trade::TradeParams::default();
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let m = Model::physical(inp).unwrap();
        let src = m.sourcing(&s.price);
        let n_p = inp.n_provinces();
        let modes = [
            ceteris_econ::EconParams::default(),
            ceteris_econ::EconParams::frictionless(),
        ];
        for (mode, params) in modes.iter().enumerate() {
            let eta = params.eta;
            let want = block::limits(&m, &src, s, &inp.ent_s_l(), params, &trade).unwrap();
            assert_eq!(s.block_limit[mode], want, "{} eta {eta}", w.name);
            let blocks = block::blocks(&m, &src, s, &inp.ent_s_l(), params, &trade);
            let (mut capped, mut top) = (0, 0.0f64);
            for (cells, b) in &blocks {
                top = top.max(b.spectral_radius(1.0 / trade.gain_share, eta));
                let scale = b.scale(eta, trade.gain_share);
                if scale < 1.0 {
                    capped += 1;
                }
                for (&j, &kappa) in cells.iter().zip(&b.gain) {
                    let limit = want[(j % n_p, j / n_p)];
                    if scale < 1.0 {
                        assert_eq!(
                            limit,
                            scale * kappa / trade.gain_share,
                            "{} cell {j}",
                            w.name
                        );
                    } else {
                        assert_eq!(limit, f64::INFINITY, "{} cell {j}", w.name);
                    }
                }
            }
            eprintln!(
                "{} eta {eta}: {} blocks, {capped} beyond their margin, largest radius at gains / \
                 m {top:.4}",
                w.name,
                blocks.len()
            );
            for (c, &l) in want.data.iter().enumerate() {
                let (p, g) = (c / inp.n_goods(), c % inp.n_goods());
                let in_block = blocks
                    .iter()
                    .any(|(cells, _)| cells.contains(&(g * n_p + p)));
                assert!(in_block || l == f64::INFINITY, "{} cell {c}", w.name);
                assert!(l > 0.0, "{} cell {c}: {l}", w.name);
            }
            if w.pack_province.is_none() {
                assert_eq!(capped, 0, "{} eta {eta}", w.name);
            }
        }
    }
}

/// ADRs 0091 and 0267: with storage on, the calibration's block limits are those of the map at
/// the export's storage parameters, whose stored cells carry their stock and expectation as
/// states; a storage loss without speculation is refused. The radii with and without those
/// states are reported (trade SPEC, Block gain).
#[test]
fn block_limits_with_storage_on() {
    let (inp, s) = fixture_with_storage();
    let trade = TradeParams {
        coverage: std::array::from_fn(|g| {
            inp.coverage
                .as_ref()
                .unwrap()
                .get(g)
                .copied()
                .unwrap_or(0.0)
        }),
        ..TradeParams::default().with_storage()
    };
    let m = Model::physical(inp).unwrap();
    let src = m.sourcing(&s.price);
    let modes = [
        ceteris_econ::EconParams::default(),
        ceteris_econ::EconParams::frictionless(),
    ];
    for (mode, params) in modes.iter().enumerate() {
        let eta = params.eta;
        let want = block::limits(&m, &src, s, &inp.ent_s_l(), params, &trade).unwrap();
        assert_eq!(s.block_limit[mode], want, "eta {eta}");
        let blocks = block::blocks(&m, &src, s, &inp.ent_s_l(), params, &trade);
        let stored: usize = blocks.iter().map(|(_, b)| b.stored.len()).sum();
        assert!(stored > 0, "no stored cell");
        let (mut capped, mut with, mut without) = (0, 0.0f64, 0.0f64);
        for (_, b) in &blocks {
            with = with.max(b.spectral_radius(1.0 / trade.gain_share, eta));
            let mut bare = b.clone();
            bare.stored.clear();
            without = without.max(bare.spectral_radius(1.0 / trade.gain_share, eta));
            if b.scale(eta, trade.gain_share) < 1.0 {
                capped += 1;
            }
        }
        let limited = want.data.iter().filter(|l| l.is_finite()).count();
        eprintln!(
            "storage on, eta {eta}: {} blocks, {stored} stored cells, {capped} beyond their \
             margin ({limited} cells limited), largest radius at gains / m {with:.4} with the \
             stock and expectation states, {without:.4} without",
            blocks.len()
        );
    }
    let mut bad = inp.clone();
    bad.storage.speculation = vec![0.0; bad.storage.speculation.len()];
    let err = steady::calibrate(&bad, &bad.u_star.clone(), &mut quiet()).unwrap_err();
    assert!(err.to_string().contains("no speculation"), "{err}");
}

/// ADR 0267: the measured radius against the Gelfand limit of the map's matrix, ||M^k||^(1/k)
/// at k = 2^40 by repeated squaring (every direction, no start vector), on each of the fixture's
/// blocks (57 cells, about 240 states), at gains / m and twice that, under every wage gain of
/// both modes. They agree to the 1e-3 the measurement reads a radius to.
#[test]
fn block_radius_matches_the_matrix_powers() {
    let w = &worlds()[0];
    let (inp, s) = (&w.inp, &w.s);
    let m = Model::physical(inp).unwrap();
    let src = m.sourcing(&s.price);
    let trade = ceteris_trade::TradeParams::default();
    let mut worst = 0.0f64;
    for params in [
        ceteris_econ::EconParams::default(),
        ceteris_econ::EconParams::frictionless(),
    ] {
        let gains = match params.frictionless {
            true => vec![0.0],
            false => vec![params.kappa_w, (1.0 - params.delta_w) * params.kappa_w],
        };
        for (_, b) in block::blocks(&m, &src, s, &inp.ent_s_l(), &params, &trade) {
            let n = b.dimension();
            for scale in [1.0 / trade.gain_share, 2.0 / trade.gain_share] {
                for &g in &gains {
                    let mut a = Mat::zeros(n, n);
                    let mut unit = vec![0.0; n];
                    for col in 0..n {
                        unit[col] = 1.0;
                        let next = b.apply(scale, params.eta, g, &unit);
                        unit[col] = 0.0;
                        for (row, v) in next.into_iter().enumerate() {
                            a[(row, col)] = v;
                        }
                    }
                    let mut log_norm = 0.0;
                    const SQUARINGS: i32 = 40;
                    for j in 0..=SQUARINGS {
                        if j > 0 {
                            let mut sq = Mat::zeros(n, n);
                            for r in 0..n {
                                for k in 0..n {
                                    let x = a[(r, k)];
                                    if x != 0.0 {
                                        for c in 0..n {
                                            sq[(r, c)] += x * a[(k, c)];
                                        }
                                    }
                                }
                            }
                            a = sq;
                            log_norm *= 2.0;
                        }
                        let norm = a.data.iter().fold(0.0f64, |m, v| m.max(v.abs()));
                        log_norm += norm.ln();
                        a.data.iter_mut().for_each(|v| *v /= norm);
                    }
                    let gelfand = (log_norm / 2f64.powi(SQUARINGS)).exp();
                    let measured = b.radius_at(scale, params.eta, g);
                    worst = worst.max((measured - gelfand).abs());
                    assert!(
                        (measured - gelfand).abs() < 1e-3,
                        "eta {} scale {scale} wage gain {g}: measured {measured}, Gelfand \
                         {gelfand}",
                        params.eta
                    );
                }
            }
        }
    }
    eprintln!("block radius against the matrix powers: worst gap {worst:.3e}");
}

/// ADR 0263 (As built): eps of a cell is the largest s_L sigma_FL / ((1 - s_L) a_VA) of its
/// enterprises, 0 where none produces; local cells take sigma (P06) as E.
#[test]
fn supply_elasticity_is_the_enterprise_maximum() {
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let s_l = inp.ent_s_l();
        let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
        let mut of_cell = vec![Vec::new(); n_p * n_g];
        for e in 0..inp.ent_good.len() {
            let x = s_l[e] * inp.sigma_fl / ((1.0 - s_l[e]) * s.a_va[e]);
            of_cell[inp.ent_province[e] * n_g + inp.ent_good[e]].push(x);
        }
        for (c, xs) in of_cell.iter().enumerate() {
            let eps = s.supply_elasticity.data[c];
            if xs.is_empty() {
                assert_eq!(eps, 0.0, "{} cell {c}", w.name);
            } else {
                assert!(
                    xs.contains(&eps) && xs.iter().all(|&x| x <= eps),
                    "{} cell {c}",
                    w.name
                );
            }
            let (p, g) = (c / n_g, c % n_g);
            if inp.active[p][g] && !inp.traded[g] {
                assert_eq!(
                    s.demand_elasticity.data[c], inp.sigma_demand,
                    "{} cell {c}",
                    w.name
                );
            }
        }
    }
}
