//! Spike for RFC 0353 D4 C (ADR 0373): Brent evaluations of the border-friction fit, warm
//! (seeded from a neighbouring fit) against cold, on a full pack. Non-authoritative: writes nothing.
//!
//! ```text
//! cargo run --release -p ceteris-calibrate --example warm_start -- PACK CACHE
//! ```

use std::path::PathBuf;

use ceteris_calibrate::epsilon::{at_epsilon, class_offsets, pack_data};
use ceteris_calibrate::steady::Limits;
use ceteris_calibrate::{border, export_pack};
use ceteris_reference::io as reference_io;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [pack, cache] = a.as_slice() else {
        panic!("usage: warm_start PACK CACHE");
    };
    let (pack, cache) = (PathBuf::from(pack), PathBuf::from(cache));
    let tmp = tempfile::tempdir().unwrap();
    export_pack(&pack, tmp.path(), false).unwrap();
    let inp = reference_io::load(tmp.path()).unwrap();
    let n_traded = inp.traded.iter().filter(|&&t| t).count();
    let theta = &inp.theta_class[..n_traded];
    let groups = border::friction_groups(theta);
    let (target, icio) = border::target_shares(&pack, &groups).unwrap();
    let data = pack_data(&pack, &cache).unwrap();
    let (ln_d0, _) = class_offsets(&inp, theta, &data.ln_d, &data.weight);
    let scale = vec![1.0; border::N_CLASSES];
    let limits = Limits::default();
    let mut quiet = |_: &str| {};

    println!("case,cold_evals,warm_evals,ratio,max_dnu");
    let (mut cold_sum, mut warm_sum) = (0usize, 0usize);
    // (a) the epsilon search's neighbouring trials: seed = previous trial's fit
    let eps = [0.30, 0.34, 0.35, 0.351, 0.3505, 0.35025];
    let mut prev: Option<Vec<f64>> = None;
    for &e in &eps {
        let at = at_epsilon(&inp, theta, &data.ln_d, &ln_d0, &scale, e).unwrap();
        let cold = border::fit_border_nu(&at, &groups, &target, &icio, limits, &mut quiet).unwrap();
        if let Some(seed) = &prev {
            let warm = border::fit_border_nu_seeded(
                &at,
                &groups,
                &target,
                &icio,
                limits,
                Some(seed),
                &mut quiet,
            )
            .unwrap();
            row(&format!("eps {e} seeded by previous"), &cold, &warm);
            cold_sum += cold.evaluations;
            warm_sum += warm.evaluations;
        }
        prev = Some(cold.nu);
    }
    // (b) a dev-loop edit: every target moved by a relative step, seed = the unedited fit
    let at = at_epsilon(&inp, theta, &data.ln_d, &ln_d0, &scale, 0.35).unwrap();
    let base = border::fit_border_nu(&at, &groups, &target, &icio, limits, &mut quiet).unwrap();
    for step in [0.01, 0.05, 0.10] {
        let t2: Vec<f64> = target.iter().map(|x| x * (1.0 + step)).collect();
        let cold = border::fit_border_nu(&at, &groups, &t2, &icio, limits, &mut quiet).unwrap();
        let warm = border::fit_border_nu_seeded(
            &at,
            &groups,
            &t2,
            &icio,
            limits,
            Some(&base.nu),
            &mut quiet,
        )
        .unwrap();
        row(&format!("targets +{}%", step * 100.0), &cold, &warm);
        cold_sum += cold.evaluations;
        warm_sum += warm.evaluations;
    }
    println!(
        "total,{cold_sum},{warm_sum},{:.2},",
        cold_sum as f64 / warm_sum as f64
    );
}

fn row(case: &str, cold: &border::Fit, warm: &border::Fit) {
    let d = cold
        .nu
        .iter()
        .zip(&warm.nu)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!(
        "{case},{},{},{:.2},{d:.3e}",
        cold.evaluations,
        warm.evaluations,
        cold.evaluations as f64 / warm.evaluations as f64
    );
}
