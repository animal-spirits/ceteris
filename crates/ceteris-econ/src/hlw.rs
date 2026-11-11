//! Endogenous neutral rate (P5 Branch B3, spec/09 9.3, ADR 0176): annual one-sided
//! Holston-Laubach-Williams (2017) Kalman filter. Each year the central bank updates its estimate
//! of r* from published output, inflation and the real policy rate. Variances are inputs fixed at
//! published values; none is estimated here. The kernel is not yet in the tick.
//!
//! Annual form of the model (lags in years; the quarterly inflation lag average of 2 to 4 becomes
//! the second lag):
//! `y_t = y*_t + a1 (y_{t-1} - y*_{t-1}) + a2 (y_{t-2} - y*_{t-2})
//!        + a_r/2 (r_{t-1} - r*_{t-1} + r_{t-2} - r*_{t-2}) + e_y`,
//! `pi_t = b_pi pi_{t-1} + (1 - b_pi) pi_{t-2} + b_y (y_{t-1} - y*_{t-1}) + e_pi`,
//! `y*_t = y*_{t-1} + g_{t-1} + e_ystar`, `g_t = g_{t-1} + e_g`, `z_t = z_{t-1} + e_z`,
//! `r*_t = c g_t + z_t`. Output is log real GDP.

const N: usize = 9;
// State: [y*_t, y*_{t-1}, y*_{t-2}, g_t, g_{t-1}, g_{t-2}, z_t, z_{t-1}, z_{t-2}].
type Vector = [f64; N];
type Matrix = [[f64; N]; N];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HlwParams {
    pub a1: f64,
    pub a2: f64,
    /// Interest-rate sensitivity of the output gap (negative).
    pub a_r: f64,
    pub b_pi: f64,
    pub b_y: f64,
    /// Loading of r* on trend growth.
    pub c: f64,
    pub sigma_y_gap: f64,
    pub sigma_pi: f64,
    pub sigma_y_star: f64,
    pub sigma_g: f64,
    pub sigma_z: f64,
}

/// One annual observation: log real GDP, inflation and the ex ante real policy rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observation {
    pub y: f64,
    pub pi: f64,
    pub r: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralRateFilter {
    params: HlwParams,
    state: Vector,
    cov: Matrix,
    lags: [Observation; 2],
    frozen: bool,
}

impl NeutralRateFilter {
    /// Start at the 2000 value `r_star0` (P35) with trend growth `g0`; `lag1` and `lag2` are the
    /// two observations before the first update, `lag1` the later one.
    pub fn new(
        params: HlwParams,
        r_star0: f64,
        g0: f64,
        lag1: Observation,
        lag2: Observation,
    ) -> Self {
        let z0 = r_star0 - params.c * g0;
        let y0 = lag1.y;
        let state = [y0 + g0, y0, lag2.y, g0, g0, g0, z0, z0, z0];
        let mut cov = [[0.0; N]; N];
        for (i, var) in [4.0, 4.0, 4.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]
            .iter()
            .enumerate()
        {
            cov[i][i] = var * 2.5e-5; // sd 1pp on trend output, 0.5pp on g and z
        }
        Self {
            params,
            state,
            cov,
            lags: [lag1, lag2],
            frozen: false,
        }
    }

    /// A filter that ignores observations and keeps its initial estimate (status-quo path).
    pub fn frozen(params: HlwParams, r_star0: f64) -> Self {
        let flat = Observation {
            y: 0.0,
            pi: 0.0,
            r: 0.0,
        };
        let mut f = Self::new(params, r_star0, 0.0, flat, flat);
        f.frozen = true;
        f
    }

    /// Current estimate of r* (the rule's intercept).
    pub fn r_star(&self) -> f64 {
        self.params.c * self.state[3] + self.state[6]
    }

    pub fn trend_growth(&self) -> f64 {
        self.state[3]
    }

    /// Filtered output gap at the last update.
    pub fn output_gap(&self) -> f64 {
        self.lags[0].y - self.state[0]
    }

