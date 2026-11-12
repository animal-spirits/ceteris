//! Comparative statics on a calibrated pack: start from the calibrated steady state, apply a
//! shock to the inputs, run frictionless (V1 mode) and write country aggregates
//! (`ceteris_calibrate::shock`, `ceteris_calibrate::benchmark`).
//!
//! ```text
//! cargo run --release -p ceteris-calibrate --example shock -- PACK SHOCKS TICKS OUT
//! ```
//!
//! SHOCKS is a shock file (`ceteris_calibrate::shock`, countries by ISO3 code), or `-` for none.
//! Writes `conv.csv` (largest |d ln p| per 50 ticks), `countries.csv`, `goods.csv` and
//! `flows.csv` (mean flows of the last 12 ticks, at origin prices) into OUT. The price rule is
//! `CETERIS_PRICE_ANCHOR` (`off`, `e1` or `e3`; `ceteris_calibrate::benchmark::price_anchor`).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use ceteris_calibrate::{benchmark, shock};
use ceteris_core::sim::Sim;
use ceteris_econ::systems::{SEED, schedule};

const EVERY: u32 = 50;

fn csv(path: &Path, header: &str, rows: impl Iterator<Item = String>) {
    let mut f = fs::File::create(path).expect("csv");
    writeln!(f, "{header}").expect("write");
    for r in rows {
        writeln!(f, "{r}").expect("write");
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [pack, shock_file, ticks, out] = a.as_slice() else {
        panic!("usage: shock PACK SHOCKS TICKS OUT");
    };
    let (pack, out) = (PathBuf::from(pack), PathBuf::from(out));
    let ticks: u32 = ticks.parse().expect("ticks");
    fs::create_dir_all(&out).expect("out");
    let shocks = if shock_file == "-" {
        Vec::new()
    } else {
        let codes = benchmark::codes(&pack).expect("country codes");
        let n_goods = benchmark::n_goods(&pack).expect("goods");
        shock::parse(
            &fs::read_to_string(shock_file).expect("shock file"),
            &codes,
            n_goods,
        )
        .expect("shock file")
    };

    let mut sim = Sim::new(
        benchmark::world(
            &pack,
            &shocks,
            benchmark::anchor_from_env().expect("anchor"),
        )
        .expect("world"),
        schedule(),
        SEED,
    );
    let mut conv = Vec::new();
    let mut last = sim.world().markets.price.clone();
    while sim.tick() < ticks {
        let next = (sim.tick() + EVERY).min(ticks);
        sim.run_until(next).expect("no violation");
        let w = sim.world();
        let step = benchmark::max_dlnp(w, &last);
        conv.push(format!("{next},{step:e},{}", w.world_price_index()));
        eprintln!("tick {next}: max |dln p| over {EVERY} ticks {step:.3e}");
        last.clone_from(&w.markets.price);
    }
    csv(
        &out.join("conv.csv"),
        "tick,max_dlnp,world_index",
        conv.into_iter(),
    );

    let w = sim.world();
    let o = benchmark::outcome(w, f64::NAN, sim.tick());
    let inp = w.inputs();
    let (nc, np) = (inp.n_countries(), inp.n_provinces());
    let (n_goods, traded) = (inp.goods.n(), inp.goods.traded);
    let country = |i: usize| inp.country_of(i);

    let mut people = vec![0.0; nc];
    for k in 0..inp.n_pops() {
        people[country(inp.pops.province[k] as usize)] += inp.pops.population[k];
    }
    csv(
        &out.join("countries.csv"),
        "country,income,population,wage_bill",
        (0..nc).map(|c| {
            let wb: f64 = (0..inp.n_enterprises())
                .filter(|&e| country(inp.enterprises.province[e] as usize) == c)
                .map(|e| w.wage[inp.enterprises.province[e] as usize] * w.employment[e])
                .sum();
            format!("{c},{},{},{wb}", o.income[c], people[c])
        }),
    );
    let (mut emp, mut outp, mut sales, mut pvol) = (
        vec![vec![0.0; n_goods]; nc],
        vec![vec![0.0; n_goods]; nc],
        vec![vec![0.0; n_goods]; nc],
        vec![vec![0.0; n_goods]; nc],
    );
    for e in 0..inp.n_enterprises() {
        let i = inp.enterprises.province[e] as usize;
        let (c, g) = (country(i), usize::from(inp.enterprises.good[e]));
        emp[c][g] += w.employment[e];
        outp[c][g] += w.output[e];
        sales[c][g] += w.sales[e];
        pvol[c][g] += w.output[e] * w.markets.price[i * n_goods + g];
    }
    csv(
        &out.join("goods.csv"),
        "country,good,omega,buyer_price,employment,output,sales,output_value",
        (0..nc).flat_map(|c| {
            let (o, emp, outp, sales, pvol) = (&o, &emp, &outp, &sales, &pvol);
            (0..n_goods).map(move |g| {
                format!(
                    "{c},{g},{},{},{},{},{},{}",
                    o.omega[c][g],
                    o.buyer_price[c][g],
                    emp[c][g],
                    outp[c][g],
                    sales[c][g],
                    pvol[c][g]
                )
            })
        }),
    );
    let mean = w.scratch.flows.mean();
    let mut rows = Vec::new();
    for ex in 0..nc {
        for im in 0..nc {
            for g in 0..traded {
                let at = (ex * nc + im) * n_goods + g;
                let mut v = mean.pair_value[at];
                if ex == im {
                    v += (0..np)
                        .filter(|&i| country(i) == ex)
                        .map(|i| mean.home_value[i * n_goods + g])
                        .sum::<f64>();
                }
                if v > 0.0 {
                    let t = inp.wedges.tariff[(im * nc + ex) * 2 + inp.goods.tariff_group(g)];
                    let t = if ex == im { 0.0 } else { f64::from(t) };
                    rows.push(format!("{ex},{im},{g},{v},{t}"));
                }
            }
        }
    }
    csv(
        &out.join("flows.csv"),
        "exporter,importer,good,value,tariff",
        rows.into_iter(),
    );
}
