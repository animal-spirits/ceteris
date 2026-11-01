# 0209. Sparse hourly war phase

Status: proposed (clock chosen by the owner, 2026-10-28)
Date: 2026-10-28
Design sections: 4.3, 4.5, 11.5, 19.2, 23.5; ADRs 0132, 0145, 0147, 0148, 0149, 0150, 0151, 0152, 0208
Phase: P7 (2.0-beta); amends 0145 and 0149

## Context

The owner asked whether war can run hour by hour, as Hearts of Iron does. 0145 runs war on a
daily sub-tick and 0149 resolves each front segment once a day. Assessment:
https://claude.ai/artifact/VTacA58HcchK1aMaTeYbJG

A day is too coarse for some war processes:

- A task group at 25 knots covers about 1,100 km a day, more than most of the 725 sea zones, so
  a daily step cannot tell whether two fleets met in a zone or passed through it (0151).
- Sortie cycles are 3 to 6 hours, and air operations depend on day and night (0150).
- HQ order latency (0147) is hours at brigade and division level; rounding to days overstates
  it.
- A breakthrough's pursuit state (0149) can carry tens of kilometres within the day it opens.
- Strike flight times are minutes to hours (0152).

Stepping every war system hourly costs 24 times the daily budget: 600 to 720 ms per simulated
day in a world war (estimate from 0145's 25 to 30 ms), about 1.5 simulated days per second.

## Options

1. Daily only, as 0145 and 0149 stand.
2. Daily commit with a sparse hourly phase.
3. Uniform hourly ticks for every war system.
4. Continuous time in integer minutes, as a discrete-event simulation.

## Decision

Option 2 (owner, 2026-10-28).

- **The day stays the commit step.** Snapshots, state hashes, the supply solve (0148), delegated
  planners, sensor search and belief updates (0153), and air and naval tasking stay daily, in
  0145's order.
- **Hourly phase.** After tasking and supply, and in place of 0145's movement and engagement
  steps, the day runs 24 hourly slots. Each slot processes, in a fixed order: orders whose
  latency elapses that hour; arrivals and contact; one combat round on each engaged segment;
  sorties and strikes that resolve that hour; naval search and interception. Encirclement, morale
  and repairs stay daily, after the 24th slot.
- **Sparse.** Only engaged segments step every hour. Moving formations, sorties, task groups
  and strikes enter the phase as events at the hour they are due. Reserves, garrisons and quiet
  fronts do nothing until an event wakes them.
- **Event order.** A 24-slot timing wheel (as for shipments, 19.4). Within a slot, events run
  by kind priority, then entity ID. Time inside a day is an integer hour; no floating time.
- **Draws.** Keys gain the hour: (run seed, war, entity, day, hour, purpose). Draws keyed by day
  in 0145 to 0154 take hour 0.
- **Orders and edits** carry (day, hour) and sort in the intervention list by them.
- **Resolver.** 0149's daily rates become per-hour rates applied each slot, with day and night
  terms in V and E. Its daily closed form remains the lab profile's step.
- **Snapshots** stay at day boundaries. Hourly state is transient and rebuilt by replaying the
  day from its start.
- **Presentation.** The hourly event log feeds 0208's playback: at speed 1 (one day per second)
  battles play hour by hour, and any past day can be replayed as an animation without
  simulation cost. 0132's real-time crisis pacing runs on hours.
- **Budget** (estimates): a large regional war adds 3 to 6 ms per day (about 10 to 15 ms in
  total); a world war adds 25 to 40 ms (about 50 to 70 ms in total), before theatre
  parallelism.

## Tests

- A segment with constant inputs stepped through 24 hourly rounds matches 0149's daily closed
  form within a stated tolerance; the lab profile still nests the extended one (0145).
- Same seed and orders give the same hash with one thread and with many, and when a day is
  replayed from its start.
- A run with no war never enters the daily or hourly phase, and its hash equals a build without
  `ceteris-war`.
- Two task groups crossing one sea zone in opposite directions on the same day meet with the
  probability of 0151's search model, not zero.

## Consequences

Naval interception, sortie timing, order latency and same-day exploitation get the resolution
they need, at about twice the daily cost in a world war and much less in a regional one.
Interactive speed in a world war falls from about 35 to about 17 simulated days per second
before theatre parallelism, which can recover 2 to 3 times on 10 to 12 cores (estimate). The
day/night cycle becomes visible on the map. The daily layers, including the supply solver,
still set the risk line.

## Migration

- P7, with stages W2 (fronts) to W7 (naval) of 0145; adds about 3 to 5 part-time weeks.
- 0145: the daily order's movement and engagement steps run inside the hourly phase; the draw
  key gains the hour. 0149: segments resolve once per hour while engaged.
- DESIGN 4.3: the hourly phase row; 19.2: war budget per this ADR.
