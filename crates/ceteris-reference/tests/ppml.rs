#![allow(clippy::disallowed_methods)] // independent oracle: deliberately uses std, never feeds a hash
//! PPML on synthetic gravity: the dummy-variable fit and the absorbed fixed effects recover known
//! elasticities, keep zero flows, drop separated rows and agree with each other.

use ceteris_reference::ppml::{self, GravityTable, PpmlError, RunFlows};

/// splitmix64 draws without a dependency.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn normal(&mut self, mean: f64, sd: f64) -> f64 {
        let (u, v) = (self.uniform(f64::EPSILON, 1.0), self.uniform(0.0, 1.0));
        mean + sd * (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }

    /// Knuth's product of uniforms, over chunks of mean at most 30.
    fn poisson(&mut self, lambda: f64) -> f64 {
        let chunks = (lambda / 30.0).ceil().max(1.0);
        let limit = (-lambda / chunks).exp();
        let mut k = 0.0;
        for _ in 0..chunks as usize {
            let mut p = self.uniform(0.0, 1.0);
            while p > limit {
                k += 1.0;
                p *= self.uniform(0.0, 1.0);
            }
        }
        k
    }
}

/// The ordered pairs of n countries without the domestic ones, exporter major.
fn foreign_pairs(n: usize) -> (Vec<usize>, Vec<usize>) {
    (0..n)
        .flat_map(|o| (0..n).filter(move |&d| d != o).map(move |d| (o, d)))
        .unzip()
}

/// Synthetic Poisson gravity: pair flows exp(a_i + b_j + beta ln d + contig c) with unit-mean
/// noise, scaled to `scale` on average over foreign pairs. Row-major n x n flows, distance and
/// contiguity.
struct World {
    n: usize,
    flows: Vec<f64>,
    dist: Vec<f64>,
    contig: Vec<f64>,
}

fn gravity_world(n: usize, beta: f64, contig: f64, scale: f64, seed: u64) -> World {
    let mut rng = Draws(seed);
    let xy: Vec<(f64, f64)> = (0..n)
        .map(|_| (rng.uniform(0.0, 10.0), rng.uniform(0.0, 10.0)))
        .collect();
    let mut dist = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            dist[i * n + j] = 500.0 * (1.0 + (xy[i].0 - xy[j].0).hypot(xy[i].1 - xy[j].1));
        }
    }
    let mut off: Vec<f64> = (0..n * n)
        .filter(|i| i / n != i % n)
        .map(|i| dist[i])
        .collect();
    off.sort_by(f64::total_cmp);
    let near = off[off.len() / 10];
    let c: Vec<f64> = (0..n * n)
        .map(|i| f64::from(u8::from(i / n != i % n && dist[i] < near)))
        .collect();
    let a: Vec<f64> = (0..n).map(|_| rng.normal(0.0, 0.7)).collect();
    let b: Vec<f64> = (0..n).map(|_| rng.normal(0.0, 0.7)).collect();
    let mut mu: Vec<f64> = (0..n * n)
        .map(|i| (a[i / n] + b[i % n] + beta * dist[i].ln() + contig * c[i]).exp())
        .collect();
    let mean = (0..n * n)
        .filter(|i| i / n != i % n)
        .map(|i| mu[i])
        .sum::<f64>()
        / (n * (n - 1)) as f64;
    for m in &mut mu {
        *m *= scale / mean;
    }
    let flows = mu.iter().map(|&m| rng.poisson(m)).collect();
    World {
        n,
        flows,
        dist,
        contig: c,
    }
}

impl World {
    fn at(&self, v: &[f64], o: &[usize], d: &[usize]) -> Vec<f64> {
        o.iter().zip(d).map(|(&o, &d)| v[o * self.n + d]).collect()
    }
}

fn filter<T: Copy>(v: &[T], keep: &[bool]) -> Vec<T> {
    v.iter()
        .zip(keep)
        .filter(|(_, k)| **k)
        .map(|(x, _)| *x)
        .collect()
}

