//! Counter-based Philox 4x32-10 (Salmon et al. 2011). A draw is a pure function of
//! (seed, tick, system, kind, entity, block), so streams never shift when entities or
//! systems are added (Sections 4.5, 14.4).

use crate::entity::{EntityId, Kind};
use crate::schedule::SystemId;

const M0: u32 = 0xD251_1F53;
const M1: u32 = 0xCD9E_8D57;
const W0: u32 = 0x9E37_79B9;
const W1: u32 = 0xBB67_AE85;

pub fn philox4x32_10(ctr: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    let (mut c, mut k) = (ctr, key);
    for round in 0..10 {
        if round > 0 {
            k = [k[0].wrapping_add(W0), k[1].wrapping_add(W1)];
        }
        let p0 = u64::from(M0) * u64::from(c[0]);
        let p1 = u64::from(M1) * u64::from(c[2]);
        c = [
            (p1 >> 32) as u32 ^ c[1] ^ k[0],
            p1 as u32,
            (p0 >> 32) as u32 ^ c[3] ^ k[1],
            p0 as u32,
        ];
    }
    c
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seed(pub u64);

impl Seed {
    pub fn stream(self, tick: u32, system: SystemId, kind: Kind, entity: EntityId) -> Stream {
        Stream {
            key: [self.0 as u32, (self.0 >> 32) as u32],
            ctr: [
                tick,
                u32::from(system.0) << 16 | u32::from(kind.0),
                entity.0,
                0,
            ],
            buf: [0; 4],
            used: 4,
        }
    }
}

pub struct Stream {
    key: [u32; 2],
    ctr: [u32; 4],
    buf: [u32; 4],
    used: usize,
}

impl Stream {
    pub fn next_u32(&mut self) -> u32 {
        if self.used == 4 {
            self.buf = philox4x32_10(self.ctr, self.key);
            self.ctr[3] += 1;
            self.used = 0;
        }
        self.used += 1;
        self.buf[self.used - 1]
    }

    pub fn next_u64(&mut self) -> u64 {
        u64::from(self.next_u32()) << 32 | u64::from(self.next_u32())
    }

    /// Uniform on [0, 1) with 53 random bits.
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Standard normal by the cosine branch of Box-Muller.
    pub fn normal(&mut self) -> f64 {
        let u1 = 1.0 - self.uniform();
        let u2 = self.uniform();
        libm::sqrt(-2.0 * libm::log(u1)) * libm::cos(2.0 * core::f64::consts::PI * u2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known-answer vectors from Random123 (kat_vectors, philox4x32 10 rounds).
    #[test]
    fn philox_known_answers() {
        assert_eq!(
            philox4x32_10([0; 4], [0; 2]),
            [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8]
        );
        assert_eq!(
            philox4x32_10([u32::MAX; 4], [u32::MAX; 2]),
            [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd]
        );
        assert_eq!(
            philox4x32_10(
                [0x243f_6a88, 0x85a3_08d3, 0x1319_8a2e, 0x0370_7344],
                [0xa409_3822, 0x299f_31d0]
            ),
            [0xd16c_fe09, 0x94fd_cceb, 0x5001_e420, 0x2412_6ea1]
        );
    }

    #[test]
    fn streams_are_addressed_not_sequential() {
        let seed = Seed(42);
        let draw = |entity| {
            seed.stream(7, SystemId(3), Kind(1), EntityId(entity))
                .next_u64()
        };
        let alone = draw(5);
        let _others: Vec<u64> = (0..100).map(draw).collect();
        assert_eq!(draw(5), alone);
        assert_ne!(draw(5), draw(6));
        assert_ne!(
            seed.stream(7, SystemId(3), Kind(1), EntityId(5)).next_u64(),
            seed.stream(7, SystemId(3), Kind(2), EntityId(5)).next_u64()
        );
    }

    #[test]
    fn uniform_and_normal_moments() {
        let mut s = Seed(1).stream(0, SystemId(0), Kind(0), EntityId(0));
        let n = 200_000;
        let (mut su, mut sz, mut szz) = (0.0, 0.0, 0.0);
        for _ in 0..n {
            let u = s.uniform();
            assert!((0.0..1.0).contains(&u));
            su += u;
            let z = s.normal();
            sz += z;
            szz += z * z;
        }
        let n = n as f64;
        assert!((su / n - 0.5).abs() < 0.005);
        assert!((sz / n).abs() < 0.01);
        assert!((szz / n - 1.0).abs() < 0.02);
    }
}
