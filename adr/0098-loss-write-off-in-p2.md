# 0098. Loss write-off in P2

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
  sales, so the cap does not shrink in a slump. **M = 3 months** (P143), not tuned to results.
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
  stability tests and both steady-state tests fail (Consequences). So province markets keep the
  signed rule, as the ruling provides: their cash flow, signed, joins the pool.

## Consequences

- At a steady state no balance is negative, so the write-off never fires: the fixture holds to
  1e-9, the pack within its bounds, and the pack's default run is byte-identical.
- The cap is nominal (base sales at base prices), so the model is no longer homogeneous of
  degree zero once a write-off fires: scaling every nominal value moves losses past the cap in
  one run and not the other. The homogeneity property test runs with the write-off off (M
  infinite). Indexing the cap to the price level would restore homogeneity; it is not ruled.
- Market losses still reach pops as negative dividends (signed rule), so pops can still hold
  negative money after a deep transient.
- M is excluded from the static digest, with the reason in `static_digest`: it fires only on
  retained losses, which the steady states the calibration and the reference solutions describe
  do not have. Changing it does not invalidate calibrations.
- P3+ finance (credit, interest, bankruptcy with creditors) replaces this rule.

## Tests

`write_off_takes_the_excess_beyond_the_cap_from_the_pops`, `write_off_spares_losses_within_the_cap`,
`write_off_takes_what_the_pops_hold_and_carries_the_rest`, `write_off_runs_are_thread_independent`
(`crates/ceteris-econ/tests/p2_properties.rs`).
