//! The goods mapping, BACI weights and the theta classes, on hand-made inputs and, when the
//! cache is present, on the 2000 sources.

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use arrow_array::cast::AsArray;
use arrow_array::types::Float64Type;
use ceteris_pack::econ::build::{BACI, HS_KEY};
use ceteris_pack::econ::goods::{
    Industry, NAMES, TRADED, good_of_cpa, good_of_division, hs_goods, icio_industry,
    weights_from_baci,
};
use ceteris_pack::econ::theta::{
    CALIENDO_PARRO, CLASS_SPREAD, REV3_OF_CPA, by_good, caliendo_parro_by_good,
    caliendo_parro_with_value, class_bands, classes, sector_values,
};
use ceteris_pack::read::baci::{baci_values, read_hs_cpa};
use ceteris_pack::read::icio::N_GOODS;
use ceteris_pack::sources;
use common::{approx, cached, hs_key, write_zip};

/// ICIO 2025 edition, 50 industries (ReadMe_ICIO_small.xlsx, "Areas and Economic Activities").
const ICIO_2025: [&str; 50] = [
    "A01", "A02", "A03", "B05", "B06", "B07", "B08", "B09", "C10T12", "C13T15", "C16", "C17_18",
    "C19", "C20", "C21", "C22", "C23", "C24A", "C24B", "C25", "C26", "C27", "C28", "C29", "C301",
    "C302T309", "C31T33", "D", "E", "F", "G", "H49", "H50", "H51", "H52", "H53", "I", "J58T60",
    "J61", "J62_63", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T",
];

fn good(code: &str) -> Option<usize> {
    match icio_industry(code)? {
        Industry::Good(g) => Some(g),
        Industry::Split => None,
    }
}

#[test]
fn icio_2025_has_fifty_industries() {
    let mut codes = ICIO_2025.to_vec();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), 50);
}

#[test]
fn every_icio_industry_is_mapped_once() {
    assert!(
        ICIO_2025
            .iter()
            .all(|c| good(c).is_some_and(|g| g < N_GOODS))
    );
    let goods: std::collections::BTreeSet<usize> =
        ICIO_2025.iter().filter_map(|c| good(c)).collect();
    assert_eq!(goods, (0..N_GOODS).collect());
}

#[test]
fn icio_map_follows_the_spec_table() {
    let services: Vec<&str> = ICIO_2025
        .iter()
        .copied()
        .filter(|c| "GHIJKLMNOPQRST".contains(&c[..1]))
        .collect();
    let expected: [(usize, Vec<&str>); 10] = [
        (0, vec!["A01", "A02", "A03"]),
        (1, vec!["B06", "B09"]),
        (2, vec!["B05", "B07", "B08"]),
        (3, vec!["C10T12"]),
        (4, vec!["C13T15", "C16", "C17_18", "C31T33"]),
        (
            5,
            vec!["C19", "C20", "C21", "C22", "C23", "C24A", "C24B", "C25"],
        ),
        (6, vec!["C26", "C27", "C28", "C29", "C301", "C302T309"]),
        (7, vec!["D", "E"]),
        (8, vec!["F"]),
        (9, services),
    ];
    for (g, codes) in expected {
        for code in codes {
            assert_eq!(good(code), Some(g), "{code}");
        }
    }
}

#[test]
fn older_edition_names_map_to_the_same_goods() {
    assert_eq!(good("A01_02"), Some(0));
    assert_eq!(good("B07_08"), Some(2));
    assert_eq!(good("C24"), Some(5));
    assert_eq!(good("C30"), Some(6));
}

#[test]
fn combined_coal_oil_gas_is_a_split_marker() {
    assert_eq!(icio_industry("B05_06"), Some(Industry::Split));
    assert_eq!(icio_industry("nonsense"), None);
}

#[test]
fn traded_and_local_goods() {
    assert_eq!(TRADED, 7);
    assert_eq!(NAMES.len(), N_GOODS);
}

