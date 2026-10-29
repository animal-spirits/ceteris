use std::fs::File;
use std::path::PathBuf;

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt32Type};
use arrow_ipc::reader::FileReader;
use ceteris_reference::demand;
use ceteris_reference::io::{self, Inputs, Mat};
use ceteris_reference::sinkhorn;
use ceteris_reference::uniqueness::{self, V1Mode};

fn fixture() -> PathBuf {
    std::env::var_os("CETERIS_FIXTURE_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture"),
        PathBuf::from,
    )
}

fn inputs() -> Inputs {
    io::load(fixture()).unwrap()
}

fn table(name: &str) -> RecordBatch {
    let file = File::open(fixture().join(format!("{name}.arrow"))).unwrap();
    let mut batches: Vec<RecordBatch> = FileReader::try_new(file, None)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(batches.len(), 1, "one batch per fixture table");
    batches.pop().unwrap()
}

fn f64s(t: &RecordBatch, col: &str) -> Vec<f64> {
    t.column_by_name(col)
        .unwrap()
        .as_primitive::<Float64Type>()
        .values()
        .to_vec()
}

fn close(got: f64, want: f64, rel: f64) -> bool {
    (got - want).abs() <= rel * want.abs()
}

/// ADR 0018: theta is read per good from `goods.arrow`.
#[test]
fn inputs_carry_theta_per_good() {
    let inp = inputs();
    assert_eq!(inp.theta, f64s(&table("goods"), "theta"));
    let mut traded: Vec<f64> = (0..inp.n_goods())
        .filter(|&g| inp.traded[g])
        .map(|g| inp.theta[g])
        .collect();
    assert!(traded.iter().all(|&t| t > 0.0));
    traded.sort_by(f64::total_cmp);
    traded.dedup();
    assert!(traded.len() > 1);
    assert!((0..inp.n_goods()).all(|g| inp.traded[g] || inp.theta[g] == 0.0));
}

/// The reference's PIGL (its own inversion of the country weights, its shares) reproduces
/// `demand::pigl` on the export's `pigl_check` draws: prices (some unreachable), money per person
/// far from base on both sides (clamped shares), income dispersion.
#[test]
fn pigl_matches_rust() {
    let inp = inputs();
    let t = table("pigl_check");
    let n_g = inp.n_goods();
    let country: Vec<usize> = t
        .column_by_name("country")
        .unwrap()
        .as_primitive::<UInt32Type>()
        .values()
        .iter()
        .map(|&c| c as usize)
        .collect();
    let (money, sigma) = (f64s(&t, "money"), f64s(&t, "sigma"));
    let rows = money.len();
    let column_mat = |prefix: &str| {
        let cols: Vec<Vec<f64>> = (0..n_g)
            .map(|g| f64s(&t, &format!("{prefix}_{g}")))
            .collect();
        Mat::from_fn(rows, n_g, |r, g| cols[g][r])
    };
    let (price, want) = (column_mat("price"), column_mat("x"));
    assert!(rows == 100 && price.data.iter().any(|p| p.is_infinite()));
    let share = demand::shares(
        &inp.pigl_weights().unwrap(),
        &country,
        &price,
        &money,
        &sigma,
        &inp.category,
        inp.sigma_demand,
        &inp.pigl,
    );
    let got = demand::quantities(&share, &price, &money);
    for (g, w) in got.data.iter().zip(&want.data) {
        assert_eq!(*g == 0.0, *w == 0.0);
        assert!(close(*g, *w, 1e-12), "{g} vs {w}");
    }
    // Shares within [0, 1] adding up, also where the raw PIGL shares left the interval.
    for r in 0..rows {
        assert!(close(share.row(r).iter().sum(), 1.0, 1e-14));
    }
    assert!(share.data.iter().all(|s| (0.0..=1.0).contains(s)));
}

