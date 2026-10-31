//! The ten goods of P2 (`specs/econ/SPEC.md`, goods table). Goods are the innermost
//! dimension of every goods-indexed array: `[entity * N_GOODS + g]`.

pub const N_GOODS: usize = 10;

/// Goods 0 to 6 move between provinces; utilities, construction and services are local.
pub const TRADED: [bool; N_GOODS] = [
    true, true, true, true, true, true, true, false, false, false,
];

/// Storable commodities (ADR 0106, under ADR 0091): agriculture, crude oil, gas and refined fuels,
/// coal and other mining, and chemicals and metals. Their price step clips at
/// `TradeParams::c_max_storable`, and their origin markets hold stock by competitive storage
/// (ADR 0091) in place of the kappa2 term.
pub const STORABLE: [bool; N_GOODS] = [
    true, true, true, false, false, true, false, false, false, false,
];

pub const NAME: [&str; N_GOODS] = [
    "Agriculture",
    "Crude oil, gas and refined fuels",
    "Coal and other mining",
    "Food",
    "Light manufacturing",
    "Chemicals and metals",
    "Machinery, electronics and vehicles",
    "Electricity and utilities",
    "Construction",
    "Services",
];

/// Tariff group of a traded good: 0 primary (goods 0 to 3), 1 manufactured (goods 4 to 6).
pub fn tariff_group(g: usize) -> usize {
    assert!(TRADED[g], "good {g} is not traded");
    usize::from(g >= 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_split_primary_and_manufactured() {
        assert_eq!((0..4).map(tariff_group).collect::<Vec<_>>(), [0; 4]);
        assert_eq!((4..7).map(tariff_group).collect::<Vec<_>>(), [1; 3]);
    }

    #[test]
    #[should_panic(expected = "not traded")]
    fn local_goods_have_no_tariff_group() {
        tariff_group(7);
    }
}