#[test]
fn division_to_good_covers_isic_goods_divisions() {
    assert_eq!(good_of_division("01"), Some(0));
    assert_eq!(good_of_division("05"), Some(2));
    assert_eq!(good_of_division("06"), Some(1));
    assert_eq!(good_of_division("24"), Some(5));
    assert_eq!(good_of_division("30"), Some(6));
    assert_eq!(good_of_division("35"), Some(7));
    assert_eq!(good_of_division("45"), Some(9));
    assert_eq!(good_of_division("99"), Some(9));
    assert_eq!(good_of_division("04"), None);
    assert_eq!(good_of_division("100"), None);
}

#[test]
fn cpa_codes_map_to_goods_through_their_division() {
    assert_eq!(good_of_cpa("A01"), Some(0));
    assert_eq!(good_of_cpa("B05"), Some(2));
    assert_eq!(good_of_cpa("B06"), Some(1));
    assert_eq!(good_of_cpa("B072"), Some(2));
    assert_eq!(good_of_cpa("C24A"), Some(5));
    assert_eq!(good_of_cpa("C254"), Some(5));
    assert_eq!(good_of_cpa("C302_309"), Some(6));
    assert_eq!(good_of_cpa("C262"), Some(6));
    assert_eq!(good_of_cpa("C32X325"), Some(4));
    assert_eq!(good_of_cpa("C325"), Some(4));
    assert_eq!(good_of_cpa("D35"), Some(7));
    assert_eq!(good_of_cpa("E38"), Some(7));
    assert_eq!(good_of_cpa("J58"), Some(9));
    assert_eq!(good_of_cpa("R90"), Some(9));
    assert_eq!(good_of_cpa("_X"), None);
    assert_eq!(good_of_cpa("C"), None);
}

