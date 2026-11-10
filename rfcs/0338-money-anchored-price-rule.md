# RFC 0338. Money-anchored split price rule (P5 Branch E1)

Status: proposed
Date: 2026-11-10
Design sections: 8.2, 12.2, 17 (P03a, P03b, P143, P177); trade SPEC Origin market, Stability region; econ SPEC Equations
Release: 1.0, P5a
Research: [P5 price-anchor spike, 2026-11-10](../docs/research/p5-price-anchor-spike-2026-11-10.md)

## Summary

ADR 0090 splits the price rule into a relative term and a world aggregate term (kappa_n / h) lbar.
Its trial (2026-10-26) and pre-P3 B6 (2026-10-30) showed the aggregate term turns a local real
shortage into world inflation, and ADR 0090 itself says P5 money can replace it. This RFC
replaces the aggregate term with a money anchor and adds the good-level term that ADR 0090's
variant C lacked, which is a design change to an accepted ADR (RFC 0337).

The spike on the fixture: no inflation from the data state (max index 1.018 against 1.065 as
built), the 5% nominal shock back within 1% in 9 to 17 ticks (77 as built; ADR 0090's test asks
under 30), and relative prices converging (V1 3.6e-3 at 1,200 ticks) only with the good-level
term (0.30 without it). The x10 border shock still fails: the anchor stops the inflation but
output still collapses and prices deflate to the bound. That is the price step at Q = 0 of
RFC 0286 (E2), not the anchor.

## Decisions

### D1. Which aggregate term does the E1 price rule use?

Options:

- A. Money anchor plus good-level term: `- kappa_m (ln(I/I0) - ln(M/M0)) + kappa_b (lbar_g - lbar)`,
  no pi^e on the level target; build E1 after a pack data-state run on the Mac.
- B. ADR 0090 as written, `(kappa_n / h) lbar`: inflates from the data state on fixture and pack.
- C. Money anchor alone (ADR 0090 variant C): nominal mode passes, relative prices freeze 30% off.

Recommendation: A, because it is the only variant that passes both the nominal test and the
data state, and B and C fail on record.

Decision: A, Antonio, 2026-11-10.
Row: none yet.

### D2. What happens to the x10 border-shock criterion of ADR 0090?

Options:

- A. Keep it as a P5a gate criterion and carry it on E2 (the Q = 0 step, RFC 0286): E1 builds
  now, the gate waits on E2.
- B. Make x10 reported only at the P5a gate and move the assertion to the storage-on gate of
  ADR 0091.

Recommendation: A, because the criterion was fixed in advance (development laws) and the spike
shows the anchor is not what fails it.

Decision: pending.
Row: none yet.

## Spec changes

- trade SPEC, Origin market: proposed money-anchored rule (added as a draft subsection marked
  "proposed, RFC 0338; not built"); rows and codes taken on acceptance (kappa_m redefines P177
  kappa_n; kappa_b takes a new P code, one of the 60 free behavioural parameters, not finance).
- trade SPEC, Stability region: re-derive J2 for the good-level mode before the build (open).
- econ SPEC, Equations: pointer to the trade SPEC rule; wage rule unchanged.

## Before the build

- Mac run (owner-side pack): the spike harness on the pack data state, frictionless and default,
  1,200 ticks. Go if the index stays within 1.25 of I0 and V1 at 1,200 is no worse than as built.
- Default-mode convergence: the good-level mode is still moving at 1,200 ticks (2.5e-4 to 7.5e-4
  per tick). Derive kappa_b from the across-goods elasticity in the Stability region and measure
  it before the build; a gain chosen after seeing results needs this RFC amended.
