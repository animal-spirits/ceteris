//! A generated small world for tests, golden hashes and the GE reference: three mainland
//! countries of eight provinces each, side by side on a plane, and a one-province island
//! country with one port far offshore. Fully deterministic.
//!
//! Countries 0, 1, 2 have a west and an east gateway each; the east gateway of one and the
//! west gateway of the next form a land crossing, the outer gateways of countries 0 and 2
//! are ports, joined by sea to each other and to the island port. Country 1 is landlocked.
//! Land legs (domestic, access, land crossings) cost 0.02 USD per tonne-km, sea crossings
//! (the three port-to-port lanes, including both island crossings) cost 0.002, bulk sea
//! freight; theta class 3 ships them at P154 times that (`Network::bulk`, ADR 0247). All legs
//! move at 40 km/h along the straight line.
//!
//! Crude oil (good 1) has no final demand and is an input only to chemicals, fuels and
//! metals (good 5), which exists in some provinces only: (province, oil) cells without
//! good 5 are inactive, some of them with oil output.

use ceteris_trade::{
    BulkLegs, Leg, LegTaus, N_GOODS, Network, TradeParams, Wedges, equal_gateway_mass,
};

use crate::inputs::{Countries, EconInputs, EntData, N_EDU, PopData, ProvinceData};
use crate::params::EconParams;

const MAINLAND: usize = 3;
const PER_COUNTRY: usize = 8;
/// USD per tonne-km on land legs.
const LAND_COST_PER_TKM: f64 = 0.02;
/// USD per tonne-km on sea crossings (bulk freight, a tenth of land).
const SEA_COST_PER_TKM: f64 = 0.002;
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
/// Gini of income: the mainland countries, then the island.
const GINI: [f64; 4] = [0.30, 0.40, 0.50, 0.45];
const EDU_SHARE: [[f64; N_EDU]; 4] = [
    [0.40, 0.40, 0.20],
    [0.50, 0.35, 0.15],
    [0.30, 0.45, 0.25],
    [0.60, 0.30, 0.10],
];
/// Labour force per inhabitant.
const PARTICIPATION: f64 = 0.47;

type Point = (f64, f64);

fn leg(from: usize, to: usize, a: Point, b: Point, cost_per_tkm: f64) -> Leg {
    let km = (a.0 - b.0).hypot(a.1 - b.1);
    Leg {
        from: from as u32,
        to: to as u32,
        cost_per_t: cost_per_tkm * km,
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
        gateway_mass: equal_gateway_mass(&gateway_country),
        ..Network::default()
    };
    for o in 0..np {
        for d in 0..np {
            if o != d && province_country[o] == province_country[d] {
                net.domestic
                    .push(leg(o, d, province_at[o], province_at[d], LAND_COST_PER_TKM));
            }
        }
        for (a, &gc) in gateway_country.iter().enumerate() {
            if gc == province_country[o] {
                net.access
                    .push(leg(o, a, province_at[o], gateway_at[a], LAND_COST_PER_TKM));
            }
        }
    }
    // land crossings east of country c to west of c + 1; sea between ports 0, 5 and island 6
    let land = (0..MAINLAND - 1).map(|c| (2 * c + 1, 2 * c + 2, LAND_COST_PER_TKM));
    let sea = [
        (0, 2 * MAINLAND - 1),
        (0, 2 * MAINLAND),
        (2 * MAINLAND - 1, 2 * MAINLAND),
    ]
    .map(|(a, b)| (a, b, SEA_COST_PER_TKM));
    // the default bulk freight rate (P154, ADR 0247) scales the sea cost per tonne-km; the
    // fixture has no ports, so P155 has no fee to scale, and straight legs no other path
    let bulk = TradeParams::default().bulk_rate();
    let mut bulk_crossings = Vec::new();
    for (a, b, rate) in land.chain(sea) {
        let bulk_rate = match bulk {
            Some((_, s, _)) if rate == SEA_COST_PER_TKM => rate * s,
            _ => rate,
        };
        for (x, y) in [(a, b), (b, a)] {
            net.crossings
                .push(leg(x, y, gateway_at[x], gateway_at[y], rate));
            bulk_crossings.push(leg(x, y, gateway_at[x], gateway_at[y], bulk_rate));
        }
    }
    net.bulk = bulk.map(|(classes, sea, port)| BulkLegs {
        classes,
        sea,
        port,
        domestic: net.domestic.clone(),
        access: net.access.clone(),
        crossings: bulk_crossings,
    });

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
        // a generated world has no 1995 to 2005 window: its 2000 rate is its natural rate
        u_star: UNEMPLOYMENT.to_vec(),
        gini: GINI.to_vec(),
        pigl: None,
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
            pack: Vec::new(),
            base_price: Vec::new(),
            gamma: Vec::new(),
        },
        pops,
        enterprises: ent,
        steady: None,
        border_nu: None,
    };
    inputs.set_base_prices(&TradeParams::default());
    inputs.set_coefficients();
    inputs.set_base_point(&EconParams::default().edu_eff);
    inputs
}

/// Machinery, which the island does not make: unreachable there in [`unreachable_island`].
pub const UNREACHABLE: usize = 6;
/// The one island sector that still uses [`UNREACHABLE`] in [`unreachable_island`].
pub const ISLAND_USER: usize = 4;