fn close(got: f64, want: f64, tol: f64, what: &str) {
    assert!((got - want).abs() <= tol, "{what}: {got} vs {want}");
}

#[test]
fn ppml_recovers_known_elasticity() {
    let w = gravity_world(40, -0.9, 0.5, 60.0, 11);
    let (o, d) = foreign_pairs(w.n);
    let ln_dist: Vec<f64> = w.at(&w.dist, &o, &d).iter().map(|d| d.ln()).collect();
    let coef = ppml::fit(
        &w.at(&w.flows, &o, &d),
        &o,
        &d,
        &[("ln_dist", &ln_dist), ("contig", &w.at(&w.contig, &o, &d))],
    )
    .unwrap();
    close(coef[0], -0.9, 0.02, "ln_dist");
    close(coef[1], 0.5, 0.1, "contig");
}

#[test]
fn ppml_scale_invariant_and_keeps_zeros() {
    let w = gravity_world(25, -0.9, 0.5, 0.8, 11); // many zero flows
    let (o, d) = foreign_pairs(w.n);
    let flows = w.at(&w.flows, &o, &d);
    assert!(flows.iter().filter(|&&f| f == 0.0).count() as f64 > 0.3 * flows.len() as f64);
    let ln_dist: Vec<f64> = w.at(&w.dist, &o, &d).iter().map(|d| d.ln()).collect();
    let reg = [("ln_dist", ln_dist.as_slice())];
    let base = ppml::fit(&flows, &o, &d, &reg).unwrap();
    let big: Vec<f64> = flows.iter().map(|f| 1e6 * f).collect();
    let scaled = ppml::fit(&big, &o, &d, &reg).unwrap();
    close(scaled[0], base[0], 1e-8, "scaled");
    let robust = ppml::fit_robust(&flows, &o, &d, &reg).unwrap();
    close(robust.coef("ln_dist"), base[0], 1e-12, "robust");
    let se = robust.se("ln_dist");
    assert!(0.0 < se && se < 0.2, "{se}");
}

#[test]
fn ppml_drops_countries_without_any_flow() {
    let n = 20;
    let mut w = gravity_world(n, -0.9, 0.5, 60.0, 11);
    w.flows[3 * n..4 * n].fill(0.0); // country 3 never exports: its effect diverges unless dropped
    let (o, d) = foreign_pairs(n);
    let flows = w.at(&w.flows, &o, &d);
    let ln_dist: Vec<f64> = w.at(&w.dist, &o, &d).iter().map(|d| d.ln()).collect();
    let contig = w.at(&w.contig, &o, &d);
    let got = ppml::fit(
        &flows,
        &o,
        &d,
        &[("ln_dist", &ln_dist), ("contig", &contig)],
    )
    .unwrap();
    let on = |keep: &[bool]| {
        ppml::fit(
            &filter(&flows, keep),
            &filter(&o, keep),
            &filter(&d, keep),
            &[
                ("ln_dist", &filter(&ln_dist, keep)),
                ("contig", &filter(&contig, keep)),
            ],
        )
        .unwrap()
    };
    // Exactly the fit on the explicitly filtered rows: country 3 stays as an importer.
    let keep: Vec<bool> = o.iter().map(|&o| o != 3).collect();
    assert_eq!(filter(&d, &keep).iter().filter(|&&d| d == 3).count(), n - 1);
    let want = on(&keep);
    for k in 0..2 {
        close(got[k], want[k], 1e-10, "filtered");
    }
    // Dropping the country altogether is a different fit, so its importer rows are used.
    let both: Vec<bool> = o.iter().zip(&d).map(|(&o, &d)| o != 3 && d != 3).collect();
    assert!((on(&both)[0] - want[0]).abs() > 1e-6);
}

