//! A generated small world for tests, golden hashes and the GE reference: three mainland
//! countries of eight provinces each, side by side on a plane, and a one-province island
//! country with one port far offshore (Review Focus 4). Fully deterministic.
//!
//! Countries 0, 1, 2 have a west and an east gateway each; the east gateway of one and the
//! west gateway of the next form a land crossing, the outer gateways of countries 0 and 2
//! are ports, joined by sea to each other and to the island port. Country 1 is landlocked.
//! Every leg costs 0.02 USD per tonne-km at 40 km/h along the straight line.
//!
//! Crude oil (good 1) has no final demand and is an input only to chemicals, fuels and
//! metals (good 5), which exists in some provinces only: (province, oil) cells without
//! good 5 are inactive, some of them with oil output.

use ceteris_trade::{Leg, N_GOODS, Network, Wedges};

use crate::inputs::{Countries, EconInputs, EntData, N_EDU, PopData, ProvinceData};
use crate::params::EconParams;

const MAINLAND: usize = 3;
const PER_COUNTRY: usize = 8;
const COST_PER_TKM: f64 = 0.02;
const KMH: f64 = 40.0;
const OIL: usize = 1;
const HEAVY: usize = 5;

/// Final demand shares, the same in every country.
const OMEGA: [f64; N_GOODS] = [0.06, 0.0, 0.01, 0.12, 0.08, 0.04, 0.07, 0.04, 0.10, 0.48];
/// Tonnes per USD.
const WEIGHT: [f64; N_GOODS] = [
    0.006, 0.005, 0.03, 0.001, 0.0002, 0.001, 0.0001, 0.0, 0.0, 0.0,
];
/// Monthly VA per million inhabitants, millions.
const VA_PER_MILLION: [f64; N_GOODS] = [6.0, 15.0, 5.0, 9.0, 12.0, 20.0, 25.0, 5.0, 9.0, 55.0];
const S_L: [f64; 4] = [0.55, 0.60, 0.50, 0.45];
const UNEMPLOYMENT: [f64; 4] = [0.06, 0.08, 0.05, 0.10];
const EDU_SHARE: [[f64; N_EDU]; 4] = [
    [0.40, 0.40, 0.20],
    [0.50, 0.35, 0.15],
    [0.30, 0.45, 0.25],
    [0.60, 0.30, 0.10],
];
/// Labour force per inhabitant.
const PARTICIPATION: f64 = 0.47;

type Point = (f64, f64);

fn leg(from: usize, to: usize, a: Point, b: Point) -> Leg {
    let km = (a.0 - b.0).hypot(a.1 - b.1);
    Leg {
        from: from as u32,
        to: to as u32,
        cost_per_t: COST_PER_TKM * km,
        hours: km / KMH,
    }
}

/// Goods produced by mainland province `j` of country `c`, rotated by country so that
/// countries specialize.
fn mainland_goods(c: usize, j: usize) -> Vec<usize> {
    let jj = (j + 3 * c) % PER_COUNTRY;
    (0..N_GOODS)
        .filter(|&g| match g {
            OIL => jj % 4 == 1,
            2 => jj.is_multiple_of(2),
            4 => jj % 2 == 1,
            HEAVY => jj.is_multiple_of(3) || jj == 5,
            6 => jj >= 5,
            _ => true,
        })
        .collect()
}

