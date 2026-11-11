//! Structural change (P4 Branch E, ADR 0031, `specs/pop/SPEC.md`): a sector-class factor on the
//! productivity catch-up rate (P162) and informal to formal transitions scaled by enforcement
//! capacity (P163). Not yet in the tick: the catch-up rate lambda is ADR 0108's (still zero), and
//! P162, P163 have no register codes.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectorClass {
    Manufacturing,
    TradableServices,
    NonTradableServices,
    Agriculture,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClassCatchUp {
    /// Off: `step` leaves productivity as it is.
    pub enabled: bool,
    /// Base monthly catch-up rate (ADR 0108; zero until it is built).
    pub lambda: f64,
    /// Class factor on `lambda`, indexed by `SectorClass` (P162); all ones is the common rate.
    pub factor: [f64; 4],
}

impl ClassCatchUp {
    pub fn new(lambda: f64) -> Self {
        Self {
            enabled: false,
            lambda,
            factor: [1.0; 4],
        }
    }

    /// One monthly step: `a` moves towards `frontier * ceiling` at rate `lambda * factor[class]`.
    /// A ceiling of 1 is unconditional convergence; below 1 it is the conditional level.
    pub fn step(&self, frontier: f64, a: &mut [f64], class: &[SectorClass], ceiling: &[f64]) {
        if !self.enabled {
            return;
        }
        for i in 0..a.len() {
            let rate = (self.lambda * self.factor[class[i] as usize]).clamp(0.0, 1.0);
            a[i] += rate * (frontier * ceiling[i] - a[i]);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Formalization {
    /// Off: `step` leaves everything as it is.
    pub enabled: bool,
    /// Monthly turnover scale of both directions.
    pub rate: f64,
    /// Benefit of formality (credit access, contract enforcement), in log-payoff units.
    pub benefit: f64,
    /// Cost of formality (taxes, regulation).
    pub cost: f64,
    /// Payoff loss from informality under full enforcement; scaled by the province's enforcement.
    pub penalty: f64,
    /// Logit dispersion.
    pub omega: f64,
    /// Formal workers per province.
    pub formal: Vec<u64>,
    pub informal: Vec<u64>,
}

impl Formalization {
    pub fn new(formal: Vec<u64>, informal: Vec<u64>) -> Self {
        Self {
            enabled: false,
            rate: 0.0,
            benefit: 0.0,
            cost: 0.0,
            penalty: 0.0,
            omega: 1.0,
            formal,
            informal,
        }
    }

    /// Stationary formal share at enforcement `e` in `[0, 1]`.
    pub fn target_share(&self, e: f64) -> f64 {
        let gain = (self.benefit + e * self.penalty - self.cost) / self.omega;
        1.0 / (1.0 + libm::exp(-gain))
    }

    /// One monthly step given enforcement capacity per province. Workers only change status.
    pub fn step(&mut self, enforcement: &[f64]) {
        if !self.enabled {
            return;
        }
        for (p, &e) in enforcement.iter().enumerate() {
            let q = self.target_share(e);
            let up = libm::round(self.rate * q * self.informal[p] as f64) as u64;
            let down = libm::round(self.rate * (1.0 - q) * self.formal[p] as f64) as u64;
            self.informal[p] = self.informal[p] - up + down;
            self.formal[p] = self.formal[p] + up - down;
        }
    }

    pub fn formal_share(&self, p: usize) -> f64 {
        self.formal[p] as f64 / (self.formal[p] + self.informal[p]) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use SectorClass::*;

    fn dispersion(x: &[f64]) -> f64 {
        let l: Vec<f64> = x.iter().map(|v| libm::log(*v)).collect();
        let m = l.iter().sum::<f64>() / l.len() as f64;
        l.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / l.len() as f64
    }

    #[test]
    fn class_factors_one_equal_status_quo() {
        let class = [Manufacturing, Agriculture];
        let ceil = [1.0, 1.0];
        let mut off = vec![0.2, 0.5];
        ClassCatchUp::new(0.02).step(1.0, &mut off, &class, &ceil);
        assert_eq!(off, [0.2, 0.5]);
        let mut on = ClassCatchUp::new(0.02);
        on.enabled = true;
        let (mut a, mut b) = ([0.2, 0.5], [0.2, 0.5]);
        for _ in 0..12 {
            on.step(1.0, &mut a, &class, &ceil);
            for x in &mut b {
                *x += 0.02 * (1.0 - *x);
            }
        }
        assert_eq!(a, b);
    }

    #[test]
    fn manufacturing_converges_while_aggregate_does_not() {
        // Five countries, equal rates; manufacturing ceiling 1 everywhere, agriculture ceiling set
        // by the country (conditional). Aggregate = fixed shares, with poor countries agricultural.
        let mut c = ClassCatchUp::new(0.01);
        c.enabled = true;
        c.factor = [2.0, 1.0, 1.0, 0.5];
        let start = [0.1, 0.2, 0.4, 0.7, 1.0];
        let ag_ceiling = [0.1, 0.2, 0.4, 0.7, 1.0];
        let share_m = [0.1, 0.2, 0.3, 0.4, 0.5];
        let mut m = start.to_vec();
        let mut ag = start.to_vec();
        let agg = |m: &[f64], ag: &[f64]| -> Vec<f64> {
            (0..5)
                .map(|i| share_m[i] * m[i] + (1.0 - share_m[i]) * ag[i])
                .collect()
        };
        let (m0, g0) = (dispersion(&m), dispersion(&agg(&m, &ag)));
        for _ in 0..12 * 24 {
            c.step(1.0, &mut m, &[Manufacturing; 5], &[1.0; 5]);
            c.step(1.0, &mut ag, &[Agriculture; 5], &ag_ceiling);
        }
        assert!(dispersion(&m) < 0.05 * m0);
        assert!(dispersion(&agg(&m, &ag)) > 0.5 * g0);
    }

    fn toy() -> Formalization {
        let mut f = Formalization::new(vec![3000, 3000], vec![7000, 7000]);
        f.rate = 0.2;
        f.benefit = 0.2;
        f.cost = 0.6;
        f.penalty = 1.0;
        f.omega = 0.5;
        f
    }

    #[test]
    fn formalization_off_equals_status_quo() {
        let mut f = toy();
        let before = f.clone();
        f.step(&[0.0, 1.0]);
        assert_eq!(f, before);
    }

    #[test]
    fn enforcement_raises_formal_employment_in_a_toy_state() {
        let mut f = toy();
        f.enabled = true;
        for _ in 0..120 {
            f.step(&[0.1, 0.9]);
            assert_eq!(f.formal[0] + f.informal[0], 10_000);
            assert_eq!(f.formal[1] + f.informal[1], 10_000);
        }
        assert!(f.formal_share(1) > f.formal_share(0));
        for (p, e) in [(0, 0.1), (1, 0.9)] {
            assert!((f.formal_share(p) - f.target_share(e)).abs() < 0.01);
        }
    }
}
