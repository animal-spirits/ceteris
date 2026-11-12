# RFC 0350. Pass bands for the free moments at the P4 and P5 gates

Status: accepted
Date: 2026-11-12
Design sections: 15.3, 15.7, 21.1; ADRs 0200, 0257, 0264, 0268
Release: 1.0 (P4 and P5 gates)
Research: none; follows `reports/economy-assessment-2026-11-12.md` and `docs/free-moment-rerun-results.md`

## Summary

ADR 0257 reruns the literature benchmark at every phase gate and grades misses by tier, but most
free moments have no fixed pass number: H2 distance is a printed diagnostic (ADR 0264), Brexit,
US-China and x10 are only reported. Each fitted wedge so far hit its target while moving a free
moment, so a band chosen after seeing a result would be worth nothing. Antonio chose on
2026-11-12 that the free moments become hard gates at the P4 and P5 gates, with bands fixed now,
before the reruns of the 12 November results are read. This RFC fixes them. No band moves after a
result is seen without a new RFC (development laws, "Right").

Ranges below are the literature values the economy assessment cites; the band is the pass range.
A moment outside its band blocks the gate, unless an accepted ADR traces the miss to a named
unbuilt mechanism and its phase (ADR 0257 tier 2).

## Decisions

### D1. Are the free moments hard gates at the P4 and P5 gates?

Options:

- A. Hard gates with bands fixed now.
- B. Diagnostics only, as ADR 0264 has H2 today.

Recommendation: A.

Decision: A, Antonio, 2026-11-12 (question card, economy assessment thread).
Row: ADR 0364

### D2. Which bands?

Options:

- A. The table below.
- B. The table below with every two-sided ratio band widened to [0.33, 3] of the literature value.
- C. Hard gates only for the moments already tested (ACR-IO, openness, Brexit and Japan signs); the
  rest stay diagnostics until their fix lands.

Recommendation: A, because the bands are the literature ranges or the existing graded bars, not
tuned to the model.

Decision: A, Antonio, 2026-11-12.
Row: ADR 0364

| Moment | Source | Pass band |
|---|---|---|
| H2 distance elasticity (pack, gravity on BACI) | Disdier and Head 2008 range, ADR 0264 | [-1.1, -0.7] |
| Hard Brexit, UK welfare | Dhingra et al. 2017: -2.66% | [-4.0%, -1.33%] (0.5x to 1.5x) |
| Soft Brexit, UK welfare | Dhingra et al. 2017: -1.34% | [-2.0%, -0.67%] (0.5x to 1.5x) |
| Sign of EU27 and Japan welfare under costlier borders | ADR 0200 | negative, both |
| ACR-IO, border x1.1 per province | ADR 0268 | no wrong sign, median [0.8, 1.25], correlation >= 0.9 |
| Openness per class against ICIO 2000 | ADR 0207 | within [1/1.5, 1.5], every class |
| Border x10 | ACR gain formula, ADR 0200 | survives 1,500 ticks, index <= 1.07, welfare gain within 0.5x to 2x of the ACR prediction |
| US-China 2018 pass-through | Amiti, Redding, Weinstein 2019: about 1 | [0.8, 1.2] |
| US-China 2018 welfare | Fajgelbaum et al. 2020: -0.04% of GDP | [-0.1%, -0.02%] |
| PPML theta by class on BACI | #241: class spread 2.9 to 16.9 | max/min across classes >= 3 |
| Short-run trade elasticity (gated once #200 lands) | BLP-N 2023: about 0.8 | [0.5, 1.5], and long-run / short-run >= 1.5 |

### D3. Which gates apply the bands?

Options:

- A. Every band from the P4 gate on, except the short-run elasticity, which applies from the gate
  after #200 lands.
- B. Only the P5a gate and later.

Recommendation: A, because P4 changes population and the wage grid, which move trade results.

Decision: A, Antonio, 2026-11-12.
Row: ADR 0365

## Spec changes

- Section 15.7 (phase-gate benchmark; ADR 0364): the table above replaces "reported" for the listed moments.
- Section 21.1 gate list (ADR 0365): a miss outside a band blocks the gate as tier 1 or tier 2 per ADR 0257.
- `docs/free-moment-rerun.md`: pass numbers refer to this table, not to ADR 0257.
- Test code: assertions of the bands in `benchmark.rs` and `p2_gate.rs` follow in a separate PR.
