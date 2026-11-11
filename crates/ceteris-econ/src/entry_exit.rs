//! Firm entry, exit and the zombie band for one enterprise cell (P5 Branch D2, ADR 0202, DESIGN 7.4).
//! The cell holds a mass `M` of firms with Pareto productivity above a cutoff `phi*`, shape `k`.
//! Quarterly: entry `E = eta_e M max(0, V_e/(w f_e) - 1)`; exit `X = delta M` plus the cutoff
//! exit `M [1 - (phi_old/phi_new)^k]+`; forbearance lowers the dominant cutoff to
//! `phi_z = phi* (1 - z_b)`, so the zombie share is `1 - (1 - z_b)^k`. The kernel is not yet in
//! the tick; `z_b` comes from the bank capital gap (Branch F) and is 0 until then.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntryExit {
    /// Entry responsiveness (P-register `eta_e`).
    pub eta_e: f64,
    /// Quarterly firm death rate.
    pub delta: f64,
    /// Pareto shape of the productivity tail.
    pub k: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub mass: f64,
    pub cutoff: f64,
}

impl EntryExit {
    pub fn entry(&self, mass: f64, value_over_cost: f64) -> f64 {
        self.eta_e * mass * (value_over_cost - 1.0).max(0.0)
    }

    /// Share of the mass removed by a cutoff move; zero when the cutoff falls.
    pub fn cutoff_exit_share(&self, old: f64, new: f64) -> f64 {
        (1.0 - libm::pow(old / new, self.k)).max(0.0)
    }

    pub fn zombie_share(&self, z_b: f64) -> f64 {
        1.0 - libm::pow(1.0 - z_b, self.k)
    }

    /// One quarter. `value_over_cost` is `V_e / (w f_e)`; deaths and cutoff exit are taken on the
    /// opening mass and entrants join after the step.
    pub fn step(&self, cell: Cell, value_over_cost: f64, new_cutoff: f64) -> Cell {
        let entrants = self.entry(cell.mass, value_over_cost);
        let survivors = cell.mass
            * (1.0 - self.delta).max(0.0)
            * (1.0 - self.cutoff_exit_share(cell.cutoff, new_cutoff));
        Cell {
            mass: survivors + entrants,
            cutoff: new_cutoff,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: EntryExit = EntryExit {
        eta_e: 0.5,
        delta: 0.02,
        k: 3.4,
    };

    /// Free-entry value falls with congestion in the cell's labour: `V/(w f) = v0 L / (M lbar)`.
    /// Trade costs do not enter the entry condition (Melitz nesting), so they are absent here.
    fn settle(labour: f64) -> f64 {
        let (v0, lbar) = (1.3, 2.0);
        let mut c = Cell {
            mass: 1.0,
            cutoff: 1.0,
        };
        for _ in 0..20_000 {
            c = R.step(c, v0 * labour / (c.mass * lbar), 1.0);
        }
        c.mass
    }

    #[test]
    fn free_entry_mass_scales_with_labour_and_not_trade_costs() {
        let m1 = settle(100.0);
        let m2 = settle(200.0);
        assert!((m2 / m1 - 2.0).abs() < 1e-9);
        // closed form: M = v0 L / (lbar (1 + delta/eta_e)) up to the death-survival factor
        let expect = 1.3 * 100.0 / 2.0 / (1.0 + R.delta / (R.eta_e * (1.0 - R.delta)));
        assert!((m1 / expect - 1.0).abs() < 0.03, "{m1} vs {expect}");
    }

    #[test]
    fn cutoff_rise_removes_exactly_the_share() {
        let c = Cell {
            mass: 1000.0,
            cutoff: 1.0,
        };
        let r = EntryExit { delta: 0.0, ..R };
        let next = r.step(c, 1.0, 1.25);
        let share = 1.0 - libm::pow(1.0 / 1.25, R.k);
        assert!((next.mass - c.mass * (1.0 - share)).abs() < 1e-12);
        assert_eq!(r.cutoff_exit_share(1.0, 0.8), 0.0);
    }

    #[test]
    fn zero_bank_gap_gives_no_zombies_and_share_rises_with_it() {
        assert_eq!(R.zombie_share(0.0), 0.0);
        assert!(R.zombie_share(0.1) > R.zombie_share(0.05));
        assert!(R.zombie_share(1.0) == 1.0);
    }

    #[test]
    fn no_entry_without_surplus_and_no_exit_at_zero_rates() {
        let r = EntryExit { delta: 0.0, ..R };
        let c = Cell {
            mass: 10.0,
            cutoff: 1.0,
        };
        assert_eq!(r.step(c, 0.9, 1.0), c);
    }
}
