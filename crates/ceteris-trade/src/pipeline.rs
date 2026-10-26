//! Goods on order per buyer: dispatched and paid, not yet delivered (ADR 0016).
//!
//! Four slots, one per future tick t + 1 to t + 4. A shipment pushed at tick t into slot s
//! (`Sourced::arrive[s]`, 1 to 4) is delivered to its buyer at tick t + s. Home deliveries
//! (slot 0) are delivered at once and never enter the pipeline. Within a tick the caller first
//! drains the due slot with [`Pipeline::pop_due`] (T1) and then pushes new dispatches.

use ceteris_core::snapshot::{Decode, DecodeError, Decoder, Encode, Encoder};

use crate::goods::N_GOODS;

pub const SLOTS: usize = 4;

/// Arrays are `[buyer * N_GOODS + g]`, buyers numbered by the caller.
#[derive(Clone, Debug, PartialEq)]
pub struct Pipeline {
    buyers: usize,
    /// Storage slot that the next `pop_due` delivers.
    cursor: usize,
    slots: [Vec<f64>; SLOTS],
    /// Scratch holding the last delivery; not state.
    due: Vec<f64>,
}

impl Pipeline {
    pub fn new(n_buyers: usize) -> Self {
        let n = n_buyers * N_GOODS;
        Self {
            buyers: n_buyers,
            cursor: 0,
            slots: std::array::from_fn(|_| vec![0.0; n]),
            due: vec![0.0; n],
        }
    }

    pub fn n_buyers(&self) -> usize {
        self.buyers
    }

    /// Adds `units` of good `g` for `buyer`, delivered `slot` ticks from now (1 to 4). Call
    /// after this tick's `pop_due`.
    pub fn push(&mut self, slot: usize, buyer: usize, g: usize, units: f64) {
        assert!(
            (1..=SLOTS).contains(&slot),
            "slot {slot} outside 1 to {SLOTS}"
        );
        assert!(buyer < self.buyers && g < N_GOODS, "buyer {buyer} good {g}");
        self.slots[(self.cursor + slot - 1) % SLOTS][buyer * N_GOODS + g] += units;
    }

    /// Delivers what is due this tick, `[buyer * N_GOODS + g]`, and advances the pipeline.
    /// The slice stays valid until the next call.
    pub fn pop_due(&mut self) -> &[f64] {
        // the delivered slot is swapped out for the zeroed previous delivery, so a slot-4 push
        // right after lands in a clean slot
        self.due.fill(0.0);
        std::mem::swap(&mut self.due, &mut self.slots[self.cursor]);
        self.cursor = (self.cursor + 1) % SLOTS;
        &self.due
    }

    /// Goods delivered `ticks` from now (1 to 4) if nothing more is pushed,
    /// `[buyer * N_GOODS + g]`.
    pub fn scheduled(&self, ticks: usize) -> &[f64] {
        assert!(
            (1..=SLOTS).contains(&ticks),
            "slot {ticks} outside 1 to {SLOTS}"
        );
        &self.slots[(self.cursor + ticks - 1) % SLOTS]
    }

    /// Goods of `g` on order for `buyer`, over every slot.
    pub fn on_order(&self, buyer: usize, g: usize) -> f64 {
        self.slots.iter().map(|s| s[buyer * N_GOODS + g]).sum()
    }

    /// Total goods of `g` in flight, the transit account balance per good.
    pub fn in_flight(&self, g: usize) -> f64 {
        self.slots
            .iter()
            .flat_map(|s| s.iter().skip(g).step_by(N_GOODS))
            .sum()
    }

    /// [`Pipeline::in_flight`] of every good in one pass, each summed in the same order.
    pub fn in_flight_all(&self) -> [f64; N_GOODS] {
        // -0.0 is where `Iterator::sum` starts
        let mut total = [-0.0; N_GOODS];
        for s in &self.slots {
            for row in s.chunks_exact(N_GOODS) {
                for (t, &x) in total.iter_mut().zip(row) {
                    *t += x;
                }
            }
        }
        total
    }
}

impl Encode for Pipeline {
    fn encode(&self, e: &mut Encoder) {
        e.count(self.buyers);
        e.u8(self.cursor as u8);
        self.slots.iter().for_each(|s| e.f64s(s));
    }
}

