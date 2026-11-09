//! Wall-time spans for the sim-loop timing harness (ADR 0294, RFC 0287 item 1). Off by default:
//! a span then costs one relaxed atomic load. Spans read the clock and never the state, so a
//! timed run is bit-identical to an untimed one.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// No good, or no rayon worker.
pub const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub tick: u32,
    pub name: &'static str,
    pub good: u32,
    /// The rayon worker index, or [`NONE`] outside the pool (the main thread).
    pub thread: u32,
    /// Nanoseconds since [`enable`].
    pub start: u64,
    pub end: u64,
}

static ON: AtomicBool = AtomicBool::new(false);
static TICK: AtomicU32 = AtomicU32::new(0);
static EPOCH: OnceLock<Instant> = OnceLock::new();
static SPANS: Mutex<Vec<Span>> = Mutex::new(Vec::new());

pub fn enable() {
    EPOCH.get_or_init(Instant::now);
    ON.store(true, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

/// The tick later spans are tagged with.
pub fn set_tick(tick: u32) {
    TICK.store(tick, Ordering::Relaxed);
}

/// Every span recorded so far, in completion order, leaving none.
pub fn take() -> Vec<Span> {
    std::mem::take(&mut *SPANS.lock().expect("timing lock"))
}

/// Records from now until the guard drops; `None` while timing is off.
pub fn span(name: &'static str, good: u32) -> Option<Guard> {
    enabled().then(|| Guard {
        name,
        good,
        start: now(),
    })
}

pub struct Guard {
    name: &'static str,
    good: u32,
    start: u64,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let span = Span {
            tick: TICK.load(Ordering::Relaxed),
            name: self.name,
            good: self.good,
            thread: rayon::current_thread_index().map_or(NONE, |i| i as u32),
            start: self.start,
            end: now(),
        };
        SPANS.lock().expect("timing lock").push(span);
    }
}

fn now() -> u64 {
    EPOCH.get().map_or(0, |e| e.elapsed().as_nanos() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_record_only_when_enabled() {
        assert!(span("off", NONE).is_none() || enabled());
        enable();
        set_tick(7);
        drop(span("on", 3));
        let spans = take();
        let s = spans.iter().find(|s| s.name == "on").expect("recorded");
        assert_eq!((s.tick, s.good, s.thread), (7, 3, NONE));
        assert!(s.end >= s.start);
    }
}
