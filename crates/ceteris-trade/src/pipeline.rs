//! Goods on order per buyer: dispatched and paid, not yet delivered (ADR 0016).
//!
//! Four slots, one per future tick t + 1 to t + 4. A shipment pushed at tick t into slot s
//! (`Sourced::arrive[s]`, 1 to 4) is delivered to its buyer at tick t + s. Home deliveries
//! (slot 0) are delivered at once and never enter the pipeline. Within a tick the caller first
//! drains the due slot with [`Pipeline::pop_due`] (T1) and then pushes new dispatches.

use rayon::prelude::*;

use ceteris_core::snapshot::{DecodeError, Decoder, Encode, Encoder};

pub const SLOTS: usize = 4;

/// A shipment for [`Pipeline::push_all`]: slot (1 to 4), buyer, units.
pub type Push = (u8, u32, f64);

/// Arrays are good-major, `[g * n_buyers + buyer]` (RFC 0319 D1, an exception to the
/// goods-innermost rule of 19.4: every hot reader works per good), buyers numbered by the
/// caller. The snapshot bytes stay buyer-major (D2).
/// The per-good in-flight sums of the current contents; never part of equality.
#[derive(Clone, Debug, Default)]
struct FlightCache(std::sync::OnceLock<Vec<f64>>);

impl PartialEq for FlightCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pipeline {
    flight: FlightCache,
    buyers: usize,
    goods: usize,
    /// Storage slot that the next `pop_due` delivers.
    cursor: usize,
    slots: [Vec<f64>; SLOTS],
    /// Scratch holding the last delivery; not state.
    due: Vec<f64>,
}

impl Pipeline {
    pub fn new(n_buyers: usize, n_goods: usize) -> Self {
        let n = n_buyers * n_goods;
        Self {
            flight: FlightCache::default(),
            buyers: n_buyers,
            goods: n_goods,
            cursor: 0,
            slots: std::array::from_fn(|_| vec![0.0; n]),
            due: vec![0.0; n],
        }
    }

    pub fn n_buyers(&self) -> usize {
        self.buyers
    }

    pub fn n_goods(&self) -> usize {
        self.goods
    }

    /// Adds `units` of good `g` for `buyer`, delivered `slot` ticks from now (1 to 4). Call
    /// after this tick's `pop_due`.
    pub fn push(&mut self, slot: usize, buyer: usize, g: usize, units: f64) {
        assert!(
            (1..=SLOTS).contains(&slot),
            "slot {slot} outside 1 to {SLOTS}"
        );
        assert!(
            buyer < self.buyers && g < self.goods,
            "buyer {buyer} good {g}"
        );
        self.flight = FlightCache::default();
        self.slots[(self.cursor + slot - 1) % SLOTS][g * self.buyers + buyer] += units;
    }

    /// [`Pipeline::push`] of every shipment in `lists[g]` for good `g`: one task per good adds
    /// its list in order to its own contiguous run of each slot, so every cell sums the same
    /// terms in the same order however many threads there are.
    pub fn push_all(&mut self, lists: &[&[Push]]) {
        self.flight = FlightCache::default();
        let (buyers, goods, cursor) = (self.buyers, self.goods, self.cursor);
        assert!(lists.len() <= goods, "{} goods of {goods}", lists.len());
        if buyers == 0 {
            assert!(lists.iter().all(|l| l.is_empty()), "no buyers");
            return;
        }
        let [s0, s1, s2, s3] = &mut self.slots;
        let mut runs = [s0, s1, s2, s3].map(|s| s.chunks_exact_mut(buyers));
        let tasks: Vec<([&mut [f64]; SLOTS], &[Push])> = lists
            .iter()
            .map(|&list| (runs.each_mut().map(|r| r.next().expect("good run")), list))
            .collect();
        tasks
            .into_par_iter()
            .enumerate()
            .for_each(|(g, (cells, list))| {
                for &(slot, buyer, units) in list {
                    assert!(
                        (1..=SLOTS).contains(&(slot as usize)),
                        "slot {slot} outside 1 to {SLOTS}"
                    );
                    assert!((buyer as usize) < buyers, "buyer {buyer} good {g}");
                    cells[(cursor + slot as usize - 1) % SLOTS][buyer as usize] += units;
                }
            });
    }

    /// Delivers what is due this tick, `[g * n_buyers + buyer]`, and advances the pipeline.
    /// The slice stays valid until the next call.
    pub fn pop_due(&mut self) -> &[f64] {
        // the delivered slot is swapped out for the zeroed previous delivery, so a slot-4 push
        // right after lands in a clean slot
        self.flight = FlightCache::default();
        self.due.fill(0.0);
        std::mem::swap(&mut self.due, &mut self.slots[self.cursor]);
        self.cursor = (self.cursor + 1) % SLOTS;
        &self.due
    }

