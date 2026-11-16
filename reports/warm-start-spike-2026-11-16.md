# Warm-start fit spike (RFC 0353 D4 C, ADR 0373, #590)

Verdict: **no-go** at the pre-set bar (at least 3x fewer Brent evaluations). Best case 2.12x, overall 1.66x.

Method: `border::fit_border_nu_seeded` (this PR) seeds each friction group's bracket at +-2% of a
seed nu (floor 1e-3), evaluates both ends together, widens x4 up to 4 times, else falls back to
the cold bracket [0, NU_MAX]. Same objective and tolerance. Harness: `examples/warm_start.rs` on
the full 2000 pack (Nov 4 build), `d_0` at the class means, scale 1, theta classes as built.
Evaluations are `Fit::evaluations` (each one scaling of every good), cold and warm counted the
same way, including the nu 0 evaluation and the final share. Nothing is written to a golden or
`epsilon.json`. Log: `~/ceteris-logs/warm-start-20261116T0308Z.log`.

| case | cold | warm | ratio | max abs dnu |
|---|---|---|---|---|
| eps 0.34, seed = fit at 0.30 | 17 | 8 | 2.12 | 1.3e-10 |
| eps 0.35, seed = fit at 0.34 | 17 | 8 | 2.12 | 1.3e-10 |
| eps 0.351, seed = fit at 0.35 | 17 | 8 | 2.12 | 1.4e-10 |
| eps 0.3505, seed = fit at 0.351 | 17 | 8 | 2.12 | 1.4e-10 |
| eps 0.35025, seed = fit at 0.3505 | 17 | 8 | 2.12 | 1.4e-10 |
| targets +1%, seed = unedited fit | 17 | 10 | 1.70 | 3.2e-11 |
| targets +5% | 17 | 16 | 1.06 | 7.7e-11 |
| targets +10% | 17 | 16 | 1.06 | 1.9e-10 |
| total | 136 | 82 | 1.66 | |

Reading:
- Fixed cost per warm fit is 4 evaluations (nu 0, two bracket ends, final share); the remaining
  Brent rounds then cost about 4, because nu is fitted to NU_TOL = 1e-9 and Brent's last steps are
  linear in log tolerance. The cold fit is only 17, so 3x needs at most 5 warm evaluations, which
  leaves no room for any widening.
- Even the closest seeds (epsilon moved by 2.5e-4) reach 2.12x, not 3x.
- Larger edits widen the bracket and the gain vanishes (1.06x).
- The root stays within 2e-10 of the cold fit, so a warm fit is safe as non-authoritative output
  but the evaluation saving is too small to matter.

Not measured: the whole epsilon search (each trial also refits d_0 and s_c, which this spike
does not touch), and wall time. The saving available to the full loop is bounded by the above
per-fit ratio.

Decision for Antonio: drop D4 C unless a cheaper bracket (skip the nu 0 and final evaluations)
is judged worth a second spike; ADR 0373 status to follow the decision.
