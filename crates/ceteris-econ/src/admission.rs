//! International admissions under annual caps (P4 Branch D2, ADR 0109, `specs/pop/SPEC.md`,
//! Migration). A destination receives applications `A_od` from the logit, admits up to its room
//! proportionally to `A_od (1 + chi s_od)`, and a fraction `z` of the rejected migrate
//! irregularly. Counts are exact integers; `admitted + irregular + stay = A_od` per origin.
//! Kernel only: the logit (D1) and the tick system 227 are not built yet, so nothing calls this.

/// Monthly ticks per year.
const MONTHS: u64 = 12;

/// Per-destination cap `Q_d` (persons per year) and admissions so far this year.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Account {
    pub cap: u64,
    pub used: u64,
}

impl Account {
    pub const fn uncapped() -> Self {
        Self {
            cap: u64::MAX,
            used: 0,
        }
    }

    /// Admissions still available at `month` (0..12): `Q_d (month + 1) / 12` minus those used, so
    /// unspent monthly room carries within the year.
    pub fn room(&self, month: u64) -> u64 {
        let cum =
            (u128::from(self.cap) * u128::from(month % MONTHS + 1) / u128::from(MONTHS)) as u64;
        cum.saturating_sub(self.used)
    }

    pub fn record(&mut self, admitted: u64) {
        self.used += admitted;
    }

    pub fn new_year(&mut self) {
        self.used = 0;
    }
}

/// Admission outcome by origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rationed {
    pub admitted: Vec<u64>,
    pub irregular: Vec<u64>,
    pub stay: Vec<u64>,
}

/// Rations `applications` to `room` with diaspora preference `chi` on `stock_share`, then sends
/// a share `z` of the rejected to irregular migration. Deterministic: sequential, fixed order.
pub fn ration(applications: &[u64], stock_share: &[f64], room: u64, chi: f64, z: f64) -> Rationed {
    let total: u64 = applications.iter().sum();
    let admitted = if total <= room {
        applications.to_vec()
    } else {
        let w: Vec<f64> = applications
            .iter()
            .zip(stock_share)
            .map(|(&a, &s)| a as f64 * (1.0 + chi * s))
            .collect();
        proportional(applications, &w, room)
    };
    let rejected: Vec<u64> = applications
        .iter()
        .zip(&admitted)
        .map(|(a, d)| a - d)
        .collect();
    let want: Vec<f64> = rejected.iter().map(|&r| z * r as f64).collect();
    let irregular = hamilton(&want, &rejected);
    let stay = rejected
        .iter()
        .zip(&irregular)
        .map(|(r, i)| r - i)
        .collect();
    Rationed {
        admitted,
        irregular,
        stay,
    }
}

/// Splits `room` over origins in proportion to `w`, no origin above its application count
/// (water-filling), rounded to integers summing to `room`. Requires `room < sum(applications)`.
fn proportional(applications: &[u64], w: &[f64], room: u64) -> Vec<u64> {
    let k = applications.len();
    let mut exact = vec![0.0; k];
    let mut fixed: Vec<bool> = applications.iter().map(|&a| a == 0).collect();
    let mut left = room as f64;
    loop {
        let wsum: f64 = (0..k).filter(|&o| !fixed[o]).map(|o| w[o]).sum();
        let capped: Vec<usize> = (0..k)
            .filter(|&o| !fixed[o] && left * w[o] / wsum >= applications[o] as f64)
            .collect();
        if capped.is_empty() {
            for o in (0..k).filter(|&o| !fixed[o]) {
                exact[o] = left * w[o] / wsum;
            }
            break;
        }
        for o in capped {
            exact[o] = applications[o] as f64;
            fixed[o] = true;
            left -= exact[o];
        }
    }
    hamilton(&exact, applications)
}

/// Largest-remainder rounding of `exact` to integers summing to `round(sum)`; ties to the
/// lowest index, no entry above its cap.
fn hamilton(exact: &[f64], cap: &[u64]) -> Vec<u64> {
    let mut out: Vec<u64> = exact
        .iter()
        .zip(cap)
        .map(|(&x, &c)| (x as u64).min(c))
        .collect();
    let floors: u64 = out.iter().sum();
    let mut left = (libm::round(exact.iter().sum::<f64>()) as u64).saturating_sub(floors);
    let frac = |k: usize| exact[k] - libm::floor(exact[k]);
    let mut order: Vec<usize> = (0..exact.len()).collect();
    order.sort_by(|&i, &j| frac(j).total_cmp(&frac(i)).then(i.cmp(&j)));
    for k in order {
        if left == 0 {
            break;
        }
        if out[k] < cap[k] {
            out[k] += 1;
            left -= 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn infinite_caps_equal_the_pure_logit() {
        let a = [120, 0, 7, 3000];
        let r = ration(
            &a,
            &[0.1, 0.2, 0.0, 0.5],
            Account::uncapped().room(0),
            4.0,
            0.1,
        );
        assert_eq!(r.admitted, a);
        assert!(r.irregular.iter().chain(&r.stay).all(|&x| x == 0));
    }

    #[test]
    fn diaspora_weight_raises_the_admission_rate() {
        let a = [1000, 1000];
        let r = ration(&a, &[0.0, 0.5], 1000, 2.0, 0.0);
        assert_eq!(r.admitted.iter().sum::<u64>(), 1000);
        assert!(r.admitted[1] > r.admitted[0]);
        let flat = ration(&a, &[0.0, 0.5], 1000, 0.0, 0.0);
        assert_eq!(flat.admitted, [500, 500]);
    }

    #[test]
    fn a_preferred_origin_is_never_admitted_above_its_applications() {
        let r = ration(&[100, 1000], &[1.0, 0.0], 1050, 10.0, 0.0);
        assert_eq!(r.admitted, [100, 950]);
    }

    #[test]
    fn room_carries_within_the_year_and_resets() {
        let mut acc = Account { cap: 1200, used: 0 };
        assert_eq!(acc.room(0), 100);
        acc.record(40);
        assert_eq!(acc.room(1), 160);
        acc.new_year();
        assert_eq!(acc.room(0), 100);
        assert_eq!(Account { cap: 1200, used: 0 }.room(11), 1200);
    }

    proptest! {
        #[test]
        fn admissions_never_exceed_the_cap_and_shares_sum_to_one(
            a in prop::collection::vec(0u64..100_000, 1..12),
            s in prop::collection::vec(0.0f64..1.0, 12),
            room in 0u64..400_000,
            chi in 0.0f64..10.0,
            z in 0.0f64..0.2,
        ) {
            let r = ration(&a, &s[..a.len()], room, chi, z);
            let total: u64 = a.iter().sum();
            let adm: u64 = r.admitted.iter().sum();
            prop_assert_eq!(adm, total.min(room));
            for (o, &n) in a.iter().enumerate() {
                prop_assert!(r.admitted[o] <= n);
                prop_assert_eq!(r.admitted[o] + r.irregular[o] + r.stay[o], n);
            }
            if adm > 0 {
                let shares: f64 = r.admitted.iter().map(|&d| d as f64 / adm as f64).sum();
                prop_assert!((shares - 1.0).abs() < 1e-9);
            }
        }
    }
}