/// The run's flows split over goods (domestic rows present, to be ignored) and the full CEPII
/// table of `w`, as owned columns.
struct Tables {
    exporter: Vec<usize>,
    importer: Vec<usize>,
    value: Vec<f64>,
    iso3: Vec<String>,
    iso3_o: Vec<String>,
    iso3_d: Vec<String>,
    distw: Vec<f64>,
    contig: Vec<f64>,
}

fn tables(w: &World) -> Tables {
    let n = w.n;
    let iso3: Vec<String> = (0..n).map(|i| format!("C{i:02}")).collect();
    let (mut exporter, mut importer, mut value) = (vec![], vec![], vec![]);
    for share in [1.0 / 3.0, 2.0 / 3.0] {
        for i in (0..n * n).filter(|&i| w.flows[i] > 0.0) {
            exporter.push(i / n);
            importer.push(i % n);
            value.push(w.flows[i] * share);
        }
    }
    Tables {
        exporter,
        importer,
        value,
        iso3_o: (0..n * n).map(|i| iso3[i / n].clone()).collect(),
        iso3_d: (0..n * n).map(|i| iso3[i % n].clone()).collect(),
        iso3,
        distw: w.dist.clone(),
        contig: w.contig.clone(),
    }
}

impl Tables {
    fn elasticity(&self, keep: &[bool]) -> Result<ppml::Elasticity, PpmlError> {
        fn strs(v: &[String]) -> Vec<&str> {
            v.iter().map(String::as_str).collect()
        }
        let (o, d) = (strs(&self.iso3_o), strs(&self.iso3_d));
        ppml::distance_elasticity(
            &RunFlows {
                exporter_country: &self.exporter,
                importer_country: &self.importer,
                value: &self.value,
            },
            &GravityTable {
                iso3_o: &filter(&o, keep),
                iso3_d: &filter(&d, keep),
                distw: &filter(&self.distw, keep),
                contig: &filter(&self.contig, keep),
            },
            &strs(&self.iso3),
        )
    }
}

#[test]
fn ppml_reports_separated_rows_and_rejects_empty() {
    let n = 20;
    let mut w = gravity_world(n, -0.9, 0.5, 60.0, 11);
    w.flows[3 * n..4 * n].fill(0.0);
    let t = tables(&w);
    let res = t.elasticity(&vec![true; n * n]).unwrap();
    assert_eq!(res.n_separated_rows, n - 1);
    assert_eq!(
        (res.n_separated_exporters, res.n_separated_importers),
        (1, 0)
    );
    assert_eq!((res.n_pairs, res.n_dropped), (n * (n - 1) - (n - 1), 0));
    let (o, d) = foreign_pairs(n);
    let ln_dist: Vec<f64> = w.at(&w.dist, &o, &d).iter().map(|d| d.ln()).collect();
    let err = ppml::fit(&vec![0.0; o.len()], &o, &d, &[("ln_dist", &ln_dist)]).unwrap_err();
    assert!(
        matches!(&err, PpmlError::Invalid(s) if s.contains("survives")),
        "{err:?}"
    );
    let err = ppml::fit(&[], &[], &[], &[("ln_dist", &[])]).unwrap_err();
    assert_eq!(err, PpmlError::Invalid("no observations".into()));
}

