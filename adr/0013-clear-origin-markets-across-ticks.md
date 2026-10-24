# 0013. Clear origin markets across ticks

Status: proposed
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
- The 8.2 rule is now the only origin clearing mechanism; ADR 0016 covers its stability.

## Tests

- Small worlds (5 to 20 provinces, 3 goods): with kappa large and transit zero, the fixed point
  matches the S02 full solve to 1e-6.
- Common-random-numbers steady-state comparison of the default and S02 on the European slice.
- Criterion benchmark: the trade system within its 19.2 budget of 5 ms on the full globe.

## Consequences

Trade responds to shocks over months rather than within the month, the intended disequilibrium.
Tariff impulse responses are checked against the timing in Amiti, Redding and Weinstein (2019).
