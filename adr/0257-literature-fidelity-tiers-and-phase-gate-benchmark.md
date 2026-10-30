# 0257. Literature-fidelity tiers and a benchmark rerun at every phase gate

Status: accepted (owner, 2026-10-30)
Date: 2026-10-30
Design sections: 3.4, 15.3, 15.7, 17, 21.1, 21.2; ADRs 0022, 0200, 0207, 0247

## Context

The econ fidelity audit of 2026-10-30 (https://claude.ai/artifact/5cw16uEFECEswpiYYn2vt3) found
the P2 structure faithful to its sources (structural gravity, exact-inversion calibration, PIGL,
Pichler rationing, ACR after ADR 0200) but its fit to the literature's numbers drifting: H2
distance went from -0.753 at the P2 gate to -0.463 on pre-P3 B, only one of four theta classes
passes H2 per class, and the P04 mode-cost fit has never converged. Of 41 alterations since the
first DESIGN snapshot, 20 are patches and 2 changed a test instead of passing it (V1's start,
ADR 0247 criterion 3). No rule says when a literature miss is a bug and when an alternative is
acceptable, and the literature benchmark of 2026-10-28 runs only when someone asks for it.

The owner chose (2026-10-30) to run a comprehensive literature-fidelity pass at the pre-P3 gate,
inside pre-P3 task C7 (the benchmark rerun), and asked for a standing rerun at later gates, since
P4, P5 and P8 change some results.

## Options

1. One pass at the pre-P3 gate and one in P9.
2. Option 1, plus a benchmark rerun at every phase gate, judged by fixed tiers.
3. Continuous: the benchmark in CI on every econ PR. Rejected: it needs the full pack, which CI
   does not have, and Actions minutes are limited.

## Decision

Option 2 (owner, 2026-10-30).

**Tiers.** Every gap between the model and the literature is judged by one of four tiers.

1. **Equilibrium and welfare.** The frictionless limit, comparative statics and welfare formulas
   (ACR, hat algebra) must match the source model. A miss is a bug, never an accepted
   alternative (DESIGN 3.4).
2. **Measured moments over borrowed parameters.** Where a literature parameter makes the model
   miss a literature moment (distance elasticity, border effect, openness, H2 per class), the
   moment wins and the parameter moves, recorded in the register (DESIGN 17). The exception is a
   miss traced to a mechanism not yet built: it is recorded with the phase that builds it and
   stays until then.
3. **Dynamics without a benchmark.** Rules that have no literature counterpart beyond their
   frictionless limit (the price rule, the local fill, money stand-ins) may use the project's own
   methods. They are judged by stability, by nesting in the limit, and by data moments (price
   volatility, pass-through, adjustment half-lives), not by fidelity to a paper.
4. **Patches.** A guard, clamp, cap or residual is acceptable only if it leaves the equilibrium
   unchanged, is registered (as a parameter or in the ADR that adds it), and names the phase that
   removes or replaces it. A change to a test or gate instead of passing it needs its own ADR
   stating what the test no longer checks.

**Pre-P3 gate (C7).** The comprehensive pass reruns the 2026-10-28 benchmark on the B8 pack and
fixes tier 1 and tier 2 misses in the real trade and production equilibrium: H2 distance, H2 per
class, P04, openness, and the ACR and shock cases. Tier 3 and 4 items are graded and listed, not
fixed. Results update the audit.

**Every later phase gate** (P4, P5a, P5b, P8, P9) reruns the same benchmark on that phase's pack
and reports, per case, the change since the previous gate and its tier. A tier 1 miss blocks the
gate. A tier 2 miss blocks it unless an ADR traces it to an unbuilt mechanism. Tier 3 and 4
changes are reported. P9 adds the second comprehensive pass with estimation.

**Harness.** Pre-P3 task B2b moves the benchmark harness (`shock.rs`, branch
`claude/project-thread-cy0p06`) into `ceteris_calibrate` as a benchmark module. C7 and every
later gate run that module. It runs owner-side on the full pack, like the P2 gate.

## Consequences

- C7 grows by the tier 1 and 2 fixes; their size is unknown until the B8 pack is measured. The
  re-rate at the pre-P3 gate (ADR 0248) takes it in.
- Each later gate adds one full-pack benchmark run and a short report.
- ADR 0247's criterion 3 and the V1 redefinition of 2026-10-26 are the two existing tier 4 test
  changes; C7 records what each no longer checks.
- Results that P5 changes (nominal dynamics, trade imbalances once deficits are financed, the
  consumption rule) are tier 3 or have a named phase, so C7 does not chase them.
- No register entry.
