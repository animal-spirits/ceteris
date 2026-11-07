//! Hazard draws by thinning (ADR 0241): one uniform per (hazard, cell, tick) whether or not the
//! rate is positive, and an event fires when the uniform is below the rate. Branches that differ
//! only in the rate of one hazard share the events of every other hazard, and a zero-rate
//! hazard consumes nothing that another draw depends on.

use crate::entity::{EntityId, Kind};
use crate::rng::Seed;
use crate::schedule::SystemId;

/// Registry of hazards; the id keys the random stream, so ids are never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HazardId(pub u16);

/// Completion of a commitment (ADR 0241 D2).
pub const COMMITMENT_COMPLETION: HazardId = HazardId(1);

const STREAM: SystemId = SystemId(0xFFF0);

pub fn draw(seed: Seed, tick: u32, hazard: HazardId, cell: EntityId) -> f64 {
    seed.stream(tick, STREAM, Kind(hazard.0), cell).uniform()
}

pub fn fires(seed: Seed, tick: u32, hazard: HazardId, cell: EntityId, rate: f64) -> bool {
    draw(seed, tick, hazard, cell) < rate
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: HazardId = HazardId(1);
    const B: HazardId = HazardId(2);

    fn events(seed: Seed, hazard: HazardId, rate: f64) -> Vec<(u32, u32)> {
        (0..200)
            .flat_map(|t| (0..20).map(move |c| (t, c)))
            .filter(|&(t, c)| fires(seed, t, hazard, EntityId(c), rate))
            .collect()
    }

    #[test]
    fn branches_share_events_of_untouched_hazards() {
        let seed = Seed(7);
        // branches differing in the rate of A: A's events nest, B's are identical
        let (base, edit) = (events(seed, A, 0.01), events(seed, A, 0.2));
        assert!(base.iter().all(|e| edit.contains(e)));
        assert_ne!(base, edit);
        assert!(!events(seed, B, 0.05).is_empty());
    }

    #[test]
    fn draw_does_not_depend_on_the_rate() {
        let seed = Seed(3);
        let d = draw(seed, 5, A, EntityId(2));
        assert_eq!(d, draw(seed, 5, A, EntityId(2)));
        assert_ne!(d, draw(seed, 5, B, EntityId(2)));
        assert_ne!(d, draw(seed, 6, A, EntityId(2)));
        assert_ne!(d, draw(seed, 5, A, EntityId(3)));
        assert!((0.0..1.0).contains(&d));
    }

    #[test]
    fn zero_rate_hazards_never_fire_and_change_no_other_draw() {
        let seed = Seed(11);
        assert!(events(seed, A, 0.0).is_empty());
        let with = events(seed, B, 0.1);
        let _ = events(seed, A, 0.0);
        assert_eq!(with, events(seed, B, 0.1));
    }

    #[test]
    fn a_higher_rate_fires_a_superset_of_events() {
        let seed = Seed(5);
        let low = events(seed, A, 0.02);
        let high = events(seed, A, 0.1);
        assert!(low.iter().all(|e| high.contains(e)));
        assert!(high.len() > low.len());
    }
}
