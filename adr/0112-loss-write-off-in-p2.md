# 0112. Loss write-off in P2

Status: accepted (owner, 2026-10-26; design a controller ruling, PLAN Task 20b)
Date: 2026-10-26
Design sections: 8.6, 8.7, 15.3 (V1); econ SPEC Income, System order, Parameters; ADR 0089

## Context

P2 has no finance. Every account may run a negative balance at an interest-free clearing facility,
and national ownership (owner, 2026-10-25) pooled enterprise and market cash flows per country and
paid them to pops, losses included. V1 from the data state of the calibrated pack (Task 20) passes
through a deep coal-led transient whose losses then had nowhere to go:

- Under the signed rule they reached pops as negative dividends. Pops in Albania, Mali, Namibia,
  Pitcairn and Uganda held debts of thousands of months of income and, holding no money, bought
  nothing (up to 6,331 pops negative): price gap 0.88 at tick 1,200, 0.82 at 4,800.
- Under limited liability for enterprises (owner, 2026-10-26, Task 20 fix round 1) the losses stayed
  with the enterprises, about 5e12 at tick 1,200, mostly coal in Sudan, Namibia, Albania, Mali and
  Uganda. Repaid from steady-state profits at about 1.5e8 a tick, they would take about 33,000
  ticks, while households held the money they owed: world price index 7.1 at 1,200, 5.3 at 4,800.
- Limited liability for province markets made the fixture's run from the data state inflate
  without bound (index 15 at tick 1,200, 129 at 2,400): markets buy their province's output and
  sell what is ordered, so stock building leaves them in loss for good under kappa2 = 0.

## Options

1. Keep losses until repaid (limited liability alone).
2. Write losses off beyond a cap against the owners, the country's pops (a bankruptcy rule).
3. Add credit and interest now (P3+ finance).

## Decision

Option 2 (owner, 2026-10-26). The design is a controller ruling fixed before any gate run:

- **Cap.** Enterprise e keeps retained losses up to T_e = M S_e, S_e its base monthly sales: the
  calibrated steady-state sales value VA0_e / a_VA_e (equal to the ADR 0089 tables' sales). Base
  sales, so the cap does not shrink in a slump. **M = 3 months** (P148), not tuned to results.
  M was registered as P143 until Task 20e; ADR 0090 defines kappa_n as P143, so M moved to P148
  (controller ruling, 2026-10-26), and P147 stays reserved for the held M_F below.
- **Write-off.** After E4's distribution, the excess E_e = max(0, -balance_e - T_e) of every
  enterprise of a country is credited to it and debited from the country's pops in proportion to
  their positive money. No pop is taken below zero. Where the pops hold less than the excess,
  what they hold is written off, shared over the enterprises by their excess, and the rest stays
  with the enterprises for the next tick.
- **Rounding.** One exact i64 split per country per tick on each side, by the largest remainder
  (floors of the exact shares, leftover units one each to the largest remainders, ties to the
  first), moved through the country's profit pool account, which ends at zero. Countries in order,
  so the result does not depend on the thread count. The journal flow is `WRITE_OFF`, outside a
  pop's income.
- **Markets.** The ruling gave markets the same limited liability and write-off, with a cap of M
  months of the market's base sales, unless the write-off did not cure the inflation of round 1.
  It did not: on the fixture's run from the data state the write-off bounds it, but the index
  still reaches 13.2 at tick 1,200 (12 at 1,600, 11.7 at 2,400), and the fixture's V1 and
  stability tests and both steady-state tests fail. So province markets keep the signed rule, as
  the ruling provides: their cash flow, signed, joins the pool.

## Consequences

- At a steady state no balance is negative, so the write-off never fires: the fixture holds to
  1e-9, the pack within its bounds, and the pack's default run is byte-identical.
- The cap is nominal (base sales at base prices), so the model is no longer homogeneous of
  degree zero once a write-off fires: scaling every nominal value moves losses past the cap in
  one run and not the other. The homogeneity property test runs with the write-off off (M
  infinite). Indexing the cap to the price level would restore homogeneity; it is not ruled.
- Market losses still reach pops as negative dividends (signed rule), so pops can still hold
  negative money after a deep transient.
- An enterprise that loses money every month becomes a perpetual levy on its country's pops once
  its cap is full.
- M is excluded from the static digest, with the reason in `static_digest`: it fires only on
  retained losses, which the steady states the calibration and the reference solutions describe
  do not have, so changing it does not invalidate calibrations. The snapshot header carries it
  after the digest and `restore` checks it; `params.json` lists it with P148.
- V1 from the data state of the calibrated pack still misses (Task 20 round 2): at tick 1,200 the
  price gap is 0.83 (Mali services), wage 0.58, flows 1.83, outside the former debtor countries
  3.8e-2; at 4,800 it is 7.9e-2, outside 8.3e-3. The write-off clears the enterprises' losses
  (8.43e12 of 8.63e12 by tick 60), but the real cause is upstream (Task 20c debug report): after
  the coal-led shock output stays at full employment while demand collapses, and merchants buy
  all of it at posted prices 30 to 60 times the reference, so Mali's markets alone lose 4.25e12,
  about 13,000 months of Mali's base sales. Under the signed rule that loss is pop debt, and
  indebted pops buy nothing; no rule for financing the loss (signed, amortized, repaid from
  positive money, local refund) moves Mali's gap from 0.83.
- P3+ finance (credit, interest, bankruptcy with creditors) replaces this rule.

## Second decision, held: merchants buy to order; producers hold inventory and plan output

Owner decision (2026-10-26, Task 20 round 3) to stop the loss at its source; controller ruling:
a market buys min(F + Q, D + max(0, S* - S)) of its producers' finished goods F and output Q;
what it does not buy stays with the producers as finished goods (a local good's perish, as its
market's stock does); each producer plans Q_plan = max(0, sold + F* - F), F* = M_F months of base
output (P147, M_F = 1), capped at capacity, and in the default mode hires only the labour the plan
needs, while frictionless labour still clears at (1 - u*) L^s. Built and tested (branch
`p2-r3-producer-stock`, c986a7c) and not adopted, because it fails its own requirements:

- Frictionless runs hyperinflate. Labour stays at the natural rate and is paid while output and
  sales fall, and local producers, whose finished goods perish, always plan sold + F* above
  capacity and lose what does not sell. Those losses are retained under limited liability, which
  leaves the money with the pops, and the write-off, which runs after the pops have spent their
  wages, finds almost no positive money to levy. The fixture's run from the data state reaches an
  index of 4.5e5 and overflows the ledger at tick 977; on the pack both V1 starts stop on an i64
  overflow of the write-off.
- The pack's default run, from the steady state, is not byte-identical and drifts: rounding noise
  in Burundi's local goods (province 262) grows into a 40% price gap by tick 1,200, a new unstable
  mode.
- The pack's steady-state test misses its boundedness check on the world index (ratio 2.23).

## Tests

`write_off_takes_the_excess_beyond_the_cap_from_the_pops`, `write_off_spares_losses_within_the_cap`,
`write_off_takes_what_the_pops_hold_and_carries_the_rest`, `write_off_runs_are_thread_independent`
(`crates/ceteris-econ/tests/p2_properties.rs`).
