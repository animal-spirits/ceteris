# RFC 0354. Per-class epsilon scale for classes whose border friction clamps at zero

Status: draft
Date: 2026-11-14
Design sections: 5.4, 15.3, 17.1; ADRs 0092, 0264, 0272
Release: 1.0 (P4)
Research: none; follows the class 3 calibration runs of 2026-11-13 and 2026-11-14 (issue #559)

## Summary

ADR 0272 fits one epsilon and a per-class border friction nu_c to ICIO's cross-border shares.
On the pack, class 3 (theta 16.91) has no room: with epsilon 0.3723 its distance cost alone
already exceeds the cost its ICIO share allows, so nu_3 clamps to 0 and d_0 sits at its smallest
linked distance floor (ADR 0272, Decision 2). `calibrate --pack` then fails with epsilon on:
the outer passes limit-cycle (69 of 73 pass lines name a class 3 price), and five variants did
not settle in 300 passes: pass-relax 0.5, no Anderson, class 3 without distance cost (change
plateaued at 1e-10), and #593's per-good Sinkhorn with outer restart (SVD failure at pass 66).
Moving d_0 cannot help: (1 + nu) D >= 1 holds only at the floor while nu stays 0.

This RFC lets a class whose nu_c clamps at 0 take its own scale s_c in (0, 1] on epsilon, so
that class's cross-border share is met with nu_c = 0 and no cost is applied in excess of the
target. Other classes keep s_c = 1. No pass tolerance changes.

## Decisions

### D1. How is the class 3 infeasibility resolved?

Options:

- A. Per-class epsilon scale, fitted only for classes whose nu_c clamps at 0.
- B. Accept the infeasible target for that class (relax its ICIO share to an undershoot).
- C. Epsilon off for 1.0.

Recommendation: A.

Decision: A, Antonio, 2026-11-14 (question card, class 3 calibration thread).
Row: <ADR NNNN, filled in once accepted>

### D2. How is s_c fitted?

Options:

- A. Joint with epsilon and nu_c: for a floored class, nu_c = 0 and s_c is the root in (0, 1]
  of the class's ICIO share residual, found by Brent's method inside each epsilon trial.
- B. Fixed at the ratio of the class's allowed cost to its unscaled cost, from one trial.

Recommendation: A, because it uses only the targets ADR 0272 already fixes and the same solver.

Decision: pending Antonio.
Row: <ADR NNNN, filled in once accepted>

## Spec changes

Written when accepted, in the PR that takes the numbers:

- 5.4 and 17.1: ln D for a crossing of class c becomes (s_c epsilon / theta_c)(ln d - ln d_0,c);
  new calibrated register entry s_c (one per class, 1 unless floored).
- ADR 0272 Decision 2: a floored class takes s_c < 1 instead of the d_0 floor alone; D >= 1 still
  holds at d_0.
- The H2 held-out check: a class with s_c < 1 is reported with its coefficient and exempt from the
  theta-invariance gate, named in the RFC 0350 results as a known miss traced to this RFC.
- Calibration tables: `econ_calib_distance_class` gains s_c; the writer and reader, and
  `epsilon.json`, carry it. Goldens move at the per-class d_0 re-bless of PR 3, not before.

## Effects

- Goldens: unchanged with epsilon off. With epsilon on, the PR 3 re-bless carries s_c.
- RFC 0350 bands: H2 and the other free moments are measured after the fit; class 3 held-out
  coefficient will not match the data's -0.8, by design, and is reported.
- Cost: one more fitted scalar per floored class, one extra root find per epsilon trial.

## Open

- Whether s_c is also needed for classes other than 3 on the full 50-good pack.
- Whether the fit needs the soft boundary at nu = 0 for the outer solve, to be tested after s_c is
  in: the root of the share residual makes nu_c = 0 a stationary point rather than a clamp.