/// ADR 0018, H2 per class: PPML on gravity with tariffs recovers theta from the coefficient on
/// ln(1 + t).
#[test]
fn ppml_recovers_trade_elasticity() {
    let mut rng = Draws(5);
    let (n, theta, beta) = (40, 6.0, -1.0);
    let xy: Vec<(f64, f64)> = (0..n)
        .map(|_| (rng.uniform(0.0, 10.0), rng.uniform(0.0, 10.0)))
        .collect();
    let dist: Vec<f64> = (0..n * n)
        .map(|i| 500.0 * (1.0 + (xy[i / n].0 - xy[i % n].0).hypot(xy[i / n].1 - xy[i % n].1)))
        .collect();
    let tariff: Vec<f64> = (0..n * n).map(|_| rng.uniform(0.0, 0.3)).collect();
    let a: Vec<f64> = (0..n).map(|_| rng.normal(0.0, 0.7)).collect();
    let b: Vec<f64> = (0..n).map(|_| rng.normal(0.0, 0.7)).collect();
    let mut mu: Vec<f64> = (0..n * n)
        .map(|i| (a[i / n] + b[i % n] - theta * tariff[i].ln_1p() + beta * dist[i].ln()).exp())
        .collect();
    let mean = (0..n * n)
        .filter(|i| i / n != i % n)
        .map(|i| mu[i])
        .sum::<f64>()
        / (n * (n - 1)) as f64;
    for m in &mut mu {
        *m *= 200.0 / mean;
    }
    let flows: Vec<f64> = mu.iter().map(|&m| rng.poisson(m)).collect();
    let (o, d) = foreign_pairs(n);
    let at = |v: &[f64]| -> Vec<f64> { o.iter().zip(&d).map(|(&o, &d)| v[o * n + d]).collect() };
    let res = ppml::trade_elasticity(&at(&flows), &o, &d, &at(&tariff), &at(&dist)).unwrap();
    close(res.theta, theta, 0.05, "theta");
    close(res.distance, beta, 0.02, "distance");
    assert!(0.0 < res.se_theta && res.se_theta < 0.5 && res.n_pairs == n * (n - 1));
}

#[test]
fn distance_elasticity_from_run_flows() {
    let w = gravity_world(40, -0.9, 0.5, 60.0, 11);
    let mut t = tables(&w);
    let all = vec![true; 40 * 40];
    let res = t.elasticity(&all).unwrap();
    close(res.elasticity, -0.9, 0.02, "elasticity");
    close(res.contiguity, 0.5, 0.1, "contiguity");
    assert_eq!((res.n_dropped, res.n_pairs), (0, 40 * 39));
    // Same estimate when the table carries only some of the pairs: missing and nonpositive
    // distance are dropped.
    let mut keep = all;
    keep[5] = false;
    t.distw[100] = f64::NAN;
    t.distw[200] = 0.0;
    let thin = t.elasticity(&keep).unwrap();
    assert_eq!((thin.n_dropped, thin.n_pairs), (3, 40 * 39 - 3));
    close(thin.elasticity, res.elasticity, 0.02, "thin");
}

/// Two-way Poisson design: n countries, two regressors, exporter and importer effects.
struct TwoWay {
    y: Vec<f64>,
    o: Vec<usize>,
    d: Vec<usize>,
    x1: Vec<f64>,
    x2: Vec<f64>,
}

fn two_way(n: usize, seed: u64) -> TwoWay {
    let mut rng = Draws(seed);
    let (o, d) = foreign_pairs(n);
    let x1: Vec<f64> = o.iter().map(|_| rng.normal(0.0, 1.0)).collect();
    let x2: Vec<f64> = o.iter().map(|_| rng.uniform(0.0, 1.0)).collect();
    let a: Vec<f64> = (0..n).map(|_| rng.normal(0.0, 1.0)).collect();
    let b: Vec<f64> = (0..n).map(|_| rng.normal(0.0, 1.0)).collect();
    let y = (0..o.len())
        .map(|i| rng.poisson((2.0 + a[o[i]] + b[d[i]] - 0.8 * x1[i] + 0.5 * x2[i]).exp()))
        .collect();
    TwoWay { y, o, d, x1, x2 }
}

fn codes(v: &[usize]) -> Vec<i64> {
    v.iter().map(|&i| i as i64).collect()
}

impl TwoWay {
    fn reg(&self) -> [(&str, &[f64]); 2] {
        [("x1", &self.x1), ("x2", &self.x2)]
    }
}