impl Decode for Pipeline {
    fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        let buyers = d.count()?;
        let cursor = usize::from(d.u8()?);
        if cursor >= SLOTS {
            return Err(DecodeError(format!("pipeline cursor {cursor}")));
        }
        let mut slots: [Vec<f64>; SLOTS] = Default::default();
        for s in &mut slots {
            *s = d.f64s()?;
            if s.len() != buyers * N_GOODS {
                return Err(DecodeError(format!(
                    "pipeline slot of {} for {buyers} buyers",
                    s.len()
                )));
            }
        }
        Ok(Self {
            buyers,
            cursor,
            slots,
            due: vec![0.0; buyers * N_GOODS],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_flight_of_all_goods_matches_each_good() {
        let mut p = Pipeline::new(3);
        for (i, units) in [0.1, 1e-17, 3.0, 0.7, 1e16, 0.3].into_iter().enumerate() {
            p.push(1 + i % SLOTS, i % 3, (i * 7) % N_GOODS, units);
        }
        for (g, total) in p.in_flight_all().iter().enumerate() {
            assert_eq!(total.to_bits(), p.in_flight(g).to_bits(), "good {g}");
        }
    }
    use ceteris_core::snapshot::canonical;

    #[test]
    fn pipeline_delivers_each_buyer_after_its_slot() {
        let buyers = 3;
        let mut p = Pipeline::new(buyers);
        // (due tick, buyer, good, units)
        let mut expect: Vec<(usize, usize, usize, f64)> = vec![];
        for t in 0..30usize {
            let due = p.pop_due().to_vec();
            assert_eq!(due.len(), buyers * N_GOODS);
            for b in 0..buyers {
                for g in 0..N_GOODS {
                    let want: f64 = expect
                        .iter()
                        .filter(|e| (e.0, e.1, e.2) == (t, b, g))
                        .map(|e| e.3)
                        .sum();
                    assert_eq!(due[b * N_GOODS + g], want, "tick {t} buyer {b} good {g}");
                }
            }
            expect.retain(|e| e.0 > t);
            // each tick ships to one buyer in slot 1 + t % 4 and, every third tick, to
            // another buyer in slot 4
            let mut pushes = vec![(1 + t % 4, t % buyers, t % 7)];
            if t % 3 == 0 {
                pushes.push((4, (t + 1) % buyers, 2));
            }
            for (slot, b, g) in pushes {
                let units = (100 * t + slot) as f64;
                p.push(slot, b, g, units);
                expect.push((t + slot, b, g, units));
            }
            for b in 0..buyers {
                for g in 0..N_GOODS {
                    let want: f64 = expect
                        .iter()
                        .filter(|e| (e.1, e.2) == (b, g))
                        .map(|e| e.3)
                        .sum();
                    assert_eq!(p.on_order(b, g), want, "tick {t} buyer {b} good {g}");
                }
            }
            for g in 0..N_GOODS {
                let want: f64 = expect.iter().filter(|e| e.2 == g).map(|e| e.3).sum();
                assert_eq!(p.in_flight(g), want);
            }
        }
    }

    #[test]
    fn slot_four_lands_in_the_slot_just_drained() {
        let mut p = Pipeline::new(1);
        p.pop_due();
        p.push(4, 0, 0, 5.0);
        p.push(1, 0, 0, 1.0);
        assert_eq!(p.pop_due()[0], 1.0);
        for _ in 0..2 {
            assert!(p.pop_due().iter().all(|&x| x == 0.0));
        }
        assert_eq!(p.pop_due()[0], 5.0);
        assert!(p.pop_due().iter().all(|&x| x == 0.0));
    }

    #[test]
    fn scheduled_shows_what_each_pop_delivers() {
        let mut p = Pipeline::new(2);
        p.pop_due();
        p.pop_due();
        for slot in 1..=SLOTS {
            p.push(slot, 1, 2, slot as f64);
        }
        for ticks in 1..=SLOTS {
            assert_eq!(p.scheduled(ticks)[N_GOODS + 2], ticks as f64);
        }
        for ticks in 1..=SLOTS {
            let want = p.scheduled(1).to_vec();
            assert_eq!(p.pop_due(), want, "pop {ticks}");
        }
    }

    #[test]
    #[should_panic(expected = "slot")]
    fn home_deliveries_do_not_enter_the_pipeline() {
        Pipeline::new(1).push(0, 0, 0, 1.0);
    }

    #[test]
    fn pipeline_round_trips_encode() {
        let mut p = Pipeline::new(3);
        p.pop_due();
        p.push(1, 0, 1, 1.0);
        p.push(4, 2, 6, 4.0);
        p.push(2, 1, 3, 2.5);
        p.pop_due();
        let bytes = canonical(7, &p);
        let mut e = Encoder::default();
        p.encode(&mut e);
        let encoded = e.into_bytes();
        let mut d = Decoder::new(&encoded);
        let mut back = Pipeline::decode(&mut d).unwrap();
        d.finish().unwrap();
        assert_eq!(canonical(7, &back), bytes);
        for _ in 0..5 {
            assert_eq!(p.pop_due(), back.pop_due());
        }
    }

    #[test]
    fn decode_rejects_bad_bytes() {
        let mut e = Encoder::default();
        e.count(2);
        e.u8(4);
        let bytes = e.into_bytes();
        assert!(Pipeline::decode(&mut Decoder::new(&bytes)).is_err());

        let mut e = Encoder::default();
        e.count(2);
        e.u8(0);
        for _ in 0..SLOTS {
            e.f64s(&[0.0; N_GOODS]);
        }
        let bytes = e.into_bytes();
        assert!(Pipeline::decode(&mut Decoder::new(&bytes)).is_err());
    }
}
