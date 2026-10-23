//! Ordered, multi-rate system pipeline (Sections 4.3, 4.4).

use crate::entity::{EntityId, Kind};
use crate::rng::{Seed, Stream};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cadence {
    Monthly,
    /// Last month of each quarter.
    Quarterly,
    Annual {
        month: u32,
    },
}

impl Cadence {
    pub fn due(self, tick: u32) -> bool {
        match self {
            Cadence::Monthly => true,
            Cadence::Quarterly => tick % 3 == 2,
            Cadence::Annual { month } => tick % 12 == month,
        }
    }
}

/// What a running system knows besides the world: the tick and its own random streams.
pub struct Ctx {
    pub tick: u32,
    pub system: SystemId,
    seed: Seed,
}

impl Ctx {
    pub fn stream(&self, kind: Kind, entity: EntityId) -> Stream {
        self.seed.stream(self.tick, self.system, kind, entity)
    }
}

/// Systems are stateless functions of the world, so a snapshot of the world is the whole state.
pub struct SystemDef<W> {
    pub id: SystemId,
    pub name: &'static str,
    pub cadence: Cadence,
    pub run: fn(&mut W, &Ctx),
}

pub struct Schedule<W> {
    systems: Vec<SystemDef<W>>,
}

impl<W> Schedule<W> {
    pub fn new(systems: Vec<SystemDef<W>>) -> Self {
        let mut ids: Vec<SystemId> = systems.iter().map(|s| s.id).collect();
        ids.sort();
        assert!(ids.windows(2).all(|w| w[0] != w[1]), "duplicate system IDs");
        Self { systems }
    }

    pub fn run_tick(&self, world: &mut W, seed: Seed, tick: u32) {
        for s in self.systems.iter().filter(|s| s.cadence.due(tick)) {
            (s.run)(
                world,
                &Ctx {
                    tick,
                    system: s.id,
                    seed,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cadences() {
        let due = |c: Cadence| (0..24).filter(|&t| c.due(t)).collect::<Vec<_>>();
        assert_eq!(due(Cadence::Monthly).len(), 24);
        assert_eq!(due(Cadence::Quarterly), vec![2, 5, 8, 11, 14, 17, 20, 23]);
        assert_eq!(due(Cadence::Annual { month: 0 }), vec![0, 12]);
    }

    #[test]
    fn systems_run_in_declared_order() {
        fn push_a(w: &mut Vec<&'static str>, _: &Ctx) {
            w.push("a");
        }
        fn push_b(w: &mut Vec<&'static str>, _: &Ctx) {
            w.push("b");
        }
        let s = Schedule::new(vec![
            SystemDef {
                id: SystemId(2),
                name: "b",
                cadence: Cadence::Monthly,
                run: push_b,
            },
            SystemDef {
                id: SystemId(1),
                name: "a",
                cadence: Cadence::Quarterly,
                run: push_a,
            },
        ]);
        let mut log = Vec::new();
        (0..3).for_each(|t| s.run_tick(&mut log, Seed(0), t));
        assert_eq!(log, vec!["b", "b", "b", "a"]);
    }

    #[test]
    #[should_panic(expected = "duplicate")]
    fn duplicate_ids_rejected() {
        fn noop(_: &mut (), _: &Ctx) {}
        let def = || SystemDef {
            id: SystemId(1),
            name: "x",
            cadence: Cadence::Monthly,
            run: noop,
        };
        Schedule::new(vec![def(), def()]);
    }
}
