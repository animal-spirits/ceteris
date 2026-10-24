//! The transit wheel: goods in flight between dispatch and delivery
//! (`specs/trade/SPEC.md`, Transit).
//!
//! Four slots indexed by arrival tick modulo 4. A shipment dispatched at tick t with
//! lag L (0 to 3) is delivered at tick t + 1 + L. Within a tick the caller first drains the
//! due slot with [`TransitWheel::pop_due`] (T1) and then pushes new dispatches (T2).

use ceteris_core::snapshot::{Decode, DecodeError, Decoder, Encode, Encoder};

use crate::goods::N_GOODS;

pub const SLOTS: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct TransitWheel {
    provinces: usize,
    /// Slot that the next `pop_due` delivers.
    cursor: usize,
    slots: [Vec<f64>; SLOTS],
    /// Scratch holding the last delivery; not state.
    due: Vec<f64>,
}

impl TransitWheel {
    pub fn new(provinces: usize) -> Self {
        let n = provinces * N_GOODS;
        Self {
            provinces,
            cursor: 0,
            slots: std::array::from_fn(|_| vec![0.0; n]),
            due: vec![0.0; n],
        }
    }

    pub fn n_provinces(&self) -> usize {
        self.provinces
    }

    /// Adds `arrive[p]` units of good `g` bound for province `p`, due at tick t + 1 + `lag`.
    /// Call after this tick's `pop_due`.
    pub fn push(&mut self, lag: usize, g: usize, arrive: &[f64]) {
        assert!(lag < SLOTS, "lag {lag} exceeds the wheel");
        assert!(g < N_GOODS, "good {g} out of range");
        assert_eq!(arrive.len(), self.provinces);
        let slot = &mut self.slots[(self.cursor + lag) % SLOTS];
        for (p, &a) in arrive.iter().enumerate() {
            slot[p * N_GOODS + g] += a;
        }
    }

    /// Delivers what is due this tick, `[province * N_GOODS + g]`, and advances the wheel.
    /// The slice stays valid until the next call.
    pub fn pop_due(&mut self) -> &[f64] {
        // the delivered slot is swapped out and replaced by the zeroed previous delivery, so
        // a lag-3 push right after lands in a clean slot
        self.due.fill(0.0);
        std::mem::swap(&mut self.due, &mut self.slots[self.cursor]);
        self.cursor = (self.cursor + 1) % SLOTS;
        &self.due
    }

    /// Total goods in flight, the transit account balance per good.
    pub fn in_flight(&self, g: usize) -> f64 {
        self.slots
            .iter()
            .flat_map(|s| s.iter().skip(g).step_by(N_GOODS))
            .sum()
    }
}

impl Encode for TransitWheel {
    fn encode(&self, e: &mut Encoder) {
        e.count(self.provinces);
        e.u8(self.cursor as u8);
        self.slots.iter().for_each(|s| e.f64s(s));
    }
}

impl Decode for TransitWheel {
    fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        let provinces = d.count()?;
        let cursor = usize::from(d.u8()?);
        if cursor >= SLOTS {
            return Err(DecodeError(format!("transit cursor {cursor}")));
        }
        let mut slots: [Vec<f64>; SLOTS] = Default::default();
        for s in &mut slots {
            *s = d.f64s()?;
            if s.len() != provinces * N_GOODS {
                return Err(DecodeError(format!(
                    "transit slot of {} for {provinces} provinces",
                    s.len()
                )));
            }
        }
        Ok(Self {
            provinces,
            cursor,
            slots,
            due: vec![0.0; provinces * N_GOODS],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ceteris_core::snapshot::canonical;

    fn arrive(p: usize, v: f64) -> Vec<f64> {
        (0..p).map(|i| v * (i + 1) as f64).collect()
    }

    #[test]
    fn wheel_delivers_after_lag_and_wraps() {
        let p = 3;
        let mut w = TransitWheel::new(p);
        // dispatch at tick t with lag L, tagged by value 100 * t + L + 1
        let mut expect: Vec<(usize, usize)> = vec![]; // (due tick, tag)
        for t in 0..25usize {
            let due = w.pop_due().to_vec();
            assert_eq!(due.len(), p * N_GOODS);
            let want: f64 = expect
                .iter()
                .filter(|(d, _)| *d == t)
                .map(|&(_, tag)| tag as f64)
                .sum();
            for pi in 0..p {
                for g in 0..N_GOODS {
                    let tag_g = if g == 2 { want } else { 0.0 };
                    let got = due[pi * N_GOODS + g];
                    assert_eq!(
                        got,
                        tag_g * (pi + 1) as f64,
                        "tick {t} province {pi} good {g}"
                    );
                }
            }
            // every tick dispatches lag (t % 4) and, every third tick, lag 3 as well
            let mut lags = vec![t % 4];
            if t % 3 == 0 {
                lags.push(3);
            }
            for lag in lags {
                let tag = 100 * t + lag + 1;
                w.push(lag, 2, &arrive(p, tag as f64));
                expect.push((t + 1 + lag, tag));
            }
        }
    }

    #[test]
    fn lag_three_lands_in_the_slot_just_drained() {
        let mut w = TransitWheel::new(1);
        w.pop_due();
        w.push(3, 0, &[5.0]);
        for _ in 0..3 {
            assert!(w.pop_due().iter().all(|&x| x == 0.0));
        }
        assert_eq!(w.pop_due()[0], 5.0);
        assert!(w.pop_due().iter().all(|&x| x == 0.0));
    }

    #[test]
    fn pushes_accumulate_and_in_flight_tracks() {
        let mut w = TransitWheel::new(2);
        w.push(1, 4, &[1.0, 2.0]);
        w.push(1, 4, &[0.5, 0.25]);
        w.push(0, 4, &[10.0, 0.0]);
        w.push(2, 5, &[7.0, 0.0]);
        assert_eq!(w.in_flight(4), 13.75);
        assert_eq!(w.in_flight(5), 7.0);
        assert_eq!(w.pop_due()[4], 10.0);
        assert_eq!(w.in_flight(4), 3.75);
        let due = w.pop_due();
        assert_eq!((due[4], due[N_GOODS + 4]), (1.5, 2.25));
        assert_eq!(w.in_flight(4), 0.0);
    }

    #[test]
    fn round_trips_through_snapshot_bytes() {
        let mut w = TransitWheel::new(3);
        w.pop_due();
        w.push(0, 1, &[1.0, 2.0, 3.0]);
        w.push(3, 6, &[0.0, 4.0, 0.0]);
        w.pop_due();
        let bytes = canonical(7, &w);
        let mut e = Encoder::default();
        w.encode(&mut e);
        let bytes_enc = e.into_bytes();
        let mut d = Decoder::new(&bytes_enc);
        let mut back = TransitWheel::decode(&mut d).unwrap();
        d.finish().unwrap();
        assert_eq!(canonical(7, &back), bytes);
        // the decoded wheel keeps delivering the same goods
        let mut orig = w.clone();
        for _ in 0..5 {
            assert_eq!(orig.pop_due(), back.pop_due());
        }
    }

    #[test]
    fn decode_rejects_bad_bytes() {
        let mut e = Encoder::default();
        e.count(2);
        e.u8(9);
        let bytes = e.into_bytes();
        assert!(TransitWheel::decode(&mut Decoder::new(&bytes)).is_err());
    }
}
