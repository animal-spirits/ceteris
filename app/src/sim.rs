//! The simulation loop (ADR 0208): a thread that steps the world and publishes one view per
//! finished tick into a bounded run-ahead ring, which the frame loop drains without blocking.
//! The thread waits when the ring is full and never draws.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, sync_channel};

use ceteris_core::sim::{Sim, State};

/// Why the run stopped (ADR 0214). `kind` is the violated invariant, `panic`, or `load` when the
/// world could not be built.
#[derive(Debug, Clone, PartialEq)]
pub struct Halt {
    pub tick: u32,
    pub kind: String,
    pub detail: String,
}

impl Halt {
    fn load(detail: String) -> Self {
        Self {
            tick: 0,
            kind: "load".into(),
            detail,
        }
    }

    fn panic(tick: u32, payload: &(dyn Any + Send)) -> Self {
        let detail = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "non-string panic payload".into());
        Self {
            tick,
            kind: "panic".into(),
            detail,
        }
    }
}

pub struct Runner<V> {
    views: Receiver<Result<V, Halt>>,
}

impl<V: Send + 'static> Runner<V> {
    /// Builds the run on the new thread, publishes the view of tick 0, then of every tick it
    /// steps. `ring` ticks may wait ahead of the frame loop; `wake` runs after each publish.
    pub fn spawn<W: State + 'static>(
        ring: usize,
        build: impl FnOnce() -> Result<Sim<W>, String> + Send + 'static,
        view: impl Fn(&Sim<W>) -> V + Send + 'static,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (ring, views) = sync_channel(ring);
        std::thread::spawn(move || {
            #[cfg(feature = "profile")]
            tracy_client::set_thread_name!("sim");
            let mut sim = match build() {
                Ok(sim) => sim,
                Err(e) => {
                    let _ = ring.send(Err(Halt::load(e)));
                    wake();
                    return;
                }
            };
            loop {
                // Blocks while the ring is full; ends when the app drops the runner.
                let published = {
                    crate::zone!("view");
                    view(&sim)
                };
                if ring.send(Ok(published)).is_err() {
                    return;
                }
                wake();
                let stepped = {
                    crate::zone!("step");
                    catch_unwind(AssertUnwindSafe(|| sim.step()))
                };
                let halt = match stepped {
                    Ok(Ok(())) => continue,
                    Ok(Err(v)) => Halt {
                        tick: v.tick,
                        kind: v.invariant.into(),
                        detail: v.detail,
                    },
                    Err(payload) => Halt::panic(sim.tick() + 1, payload.as_ref()),
                };
                let _ = ring.send(Err(halt));
                wake();
                return;
            }
        });
        Self { views }
    }

    /// The next published view, if one is ready. Never blocks.
    pub fn next(&self) -> Option<Result<V, Halt>> {
        self.views.try_recv().ok()
    }

    /// Blocks for the next view; for tests and headless use.
    pub fn wait(&self) -> Option<Result<V, Halt>> {
        self.views.recv().ok()
    }
}
