use std::fs::File;
use std::path::PathBuf;

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt32Type};
use arrow_ipc::reader::FileReader;
use ceteris_reference::demand;
use ceteris_reference::io::{self, Inputs, Mat};

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

#[test]
fn pigl_inversion_reproduces_base_shares() {
    let inp = inputs();
    let w = inp.pigl_weights().unwrap();
    let n_c = inp.n_countries();
    let c: Vec<usize> = (0..n_c).collect();
    let share = demand::shares(
        &w,
        &c,
        &Mat::filled(n_c, inp.n_goods(), 1.0),
        &inp.base_spend(),
        &inp.sigma_income(),
        &inp.category,
        inp.sigma_demand,
        &inp.pigl,
    );
    let total = inp.omega.row_sums();
    for r in 0..n_c {
        for g in 0..inp.n_goods() {
            assert!((share[(r, g)] - inp.omega[(r, g)] / total[r]).abs() <= 1e-12);
        }
    }
    assert!((0..n_c).all(|r| w.omega[(r, 0)] == 0.0 && w.scale[r] > 0.0));
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
fn load_rejects_another_export_format() {
    let dir = std::env::temp_dir().join(format!("ceteris-reference-format-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("params.json"), r#"{"format": "8"}"#).unwrap();
    let err = io::load(&dir).unwrap_err();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(err, "export format 8, expected 9");
}