/// The small world with machinery unreachable at the island: no route to or
/// from the island port for that good, every other leg at its least-cost factor (S05) under
/// `trade`, so the island's delivered price of machinery is infinite. Of the island's sectors
/// only light manufacturing still uses machinery; were every sector to use it, the whole
/// island would be input capped and its labour market would have no solution. The base point
/// does not depend on input coefficients, so it stays that of [`small_world`], and so do the
/// base prices: coefficients are set in the base world, the cut comes after. Route taus and
/// bulk legs are exclusive (ADR 0247), so this world drops the bulk legs and runs with the bulk
/// freight rate off: `trade` must have P154 and P155 at 1, or the kernel refuses it.
pub fn unreachable_island(trade: &TradeParams) -> EconInputs {
    let mut inputs = small_world();
    inputs.network.bulk = None;
    let island_port = (2 * MAINLAND) as u32;
    let weight = inputs.weight;
    let factors = |legs: &[Leg], crossings: bool| -> (Vec<[f64; N_GOODS]>, Vec<[f64; N_GOODS]>) {
        legs.iter()
            .map(|l| {
                let cut = crossings && (l.from == island_port || l.to == island_port);
                let tau = std::array::from_fn(|g| {
                    if cut && g == UNREACHABLE {
                        f64::INFINITY
                    } else {
                        // the kernel's least-cost factor, so every other leg is unchanged
                        1.0 + l.cost_per_t * weight[g] + trade.time_cost[g] * l.hours / 24.0
                    }
                });
                (tau, [l.hours; N_GOODS])
            })
            .unzip()
    };
    let net = &inputs.network;
    let (domestic, domestic_hours) = factors(&net.domestic, false);
    let (access, access_hours) = factors(&net.access, false);
    let (crossings, crossing_hours) = factors(&net.crossings, true);
    inputs.network.route_taus = Some(LegTaus {
        domestic,
        access,
        crossings,
        domestic_hours,
        access_hours,
        crossing_hours,
    });
    let island = &mut inputs.countries.io[MAINLAND];
    for g in (0..N_GOODS).filter(|&g| g != ISLAND_USER) {
        island[UNREACHABLE * N_GOODS + g] = 0.0;
    }
    inputs.set_coefficients();
    inputs
}

#[cfg(test)]
mod tests {
    use super::*;
    use ceteris_trade::Kernel;

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
    fn fixture_sea_legs_are_cheaper_than_land() {
        let w = small_world();
        let island_port = 2 * MAINLAND;
        let (mut island_legs, mut land_legs) = (0, 0);
        for l in &w.network.crossings {
            let km = l.hours * KMH;
            let touches_island = l.from as usize == island_port || l.to as usize == island_port;
            let rate = l.cost_per_t / km;
            if touches_island {
                island_legs += 1;
                assert!((rate - 0.002).abs() < 1e-12);
            } else if rate > 0.003 {
                land_legs += 1;
                assert!((rate - 0.02).abs() < 1e-12);
            }
        }
        assert_eq!((island_legs, land_legs), (4, 4));
        let p = TradeParams::default();
        let k = Kernel::build(&w.network, &w.weight, &w.wedges, &p);
        let coal = 2;
        for c in k.good(coal).crossings.row(island_port) {
            let tau = 1.0 / (c.inv_loss * (1.0 + p.border_nu_of(coal)));
            assert!(tau < 2.5, "island coal crossing tau {tau}");
        }
    }

    /// ADR 0247: at the default bulk freight rate, class 3's sea crossings cost P154 times the
    /// sea rate; every other leg is the base one.
    #[test]
    fn fixture_bulk_legs_scale_the_sea_crossings() {
        let net = small_world().network;
        let (classes, sea, _) = TradeParams::default().bulk_rate().unwrap();
        let b = net.bulk.as_ref().unwrap();
        assert_eq!(b.classes, classes);
        assert_eq!((&b.domestic, &b.access), (&net.domestic, &net.access));
        let mut scaled = 0;
        for (x, y) in b.crossings.iter().zip(&net.crossings) {
            assert_eq!((x.from, x.to, x.hours), (y.from, y.to, y.hours));
            if (y.cost_per_t / (y.hours * KMH) - SEA_COST_PER_TKM).abs() < 1e-12 {
                assert!((x.cost_per_t - sea * y.cost_per_t).abs() <= 1e-12 * y.cost_per_t);
                scaled += 1;
            } else {
                assert_eq!(x, y);
            }
        }
        // the three sea lanes, each way
        assert_eq!(scaled, 6);
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
    fn unreachable_island_cuts_only_the_island_machinery_crossings() {
        // the cut world runs with the bulk freight rate off, so compare with the plain one off
        let p = TradeParams::default().bulk_off();
        let (mut plain, cut) = (small_world(), unreachable_island(&p));
        plain.network.bulk = None;
        cut.validate();
        assert_eq!(cut.enterprises.l0, plain.enterprises.l0);
        let kernel = |w: &EconInputs| Kernel::build(&w.network, &w.weight, &w.wedges, &p);
        let (a, b) = (kernel(&plain), kernel(&cut));
        let mut changed = 0;
        for g in (0..N_GOODS).filter(|&g| ceteris_trade::goods::TRADED[g]) {
            let (a, b) = (a.good(g), b.good(g));
            assert_eq!(a.domestic.entries, b.domestic.entries);
            assert_eq!(a.access.entries, b.access.entries);
            for (x, y) in a.crossings.entries.iter().zip(&b.crossings.entries) {
                if x != y {
                    assert_eq!((g, y.k, y.inv_loss), (UNREACHABLE, 0.0, 0.0));
                    changed += 1;
                }
            }
        }
        // both sea lanes of the island, each way
        assert_eq!(changed, 4);
        let island = (0..cut.n_enterprises()).filter(|&e| cut.enterprises.province[e] == 24);
        for e in island {
            let uses = cut.coef(e, UNREACHABLE) > 0.0;
            assert_eq!(uses, usize::from(cut.enterprises.good[e]) == ISLAND_USER);
        }
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
            let u = w.countries.u_star[w.country_of(i)];
            assert!((l / ((1.0 - u) * s) - 1.0).abs() < 1e-12);
        }
    }
}
