# 0016. Clear origin markets across ticks

Status: accepted (owner, 2026-10-24, PR 1); implemented on p2-econ (MIGRATION Task 3)
Date: 2026-10-24
Design sections: 8.2, 8.3, 17.2 (S02), 19.2, 19.4

## Context

Running Sinkhorn to convergence each month solves a within-tick spatial equilibrium, and the
inventory rule of 8.2 then moves prices on top of it: two adjustment mechanisms for one gap. It
also costs 5 to 10 iterations of two half-steps per good class per tick, the largest line in
the 19.2 budget.

## Options

(1) Full Sinkhorn each tick. (2) One destination normalization per tick at posted prices;
origin excess supply goes to inventory and moves p_o through 8.2. (3) A fixed small number of
sweeps per tick.

## Decision

Option 2 by default; option 1 stays as switch S02 for nesting tests. A destination
normalization followed by an origin price update is one Sinkhorn sweep, so option 2 is Sinkhorn
spread over ticks, and its frictionless fixed point is the same structural-gravity equilibrium.
Option 3 adds a free iteration count that changes the dynamics.

## Migration

- Replace the per-tick solver loop with one normalization per (theta class, weight class).
- Remove the Sinkhorn scaling vectors from tick state and snapshots; keep them only inside the
  S02 test path.
- Route origin excess supply (output plus arrivals minus sales) into merchant inventory. Take
  the unbalanced-transport path out of the default system order.
- The 8.2 rule is now the only origin clearing mechanism; ADR 0019 covers its stability.

## Tests

- Small worlds (5 to 20 provinces, 3 goods): with kappa large and transit zero, the fixed point
  matches the S02 full solve to 1e-6.
- Common-random-numbers steady-state comparison of the default and S02 on the European slice.
- Criterion benchmark: the trade system within its 19.2 budget of 5 ms on the full globe.

## Mapping to P2 as built

P2 runs no Sinkhorn per tick and holds no scaling vectors, so the solver loop has nothing to
remove. The change is where markets clear: P2 clears traded goods at the destination (supply is
home-routed output plus arrivals) and prices move on the destination gap.

- Traded goods clear at the origin: Q_o is output plus merchant inventory, D_o is the sum of
  orders from every destination at posted prices, shortfalls are rationed by ADR 0020, and the
  excess stays in inventory. The 8.2 rule moves p_o on that gap. Local goods keep their province
  market.
- Buyers pay at dispatch and receive after transit (domestic one tick, crossings 1 + lag ticks):
  arrivals go straight to household consumption or enterprise input stocks (agent default, this
  ADR).
- S02 lives in the GE reference only: its balanced Sinkhorn on the same kernel is the full solve,
  and the 1e-6 fixed-point test compares a Rust run (kappa large, lags zero) with it. No Rust S02
  path in P2 (agent default).
- The European-slice comparison runs on the fixture instead (full globe, ADR 0012).
- Stability: the owner decision of 2026-10-24 (kappa1 0.02, kappa2 0) was measured on destination
  clearing with routed supply arriving late; it is re-derived under origin clearing (ADR 0019).
  Outcome: kappa1 0.05, kappa2 0 (owner, 2026-10-24, MIGRATION Task 6), on the derived region's
  edge kappa2 = 0, which counts as inside (owner ruling, MIGRATION Task 6).
- Budget: the trade line drops from 30 ms to 5 ms per tick on the full pack (19.2 at rev 226).

## Consequences

Trade responds to shocks over months rather than within the month, the intended disequilibrium.
Tariff impulse responses are checked against the timing in Amiti, Redding and Weinstein (2019).