#[test]
fn hdfe_matches_the_dummy_variable_fit() {
    let t = two_way(25, 3);
    let robust = ppml::fit_robust(&t.y, &t.o, &t.d, &t.reg()).unwrap();
    let (o, d) = (codes(&t.o), codes(&t.d));
    let fit = ppml::hdfe(&t.y, &[&o, &d], &t.reg(), None).unwrap();
    for k in ["x1", "x2"] {
        close(fit.coef(k), robust.coef(k), 1e-8, k);
        let (got, want) = (fit.se(k), robust.se(k));
        assert!((got - want).abs() <= 1e-6 * want, "{k}: {got} vs {want}");
    }
    assert!(fit.n == t.y.len() && fit.n_separated == 0);
    assert!(0.0 < fit.pseudo_r2 && fit.pseudo_r2 < 1.0);
}

/// The deviance is of the flows scaled to mean 1, so it does not move with the flows' units, and
/// a fit on fewer regressors of the same sample has a larger one.
#[test]
fn hdfe_deviance_compares_fits_of_one_sample() {
    let t = two_way(25, 3);
    let (o, d) = (codes(&t.o), codes(&t.d));
    let full = ppml::hdfe(&t.y, &[&o, &d], &t.reg(), None).unwrap();
    let big: Vec<f64> = t.y.iter().map(|y| 1e6 * y).collect();
    let scaled = ppml::hdfe(&big, &[&o, &d], &t.reg(), None).unwrap();
    assert!((scaled.deviance - full.deviance).abs() <= 1e-9 * full.deviance);
    let fewer = ppml::hdfe(&t.y, &[&o, &d], &[("x1", &t.x1)], None).unwrap();
    assert!(0.0 < full.deviance && full.deviance < fewer.deviance);
}

#[test]
fn hdfe_clusters_and_drops_separated_groups() {
    let mut t = two_way(25, 3);
    for i in 0..t.y.len() {
        if t.o[i] == 4 {
            t.y[i] = 0.0; // exporter 4 never exports
        }
    }
    let (o, d) = (codes(&t.o), codes(&t.d));
    let fit = ppml::hdfe(&t.y, &[&o, &d], &t.reg(), None).unwrap();
    let keep: Vec<bool> = t.o.iter().map(|&o| o != 4).collect();
    let (x1, x2) = (filter(&t.x1, &keep), filter(&t.x2, &keep));
    let want = ppml::hdfe(
        &filter(&t.y, &keep),
        &[&filter(&o, &keep), &filter(&d, &keep)],
        &[("x1", &x1), ("x2", &x2)],
        None,
    )
    .unwrap();
    assert_eq!(fit.n_separated, keep.iter().filter(|&&k| !k).count());
    assert_eq!(fit.keep, keep);
    close(fit.coef("x1"), want.coef("x1"), 1e-10, "x1");
    // One cluster per observation is HC0 times G / (G - 1).
    let singletons: Vec<i64> = (0..t.y.len() as i64).collect();
    let ones = ppml::hdfe(&t.y, &[&o, &d], &t.reg(), Some(&singletons)).unwrap();
    let g = fit.n as f64;
    let (got, want) = (ones.se("x1"), fit.se("x1") * (g / (g - 1.0)).sqrt());
    assert!((got - want).abs() <= 1e-9 * want, "{got} vs {want}");
}

/// The direct two-way start and alternating projections alone (a third, constant, fixed effect
/// that changes nothing) reach the same fit.
#[test]
fn hdfe_direct_start_matches_projections() {
    let t = two_way(30, 8);
    let (o, d) = (codes(&t.o), codes(&t.d));
    let direct = ppml::hdfe(&t.y, &[&o, &d], &t.reg(), None).unwrap();
    let constant = vec![0; t.y.len()];
    let projected = ppml::hdfe(&t.y, &[&o, &d, &constant], &t.reg(), None).unwrap();
    for k in ["x1", "x2"] {
        close(direct.coef(k), projected.coef(k), 1e-10, k);
        let (got, want) = (direct.se(k), projected.se(k));
        assert!((got - want).abs() <= 1e-8 * want, "{k}: {got} vs {want}");
    }
}

