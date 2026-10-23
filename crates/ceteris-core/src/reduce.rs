//! Reductions whose result depends only on input length, never on thread count
//! (Section 4.5, floating point; Section 19.4, parallelism).

use rayon::prelude::*;

/// Chunk size fixed independently of the thread pool.
pub const CHUNK: usize = 1024;

pub fn fixed_sum(xs: &[f64]) -> f64 {
    if xs.len() <= CHUNK {
        return pairwise(xs);
    }
    let partial: Vec<f64> = xs.par_chunks(CHUNK).map(pairwise).collect();
    pairwise(&partial)
}

fn pairwise(xs: &[f64]) -> f64 {
    match xs.len() {
        0 => 0.0,
        1 => xs[0],
        n => pairwise(&xs[..n / 2]) + pairwise(&xs[n / 2..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn in_pool(threads: usize, xs: &[f64]) -> f64 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| fixed_sum(xs))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        #[test]
        fn identical_bits_across_thread_counts(xs in proptest::collection::vec(-1e12f64..1e12, 0..20_000)) {
            let one = in_pool(1, &xs);
            prop_assert_eq!(one.to_bits(), in_pool(8, &xs).to_bits());
            prop_assert_eq!(one.to_bits(), in_pool(3, &xs).to_bits());
        }
    }

    #[test]
    fn chunked_tree_matches_explicit_tree() {
        let xs: Vec<f64> = (0..5000).map(|i| (i as f64).sin()).collect();
        let chunks: Vec<f64> = xs.chunks(CHUNK).map(pairwise).collect();
        assert_eq!(fixed_sum(&xs).to_bits(), pairwise(&chunks).to_bits());
    }
}