/// ADR 0094: at prices 1 the weights reproduce each country's final demand shares Omega as the
/// aggregate of its pops' shares, weighted by their base spending, each pop at its base income
/// per person and the country's within-pop dispersion.
#[test]
fn pigl_inversion_reproduces_base_shares() {
    let inp = inputs();
    let w = inp.pigl_weights().unwrap();
    let (n_c, n_g) = (inp.n_countries(), inp.n_goods());
    let country = inp.pop_country();
    let income = inp.pop_base_income();
    let sigma: Vec<f64> = country.iter().map(|&c| w.sigma[c]).collect();
    let share = demand::shares(
        &w,
        &country,
        &Mat::filled(country.len(), n_g, 1.0),
        &income,
        &sigma,
        &inp.category,
        inp.sigma_demand,
        &inp.pigl,
    );
    let mut spent = Mat::zeros(n_c, n_g);
    for (k, &c) in country.iter().enumerate() {
        for g in 0..n_g {
            spent[(c, g)] += share[(k, g)] * income[k] * inp.pop_population[k];
        }
    }
    let (total, spent_total) = (inp.omega.row_sums(), spent.row_sums());
    for c in 0..n_c {
        for g in 0..n_g {
            let got = spent[(c, g)] / spent_total[c];
            let want = inp.omega[(c, g)] / total[c];
            assert!(
                (got - want).abs() <= 1e-12,
                "country {c} good {g}: {got} vs {want}"
            );
        }
    }
    assert!((0..n_c).all(|r| w.omega[(r, 0)] == 0.0 && w.scale[r] > 0.0));
}

/// ADR 0094: with one pop per country at prices 1 and the country's base spending, the joint
/// inversion is the closed form of one lognormal (ADR 0021).
#[test]
fn one_pop_inversion_equals_old_at_prices_one() {
    let inp = inputs();
    let (n_c, n_g) = (inp.n_countries(), inp.n_goods());
    let country: Vec<usize> = (0..n_c).collect();
    let spend = inp.base_spend();
    let sigma = inp.within_sigma();
    let ones = Mat::filled(n_c, n_g, 1.0);
    let pops = demand::Pops {
        country: &country,
        price: &ones,
        income: &spend,
        spend: &spend,
    };
    let joint = demand::invert_pops(
        &inp.omega,
        &pops,
        &sigma,
        &inp.category,
        inp.sigma_demand,
        &inp.pigl,
    )
    .unwrap();
    let old = demand::invert(&inp.omega, &spend, &sigma, &inp.category, &inp.pigl).unwrap();
    for (what, a, b) in [
        ("scale", &joint.scale, &old.scale),
        ("omega", &joint.omega.data, &old.omega.data),
        ("within", &joint.within.data, &old.within.data),
    ] {
        for (x, y) in a.iter().zip(b) {
            assert!(
                (x - y).abs() <= 1e-13 * y.abs().max(1e-300) + 1e-15,
                "{what}: {x} vs {y}"
            );
        }
    }
    assert_eq!(joint.sigma, sigma);
}

/// ADR 0094: the within-pop dispersion and the population-weighted variance of the pops' log
/// base income per person add up to the national s^2 = (2 erfinv G)^2, or s_w = 0 where the
/// between-pop variance alone exceeds it.
#[test]
fn between_and_within_variance_add_up() {
    let inp = inputs();
    let national = demand::lognormal_sigma(&inp.gini);
    let within = inp.within_sigma();
    let (country, income) = (inp.pop_country(), inp.pop_base_income());
    let mut zero = 0;
    for c in 0..inp.n_countries() {
        let pops: Vec<usize> = (0..country.len())
            .filter(|&k| country[k] == c && inp.pop_population[k] > 0.0)
            .collect();
        let people: f64 = pops.iter().map(|&k| inp.pop_population[k]).sum();
        let mean = pops
            .iter()
            .map(|&k| inp.pop_population[k] * income[k].ln())
            .sum::<f64>()
            / people;
        let between = pops
            .iter()
            .map(|&k| inp.pop_population[k] * (income[k].ln() - mean).powi(2))
            .sum::<f64>()
            / people;
        assert!(between > 0.0, "country {c}: pops differ in income");
        if within[c] > 0.0 {
            let sum = within[c].powi(2) + between;
            assert!((sum - national[c].powi(2)).abs() <= 1e-12 * national[c].powi(2));
        } else {
            assert!(between >= national[c].powi(2));
            zero += 1;
        }
    }
    println!("{zero} of {} countries with s_w = 0", inp.n_countries());
}

