//! Energy policy held at t0 (P5 Branch C3, ADR 0278, `specs/money/SPEC.md`). Fuel excises and
//! subsidies are per-unit wedges on goods B06, C19 and D at their 2000 rates per state, held for
//! the run like trade policy. A positive wedge is an excise paid to the government, a negative one
//! a subsidy that is a fiscal cost at wedge times quantity. The kernel is not yet in the tick.

/// Crude oil and gas extraction (B06), refined fuels (C19), electricity (D).
pub const N_ENERGY_GOODS: usize = 3;

/// Per-unit wedges of one state in money units per physical unit; all zero is the status quo.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EnergyWedges {
    pub wedge: [i128; N_ENERGY_GOODS],
}

impl EnergyWedges {
    /// Price the buyer pays: the producer price plus the wedge.
    pub fn buyer_price(&self, good: usize, producer_price: i128) -> i128 {
        producer_price + self.wedge[good]
    }

    /// Excise revenue `sum_g max(w_g, 0) q_g`.
    pub fn excise_revenue(&self, quantity: &[f64; N_ENERGY_GOODS]) -> i128 {
        self.sum(quantity, |w| w.max(0))
    }

    /// Subsidy cost `sum_g max(-w_g, 0) q_g`, a fiscal cost.
    pub fn subsidy_cost(&self, quantity: &[f64; N_ENERGY_GOODS]) -> i128 {
        self.sum(quantity, |w| (-w).max(0))
    }

    fn sum(&self, quantity: &[f64; N_ENERGY_GOODS], part: impl Fn(i128) -> i128) -> i128 {
        self.wedge
            .iter()
            .zip(quantity)
            .map(|(&w, &q)| ((part(w) as f64) * q).round() as i128)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_wedges_off_equals_status_quo() {
        let w = EnergyWedges::default();
        let q = [1.0e6, 2.5e5, 7.0e7];
        for g in 0..N_ENERGY_GOODS {
            assert_eq!(w.buyer_price(g, 1234), 1234);
        }
        assert_eq!(w.excise_revenue(&q), 0);
        assert_eq!(w.subsidy_cost(&q), 0);
    }

    #[test]
    fn subsidy_cost_equals_wedge_times_quantity() {
        let w = EnergyWedges { wedge: [-3, 5, -2] };
        let q = [1000.0, 400.0, 50.0];
        assert_eq!(w.subsidy_cost(&q), 3 * 1000 + 2 * 50);
        assert_eq!(w.excise_revenue(&q), 5 * 400);
        assert_eq!(w.buyer_price(0, 100), 97);
        assert_eq!(w.buyer_price(1, 100), 105);
    }
}