fn names(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

#[test]
fn hs_goods_put_each_line_in_one_good() {
    let key = names(&[
        ("270111", "B05"),
        ("270900", "B06"),
        ("270120", "C19"),
        ("999999", "_X"),
    ]);
    let got: Vec<(&str, usize)> = hs_goods(&key).into_iter().collect();
    assert_eq!(got, [("270111", 2), ("270120", 5), ("270900", 1)]);
}

#[test]
fn gold_and_books_take_the_adapted_goods() {
    let dir = tempfile::tempdir().unwrap();
    let lines = [
        ("710812", "B072"),
        ("710813", "C24B"),
        ("490199", "J58"),
        ("852410", "J59_60"),
        ("271600", "D35"),
        ("970110", "R90"),
    ];
    let key = read_hs_cpa(&hs_key(&dir.path().join("key.xlsx"), &lines)).unwrap();
    let got = hs_goods(&key);
    assert_eq!(got["710812"], 5);
    assert_eq!(got["490199"], 4);
    assert_eq!(got["852410"], 4);
    assert_eq!(got["271600"], 7);
    assert_eq!(got["970110"], 9);
}

fn baci(path: &Path, rows: &[&str]) -> std::path::PathBuf {
    let y2000 = format!("t,i,j,k,v,q\n{}\n", rows.join("\n"));
    write_zip(
        path,
        &[
            ("BACI_HS92_Y2000_V202601.csv", &y2000),
            (
                "BACI_HS92_Y2001_V202601.csv",
                "t,i,j,k,v,q\n2001,1,2,10121,1,999\n2001,1,2,440310,1000,1\n",
            ),
        ],
    )
}

#[test]
fn weights_from_baci_pins_tonnes_per_usd() {
    let dir = tempfile::tempdir().unwrap();
    // One HS line per traded good: hs6 to the CPA 2.1 activity in divisions 01, 06, 05, 10, 13,
    // 20 and 26.
    let lines = [
        ("010121", "A01"),
        ("270900", "B06"),
        ("270112", "B05"),
        ("020110", "C10"),
        ("520100", "C13"),
        ("290110", "C20"),
        ("850110", "C261"),
    ];
    // BACI v is thousands of USD, q is tonnes; k carries no leading zero. t,i,j,k,v,q.
    let rows = [
        "2000,1,2,10121,2,3",  // good 0: 3 t for 2000 USD
        "2000,1,3,10121,3,1",  // good 0: 1 t for 3000 USD
        "2000,2,3,10121,100,", // good 0: no q, out of numerator and denominator
        "2000,1,2,10121,0.5,", // good 0: no q, fractional value
        "2000,1,2,270900,8,2",
        "2000,1,2,270112,4,40",
        "2000,1,2,20110,5,10",
        "2000,1,2,520100,10,1",
        "2000,1,2,290110,20,4",
        "2000,1,2,850110,50,1",
        "2000,1,2,999999,7,7", // absent from the key
    ];
    let w = weights_from_baci(
        &baci(&dir.path().join("baci.zip"), &rows),
        &hs_key(&dir.path().join("key.xlsx"), &lines),
    )
    .unwrap();
    // tonnes / (v x 1000): the 100 and 0.5 kUSD rows without q must not enter the denominator.
    let expected = [
        4.0 / 5000.0,
        2.0 / 8000.0,
        40.0 / 4000.0,
        10.0 / 5000.0,
        1.0 / 10000.0,
        4.0 / 20000.0,
        1.0 / 50000.0,
    ];
    for (g, want) in expected.into_iter().enumerate() {
        assert!(approx(w[g], want, 1e-12), "good {g}: {} vs {want}", w[g]);
    }
    assert!(w[7..].iter().all(|x| *x == 0.0));
}

/// Divisions of ISIC Rev 3 codes of Table A.1 ("1-5", "36 -37", "351 - 359") as two-digit
/// divisions present in Rev 3.
fn isic_divisions(span: &str) -> Vec<String> {
    let (lo, hi) = span.split_once('-').unwrap_or((span, span));
    let two = |t: &str| -> u32 {
        format!("{:0>2}", &t.trim()[..t.trim().len().min(2)])
            .parse()
            .unwrap()
    };
    (two(lo)..=two(hi))
        .filter(|d| matches!(d, 1 | 2 | 5 | 10..=37))
        .map(|d| format!("{d:02}"))
        .collect()
}

#[test]
fn caliendo_parro_values_are_transcribed() {
    let adr = std::fs::read_to_string(
        sources::workspace_root().join("adr/0023-trade-elasticity-classes.md"),
    )
    .unwrap();
    assert!(adr.contains("Table 1") && adr.contains("p. 18") && adr.contains("99% sample"));
    let rows: Vec<(String, Vec<String>, f64, f64)> = adr
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line
                .strip_prefix("| ")?
                .strip_suffix(" |")?
                .split(" | ")
                .collect();
            let [n, name, span, theta, se] = cells[..] else {
                return None;
            };
            n.chars().all(|c| c.is_ascii_digit()).then_some(())?;
            let se = se.strip_prefix('(')?.strip_suffix(')')?;
            Some((
                name.to_owned(),
                isic_divisions(span),
                theta.parse().ok()?,
                se.parse().ok()?,
            ))
        })
        .collect();
    assert_eq!(rows.len(), 20);
    for (row, sector) in rows.iter().zip(CALIENDO_PARRO) {
        assert_eq!(row.0, sector.name);
        assert_eq!(row.1, sector.divisions, "{}", sector.name);
        assert_eq!((row.2, row.3), (sector.theta, sector.se), "{}", sector.name);
    }
    let thetas: Vec<f64> = rows.iter().map(|r| r.2).collect();
    assert_eq!(thetas.iter().copied().fold(f64::INFINITY, f64::min), 0.39);
    assert_eq!(thetas.iter().copied().fold(0.0, f64::max), 64.85);
}