    /// Advance one year with the newly published `obs`; returns the new r* estimate.
    pub fn update(&mut self, obs: Observation) -> f64 {
        if self.frozen {
            return self.r_star();
        }
        let p = &self.params;
        let [l1, l2] = self.lags;

        // Predict.
        let mut f = [[0.0; N]; N];
        f[0][0] = 1.0; // y*_t = y*_{t-1} + g_{t-1}; the old state's index 3 is last year's g
        f[0][3] = 1.0;
        f[1][0] = 1.0;
        f[2][1] = 1.0;
        f[3][3] = 1.0;
        f[4][3] = 1.0;
        f[5][4] = 1.0;
        f[6][6] = 1.0;
        f[7][6] = 1.0;
        f[8][7] = 1.0;
        let x_pred = mat_vec(&f, &self.state);
        let mut q = [[0.0; N]; N];
        q[0][0] = p.sigma_y_star.powi(2);
        q[3][3] = p.sigma_g.powi(2);
        q[6][6] = p.sigma_z.powi(2);
        let mut p_pred = mat_mul(&mat_mul(&f, &self.cov), &transpose(&f));
        for i in 0..N {
            p_pred[i][i] += q[i][i];
        }

        // Observation equations as v = H x + e.
        let mut h = [[0.0; N]; 2];
        h[0][0] = 1.0;
        h[0][1] = -p.a1;
        h[0][2] = -p.a2;
        let half_ar = p.a_r / 2.0;
        h[0][4] = -half_ar * p.c;
        h[0][7] = -half_ar;
        h[0][5] = -half_ar * p.c;
        h[0][8] = -half_ar;
        h[1][1] = -p.b_y;
        let v = [
            obs.y - p.a1 * l1.y - p.a2 * l2.y - half_ar * (l1.r + l2.r),
            obs.pi - p.b_pi * l1.pi - (1.0 - p.b_pi) * l2.pi - p.b_y * l1.y,
        ];
        let r_diag = [p.sigma_y_gap.powi(2), p.sigma_pi.powi(2)];

        // Innovation covariance S = H P H' + R (2x2) and gain K = P H' S^-1.
        let ph_t: [[f64; 2]; N] = std::array::from_fn(|i| {
            std::array::from_fn(|j| (0..N).map(|k| p_pred[i][k] * h[j][k]).sum())
        });
        let mut s = [[0.0; 2]; 2];
        for i in 0..2 {
            for j in 0..2 {
                s[i][j] = (0..N).map(|k| h[i][k] * ph_t[k][j]).sum();
            }
            s[i][i] += r_diag[i];
        }
        let det = s[0][0] * s[1][1] - s[0][1] * s[1][0];
        let s_inv = [
            [s[1][1] / det, -s[0][1] / det],
            [-s[1][0] / det, s[0][0] / det],
        ];
        let k: [[f64; 2]; N] = std::array::from_fn(|i| {
            std::array::from_fn(|j| (0..2).map(|m| ph_t[i][m] * s_inv[m][j]).sum())
        });
        let innov: [f64; 2] =
            std::array::from_fn(|i| v[i] - (0..N).map(|j| h[i][j] * x_pred[j]).sum::<f64>());

        self.state = std::array::from_fn(|i| x_pred[i] + k[i][0] * innov[0] + k[i][1] * innov[1]);
        // P = (I - K H) P_pred
        let mut cov = p_pred;
        for i in 0..N {
            for j in 0..N {
                let kh: f64 = (0..2)
                    .map(|m| k[i][m] * (0..N).map(|l| h[m][l] * p_pred[l][j]).sum::<f64>())
                    .sum();
                cov[i][j] -= kh;
            }
        }
        self.cov = cov;
        self.lags = [obs, l1];
        self.r_star()
    }
}

fn mat_vec(a: &Matrix, x: &Vector) -> Vector {
    std::array::from_fn(|i| (0..N).map(|j| a[i][j] * x[j]).sum())
}

fn mat_mul(a: &Matrix, b: &Matrix) -> Matrix {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..N).map(|k| a[i][k] * b[k][j]).sum()))
}

