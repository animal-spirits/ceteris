//! Spike instrumentation (RFC 0361 step 1): inclusive wall-clock spans and one record per scaling.
//! Not for merge.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct ScalingRecord {
    pub ctx: &'static str,
    pub tol: f64,
    pub sweeps: usize,
    pub ns: u128,
    pub warm: bool,
    pub accel_goods: usize,
    pub slowest_good: usize,
    /// Sweeps each good needed on its own (the sweep after which its gap stayed within tol).
    pub good_max: usize,
    pub good_sum: usize,
    pub ok: bool,
}

#[derive(Default)]
struct Store {
    spans: BTreeMap<&'static str, (u64, u128)>,
    scalings: Vec<ScalingRecord>,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

thread_local! {
    static STACK: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

pub struct Span {
    name: &'static str,
    start: Instant,
}

pub fn span(name: &'static str) -> Span {
    STACK.with(|s| s.borrow_mut().push(name));
    Span {
        name,
        start: Instant::now(),
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        let ns = self.start.elapsed().as_nanos();
        STACK.with(|s| s.borrow_mut().pop());
        let mut g = STORE.lock().unwrap();
        let e = g.get_or_insert_with(Store::default).spans.entry(self.name).or_default();
        e.0 += 1;
        e.1 += ns;
    }
}

pub fn context() -> &'static str {
    STACK.with(|s| s.borrow().last().copied().unwrap_or("-"))
}

pub fn record(r: ScalingRecord) {
    STORE.lock().unwrap().get_or_insert_with(Store::default).scalings.push(r);
}

pub fn reset() {
    *STORE.lock().unwrap() = Some(Store::default());
}

pub fn scalings() -> Vec<ScalingRecord> {
    STORE.lock().unwrap().as_ref().map(|s| s.scalings.clone()).unwrap_or_default()
}

pub fn report() -> String {
    let g = STORE.lock().unwrap();
    let Some(s) = g.as_ref() else {
        return String::new();
    };
    let mut out = String::from("span (inclusive)                     calls      total s     mean ms\n");
    let mut spans: Vec<_> = s.spans.iter().collect();
    spans.sort_by_key(|(_, (_, ns))| std::cmp::Reverse(*ns));
    for (name, (n, ns)) in spans {
        out += &format!(
            "{name:<34} {n:>7} {:>12.3} {:>11.3}\n",
            *ns as f64 / 1e9,
            *ns as f64 / 1e6 / *n as f64
        );
    }
    let mut by: BTreeMap<&str, Vec<&ScalingRecord>> = BTreeMap::new();
    for r in &s.scalings {
        by.entry(r.ctx).or_default().push(r);
    }
    out += "\nscalings by caller: count, total s, sweeps min/median/max, sum of sweeps, goods-waste (n_goods*sweeps / sum per-good), failed\n";
    for (ctx, rs) in by {
        let mut sw: Vec<usize> = rs.iter().map(|r| r.sweeps).collect();
        sw.sort_unstable();
        let total_ns: u128 = rs.iter().map(|r| r.ns).sum();
        let waste: f64 = rs.iter().map(|r| (r.accel_goods * r.sweeps) as f64).sum::<f64>()
            / rs.iter().map(|r| r.good_sum as f64).sum::<f64>().max(1.0);
        out += &format!(
            "{ctx:<24} {:>5} {:>9.3} {:>6}/{:>6}/{:>6} {:>9} {:>6.2}x {:>3}\n",
            rs.len(),
            total_ns as f64 / 1e9,
            sw[0],
            sw[sw.len() / 2],
            sw[sw.len() - 1],
            sw.iter().sum::<usize>(),
            waste,
            rs.iter().filter(|r| !r.ok).count()
        );
    }
    out
}
