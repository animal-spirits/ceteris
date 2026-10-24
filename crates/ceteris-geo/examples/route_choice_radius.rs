//! Measure the route-choice dispersion theta_r (P44) on the built pack (ADR 0017, MIGRATION
//! Task 4): per traded good, the smallest theta_r with spectral radius below 0.99 (bisection on
//! log theta_r), the radius at the prior's ends, and the time of all domestic and gateway solves.
//!
//! `cargo run -p ceteris-geo --release --example route_choice_radius [-- theta_r...]`; extra
//! arguments are theta_r values at which to time the solves as well.

use std::time::Instant;

use ceteris_geo::graph::{NodeKind, TransportParams};
use ceteris_geo::pack::{Pack, default_dir};
use ceteris_geo::paths::shortest;
use ceteris_geo::route_choice::{EdgeFactors, solve_from};
use rayon::prelude::*;

/// Weight per value w_g in t/USD (BACI 2000, PLAN Task 13 report) and lambda_g (P43 starts).
const GOODS: [(&str, f64, f64); 7] = [
    ("agriculture", 0.0026752, 0.002),
    ("oil and gas", 0.0049491, 0.002),
    ("coal and mining", 0.0190502, 0.002),
    ("food", 0.0016041, 0.005),
    ("light manufacturing", 0.00026893, 0.010),
    ("chemicals, fuels, metals", 0.0015934, 0.010),
    ("machinery", 0.00010144, 0.010),
];
/// P44 prior: Fan, Lu and Luo (2021) theta = 111.5, s.e. 35.4; point estimate +- 2 s.e.
const PRIOR: (f64, f64) = (41.0, 182.0);
const RHO_MAX: f64 = 0.99;
/// Gateway proxy until `gateways.arrow` exists (PLAN Task 17): about this many ports and
/// airports, every k-th in node order.
const GATEWAYS: usize = 600;
/// Rotterdam, then Hamburg, Shanghai, New York.
const PROBES: [&str; 4] = ["NLD-3485", "DEU-1578", "CHN-1819", "USA-3559"];

fn radius(pack: &Pack, w: f64, lambda: f64, theta_r: f64) -> f64 {
    EdgeFactors::new(&pack.graph, w, lambda, theta_r).spectral_radius()
}

