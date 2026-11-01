//! The goods of a world (`specs/trade/SPEC.md`, Goods). Their count is a pack value: the rows
//! of `econ_goods`. Traded goods come first, then local ones. Goods are the innermost
//! dimension of every goods-indexed array: `[entity * n + g]`.

use std::ops::Range;

/// The goods table: names, how many are traded, which are storable, the tariff group of each
/// traded good and the final demand category of each good.
#[derive(Clone, Debug, PartialEq)]
pub struct Goods {
    pub name: Vec<String>,
    /// Goods `0..traded` move between provinces; the rest are local.
    pub traded: usize,
    /// Storable commodities (ADR 0106, under ADR 0091): their price step clips at
    /// `TradeParams::c_max_storable`, and their origin markets hold stock by competitive storage
    /// (ADR 0091) in place of the kappa2 term.
    pub storable: Vec<bool>,
    /// Tariff group of each traded good, 0 primary or 1 manufactured (`Wedges`); local goods 0
    /// (unused).
    pub group: Vec<u8>,
    /// PIGL category of each good (ADR 0021): 0 agriculture and food (A), 1 crude oil, mining
    /// and manufactures (M), 2 utilities, construction and services (S).
    pub category: Vec<u8>,
}

impl Goods {
    /// Panics on a table whose columns differ in length, more traded goods than goods, a
    /// storable local good, a tariff group other than 0 or 1, or a category above 2.
    pub fn new(
        name: Vec<String>,
        traded: usize,
        storable: Vec<bool>,
        group: Vec<u8>,
        category: Vec<u8>,
    ) -> Self {
        let n = name.len();
        assert!(
            storable.len() == n && group.len() == n && category.len() == n && traded <= n,
            "goods table: {n} names, {} storable, {} groups, {} categories, {traded} traded",
            storable.len(),
            group.len(),
            category.len()
        );
        assert!(
            !storable[traded..].contains(&true),
            "a local good is storable"
        );
        assert!(group.iter().all(|&t| t < 2), "tariff groups are 0 or 1");
        assert!(category.iter().all(|&c| c < 3), "categories are 0 to 2");
        Self {
            name,
            traded,
            storable,
            group,
            category,
        }
    }

    /// The ten goods of P2 (`specs/econ/SPEC.md`, goods table), which the fixture and the 2000
    /// pack use.
    pub fn p2() -> Self {
        Self::new(
            p2::NAME.map(String::from).to_vec(),
            p2::TRADED,
            p2::STORABLE.to_vec(),
            (0..p2::N)
                .map(|g| u8::from((4..p2::TRADED).contains(&g)))
                .collect(),
            p2::CATEGORY.to_vec(),
        )
    }

    pub fn n(&self) -> usize {
        self.name.len()
    }

    pub fn is_traded(&self, g: usize) -> bool {
        g < self.traded
    }

    pub fn traded_goods(&self) -> Range<usize> {
        0..self.traded
    }

    pub fn local_goods(&self) -> Range<usize> {
        self.traded..self.n()
    }

    pub fn n_local(&self) -> usize {
        self.n() - self.traded
    }

    /// Tariff group of a traded good.
    pub fn tariff_group(&self, g: usize) -> usize {
        assert!(self.is_traded(g), "good {g} is not traded");
        usize::from(self.group[g])
    }
}

/// The P2 goods, which the fixture and the 2000 pack use.
impl Default for Goods {
    fn default() -> Self {
        Self::p2()
    }
}

/// The P2 goods table as constants, for the fixture, the parameter defaults and tests. Code
/// that runs on a pack reads [`Goods`] instead.
pub mod p2 {
    pub const N: usize = 10;
    /// Goods 0 to 6 move between provinces; utilities, construction and services are local.
    pub const TRADED: usize = 7;
    /// Agriculture, crude oil, gas and refined fuels, coal and other mining, and chemicals and
    /// metals.
    pub const STORABLE: [bool; N] = [
        true, true, true, false, false, true, false, false, false, false,
    ];
    pub const CATEGORY: [u8; N] = [0, 1, 1, 0, 1, 1, 1, 2, 2, 2];
    pub const NAME: [&str; N] = [
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_split_primary_and_manufactured() {
        let goods = Goods::p2();
        let group = |r: Range<usize>| r.map(|g| goods.tariff_group(g)).collect::<Vec<_>>();
        assert_eq!(group(0..4), [0; 4]);
        assert_eq!(group(4..7), [1; 3]);
    }

    #[test]
    #[should_panic(expected = "not traded")]
    fn local_goods_have_no_tariff_group() {
        Goods::p2().tariff_group(7);
    }

    #[test]
    fn p2_has_seven_traded_and_three_local_goods() {
        let goods = Goods::p2();
        assert_eq!(
            (goods.n(), goods.traded_goods(), goods.local_goods()),
            (10, 0..7, 7..10)
        );
    }

    #[test]
    #[should_panic(expected = "local good is storable")]
    fn a_storable_local_good_is_refused() {
        let mut storable = vec![false; 3];
        storable[2] = true;
        let name = vec!["a".into(), "b".into(), "c".into()];
        Goods::new(name, 2, storable, vec![0; 3], vec![0; 3]);
    }
}