pub fn small_world() -> EconInputs {
    let island = MAINLAND;
    let n_countries = MAINLAND + 1;

    let mut province_country = Vec::new();
    let mut province_at: Vec<Point> = Vec::new();
    let mut population = Vec::new();
    let mut goods: Vec<Vec<usize>> = Vec::new();
    for c in 0..MAINLAND {
        for j in 0..PER_COUNTRY {
            let (col, row) = (j % 4, j / 4);
            province_country.push(c as u16);
            province_at.push(((c * 800 + 100 + col * 200) as f64, (100 + row * 200) as f64));
            population.push(1e6 * (1.0 + 0.25 * ((3 * j + c) % 8) as f64));
            goods.push(mainland_goods(c, j));
        }
    }
    province_country.push(island as u16);
    province_at.push((1200.0, 15000.0));
    population.push(0.6e6);
    goods.push(vec![0, 3, 4, 7, 8, 9]);

    let mut gateway_country = Vec::new();
    let mut gateway_at: Vec<Point> = Vec::new();
    for c in 0..MAINLAND {
        let x = (c * 800) as f64;
        gateway_country.extend([c as u16, c as u16]);
        gateway_at.extend([(x + 5.0, 200.0), (x + 795.0, 200.0)]);
    }
    gateway_country.push(island as u16);
    gateway_at.push((1200.0, 14990.0));

    let np = province_country.len();
    let mut net = Network {
        province_country: province_country.clone(),
        gateway_country: gateway_country.clone(),
        ..Network::default()
    };
    for o in 0..np {
        for d in 0..np {
            if o != d && province_country[o] == province_country[d] {
                net.domestic.push(leg(o, d, province_at[o], province_at[d]));
            }
        }
        for (a, &gc) in gateway_country.iter().enumerate() {
            if gc == province_country[o] {
                net.access.push(leg(o, a, province_at[o], gateway_at[a]));
            }
        }
    }
    // land crossings east of country c to west of c + 1; sea between ports 0, 5 and island 6
    let mut pairs: Vec<(usize, usize)> =
        (0..MAINLAND - 1).map(|c| (2 * c + 1, 2 * c + 2)).collect();
    pairs.extend([
        (0, 2 * MAINLAND - 1),
        (0, 2 * MAINLAND),
        (2 * MAINLAND - 1, 2 * MAINLAND),
    ]);
    for (a, b) in pairs {
        net.crossings.push(leg(a, b, gateway_at[a], gateway_at[b]));
        net.crossings.push(leg(b, a, gateway_at[b], gateway_at[a]));
    }

    let mut wedges = Wedges::none(n_countries);
    for imp in 0..n_countries {
        for exp in 0..n_countries {
            if imp != exp {
                for group in 0..2 {
                    wedges.tariff[(imp * n_countries + exp) * 2 + group] =
                        0.04 + 0.02 * group as f32 + 0.01 * imp as f32;
                }
            }
        }
    }

    let mut io = [0.0; N_GOODS * N_GOODS];
    for k in 0..N_GOODS {
        for g in 0..N_GOODS {
            io[k * N_GOODS + g] = match (k, g) {
                (OIL, HEAVY) => 0.2,
                (OIL, _) => 0.0,
                _ if k == g => 0.1,
                _ => 0.03,
            };
        }
    }
    let countries = Countries {
        omega: vec![OMEGA; n_countries],
        io: vec![io; n_countries],
        s_l: S_L.to_vec(),
        unemployment: UNEMPLOYMENT.to_vec(),
    };

    let mut pops = PopData::default();
    let mut ent = EntData::default();
    for i in 0..np {
        let c = usize::from(province_country[i]);
        for (edu, share) in EDU_SHARE[c].iter().enumerate() {
            let people = population[i] * share;
            pops.province.push(i as u32);
            pops.edu.push(edu as u8);
            pops.population.push(people);
            pops.labour_force.push(PARTICIPATION * people);
        }
        for &g in &goods[i] {
            let tilt = if g < 7 {
                1.0 + 0.1 * ((i + g) % 3) as f64
            } else {
                1.0
            };
            let va0 = population[i] * VA_PER_MILLION[g] * tilt;
            ent.province.push(i as u32);
            ent.good.push(g as u8);
            ent.a.push(1.0);
            ent.va0.push(va0);
            ent.va_target.push(va0);
        }
    }

    let mut inputs = EconInputs {
        network: net,
        weight: WEIGHT,
        wedges,
        countries,
        provinces: ProvinceData {
            country: province_country,
            population,
            w0: Vec::new(),
        },
        pops,
        enterprises: ent,
    };
    inputs.set_base_point(&EconParams::default().edu_eff);
    inputs
}

#[cfg(test)]
mod tests {
    use super::*;
    use ceteris_trade::{Kernel, TradeParams};

    #[test]
    fn fixture_has_four_countries_and_twenty_five_provinces() {
        let w = small_world();
        w.validate();
        assert_eq!((w.n_countries(), w.n_provinces()), (4, 25));
        assert_eq!(w.network.n_gateways(), 7);
        assert_eq!(w.n_pops(), 75);
        let island: Vec<usize> = (0..25).filter(|&i| w.country_of(i) == 3).collect();
        assert_eq!(island, [24]);
        let island_gateways = w.network.gateway_country.iter().filter(|&&c| c == 3);
        assert_eq!(island_gateways.count(), 1);
    }

    #[test]
    fn fixture_is_deterministic() {
        assert_eq!(small_world().digest(), small_world().digest());
    }

    #[test]
    fn fixture_kernel_reaches_every_trading_partner() {
        let w = small_world();
        let k = Kernel::build(&w.network, &w.weight, &w.wedges, &TradeParams::default());
        // the island port has a sea lane to both mainland ports, about 15,000 km: lag 1
        let row = k.good(4).crossings.row(6);
        assert_eq!(row.len(), 2);
        assert!(row.iter().all(|e| e.lag == 1));
        // land crossings are short: lag 0
        assert_eq!(k.good(4).crossings.row(1)[0].lag, 0);
    }

    #[test]
    fn oil_feeds_heavy_industry_only() {
        let w = small_world();
        assert_eq!(w.countries.omega[0][OIL], 0.0);
        for g in 0..N_GOODS {
            assert_eq!(w.a(0, OIL, g) > 0.0, g == HEAVY);
        }
        let has = |i: usize, g: usize| {
            (0..w.n_enterprises()).any(|e| {
                w.enterprises.province[e] as usize == i && usize::from(w.enterprises.good[e]) == g
            })
        };
        let oil_without_heavy = (0..25).filter(|&i| has(i, OIL) && !has(i, HEAVY)).count();
        let oil_with_heavy = (0..25).filter(|&i| has(i, OIL) && has(i, HEAVY)).count();
        assert!(oil_without_heavy > 0 && oil_with_heavy > 0);
        assert!((0..25).any(|i| !has(i, HEAVY)));
    }

    #[test]
    fn base_wages_clear_base_employment() {
        let w = small_world();
        let supply = w.labour_supply(&EconParams::default().edu_eff);
        for (i, &s) in supply.iter().enumerate() {
            let l: f64 = (0..w.n_enterprises())
                .filter(|&e| w.enterprises.province[e] as usize == i)
                .map(|e| w.enterprises.l0[e])
                .sum();
            let u = w.countries.unemployment[w.country_of(i)];
            assert!((l / ((1.0 - u) * s) - 1.0).abs() < 1e-12);
        }
    }
}