/// Smallest theta_r with radius below `RHO_MAX`, to 0.5% (radius falls as theta_r rises).
fn min_theta(pack: &Pack, w: f64, lambda: f64) -> f64 {
    let (mut lo, mut hi) = (0.0f64, libm::log(1e6));
    assert!(radius(pack, w, lambda, libm::exp(hi)) < RHO_MAX);
    while hi - lo > 0.005 {
        let mid = 0.5 * (lo + hi);
        if radius(pack, w, lambda, libm::exp(mid)) < RHO_MAX {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    libm::exp(hi)
}

struct Timing {
    gateway_s: f64,
    domestic_s: f64,
    /// Domestic province pairs with tau below 1, and all domestic pairs.
    below_one: usize,
    pairs: usize,
    /// Domestic pairs with no route (tau infinite) although Dijkstra reaches them.
    unreached: usize,
}

fn time_solves(pack: &Pack, f: &EdgeFactors, gateways: &[u32], countries: &[Vec<u32>]) -> Timing {
    let t0 = Instant::now();
    let reached: usize = gateways
        .par_iter()
        .map(|&s| {
            let r = solve_from(f, s, None, None);
            r.tau[..pack.provinces.len()]
                .iter()
                .filter(|t| t.is_finite())
                .count()
        })
        .sum();
    let gateway_s = t0.elapsed().as_secs_f64();
    assert!(reached > 0);
    let t0 = Instant::now();
    let jobs: Vec<(usize, u32)> = countries
        .iter()
        .enumerate()
        .flat_map(|(c, nodes)| {
            nodes
                .iter()
                .filter(|&&k| (k as usize) < pack.provinces.len())
                .map(move |&k| (c, k))
        })
        .collect();
    let stats: Vec<(usize, usize, usize)> = jobs
        .par_iter()
        .map(|&(c, s)| {
            let nodes = &countries[c];
            let r = solve_from(f, s, Some(nodes), None);
            let mut out = (0, 0, 0);
            for (i, &k) in nodes.iter().enumerate() {
                if k == s || (k as usize) >= pack.provinces.len() {
                    continue;
                }
                out.1 += 1;
                if r.tau[i] < 1.0 {
                    out.0 += 1;
                }
                if !r.tau[i].is_finite() {
                    out.2 += 1;
                }
            }
            out
        })
        .collect();
    let domestic_s = t0.elapsed().as_secs_f64();
    let sum = |i: usize| -> usize { stats.iter().map(|s| [s.0, s.1, s.2][i]).sum() };
    Timing {
        gateway_s,
        domestic_s,
        below_one: sum(0),
        pairs: sum(1),
        unreached: sum(2),
    }
}

/// Nodes of each 2000 country's subgraph: its provinces and the rail, port and airport nodes
/// of those provinces; sea nodes belong to no country.
fn country_subgraphs(pack: &Pack) -> Vec<Vec<u32>> {
    let mut names: Vec<&str> = pack.provinces.country.iter().map(String::as_str).collect();
    names.sort_unstable();
    names.dedup();
    let mut out = vec![Vec::new(); names.len()];
    let g = &pack.graph;
    for k in 0..g.nodes() {
        let p = g.node_province[k];
        if p == u32::MAX || g.kind[k] == NodeKind::Sea {
            continue;
        }
        let c = names
            .binary_search(&pack.provinces.country[p as usize].as_str())
            .unwrap();
        out[c].push(k as u32);
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pack = Pack::load(&default_dir(), &TransportParams::default())?;
    let g = &pack.graph;
    let extra: Vec<f64> = std::env::args()
        .skip(1)
        .map(|a| a.parse())
        .collect::<Result<_, _>>()?;
    let hubs: Vec<u32> = (0..g.nodes() as u32)
        .filter(|&k| matches!(g.kind[k as usize], NodeKind::Port | NodeKind::Airport))
        .collect();
    let step = hubs.len().div_ceil(GATEWAYS);
    let gateways: Vec<u32> = hubs.iter().copied().step_by(step).collect();
    let countries = country_subgraphs(&pack);
    println!(
        "pack: {} nodes, {} edges; {} gateway proxies (every {step}th of {} ports and airports); \
         {} country subgraphs, largest {} nodes; {} threads",
        g.nodes(),
        g.edges(),
        gateways.len(),
        hubs.len(),
        countries.len(),
        countries.iter().map(Vec::len).max().unwrap_or(0),
        rayon::current_num_threads()
    );
    println!("prior theta_r {} to {}", PRIOR.0, PRIOR.1);
    println!(
        "{:<26} {:>10} {:>9} {:>9} {:>10} {:>11} {:>8} {:>13} {:>10}",
        "good",
        "min th_r",
        "rho@lo",
        "rho@hi",
        "gateway s",
        "domestic s",
        "tau<1",
        "domestic pairs",
        "no route"
    );
    let probes: Vec<u32> = PROBES
        .iter()
        .map(|id| pack.provinces.index_of(id).expect("probe in pack"))
        .collect();
    let least = shortest(g, probes[0]);
    for &(name, w, lambda) in &GOODS {
        let t0 = Instant::now();
        let theta = min_theta(&pack, w, lambda);
        let search_s = t0.elapsed().as_secs_f64();
        let f = EdgeFactors::new(g, w, lambda, theta);
        let t = time_solves(&pack, &f, &gateways, &countries);
        println!(
            "{name:<26} {theta:>10.1} {:>9.4} {:>9.4} {:>10.1} {:>11.1} {:>8} {:>13} {:>10}   (search {search_s:.0} s)",
            radius(&pack, w, lambda, PRIOR.0),
            radius(&pack, w, lambda, PRIOR.1),
            t.gateway_s,
            t.domestic_s,
            t.below_one,
            t.pairs,
            t.unreached,
        );
        for &theta in std::iter::once(&theta).chain(&extra) {
            let f = EdgeFactors::new(g, w, lambda, theta);
            if f.spectral_radius() >= 1.0 {
                println!("    theta_r {theta}: radius {} >= 1", f.spectral_radius());
                continue;
            }
            let r = solve_from(&f, probes[0], None, None);
            let row: Vec<String> = probes[1..]
                .iter()
                .map(|&d| {
                    let d = d as usize;
                    let s05 = 1.0 + least.cost[d] * w + lambda * least.hours[d] / 24.0;
                    format!(
                        "{:.4} (S05 {:.4}, {:.0} h vs {:.0} h)",
                        r.tau[d], s05, r.hours[d], least.hours[d]
                    )
                })
                .collect();
            println!(
                "    theta_r {theta:.1}, Rotterdam to {:?}: {row:?}",
                &PROBES[1..]
            );
        }
        for &theta in &extra {
            let f = EdgeFactors::new(g, w, lambda, theta);
            if f.spectral_radius() < 1.0 {
                let t = time_solves(&pack, &f, &gateways, &countries);
                println!(
                    "    theta_r {theta}: rho {:.4}, gateway {:.1} s, domestic {:.1} s, tau<1 {}, no route {}",
                    f.spectral_radius(),
                    t.gateway_s,
                    t.domestic_s,
                    t.below_one,
                    t.unreached
                );
            }
        }
    }
    Ok(())
}
