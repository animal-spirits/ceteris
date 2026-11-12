# Approximate solves to speed the tick (2026-11-04)

Question (Antonio): accept computed approximations, at least on a first pass, to unlock the next
tick sooner. Research only; no code. Sources: `tick-speed-brainstorm-2026-11-04.md` (per-system
times, items #19 and #20), `spec/08` 8.2 to 8.3, ADRs 0016, 0093, 0097, and the solver code in
`crates/ceteris-econ/src/{labour,local,demand,systems}.rs`.

## Short answer

The biggest piece of the idea is already the design. ADR 0016 clears origin markets across
ticks: each tick runs one destination normalization at last tick's prices, so price formation is
a Sinkhorn solve spread over time, never iterated within a tick (spec 08, "Clearing across
ticks"; the kernel's per-unit cost also uses last tick's prices). Dispatch (about 24 ms, the
bulk of the tick) is a fixed sequence of kernel products with no convergence loop, so it has no
tolerance to loosen. The iterative solves left in the tick are small (labour 2.8 to 4.8 ms in
total, local chain 1.5 ms, PIGL already 1 round), and loosening them saves at most about 1 ms
while it breaks the calibrated steady state's 1e-12 fixed-point claim.

With the tick at about 32.5 ms against the 35 ms budget, any gain here is headroom, not need.
**Recommendation: adopt none now.** Revisit only option 6 (skip unchanged goods) if the 50-goods
pack pushes dispatch past budget after the exact items of RFC 0287 / ADRs 0294 to 0298.

## Where the tick iterates today

| solve | per tick | method, tolerance | time |
|---|---|---|---|
| origin prices / trade (ADR 0016) | one sweep, lagged prices | none to loosen | dispatch 24.7, consume 6 to 7 (price_refresh 4.4, closed form) |
| frictionless wage, per country (`labour::clearing_wage`) | bracket by doublings from last wage, then safeguarded Newton with a central-difference slope | relative 1e-13 on w; 3 demand sums per iteration | most of labour 2.8 to 4.8 |
| local chain fills (`local::fills`) | bounded passes, cycle jumps | FIT 1e-12 (ADR 0093, bounded-work amendment) | 1.5 (inside produce) |
| PIGL inversion (`demand::invert_pigl_pops`) | fixed point | 1 round on every current caller | negligible |
| income settle (`equilibrium::settle`) | fixed point, 1e-15 | run start only, not per tick | 0 per tick |

Calibration solves (`ceteris-calibrate`) run before a run, not per tick.

## Options

Determinism note for all: a solve whose stopping rule and operation order are fixed, and that
runs per country or per good with no cross-thread reduction, gives the same bits at any thread
count. Money is posted in integer minor units, so the ledger's double entry closes whatever the
prices are. What an approximation can break is the goods and dispatch invariants (1e-9, 1e-12)
and the calibrated fixed point, not the journal's reproducibility.

**1. Looser wage tolerance (1e-13 to 1e-9).** Saves about one Newton step of four to six, so
roughly 0.3 to 1 ms. Error 1e-9 relative on w per tick; the wage map is contracting (the f32
probe found perturbations decay, not grow), so it does not accumulate, but it puts a 1e-9
wobble on every wage. Deterministic. Golden move. Conflicts with `steady.rs` PASS_TOL 1e-13 and
the pack's claim to be a fixed point of the tick to 1e-12: the steady state would only be a
fixed point to 1e-9, so calibration checks need loosening too. Touches ADR 0089/0093 (wage
root), spec 08 labour section. Not worth it. The exact alternative (analytic slope in place of
the central difference, 1 demand sum per step instead of 3) likely saves more and is item-#16
territory, not an approximation.

**2. Warm starts carried in the snapshot (brainstorm #19).** The wage solve is already warm
(it starts from last tick's wage, which is state). PIGL needs 1 round. Local chain fills could
start from last tick's fills, but that makes the fills state (snapshot schema change, ADR 0097
allows warm starts only through the snapshot) to save part of 1.5 ms. Deterministic if saved;
a fork from a snapshot must restore the start point or it diverges. Low value.

**3. Early exit with carried residual.** Stop a solve after k steps and carry the residual into
next tick, as ADR 0016 does for markets. For the wage this is the natural extension of 0016 (a
wage that clears across ticks), but it is a model change: wages would lag labour demand by an
extra tick, which moves the Phillips-curve dynamics of ADRs 0089/0106 that C6/C7 calibrate
against. Residual becomes state (schema). Error bounded by the contraction rate; does not
accumulate if the map contracts, can oscillate if it does not. Saves most of labour (2 to 3 ms).
Needs an RFC and recalibration. Not now.

**4. "First pass unlocks the next tick" (speculative pipelining).** Start tick t+1 on
approximate tick-t values while tick t refines. Either the approximate values are kept (that is
option 3, a model change) or they are thrown away and t+1 recomputed when the refined values
arrive; with floats the speculation never matches bit for bit, so it is pure overhead. What can
genuinely overlap is work that does not depend on the refined values: that is the exact task
graph (brainstorm #18, 3 to 8 ms), which needs no approximation.

**5. Coarse then refine (e.g. fewer gravity terms first, full kernel later).** Dispatch has no
refinement loop; a coarse kernel would be the output, so this reduces to dropping or low-ranking
sea legs (brainstorm #20, class C). Breaks the dispatch-mass invariant unless an exact closing
pass per origin follows (the f32 study found the same). Large gain possible, high risk.

**6. Skip or reuse a good's dispatch when its prices and fills did not move (brainstorm #20).**
Exact when inputs are bit-identical (then it is caching, not approximation); approximate with a
threshold. With threshold eps on relative price change, error per tick is O(eps) on that good's
flows and it does accumulate as long as the good stays frozen, unless the skip is limited to k
ticks or the residual is tracked. Deterministic (per-good decision on state). Gain up to a
good's whole dispatch slice (2 to 12 ms per skipped good) but in a monthly tick prices rarely
sit still, so the hit rate is unknown; measure it before anything else. Touches ADR 0016, 0097,
spec 08.3, the dispatch-mass invariant.

**7. Lower precision in non-state paths.** Settled (f32-recheck 2026-10-29): 6% on the crossing
pass, closing pass needed for conservation. Not reopened.

## Summary

| option | ms saved | error | accumulates | determinism | spec/ADR |
|---|---|---|---|---|---|
| 1 looser wage tol | 0.3 to 1 | 1e-9 rel on w | no (contracting) | kept; golden move | 0089, 0093, calibration PASS_TOL |
| 2 snapshot warm starts | < 1 | none | no | kept if saved in snapshot | 0097, snapshot schema |
| 3 early exit + residual | 2 to 3 | model lag | no if contracting | kept; residual is state | new RFC, 0016-like, 0089/0106 |
| 4 speculative next tick | 0 or negative | n/a | n/a | n/a | none |
| 5 coarse dispatch | large | invariant breach | yes without close | kept | 0016, invariants |
| 6 skip unchanged goods | 0 to 12 | O(eps) per frozen tick | yes unless capped | kept | 0016, 0097, 08.3 |
| 7 f32 | settled | | | | f32-recheck |

Recommendation: none now (gain is headroom at 32.5 / 35 ms). If 50 goods breaks the budget after
the exact speed items, first measure option 6's exact-match hit rate (zero error), then consider
option 3 for wages through an RFC.