    /// Goods delivered `ticks` from now (1 to 4) if nothing more is pushed,
    /// `[g * n_buyers + buyer]`.
    pub fn scheduled(&self, ticks: usize) -> &[f64] {
        assert!(
            (1..=SLOTS).contains(&ticks),
            "slot {ticks} outside 1 to {SLOTS}"
        );
        &self.slots[(self.cursor + ticks - 1) % SLOTS]
    }

    /// Goods of `g` on order for `buyer`, over every slot.
    pub fn on_order(&self, buyer: usize, g: usize) -> f64 {
        self.slots.iter().map(|s| s[g * self.buyers + buyer]).sum()
    }

    /// Total goods of `g` in flight, the transit account balance per good: slots in storage
    /// order, buyers in order, from -0.0.
    pub fn in_flight(&self, g: usize) -> f64 {
        self.slots.iter().flat_map(|s| self.run(s, g)).sum()
    }

    fn run<'a>(&self, slot: &'a [f64], g: usize) -> &'a [f64] {
        &slot[g * self.buyers..(g + 1) * self.buyers]
    }

    /// [`Pipeline::in_flight`] of every good, one task per good over its contiguous runs.
    pub fn in_flight_all(&self) -> Vec<f64> {
        self.in_flight_cached().to_vec()
    }

    /// [`Pipeline::in_flight_all`] kept until the next push or pop, so a later caller on
    /// unchanged contents reuses the scan with the same bits.
    pub fn in_flight_cached(&self) -> &[f64] {
        self.flight.0.get_or_init(|| self.scan_in_flight())
    }

    fn scan_in_flight(&self) -> Vec<f64> {
        let _span = ceteris_core::timing::span("in_flight_all", ceteris_core::timing::NONE);
        (0..self.goods)
            .into_par_iter()
            .map(|g| self.in_flight(g))
            .collect()
    }
}

impl Encode for Pipeline {
    fn encode(&self, e: &mut Encoder) {
        e.count(self.buyers);
        e.u8(self.cursor as u8);
        // buyer-major bytes, as before good-major slots (RFC 0319 D2)
        let (buyers, goods) = (self.buyers, self.goods);
        for s in &self.slots {
            let mut out = vec![0.0; buyers * goods];
            if goods > 0 {
                out.par_chunks_mut(goods).enumerate().for_each(|(b, row)| {
                    for (g, x) in row.iter_mut().enumerate() {
                        *x = s[g * buyers + b];
                    }
                });
            }
            e.f64s(&out);
        }
    }
}