/// A regressor twice: the minimum-norm least squares step splits its coefficient evenly, as
/// numpy's `lstsq`. A regressor the fixed effects absorb gets a coefficient of 0 (up to
/// rounding) and leaves the others as they were.
#[test]
fn hdfe_rank_deficient_designs_take_the_minimum_norm_step() {
    let t = two_way(25, 3);
    let (o, d) = (codes(&t.o), codes(&t.d));
    let base = ppml::hdfe(&t.y, &[&o, &d], &t.reg(), None).unwrap();
    let twice = ppml::hdfe(
        &t.y,
        &[&o, &d],
        &[("x1", &t.x1), ("x2", &t.x2), ("x1_again", &t.x1)],
        None,
    )
    .unwrap();
    close(twice.coef("x1"), base.coef("x1") / 2.0, 1e-10, "x1");
    close(
        twice.coef("x1_again"),
        base.coef("x1") / 2.0,
        1e-10,
        "x1_again",
    );
    close(twice.coef("x2"), base.coef("x2"), 1e-10, "x2");

    let exporter: Vec<f64> = t.o.iter().map(|&o| 0.37 * o as f64).collect();
    let absorbed = ppml::hdfe(
        &t.y,
        &[&o, &d],
        &[("x1", &t.x1), ("x2", &t.x2), ("exporter", &exporter)],
        None,
    )
    .unwrap();
    assert!(
        absorbed.coef("exporter").abs() <= 1e-12,
        "{}",
        absorbed.coef("exporter")
    );
    for k in ["x1", "x2"] {
        close(absorbed.coef(k), base.coef(k), 1e-10, k);
        let (got, want) = (absorbed.se(k), base.se(k));
        assert!((got - want).abs() <= 1e-8 * want, "{k}: {got} vs {want}");
    }
}

#[test]
fn hdfe_rejects_bad_input() {
    let t = two_way(10, 3);
    let (o, d) = (codes(&t.o), codes(&t.d));
    let err = ppml::hdfe(&[], &[&[], &[]], &[("x", &[])], None).unwrap_err();
    assert_eq!(err, PpmlError::Invalid("no observations".into()));
    let mut y = t.y.clone();
    y[0] = -1.0;
    let err = ppml::hdfe(&y, &[&o, &d], &t.reg(), None).unwrap_err();
    assert!(matches!(&err, PpmlError::Invalid(s) if s.starts_with("flows must be finite")));
    // One cluster: G / (G - 1) has no value (a division by zero).
    let one = vec![7; t.y.len()];
    let err = ppml::hdfe(&t.y, &[&o, &d], &t.reg(), Some(&one)).unwrap_err();
    assert_eq!(
        err,
        PpmlError::Invalid("one cluster: no cluster-robust covariance".into())
    );
    let zeros = vec![0.0; t.y.len()];
    let err = ppml::hdfe(&zeros, &[&o, &d], &t.reg(), None).unwrap_err();
    assert!(
        matches!(&err, PpmlError::Invalid(s) if s.contains("survives")),
        "{err:?}"
    );
}

/// Groups along a chain (each row joins one group of each of two staggered partitions, plus a
/// constant third) mix so slowly that the projections stop at their sweep cap.
#[test]
fn hdfe_errs_when_the_fixed_effects_do_not_converge() {
    let n = 4000;
    let mut rng = Draws(9);
    let y: Vec<f64> = (0..n).map(|_| 1.0 + rng.poisson(3.0)).collect();
    let x: Vec<f64> = (0..n).map(|i| (i as f64 / n as f64).powi(2)).collect();
    let first: Vec<i64> = (0..n as i64).map(|i| i / 2).collect();
    let second: Vec<i64> = (0..n as i64).map(|i| (i + 1) / 2).collect();
    let constant = vec![0; n];
    let err = ppml::hdfe(&y, &[&first, &second, &constant], &[("x", &x)], None).unwrap_err();
    assert_eq!(
        err,
        PpmlError::NoConvergence(format!(
            "fixed effects did not converge in {} sweeps",
            ppml::DEMEAN_MAX_SWEEPS
        ))
    );
}