#[test]
fn theta_by_good_is_weighted_by_trade() {
    let dir = tempfile::tempdir().unwrap();
    // Two light-manufacturing lines (Textile 8.10, Wood 11.50) and one food line (2.62); one line
    // out of the key.
    let lines = [("520100", "C13"), ("440310", "C16"), ("020110", "C10")];
    let rows = [
        "2000,1,2,520100,1,", // no q: still weighs by value
        "2000,1,3,520100,2,5",
        "2000,1,2,440310,1,2",
        "2000,1,2,20110,4,1",
        "2000,1,2,999999,100,1",
    ];
    let t = caliendo_parro_by_good(
        &baci(&dir.path().join("baci.zip"), &rows),
        &hs_key(&dir.path().join("key.xlsx"), &lines),
    )
    .unwrap();
    assert!(approx(t[4], (3.0 * 8.10 + 1.0 * 11.50) / 4.0, 1e-12));
    assert!(approx(t[3], 2.62, 1e-12));
    for g in [0, 1, 2, 5, 6, 7, 8, 9] {
        assert_eq!(t[g], 0.0, "good {g}");
    }
}

#[test]
fn classes_follow_the_rule() {
    // Sorted 2, 2.9 | 4, 5.9 | 8, 9, 11: the only split into 3 within a factor 1.5.
    let t = [2.0, 4.0, 5.9, 2.9, 8.0, 9.0, 11.0];
    let v = [1.0, 1.0, 3.0, 1.0, 1.0, 1.0, 2.0];
    let (of, ct) = classes(&t, &v).unwrap();
    assert_eq!(of, [0, 1, 1, 0, 2, 2, 2]);
    let want = [2.45, (4.0 + 3.0 * 5.9) / 4.0, (8.0 + 9.0 + 22.0) / 4.0];
    for (got, want) in ct.iter().zip(want) {
        assert!(approx(*got, want, 1e-12));
    }
    for c in 0..ct.len() {
        let members: Vec<f64> = t
            .iter()
            .zip(&of)
            .filter(|(_, o)| usize::from(**o) == c)
            .map(|(x, _)| *x)
            .collect();
        let (lo, hi) = (
            members.iter().copied().fold(f64::INFINITY, f64::min),
            members.iter().copied().fold(0.0, f64::max),
        );
        assert!(hi / lo <= CLASS_SPREAD);
    }
    // Fewer distinct values still give 3 classes, the split with the smallest largest spread.
    let (of, _) = classes(&[2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0], &[1.0; 7]).unwrap();
    assert_eq!(of, [0, 1, 1, 1, 2, 2, 2]); // the first such split in order
    // A spread of exactly 1.5 is inside the bound.
    let (of, _) = classes(&[1.0, 1.5, 3.0, 4.5, 9.0, 13.5, 13.5], &[1.0; 7]).unwrap();
    assert_eq!(of, [0, 0, 1, 1, 2, 2, 2]);
    let err = classes(&[1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0], &[1.0; 7]).unwrap_err();
    assert!(err.contains("5 classes"), "{err}");
}

#[test]
fn class_bands_weight_the_sector_estimates() {
    // One class holding two goods: sectors 0 and 1 (Agriculture 9.11, Mining 13.53) weighted 1:3.
    let mut a = [0.0; 20];
    let mut b = [0.0; 20];
    (a[0], b[1]) = (1.0, 3.0);
    let (centre, se) = class_bands(&[a, b], &[0, 0]);
    let (w0, w1) = (0.25, 0.75);
    assert!(approx(centre[0], w0 * 9.11 + w1 * 13.53, 1e-12));
    let var = w0 * w0 * 2.01 * 2.01 + w1 * w1 * 3.67 * 3.67;
    assert!(approx(se[0], f64::sqrt(var), 1e-12));
}

