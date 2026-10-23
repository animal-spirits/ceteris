//! Print least costs per tonne from one province to a few others:
//! `cargo run --release -p ceteris-geo --example costs -- NLD-3485 CHN-1177 USA-3521`

use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::{Pack, default_dir};
use ceteris_geo::paths::shortest;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pack = Pack::load(&default_dir(), &TransportParams::default())?;
    let p = &pack.provinces;
    let source = p.index_of(&args[0]).ok_or("unknown source province")?;
    let tree = shortest(&pack.graph, source);
    let unreachable: Vec<&str> = (0..p.len())
        .filter(|&i| !tree.cost[i].is_finite())
        .map(|i| p.id[i].as_str())
        .collect();
    println!(
        "from {} ({}): unreachable {unreachable:?}",
        p.name[source as usize], p.country[source as usize]
    );
    for id in &args[1..] {
        let i = p.index_of(id).ok_or("unknown province")? as usize;
        println!(
            "  {:<28} {} {:>8.2} USD/t {:>7.1} h",
            p.name[i], p.country[i], tree.cost[i], tree.hours[i]
        );
    }
    Ok(())
}
