# 0208. Presentation pipeline and time controls

Status: accepted (owner, 2026-10-28)
Date: 2026-10-28
Design sections: 4.4, 14.3, 19.2, 19.4, 20.3; ADRs 0011, 0088, 0097, 0145, 0188, 0196, 0207

## Context

The owner asked how time can flow at maximum smooth speed with a stable, high frame rate.
Assessment: https://claude.ai/artifact/VTacA58HcchK1aMaTeYbJG

Tick costs vary by more than an order of magnitude: the econ tick is 34.9 ms (measured
2026-10-28, migration thread); 0145 estimates 0.2 to 0.9 s per simulated month at war. A frame
at 120 Hz is 8.3 ms. If the frame loop waits on a tick, or a tick waits on a frame, playback
stutters whenever tick cost changes. DESIGN 20.3 has five speeds but does not say what a speed
is; ticks per second would make war visibly slow time at every speed.

ADR 0088 already draws the map into a texture from Slint's rendering notifier, and 0188 lands
edits at the view tick, not at the head. Neither says how the simulation and the frame loop are
scheduled against each other.

## Options

Speed: (1) ticks per second; (2) simulated time per wall second.

Scheduling: (a) the frame loop steps the simulation; (b) the simulation runs on its own pool and
publishes finished ticks, playback follows; (c) option b, and the simulation also keeps
computing ahead while paused.

## Decision

Option 2 and option (c) (owner, 2026-10-28).

- **Two loops.** The simulation runs fixed ticks (monthly, daily, hourly per 0145 and 0209) on
  its own thread pool and never draws. The frame loop runs at display refresh on the main
  thread and never calls into a tick. Each finished tick publishes an immutable view: the
  values of every map mode per province, changed cells, unit positions, new events. The frame
  loop takes the latest views by an atomic swap.
- **Run-ahead ring.** Finished ticks wait in a ring ahead of the presented time: 2 to 4 ticks,
  or about half a second of presented time, whichever is smaller. If the ring is full the
  simulation waits; if it runs dry, presented time eases down to the rate the simulation
  sustains and the time controls show a sim-bound chip. No frame is skipped either way.
- **Thinking ahead while paused.** While paused, idle cores keep filling the ring and then a
  further buffer up to a memory cap, so play resumes at once.
- **Edits.** An edit lands at the view tick (0188). Every computed tick after it is dropped and
  recomputed. Determinism (4.5, 0097) makes the kept ticks identical to a fresh run, so
  run-ahead changes no hash.
- **No leaks in seat play.** Nothing in the buffer reaches the screen before its presented
  time. Watchers, alerts and pause-on rules (0188) fire when an event is presented, never when
  it is computed.
- **Interpolation.** The map shader receives the two bracketing ticks' values (uploaded once per
  tick) and one factor per frame, and mixes them through the feature colour texture
  (0010, 0011). Units move between their published positions as instanced sprites. The date
  counts days between monthly ticks. Panels, tooltips and exports always show tick values,
  never interpolated ones, and name the tick.
- **Changed cells only.** Cell-grain layers (0115) upload a list of (cell, value) pairs per
  tick, scattered by a compute pass, not the whole grid.
- **Speeds** are simulated time per wall second: 1 = one day, 2 = one week, 3 = one month,
  4 = one quarter, 5 = as fast as the machine runs. Keys stay as in 0188.
- **Thread classes.** On macOS the frame loop runs at user-interactive QoS on a performance
  core; the simulation pool on the remaining performance cores; history compression, snapshot
  writes and ensemble lanes at utility QoS in separate pools. Elsewhere, the same split by
  thread priority. History and snapshot writes for tick t overlap tick t+1's compute.
- **Frame loop rules.** No allocation, file I/O or lock waits while drawing; pipelines compiled
  and cached at start; panel models diff visible rows only (0088) and refresh when the
  presented tick changes, not every frame.
- **Run to next event** (alert, situation change, date) throttles drawing to a progress view so
  the simulation gets every core but one.

## Tests

- Run-ahead on and off give the same state hash at every snapshot.
- An edit at a view tick behind the head gives the same hash as the same edit on a run that
  never computed ahead.
- Frame-time p99 under 8.3 ms on the 0088 spike hardware while playing at speed 5 on the full
  pack, measured with GPU timestamps.
- A seat-play replay shows no event, alert or pause before its presented tick.

## Consequences

Playback stays smooth across peace and war, and speed means the same thing in both. Resume is
instant after a pause. The cost is memory for the ring and buffer (views are small; about 19 KB
of map values per tick per mode) and the discipline of keeping the frame loop free of
simulation calls. Scrubbing back through history and the auto-pacing and side-by-side branch
ideas in the assessment are not decided here.

## Migration

- App shell phase (0207), with ADR 0088 stages 1 and 2: the two loops, ring, interpolation,
  speeds, thread classes. Run to next event and thinking ahead while paused may slip to P3.
  Estimate 1.5 to 2.5 part-time weeks.
- DESIGN 20.3: speeds defined as above; 19.4: the two-loop rule.