#[test]
fn every_traded_activity_of_the_key_has_a_sector_estimate() {
    let Some(key) = key() else { return };
    let traded: std::collections::BTreeSet<&String> = key
        .values()
        .filter(|c| good_of_cpa(c).is_some_and(|g| g < TRADED))
        .collect();
    let known: std::collections::BTreeSet<&str> = REV3_OF_CPA.iter().map(|(c, _)| *c).collect();
    assert!(traded.iter().all(|c| known.contains(c.as_str())));
    let divisions: std::collections::BTreeSet<&str> = CALIENDO_PARRO
        .iter()
        .flat_map(|s| s.divisions.iter().copied())
        .collect();
    assert!(REV3_OF_CPA.iter().all(|(_, d)| divisions.contains(d)));
    // Coal, crude oil and natural gas sit wholly in their goods.
    let lines = hs_goods(key);
    assert_eq!(lines["270112"], 2);
    assert_eq!(lines["270900"], 1);
    assert_eq!(lines["271121"], 1);
}

// Tests below need the downloaded BACI and key; CI is data-free.

fn key() -> Option<&'static BTreeMap<String, String>> {
    static KEY: OnceLock<Option<BTreeMap<String, String>>> = OnceLock::new();
    KEY.get_or_init(|| Some(read_hs_cpa(&cached(HS_KEY)?).unwrap()))
        .as_ref()
}

fn weights() -> Option<[f64; N_GOODS]> {
    static W: OnceLock<Option<[f64; N_GOODS]>> = OnceLock::new();
    *W.get_or_init(|| Some(weights_from_baci(&cached(BACI)?, &cached(HS_KEY)?).unwrap()))
}

fn cp() -> Option<([f64; N_GOODS], [f64; N_GOODS])> {
    static CP: OnceLock<Option<([f64; N_GOODS], [f64; N_GOODS])>> = OnceLock::new();
    *CP.get_or_init(|| Some(caliendo_parro_with_value(&cached(BACI)?, &cached(HS_KEY)?).unwrap()))
}