impl Pipeline {
    /// Restores a pipeline of `n_goods` goods per buyer encoded by [`Encode`].
    pub fn decode(d: &mut Decoder, n_goods: usize) -> Result<Self, DecodeError> {
        let buyers = d.count()?;
        let cursor = usize::from(d.u8()?);
        if cursor >= SLOTS {
            return Err(DecodeError(format!("pipeline cursor {cursor}")));
        }
        let mut slots: [Vec<f64>; SLOTS] = Default::default();
        for s in &mut slots {
            let bytes = d.f64s()?;
            if bytes.len() != buyers * n_goods {
                return Err(DecodeError(format!(
                    "pipeline slot of {} for {buyers} buyers",
                    bytes.len()
                )));
            }
            *s = vec![0.0; bytes.len()];
            if buyers > 0 {
                s.par_chunks_mut(buyers).enumerate().for_each(|(g, run)| {
                    for (b, x) in run.iter_mut().enumerate() {
                        *x = bytes[b * n_goods + g];
                    }
                });
            }
        }
        Ok(Self {
            flight: FlightCache::default(),
            buyers,
            goods: n_goods,
            cursor,
            slots,
            due: vec![0.0; buyers * n_goods],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N_GOODS: usize = crate::goods::p2::N;

    #[test]
    fn push_all_is_push_in_turn() {
        let (mut a, mut b) = (Pipeline::new(5, 3), Pipeline::new(5, 3));
        let lists: [Vec<(u8, u32, f64)>; 2] = [
            vec![(1, 0, 0.5), (4, 2, 1e-17), (1, 0, 0.25), (2, 4, 3.0)],
            vec![(3, 1, 0.1), (1, 0, 0.2), (3, 1, 0.7)],
        ];
        for tick in 0..6 {
            for (g, list) in lists.iter().enumerate() {
                for &(slot, buyer, units) in list {
                    a.push(
                        slot as usize,
                        buyer as usize,
                        g,
                        units * f64::from(tick + 1),
                    );
                }
            }
            let scaled: Vec<Vec<(u8, u32, f64)>> = lists
                .iter()
                .map(|l| {
                    l.iter()
                        .map(|&(s, k, u)| (s, k, u * f64::from(tick + 1)))
                        .collect()
                })
                .collect();
            let refs: Vec<&[Push]> = scaled.iter().map(|l| &l[..]).collect();
            b.push_all(&refs);
            assert_eq!(a.pop_due(), b.pop_due());
            assert_eq!(a, b);
        }
    }
    use ceteris_core::snapshot::canonical;

    #[test]
    fn in_flight_of_all_goods_matches_each_good() {
        let mut p = Pipeline::new(3, N_GOODS);
        for (i, units) in [0.1, 1e-17, 3.0, 0.7, 1e16, 0.3].into_iter().enumerate() {
            p.push(1 + i % SLOTS, i % 3, (i * 7) % N_GOODS, units);
        }
        for (g, total) in p.in_flight_all().iter().enumerate() {
            assert_eq!(total.to_bits(), p.in_flight(g).to_bits(), "good {g}");
        }
    }

    /// Buyer-major reference of the pipeline contents, by the pre-RFC 0319 layout, in storage-slot order.
    fn buyer_major(p: &Pipeline) -> [Vec<f64>; SLOTS] {
        std::array::from_fn(|k| {
            let s = p.scheduled((k + SLOTS - p.cursor) % SLOTS + 1);
            let (b, g) = (p.n_buyers(), p.n_goods());
            (0..b * g).map(|i| s[(i % g) * b + i / g]).collect()
        })
    }

    #[test]
    fn in_flight_and_snapshot_match_the_buyer_major_layout() {
        let buyers = 37;
        let mut p = Pipeline::new(buyers, N_GOODS);
        let mut x = 0.123_f64;
        for t in 0..9 {
            p.pop_due();
            let lists: Vec<Vec<Push>> = (0..N_GOODS)
                .map(|g| {
                    (0..11)
                        .map(|i| {
                            x = (x * 7.3 + 0.1).fract();
                            let slot = 1 + (i + g + t) % SLOTS;
                            (
                                slot as u8,
                                ((i * 5 + g * 3) % buyers) as u32,
                                x * 1e-3_f64.powi((i % 3) as i32),
                            )
                        })
                        .collect()
                })
                .collect();
            let refs: Vec<&[Push]> = lists.iter().map(|l| &l[..]).collect();
            p.push_all(&refs);
        }
        let ref_slots = buyer_major(&p);
        for g in 0..N_GOODS {
            let want: f64 = ref_slots
                .iter()
                .flat_map(|s| s.iter().skip(g).step_by(N_GOODS))
                .sum();
            assert_eq!(p.in_flight(g).to_bits(), want.to_bits(), "good {g}");
            assert_eq!(p.in_flight_all()[g].to_bits(), want.to_bits(), "good {g}");
        }
        // snapshot bytes are the buyer-major bytes
        let mut e = Encoder::default();
        p.encode(&mut e);
        let mut want = Encoder::default();
        want.count(buyers);
        want.u8(p.cursor as u8);
        ref_slots.iter().for_each(|s| want.f64s(s));
        assert_eq!(e.into_bytes(), want.into_bytes());
    }

    #[test]
    fn pipeline_delivers_each_buyer_after_its_slot() {
        let buyers = 3;
        let mut p = Pipeline::new(buyers, N_GOODS);
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
                    assert_eq!(due[g * buyers + b], want, "tick {t} buyer {b} good {g}");
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
        let mut p = Pipeline::new(1, N_GOODS);
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
        let mut p = Pipeline::new(2, N_GOODS);
        p.pop_due();
        p.pop_due();
        for slot in 1..=SLOTS {
            p.push(slot, 1, 2, slot as f64);
        }
        for ticks in 1..=SLOTS {
            assert_eq!(p.scheduled(ticks)[2 * 2 + 1], ticks as f64);
        }
        for ticks in 1..=SLOTS {
            let want = p.scheduled(1).to_vec();
            assert_eq!(p.pop_due(), want, "pop {ticks}");
        }
    }

    #[test]
    #[should_panic(expected = "slot")]
    fn home_deliveries_do_not_enter_the_pipeline() {
        Pipeline::new(1, N_GOODS).push(0, 0, 0, 1.0);
    }

    #[test]
    fn pipeline_round_trips_encode() {
        let mut p = Pipeline::new(3, N_GOODS);
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
        let mut back = Pipeline::decode(&mut d, N_GOODS).unwrap();
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
        assert!(Pipeline::decode(&mut Decoder::new(&bytes), N_GOODS).is_err());

        let mut e = Encoder::default();
        e.count(2);
        e.u8(0);
        for _ in 0..SLOTS {
            e.f64s(&[0.0; N_GOODS]);
        }
        let bytes = e.into_bytes();
        assert!(Pipeline::decode(&mut Decoder::new(&bytes), N_GOODS).is_err());
    }
}