/// A country with no economy province (the pack's uninhabited territories keep a gateway) has
/// base spending 0, not 0 / 0, and the PIGL inversion stays finite.
#[test]
fn country_without_people_spends_nothing() {
    let inp = inputs();
    let one = |v: &[f64]| [v, &v[v.len() - 1..]].concat();
    let mut empty = inp.clone();
    empty.s_l = one(&inp.s_l);
    empty.unemployment = one(&inp.unemployment);
    empty.u_star = one(&inp.u_star);
    empty.gini = one(&inp.gini);
    let last = inp.n_countries() - 1;
    empty.omega = inp
        .omega
        .select_rows(&(0..inp.n_countries()).chain([last]).collect::<Vec<_>>());
    empty.io.push(inp.io[last].clone());
    let spend = empty.base_spend();
    assert_eq!(spend[last + 1], 0.0);
    assert_eq!(spend[..=last], inp.base_spend()[..]);
    assert!(
        empty
            .pigl_weights()
            .unwrap()
            .scale
            .iter()
            .all(|s| s.is_finite())
    );
}

#[test]
fn validate_rejects_a_gini_outside_the_unit_interval() {
    let mut inp = inputs();
    for g in [1.0, 1.5, -0.1, f64::NAN] {
        inp.gini[1] = g;
        assert_eq!(
            io::validate(&inp).unwrap_err(),
            "Gini outside [0, 1) in countries [1]"
        );
    }
    inp.gini[1] = 0.0;
    io::validate(&inp).unwrap();
}