#[test]
fn key_lines_ascend_so_sums_run_in_the_python_order() {
    let Some(path) = cached(HS_KEY) else { return };
    let mut lines = Vec::new();
    for r in ceteris_pack::read::xlsx::sheet(&path, ceteris_pack::read::baci::KEY_SHEET).unwrap() {
        if r["HS-version"].number() == Some(0.0) {
            lines.push(r["HS-code"].to_text());
        }
    }
    assert!(lines.len() > 5000);
    assert!(lines.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn weights_are_positive_for_traded_goods() {
    let Some(w) = weights() else { return };
    // Tonnes per USD: a dropped or doubled x1000 moves every weight by 1000x, out of this band.
    assert!(
        w[..7]
            .iter()
            .all(|x| x.is_finite() && *x > 1e-5 && *x < 1e-1)
    );
}

#[test]
fn local_goods_weigh_nothing() {
    let Some(w) = weights() else { return };
    assert!(w[7..].iter().all(|x| *x == 0.0));
}

#[test]
fn bulk_is_heavier_than_machinery() {
    let Some(w) = weights() else { return };
    assert!(w[2] > 10.0 * w[6]);
}

#[test]
fn caliendo_parro_by_good_from_baci() {
    let Some((t, v)) = cp() else { return };
    let want = [9.11, 13.53, 13.53, 2.62, 9.15, 10.57, 5.19, 0.0, 0.0, 0.0];
    for (g, w) in want.into_iter().enumerate() {
        assert!((t[g] - w).abs() <= 0.005, "good {g}: {} vs {w}", t[g]);
    }
    assert!(v[..7].iter().all(|x| *x > 1e11) && v[7..].iter().all(|x| *x == 0.0));
    let (of, ct) = classes(&t[..7], &v[..7]).unwrap();
    assert_eq!(of, [2, 2, 2, 0, 2, 2, 1]);
    for (got, want) in ct.iter().zip([2.62, 5.19, 10.57]) {
        assert!((got - want).abs() <= 0.005);
    }
    assert!((ct[2] / ct[0]).ln() > CLASS_SPREAD.ln());
}

/// The fixture's goods table as exported from `TradeParams::default()`.
fn fixture_goods() -> Option<(Vec<f64>, Vec<f64>, Vec<bool>)> {
    let path = sources::workspace_root().join("tests/golden/p2-fixture/goods.arrow");
    if !path.exists() {
        eprintln!("skipping: no export in {}", path.parent()?.display());
        return None;
    }
    let reader =
        arrow_ipc::reader::FileReader::try_new(std::fs::File::open(path).unwrap(), None).unwrap();
    let (mut theta, mut class, mut traded) = (Vec::new(), Vec::new(), Vec::new());
    for batch in reader {
        let batch = batch.unwrap();
        let floats = |name: &str| {
            let a = arrow_cast::cast(
                batch.column_by_name(name).unwrap(),
                &arrow_schema::DataType::Float64,
            )
            .unwrap();
            a.as_primitive::<Float64Type>().values().to_vec()
        };
        theta.extend(floats("theta"));
        class.extend(floats("theta_class"));
        traded.extend(
            batch
                .column_by_name("traded")
                .unwrap()
                .as_boolean()
                .iter()
                .flatten(),
        );
    }
    Some((theta, class, traded))
}

#[test]
fn fixture_theta_follows_the_classes() {
    let Some((t, of, traded)) = fixture_goods() else {
        return;
    };
    for g in (0..t.len()).filter(|g| !traded[*g]) {
        assert_eq!((t[g], of[g]), (0.0, 0.0));
    }
    let used: Vec<f64> = {
        let mut u: Vec<f64> = (0..t.len()).filter(|g| traded[*g]).map(|g| of[g]).collect();
        u.sort_by(f64::total_cmp);
        u.dedup();
        u
    };
    assert!((3..=5).contains(&used.len()));
    assert!(used.iter().enumerate().all(|(i, c)| *c == i as f64));
    let thetas: Vec<f64> = used
        .iter()
        .map(|c| {
            let members: Vec<f64> = (0..t.len())
                .filter(|g| traded[*g] && of[*g] == *c)
                .map(|g| t[g])
                .collect();
            assert!(members.iter().all(|x| *x == members[0]));
            members[0]
        })
        .collect();
    assert!(thetas.is_sorted());
}

#[test]
fn fixture_theta_defaults_follow_the_estimator() {
    let (Some((t, v)), Some((ft, fof, _))) = (cp(), fixture_goods()) else {
        return;
    };
    let (of, ct) = classes(&t[..7], &v[..7]).unwrap();
    for g in 0..N_GOODS {
        let (want_class, want_theta) = if g < 7 {
            (f64::from(of[g]), ct[usize::from(of[g])])
        } else {
            (0.0, 0.0)
        };
        assert_eq!(fof[g], want_class, "class of good {g}");
        assert!((ft[g] - want_theta).abs() <= 0.005, "theta of good {g}");
    }
}

#[test]
fn h2_prior_bands_follow_the_standard_errors() {
    let (Some(key), Some(baci_path)) = (key(), cached(BACI)) else {
        return;
    };
    let lines = hs_goods(key);
    let value = baci_values(&baci_path).unwrap();
    let (t, v) = by_good(key, &lines, &value);
    let (of, ct) = classes(&t[..7], &v[..7]).unwrap();
    let (centre, se) = class_bands(&sector_values(key, &lines, &value)[..7], &of);
    for (c, w) in centre.iter().zip(&ct) {
        assert!(approx(*c, *w, 1e-12));
    }
    for (got, want) in se.iter().zip([0.61, 0.917, 1.211]) {
        assert!((got - want).abs() <= 5e-4, "{got} vs {want}");
    }
    for (got, want) in se.iter().zip([1.22, 1.83, 2.42]) {
        assert!((2.0 * got - want).abs() <= 5e-3);
    }
}
