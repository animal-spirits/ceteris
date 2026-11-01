//! The calibration's report: the summary `ceteris calibrate` prints after the pass lines.

use ceteris_reference::io::{Inputs, pairwise_sum};

use crate::json::Json;
use crate::steady::{Info, Steady};

impl Info {
    /// The summary's entries: r, the passes and their history, the Sinkhorn sweeps and
    /// residual, the weight spread per good and the time.
    pub fn json(&self) -> Vec<(String, Json)> {
        let floats = |v: &[f64]| Json::List(v.iter().map(|&x| Json::Float(x)).collect());
        let history = self
            .pass_history
            .iter()
            .map(|p| {
                Json::object([
                    ("pass", Json::Int(p.pass as i64)),
                    ("change", Json::Float(p.change)),
                    ("sweeps", Json::Int(p.sweeps as i64)),
                    ("sinkhorn_residual", Json::Float(p.sinkhorn_residual)),
                ])
            })
            .collect();
        let sweeps = self
            .pass_history
            .iter()
            .map(|p| Json::Int(p.sweeps as i64))
            .collect();
        let weights = self
            .min_log10_relative_weight
            .iter()
            .map(|w| w.map_or(Json::Null, Json::Float))
            .collect();
        vec![
            ("r".into(), floats(&self.r)),
            ("passes".into(), Json::Int(self.passes as i64)),
            ("pass_history".into(), Json::List(history)),
            ("sinkhorn_sweeps".into(), Json::List(sweeps)),
            (
                "sinkhorn_residual".into(),
                Json::Float(self.sinkhorn_residual()),
            ),
            ("min_log10_relative_weight".into(), Json::List(weights)),
            ("seconds".into(), Json::Float(self.seconds)),
        ]
    }
}

/// The `percentile` of numpy (linear method) of `x` at `q` percent; NaN for an empty `x`.
fn percentile(x: &[f64], q: f64) -> f64 {
    if x.is_empty() || x.iter().any(|v| v.is_nan()) {
        return f64::NAN;
    }
    let mut sorted = x.to_vec();
    sorted.sort_by(f64::total_cmp);
    let last = sorted.len() - 1;
    let virtual_index = last as f64 * (q / 100.0);
    // numpy points both neighbours at the last value from there on and measures gamma from -1.
    let (prev, next, below) = if virtual_index >= last as f64 {
        (last, last, -1.0)
    } else {
        let below = virtual_index.floor();
        (below as usize, below as usize + 1, below)
    };
    let t = virtual_index - below;
    let (a, b) = (sorted[prev], sorted[next]);
    let diff = b - a;
    if t >= 0.5 {
        b - diff * (1.0 - t)
    } else {
        a + diff * t
    }
}

/// Full-pack report numbers: r, passes, Sinkhorn, local VA's national gap to ICIO, ln A, and the
/// countries whose between-pop income spread already exceeds the national Gini's (within-pop
/// dispersion s_w = 0, ADR 0094).
pub fn summary(inp: &Inputs, s: &Steady) -> Json {
    let n_c = inp.n_countries();
    let local: Vec<usize> = (0..inp.ent_good.len())
        .filter(|&e| !inp.traded[inp.ent_good[e]])
        .collect();
    let country = inp.ent_country();
    let target = inp.ent_va_target.as_deref().unwrap_or_default();
    let at = |v: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; n_c];
        for &e in &local {
            out[country[e]] += v[e];
        }
        out
    };
    let (model_va, data_va) = (at(&s.va0), at(target));
    let gap: Vec<f64> = (0..n_c)
        .map(|k| {
            if data_va[k] > 0.0 {
                model_va[k] / data_va[k] - 1.0
            } else {
                f64::NAN
            }
        })
        .collect();
    let has: Vec<usize> = (0..n_c).filter(|&k| data_va[k] > 0.0).collect();
    let key: Vec<f64> = (0..n_c)
        .map(|k| -(if data_va[k] > 0.0 { gap[k] } else { 0.0 }).abs())
        .collect();
    let mut order: Vec<usize> = (0..n_c).collect();
    // NaN sorts last. The sort is stable where numpy's argsort is not: only exactly tied keys
    // can differ in order.
    order.sort_by(|&a, &b| match (key[a].is_nan(), key[b].is_nan()) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => key[a].partial_cmp(&key[b]).expect("not NaN"),
    });
    let quantiles = |x: &[f64]| {
        Json::Object(
            [0u32, 1, 5, 25, 50, 75, 95, 99, 100]
                .iter()
                .map(|&q| (q.to_string(), Json::Float(percentile(x, f64::from(q)))))
                .collect(),
        )
    };
    let ln_a: Vec<f64> = s.a.iter().map(|a| a.ln()).collect();
    let ln_a_traded: Vec<f64> = (0..ln_a.len())
        .filter(|&e| inp.traded[inp.ent_good[e]])
        .map(|e| ln_a[e])
        .collect();
    let worst = order
        .iter()
        .take(10)
        .map(|&k| {
            Json::object([
                ("country", Json::Int(k as i64)),
                ("gap", Json::Float(gap[k])),
            ])
        })
        .collect();
    let gaps: Vec<f64> = has.iter().map(|&k| gap[k]).collect();
    let mut entries = s.info.json();
    entries.extend([
        ("local_va_gap_quantiles".into(), quantiles(&gaps)),
        ("local_va_gap_worst".into(), Json::List(worst)),
        (
            "local_va_world_gap".into(),
            Json::Float(pairwise_sum(&model_va) / pairwise_sum(&data_va) - 1.0),
        ),
        ("ln_a_quantiles".into(), quantiles(&ln_a)),
        ("ln_a_quantiles_traded".into(), quantiles(&ln_a_traded)),
        (
            "within_sigma_zero".into(),
            Json::List(
                (0..n_c)
                    .filter(|&k| s.weights.sigma[k] == 0.0)
                    .map(|k| Json::Int(k as i64))
                    .collect(),
            ),
        ),
    ]);
    Json::Object(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `np.percentile([3.0, 1.0, 2.0, 10.0, -4.0], q)` printed with `repr`.
    #[test]
    fn percentile_matches_numpy() {
        let x = [3.0, 1.0, 2.0, 10.0, -4.0];
        for (q, want) in [
            (0.0, -4.0),
            (1.0, -3.8),
            (5.0, -3.0),
            (25.0, 1.0),
            (50.0, 2.0),
            (75.0, 3.0),
            (95.0, 8.599999999999998),
            (99.0, 9.719999999999999),
            (100.0, 10.0),
        ] {
            assert_eq!(percentile(&x, q), want, "{q}");
        }
        assert!(percentile(&[], 50.0).is_nan());
    }
}
