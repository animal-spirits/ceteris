//! Fuzzing the fixture: any seed and world size runs without invariant violations.

use ceteris_core::rng::Seed;
use ceteris_core::toy::Toy;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]
    #[test]
    fn any_seed_and_size_keeps_invariants(seed in any::<u64>(), provinces in 1usize..40) {
        let mut sim = Toy::sim(provinces, Seed(seed));
        prop_assert!(sim.run_until(360).is_ok());
        prop_assert!(sim.world().firm_id.len() >= provinces);
    }
}