#[test]
fn load_rejects_another_export_format() {
    let dir = std::env::temp_dir().join(format!("ceteris-reference-format-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("params.json"), r#"{"format": "9"}"#).unwrap();
    let err = io::load(&dir).unwrap_err();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(err, "export format 9, expected 11");
}

/// Flows of the form a_o K_od b_d are recovered from their own marginals.
#[test]
fn balanced_recovers_scaled_kernel() {
    let k = Mat::from_fn(5, 5, |o, d| {
        if o == d {
            1.0
        } else {
            (-((o as f64) - (d as f64)).abs()).exp()
        }
    });
    let a = [0.3, 1.2, 0.0, 2.5, 0.7];
    let b = [1.1, 0.4, 0.9, 0.0, 3.0];
    let flows = Mat::from_fn(5, 5, |o, d| a[o] * k[(o, d)] * b[d]);
    let col_sums: Vec<f64> = (0..5)
        .map(|d| (0..5).map(|o| flows[(o, d)]).sum())
        .collect();
    let solved = sinkhorn::balanced(&k, &flows.row_sums(), &col_sums).unwrap();
    for (got, want) in solved.data.iter().zip(&flows.data) {
        assert!(
            (got - want).abs() <= 1e-12 * want.abs().max(1e-300) || *want == 0.0 && *got == 0.0
        );
    }
}

/// Section 3 urban model, one type, constant elasticity spillovers (system 13) in the form of
/// equation (8): types (workers, residents), own-location exponents theta alpha and theta beta.
fn urban(theta: f64, alpha: f64, beta: f64) -> Mat {
    uniqueness::change_of_variables(
        &Mat::identity(2),
        &Mat::from_rows(&[vec![0.0, theta * beta], vec![theta * alpha, 0.0]]),
        Some(&Mat::from_rows(&[
            vec![theta * alpha, 0.0],
            vec![0.0, theta * beta],
        ])),
    )
    .unwrap()
}

/// Allen, Arkolakis and Li (2024), January 2024 version: worked examples of Theorem 1.
#[test]
fn bound_reproduces_paper_examples() {
    // Footnote 3 (p. 4): the stacked contraction bound is 2, the spectral radius sqrt(2/3).
    let a = Mat::from_rows(&[vec![0.0, 2.0], vec![1.0 / 3.0, 0.0]]);
    assert!(close(
        uniqueness::spectral_radius(&a).unwrap(),
        (2.0_f64 / 3.0).sqrt(),
        1e-14
    ));
    assert_eq!(a.row_sums().into_iter().fold(0.0, f64::max), 2.0);
    // Footnote 6 (p. 11): x_i = sum_j K_ij x_i^beta x_j^alpha, alpha 1, beta -1/2: the change of
    // variables (Remark 5) gives |alpha / (1 - beta)| = 2/3 where the direct bound is 1.5.
    let fn6 = uniqueness::change_of_variables(
        &Mat::identity(1),
        &Mat::from_rows(&[vec![1.0]]),
        Some(&Mat::from_rows(&[vec![-0.5]])),
    )
    .unwrap();
    assert!(close(
        uniqueness::spectral_radius(&fn6).unwrap(),
        2.0 / 3.0,
        1e-14
    ));
    // Section 3 (pp. 14-15, Figure 1 p. 21): theta 1, alpha = beta. The off-diagonal blocks are
    // theta alpha / (1 - theta alpha); at 0.5 the radius is 1 (unique up to scale, part ii.b: one
    // equilibrium in panels a, d, g); at 0.55 and 0.6 it exceeds 1 and panels f, h, i show three.
    for (alpha, radius, mode) in [
        (0.5, 1.0, V1Mode::Tolerance),
        (0.55, 0.55 / 0.45, V1Mode::Report),
        (0.6, 1.5, V1Mode::Report),
    ] {
        let a = urban(1.0, alpha, alpha);
        let off = alpha / (1.0 - alpha);
        for (got, want) in a.data.iter().zip([0.0, off, off, 0.0]) {
            assert!(close(*got, want, 1e-14));
        }
        let rho = uniqueness::spectral_radius(&a).unwrap();
        assert!(close(rho, radius, 1e-12));
        assert_eq!(uniqueness::v1_mode(rho), mode);
    }
    // Online Appendix A.3 (pp. 8-9): Eaton-Kortum with intermediates and quasi-symmetric costs,
    // rho(B Gamma^-1) = (beta + theta - beta theta) / (beta + theta) < 1, theta = 1 / trade
    // elasticity.
    for elasticity in [0.5, 2.62, 5.88, 12.55, 100.0] {
        for beta in [0.05, 0.46, 0.73, 1.0] {
            let t = 1.0 / elasticity;
            let rho = uniqueness::spectral_radius(
                &uniqueness::trade_with_intermediates(elasticity, beta).unwrap(),
            )
            .unwrap();
            assert!(close(rho, (beta + t - beta * t) / (beta + t), 1e-12));
            assert!(rho < 1.0);
        }
    }
}

/// `numpy.abs(numpy.linalg.eigvals(abs(a))).max()` of matrices the A.3 bound does not produce:
/// periodic, reducible, defective, nilpotent, signed, with tied row sums.
#[test]
fn spectral_radius_matches_numpy_eigvals() {
    let dense4: Vec<Vec<f64>> = (0..4)
        .map(|i| (0..4).map(|j| ((i * 7 + j * 3) % 5) as f64 / 3.0).collect())
        .collect();
    let cycle = |c: f64| vec![vec![0.0, 1.0, 0.0], vec![0.0, 0.0, 1.0], vec![c, 0.0, 0.0]];
    let cases: [(Vec<Vec<f64>>, f64); 13] = [
        (
            vec![vec![0.0, 2.0], vec![1.0 / 3.0, 0.0]],
            0.816496580927726,
        ),
        (vec![vec![0.5, 0.3], vec![0.0, 0.2]], 0.5),
        (vec![vec![0.5, 0.0], vec![0.0, 0.2]], 0.5),
        (vec![vec![0.0, 100.0], vec![0.0, 0.0]], 0.0),
        (vec![vec![0.7, 1.0], vec![0.0, 0.7]], 0.7),
        (
            vec![
                vec![-0.4, 0.9, 0.1],
                vec![0.2, -0.3, 0.5],
                vec![0.6, 0.1, -0.8],
            ],
            1.3107198535269964,
        ),
        (
            vec![
                vec![0.0, 1.0, 0.0, 0.0],
                vec![0.0, 0.0, 2.0, 0.0],
                vec![0.0, 0.0, 0.0, 3.0],
                vec![0.25, 0.0, 0.0, 0.0],
            ],
            1.1066819197003221,
        ),
        (dense4, 2.4598558530124772),
        // Tied Collatz-Wielandt ratios: the upper end stays flat for several exact steps.
        (cycle(0.25), 0.6299605249474369),
        (cycle(0.5), 0.7937005259840998),
        (cycle(0.9), 0.96548938460563),
        (
            vec![
                vec![0.2, 0.6, 0.0],
                vec![0.0, 0.2, 0.6],
                vec![0.5, 0.1, 0.0],
            ],
            0.7366978957327053,
        ),
        (
            vec![
                vec![0.0, 1.0, 0.0, 0.0, 0.0],
                vec![0.0, 0.0, 1.0, 0.0, 0.0],
                vec![0.0, 0.0, 0.0, 1.0, 0.0],
                vec![0.0, 0.0, 0.0, 0.0, 1.0],
                vec![0.1, 0.0, 0.0, 0.0, 0.0],
            ],
            0.6309573444801935,
        ),
    ];
    for (rows, want) in cases {
        let got = uniqueness::spectral_radius(&Mat::from_rows(&rows)).unwrap();
        assert!(
            close(got, want, 1e-14) || (want == 0.0 && got == 0.0),
            "{rows:?}: {got} vs {want}"
        );
    }
}

/// Eigenvalues 1 +- 3.2e-8: the power iteration closes the bracket by a factor of about 1 + 3e-8
/// per step, far beyond the iteration cap, and the radius is an error rather than a bound.
#[test]
fn nearly_decoupled_radius_is_an_error() {
    let a = Mat::from_rows(&[vec![1.0, 1e-6], vec![1e-9, 1.0]]);
    let err = uniqueness::spectral_radius(&a).unwrap_err();
    assert!(err.starts_with("power iteration did not converge"), "{err}");
}

/// P2 defaults (theta per good of the fixture export, no agglomeration, no migration): the bound
/// is below 1, so V1 runs in tolerance mode.
#[test]
fn trade_only_is_unique() {
    let inp = inputs();
    let va = uniqueness::value_added_share(&inp);
    assert!(va.len() == inp.n_goods() && va.iter().all(|&v| v > 0.0 && v <= 1.0));
    let bound = uniqueness::spectral_bound(&inp.theta, 0.0, None, &va).unwrap();
    let want = (0..inp.n_goods())
        .filter(|&g| inp.traded[g])
        .map(|g| {
            uniqueness::spectral_radius(
                &uniqueness::trade_with_intermediates(inp.theta[g], va[g]).unwrap(),
            )
            .unwrap()
        })
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(close(bound, want, 1e-14));
    assert!(bound < 1.0 && uniqueness::v1_mode(bound) == V1Mode::Tolerance);
    // Labour the only input (value-added share 1) gives the smaller bound theta / (1 + theta).
    let ones = vec![1.0; inp.n_goods()];
    let labour_only = uniqueness::spectral_bound(&inp.theta, 0.0, None, &ones).unwrap();
    let theta_max = (0..inp.n_goods())
        .filter(|&g| inp.traded[g])
        .map(|g| inp.theta[g])
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(close(labour_only, theta_max / (1.0 + theta_max), 1e-12));
    assert!(labour_only <= bound);
    assert!(uniqueness::spectral_bound(&inp.theta, 0.05, None, &va).is_err());
    assert!(uniqueness::spectral_bound(&inp.theta, 0.0, Some(2.0), &va).is_err());
}

/// numpy's `solve` raises on a singular matrix; so does the change of variables.
#[test]
fn singular_change_of_variables_is_an_error() {
    let singular = Mat::from_rows(&[vec![1.0, 2.0], vec![2.0, 4.0]]);
    let b = Mat::identity(2);
    assert!(uniqueness::change_of_variables(&singular, &b, None).is_err());
    assert!(uniqueness::change_of_variables(&b, &b, Some(&b)).is_err());
    assert!(uniqueness::trade_with_intermediates(0.0, 0.5).is_err());
}

#[test]
fn v1_mode() {
    use V1Mode::{Report, Tolerance};
    assert_eq!(uniqueness::v1_mode(0.0), Tolerance);
    assert_eq!(uniqueness::v1_mode(0.93), Tolerance);
    assert_eq!(uniqueness::v1_mode(1.0), Tolerance);
    assert_eq!(uniqueness::v1_mode(1.0_f64.next_up()), Report);
    assert_eq!(uniqueness::v1_mode(1.5), Report);
    assert_eq!(uniqueness::v1_mode(f64::INFINITY), Report);
}
