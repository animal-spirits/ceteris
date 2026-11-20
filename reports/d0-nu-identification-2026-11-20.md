# d_0 and nu identification: options

Date: 2026-11-20. Status: analysis for Antonio's decision if the d5d refit misses the RFC 0350
bands. No code, kernel or test changes. Context: RFC 0355 (draft PR #600, "Known gap"), ADR 0272
Decisions 1 and 2, `crates/ceteris-calibrate/src/epsilon.rs` (`trial`, `class_offsets`,
`product_misses`).

## Finding: the gap is a gauge, not a missing moment

ADR 0272 D1 puts one crossing factor on every crossing of class c:
`ln F = ln(1 + nu) + b (ln d - ln d_0)`, with `b = s_c epsilon / theta_c`. Rewrite it at the class's
smallest linked distance `d_min`:

```
ln F = a + b (ln d - ln d_min),   a = ln(1 + nu) - b (ln d_0 - ln d_min)
```

- Only `a` and `b` reach the equilibrium. nu and d_0 enter through `a` alone, so for any class, not
  only a single-good one, they are never separately identified. Today d_0 is pinned by a rule (the
  trade-weighted mean), not by data.
- ADR 0272 D2's product rule `(1 + nu) D >= 1` on every linked pair is exactly `a >= 0`. At the mean
  d_0, ADR 0092's `nu >= 0` is `a >= -b ln(d_0 / d_min)`, looser, so the product rule binds first.
- The fixture's two solutions (floored d_0 with nu 0; mean d_0 with nu 0.024) are the same point
  `a = 0`: equal fits imply `ln 1.024 = 0.0237 = b ln(d_mean / d_min)` in the fixture (inferred, not
  measured). The trial's floor rule only
  picks which label that point carries.
- The shocks the bands use act on the total crossing factor: `border` and `ntb` multiply the
  crossing's route tau (`shock.rs`), tariffs multiply separately. None reads nu alone.

Consequence for the refit: two representations of one point give the same equilibrium, the same
shares, the same H2 coefficient and the same Brexit, ACR, border x10 and US-China results. **The gap
cannot by itself cause a band miss.** It does matter for: the reported nu and d_0, the ledger's
split of border loss from `distance_loss`, the meaning of a user's `border_nu` edit (`op = mul` on a
nu of 0 does nothing), and test truths written in (nu, d_0) instead of (a, b).

If d5d misses, the structural constraint to examine is `a >= 0` with one slope per class: a class
pinned there (nu 0 after the floor, s_c below 1) is one where the data want less crossing cost at
near pairs than a frictionless nearest pair allows. That points at option 3 or at theta, not at d_0.

## Options, ranked

### 1. Reparametrise: d_0 at the smallest linked distance for every class (recommended)

- Assumes: d_0 is a normalization (ADR 0272 D2 already says D "redistributes" cost). nu becomes the
  whole level above a frictionless nearest pair: `nu = exp(a) - 1 >= 0`.
- Identification: exact. The product rule and ADR 0092's clamp become one constraint; the floor
  branch and `product_misses` go; `is_pinned` (nu <= 1e-6) tests the constraint that actually binds.
- Bands: none at the same fit point (same feasible set of (a, b), same equilibria). Reported nu
  values fall in every class and more classes read as pinned, which is the truth today hidden by the
  mean.
- Cost: an RFC amending ADR 0272 D2 (deviation from an accepted ADR); an estimated 30 lines in
  `epsilon.rs` (offsets return `d_min`, the floor loop deleted); one test truth rewritten in (a, b).
  Fixture goldens may move at rounding level (same point, different arithmetic), a reviewed event.
  Fits after the freeze lifts or as part of the RFC 0355 spec PR.
- Risk: low. d_min is set by one pair, but it is a gauge, so its sensitivity only moves the labels.
  A border drawn after 2000 (ADR 0253) closer than d_min gets D below 1, as floored classes already
  can today; the rule should say whether `a >= 0` holds at run time or only on the 2000 pairs.

### 2. Keep the mean, make the label canonical

- Assumes: the current form; the trial maps a fitted `a` to (nu, d_0) by a stated rule (mean if the
  implied nu clears the product bound, else floor), independent of the order the fit reaches it.
- Identification: by convention, as today, but deterministic and documented in RFC 0355.
- Bands: none.
- Cost: smallest. A paragraph in RFC 0355 and test truths compared in (a, b).
- Risk: d_0 jumps between mean and minimum across classes and epsilon trials, so nu is not
  comparable across classes or runs; two code paths remain. A patch, not a fix.

### 3. Add a moment: distance cost on domestic legs, nu as the pure border effect

- Assumes: the dark distance cost applies to all trade, not only crossings (a kernel and DESIGN
  change). Then nu is identified by trade inside versus across borders at equal distance
  (McCallum 1995; Anderson and van Wincoop 2003; Hillberry and Hummels 2008 on within-country
  distance), and `a >= 0` no longer forces near-border pairs frictionless.
- Identification: nu and the distance level both from data.
- Bands: every band moves and needs a full refit; bands stay as fixed (RFC 0350).
- Cost: high. Kernel change (frozen until a flagship passes), province-level domestic flow data
  (US CFS, or ICIO domestic shares as a weaker moment), a new calibration target.
- Risk: medium to high on schedule; the most correct structure if option 1 shows classes pinned at
  `a = 0` that still miss their shares. Candidate for after 1.0, or the model-structure decision if
  d5d misses for that reason.

### 4. Fix one parameter from outside data, or pool classes (not recommended)

- Fixing nu from a published border-effect estimate and fitting d_0 (or the reverse) attaches an
  outside number to a split the model cannot see: `a` is still all that is fitted, so the number
  constrains nothing and its label is misleading.
- Pooling class 3 with fuels undoes RFC 0355 D1 (good 2 then misses its ICIO share again) and does
  not remove the gauge, which every class has.

## Recommendation

Option 1, in the RFC 0355 spec PR (it amends ADR 0272 D2), whatever d5d returns: it removes the gap
at its cause with no band effect. If d5d misses, do not tune d_0 or nu; check whether the missing
classes sit at `a = 0` with s_c below 1, and if so put option 3 to Antonio as the model-structure
decision.