fn transpose(a: &Matrix) -> Matrix {
    std::array::from_fn(|i| std::array::from_fn(|j| a[j][i]))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Illustrative values, not the published HLW estimates (those need the pack, ADR 0182).
    fn params() -> HlwParams {
        HlwParams {
            a1: 1.0,
            a2: -0.2,
            a_r: -0.3,
            b_pi: 0.7,
            b_y: 0.1,
            c: 1.0,
            sigma_y_gap: 0.005,
            sigma_pi: 0.005,
            sigma_y_star: 0.004,
            sigma_g: 0.0005,
            sigma_z: 0.0015,
        }
    }

    /// splitmix64 uniform in (-1, 1)
    fn noise(s: &mut u64) -> f64 {
        *s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_133E_B11B);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }

    /// Simulate the model with r* falling from 2.5% to 0.5% and a policy rate stuck at the old intercept.
    fn simulate(years: usize) -> (Vec<Observation>, Vec<f64>) {
        let p = params();
        let mut seed = 7;
        let (mut ystar, mut g, mut z) = (0.0, 0.02, 0.005);
        let mut gap = [0.0, 0.0];
        let mut hist = vec![
            Observation {
                y: 0.0,
                pi: 0.02,
                r: 0.025
            };
            2
        ];
        let mut rstar_path = vec![0.025; 2];
        let mut out = Vec::new();
        let mut truth = Vec::new();
        for t in 0..years {
            ystar += g + p.sigma_y_star * noise(&mut seed);
            g += 0.0 * noise(&mut seed);
            if (8..18).contains(&t) {
                z -= 0.002; // drift of the neutral rate
            }
            let rstar = p.c * g + z;
            let n = hist.len();
            let (l1, l2) = (hist[n - 1], hist[n - 2]);
            let rgap = 0.5 * ((l1.r - rstar_path[n - 1]) + (l2.r - rstar_path[n - 2]));
            let g0 = p.a1 * gap[0]
                + p.a2 * gap[1]
                + p.a_r * rgap
                + p.sigma_y_gap * 2.0 * noise(&mut seed);
            let y = ystar + g0;
            let pi = p.b_pi * l1.pi
                + (1.0 - p.b_pi) * l2.pi
                + p.b_y * gap[0]
                + p.sigma_pi * noise(&mut seed);
            // the bank keeps a fixed intercept, so policy turns tight as r* falls
            let r = 0.025 + 0.5 * (pi - 0.02) + 0.5 * g0;
            gap = [g0, gap[0]];
            let obs = Observation { y, pi, r };
            hist.push(obs);
            rstar_path.push(rstar);
            out.push(obs);
            truth.push(rstar);
        }
        (out, truth)
    }

    fn start() -> NeutralRateFilter {
        let flat = Observation {
            y: 0.0,
            pi: 0.02,
            r: 0.025,
        };
        NeutralRateFilter::new(params(), 0.025, 0.02, flat, flat)
    }

    #[test]
    fn frozen_filter_leaves_hashes_unchanged() {
        let mut f = NeutralRateFilter::frozen(params(), 0.02);
        let (obs, _) = simulate(30);
        for o in obs {
            assert_eq!(f.update(o).to_bits(), 0.02f64.to_bits());
        }
    }

    #[test]
    fn filter_replays_bit_identically() {
        let (obs, _) = simulate(30);
        let run = || {
            let mut f = start();
            obs.iter()
                .map(|&o| f.update(o).to_bits())
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn filter_tracks_a_two_point_fall_in_the_neutral_rate() {
        let (obs, truth) = simulate(40);
        let mut f = start();
        let est: Vec<f64> = obs.iter().map(|&o| f.update(o)).collect();
        let (e0, e1) = (est[5], est[39]);
        assert!(e0 - e1 > 0.01, "estimate fell {} -> {}", e0, e1);
        assert!(
            (est[39] - truth[39]).abs() < 0.01,
            "est {} truth {}",
            est[39],
            truth[39]
        );
    }

    #[test]
    fn steady_inputs_hold_the_estimate_at_the_initial_value() {
        let mut f = start();
        let mut y = 0.0;
        for _ in 0..30 {
            y += 0.02;
            f.update(Observation {
                y,
                pi: 0.02,
                r: 0.025,
            });
        }
        assert!((f.r_star() - 0.025).abs() < 2e-3);
        assert!(f.output_gap().abs() < 2e-3);
    }
}
