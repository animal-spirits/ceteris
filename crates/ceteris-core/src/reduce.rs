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

/// [`fixed_sum`] of the column `data[g], data[g + stride], ...` of `n` entries, without copying
/// it: the same chunks and the same trees, so the same bits. One task per (column, chunk), then
/// each column's partials in order.
pub fn fixed_sum_columns(data: &[f64], stride: usize, n: usize) -> Vec<f64> {
    let chunks = n.div_ceil(CHUNK);
    let partial: Vec<f64> = (0..stride * chunks)
        .into_par_iter()
        .map(|i| {
            let (g, c) = (i / chunks, i % chunks);
            let lo = c * CHUNK;
            pairwise_strided(data, g, stride, lo, CHUNK.min(n - lo))
        })
        .collect();
    (0..stride)
        .map(|g| pairwise(&partial[g * chunks..(g + 1) * chunks]))
        .collect()
}

/// [`pairwise`] of `n` entries from row `lo`, column `g` of a matrix of `stride` columns.
fn pairwise_strided(data: &[f64], g: usize, stride: usize, lo: usize, n: usize) -> f64 {
    match n {
        0 => 0.0,
        1 => data[lo * stride + g],
        n => {
            pairwise_strided(data, g, stride, lo, n / 2)
                + pairwise_strided(data, g, stride, lo + n / 2, n - n / 2)
        }
    }
}

fn pairwise(xs: &[f64]) -> f64 {
    match xs.len() {
        0 => 0.0,
        1 => xs[0],
        n => pairwise(&xs[..n / 2]) + pairwise(&xs[n / 2..]),
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
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

    #[test]
    fn columns_match_fixed_sum_of_the_copied_column() {
        let (stride, n) = (5, 3000);
        let data: Vec<f64> = (0..stride * n)
            .map(|i| ((i * 7919 % 10007) as f64 - 5000.0) * 1e-3 + 1e-13 * i as f64)
            .collect();
        let sums = fixed_sum_columns(&data, stride, n);
        for (g, s) in sums.iter().enumerate() {
            let col: Vec<f64> = (0..n).map(|a| data[a * stride + g]).collect();
            assert_eq!(s.to_bits(), fixed_sum(&col).to_bits(), "column {g}");
        }
        for n in [0, 1, 1023, 1024, 1025] {
            let sums = fixed_sum_columns(&data[..stride * n], stride, n);
            let col: Vec<f64> = (0..n).map(|a| data[a * stride]).collect();
            assert_eq!(sums[0].to_bits(), fixed_sum(&col).to_bits(), "n {n}");
        }
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
