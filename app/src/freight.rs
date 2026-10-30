//! Freight cost from an origin (P1 map mode): least-cost trees from `ceteris-geo`, on a worker
//! thread so the pack load and each tree stay off the frame loop.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_geo::paths::PathCache;

/// USD per tonne and hours from `origin` to each province.
pub struct Costs {
    pub origin: u32,
    pub cost: Vec<f64>,
    pub hours: Vec<f64>,
}

pub struct Freight {
    requests: Sender<u32>,
    results: Receiver<Result<Costs, String>>,
}

impl Freight {
    /// Loads the pack in `dir` and checks its provinces are `ids`, in order; `wake` runs after
    /// each result is ready.
    pub fn spawn(dir: PathBuf, ids: Vec<String>, wake: impl Fn() + Send + 'static) -> Self {
        let (requests, inbox) = channel::<u32>();
        let (outbox, results) = channel();
        std::thread::spawn(move || {
            let pack = Pack::load(&dir, &TransportParams::default())
                .map_err(|e| format!("freight cost unavailable: {e}"))
                .and_then(|pack| {
                    if pack.provinces.id == ids {
                        Ok(pack)
                    } else {
                        Err("freight cost unavailable: the pack's provinces.arrow and web/provinces.json differ; rebuild the pack".into())
                    }
                });
            let pack = match pack {
                Ok(pack) => pack,
                Err(e) => {
                    let _ = outbox.send(Err(e));
                    wake();
                    return;
                }
            };
            let n = pack.provinces.len();
            let mut cache = PathCache::default();
            while let Ok(mut origin) = inbox.recv() {
                // Only the latest origin matters.
                while let Ok(next) = inbox.try_recv() {
                    origin = next;
                }
                let tree = cache.tree(&pack.graph, origin);
                let costs = Costs {
                    origin,
                    cost: tree.cost[..n].to_vec(),
                    hours: tree.hours[..n].to_vec(),
                };
                if outbox.send(Ok(costs)).is_err() {
                    return;
                }
                wake();
            }
        });
        Self { requests, results }
    }

    pub fn request(&self, origin: u32) {
        let _ = self.requests.send(origin);
    }

    /// The latest finished result, if any. Never blocks.
    pub fn poll(&self) -> Option<Result<Costs, String>> {
        self.results.try_iter().last()
    }
}
