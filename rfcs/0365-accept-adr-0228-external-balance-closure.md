# RFC 0365. Accept ADR 0228 for P5a as the external-balance closure, with the credit limit

Status: accepted, spike no-go for the proxy tested; not built
Date: 2026-11-26
Design sections: 8.2, 8.8, 9.2, 9.9, 12.2, 17; trade SPEC Money-anchored rule; econ SPEC E2, E4; money SPEC 9.9
Release: 1.0, P5a
Research: `reports/trade-structure-decision-2026-11-26.md` (project files); [RFC 0350 bands at the fitted values](../docs/research/rfc0350-bands-fitted.md); [P5 E3 D1/D2 spike and credit-limit follow-up](../docs/research/p5-e3-d1d2-spike-2026-11-10.md); issues #216, #561, #673

## Summary

The d5e2 refit missed RFC 0350 bands, so the calibration rule stopped tuning and put the model
structure to Antonio. The misses fall into two families. Stability: border x10 runs away and
panics the ledger, the E3 base run is not stationary, and x10 survival flips with a 1e-5 change
in BALANCE (#673). All three are the mechanism RFC 0341 traced: a province or country can run
an unbounded external deficit and nothing in the model prices it. Both spikes that bounded it
from inside (stock term plus pay on sales, then a credit limit alone) were no-go, and Antonio's
note on RFC 0341 says the model needs both a credit limit and a balance-of-payments price. The
price was never built. Welfare size (Brexit at 0.21x to 0.28x of Dhingra et al., US-China sign):
ACR-IO and per-class openness pass, so that gap is exposure (no services trade before P8, the
2000 base year) and shock mapping, not the trade core, and this RFC does not address it.

ADR 0228 (Gabaix and Maggiori exchange rates in P5a, still proposed) is the price closure:
each floating currency's rate clears the month's net currency flow against financiers with
limited risk-bearing capacity. This RFC accepts it for P5a together with the credit limit of
the RFC 0341 follow-up, as the one named exception to the kernel freeze, gated by a spike, and
fixes the rescore that follows. Antonio chose this as option A on the structure card.

## Decisions

### D1. How is the external deficit closed after the d5e2 band misses?

Options:

- A. External-balance closure: accept ADR 0228 for P5a as the balance-of-payments price, with
  the credit limit, as the one exception to the kernel freeze; then rescore every band once.
- B. Domestic-leg distance cost (option 3 of the d_0/nu report): new domestic-flow moment and a
  full refit; leaves the price runaway, so x10 and E3 still fail.
- C. Per-class distance scale (#212, RFC 0354): aimed at per-class H2 only; same gaps as B.
- D. Research-preview 1.0: keep the kernel, document every miss, lift the freeze; x10 stays a
  crash and the flagship never passes.

Recommendation: A, because it fixes the shared cause of x10, the E3 base drift and #673, and
the passing ACR-IO and openness guards say the welfare gap is not a trade-core fault. B or C
can follow later; neither replaces A.

Decision: A, Antonio, 2026-11-26 (structure card, 18:07Z).
Row: none new; ADR 0228 keeps its row and reads accepted (RFC 0365) in the index. ADR 0228's
file stays a record and is not rewritten.

What A fixes, read with ADR 0228's text:

- The rule is ADR 0228's: `q_c(t) = q_c(t-1) + F_c(t)` and
  `E[e_c(t+1)] / e_c(t) (1 + i*_t) - (1 + i_c,t) = Gamma_c q_c(t)`, with F_c the month's trade
  balance, income flows, the ADR 0100 transfer and intervention; pegs absorb F_c from reserves
  (ADR 0210); unions share one currency. It lives in `ceteris-finance` and is replaced by the
  9.9 demand system in P5b, kept as a switch for the nesting test (ADR 0228, Migration).
- The credit limit is the one spiked in the RFC 0341 follow-up: each enterprise's traded input
  spending is bounded by its cash plus CL loss caps (ADR 0112), wages unbounded. That spike
  showed CL of at least 1 leaves the ntb 1 steady state exact and CL 0 deadlocks; CL is set by
  the spike below within CL >= 1.
- Both are on together in the E3 price package (RFC 0341 D1, D2; RFC 0342). Neither alone is
  the closure: the credit limit alone closed the leak and the mainland still died (RFC 0341
  follow-up), and a price without a limit leaves an account that may go negative without bound.

### D2. What gate does the kernel pass before it is built?

Spike first (development laws, spike before build), one day, with a go/no-go number:

- Go: on the `small_world` fixture with the E3 package and the closure on, border x10 survives
  1,500 ticks with the world price index at most 1.07 of base throughout, and the ntb 1 run
  holds its steady state.
- No-go: nothing is built; the result is recorded here as a negative result and the structure
  goes back to Antonio. No band, tolerance or go number moves.

Spike result (2026-11-26): no-go for the proxy tested, ADR 0228 in full unrun. Baseline E3
package reaches 1.0712 at 1,500 ticks (gate 1.07); an import wedge on cumulative money change
collapsed the world (index 169 to 13,546) in two attempts. Study:
[docs/research/p5-bop-price-spike-2026-11-26.md](../docs/research/p5-bop-price-spike-2026-11-26.md).
Former text: Gamma_c's value on the fixture, CL within CL >= 1, and the
fixture's currency areas are set by the spike and written here with its number. Anything the
spike needs beyond ADR 0228's text and this RFC goes to Antonio as a card first.

Decision: A as stated, Antonio, 2026-11-26 (structure card: "x10 survives 1,500 ticks,
index <= 1.07").
Row: none yet, taken in the spec PR once the spike passes.

### D3. What follows a go?

- The kernel is the only exception to the kernel freeze (calibration rule, 2026-11-16). Its
  wiring PR carries a switch-on behaviour test (x10 at the gate above) and turns the closure on
  with the E3 package (#561).
- Then every RFC 0350 band is rescored once, unchanged (ADR 0364, ADR 0365). What still misses
  goes to a tier-2 ADR (ADR 0257) naming the unbuilt mechanism and phase (services trade, P8;
  2000 base exposure) with a pinning test and an owner. No open-ended tuning.
- Inside the freeze, before the rescore: an accounting note splitting the Brexit and US-China
  gaps into exposure (services, base year) and shock mapping, with numbers.

Decision: A as stated, Antonio, 2026-11-26 (structure card).
Row: none yet; the rescore and tier-2 ADR take their own rows.

## Spec changes

None until the spike passes. Then, in the spec PR that builds it: money SPEC 9.9 and spec/09
9.2 (P5a rule of ADR 0228 until the demand system), econ SPEC E2 and E4 (credit limit), Section
17 (Gamma_c and CL codes from `next-code P`), rows for D2 and D3.
