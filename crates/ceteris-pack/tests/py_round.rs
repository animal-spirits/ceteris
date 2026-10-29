//! Python's `round(x, 4)` and float `repr`, which the map's GeoJSON writes every coordinate
//! with, against values CPython computed (`scripts/parity/py_round4.py`).

use ceteris_pack::numeric::{repr_f64, round};
use proptest::prelude::*;

#[test]
fn py_round_and_repr_match_python() {
    let text = include_str!("data/py_round4.txt");
    let mut n = 0;
    for line in text.lines() {
        let [bits, rounded, repr] = line.split(' ').collect::<Vec<_>>()[..] else {
            panic!("bad line {line}");
        };
        let x = f64::from_bits(u64::from_str_radix(bits, 16).unwrap());
        assert_eq!(repr_f64(round(x, 4)), rounded, "round({repr}, 4)");
        assert_eq!(repr_f64(x), repr, "repr of {bits}");
        n += 1;
    }
    assert!(n > 1000, "{n} values");
}

proptest! {
    #[test]
    fn repr_round_trips(bits in any::<u64>()) {
        let x = f64::from_bits(bits);
        prop_assume!(x.is_finite());
        let back: f64 = repr_f64(x).parse().unwrap();
        prop_assert_eq!(back.to_bits(), bits);
    }

    #[test]
    fn rounded_coordinates_have_four_decimals_at_most(x in -180.0f64..180.0) {
        let r = round(x, 4);
        prop_assert!((r - x).abs() <= 0.5e-4 + 1e-12, "{} {}", x, r);
        prop_assert_eq!(round(r, 4).to_bits(), r.to_bits());
        let text = repr_f64(r);
        let decimals = text.split_once('.').map_or(0, |(_, f)| f.len());
        prop_assert!(decimals <= 4, "{}", text);
    }
}
