#![feature(float_algebraic)]
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

fn main() {
    let v = vec![1.0_f64; 4];
    let _ = v.par_iter().copied().sum::<f64>();
    let _ = v.par_iter().copied().product::<f64>();
    let _ = v.par_iter().copied().reduce(|| 0.0, |a, b| a + b);
    let _ = v.par_iter().copied().reduce_with(|a, b| a + b);
    let _ = v.par_iter().copied().fold(|| 0.0, |a, b| a + b).count();
    let _ = v.par_iter().copied().fold_with(0.0, |a, b| a + b).count();
    let _ = v.par_iter().copied().try_fold(|| 0.0, |a, b| Some(a + b)).count();
    let _ = v.par_iter().copied().try_fold_with(0.0, |a, b| Some(a + b)).count();
    let _ = v.par_iter().copied().map(Some).try_reduce(|| 0.0, |a, b| Some(a + b));
    let _ = v.par_iter().copied().map(Some).try_reduce_with(|a, b| Some(a + b));

    let _: HashMap<u8, u8> = HashMap::new();
    let _: HashSet<u8> = HashSet::new();

    let (a, b) = (1.0_f64, 2.0_f64);
    let _ = a.algebraic_add(b);
    let _ = a.algebraic_sub(b);
    let _ = a.algebraic_mul(b);
    let _ = a.algebraic_div(b);
    let _ = a.algebraic_rem(b);
    let (a, b) = (1.0_f32, 2.0_f32);
    let _ = a.algebraic_add(b);
    let _ = a.algebraic_sub(b);
    let _ = a.algebraic_mul(b);
    let _ = a.algebraic_div(b);
    let _ = a.algebraic_rem(b);
}
