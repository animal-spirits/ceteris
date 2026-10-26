# 0106. Price clip on the gap only, and money width

Status: proposed
Date: 2026-10-26
Design sections: 4.5, 4.6, 8.2, 17.1 (P03c); trade and econ SPEC price and wage rules; amends
ADRs 0089 (as-built notes) and 0090

## Context

- **Clip.** The price rule of ADR 0090 and the wage rule of ADR 0089 clip the whole step:
  dlnp = clip(pi^e + relative + aggregate - stock terms, +-c_max) and
  dlnw = clip(pi^e + kappa_w (x + u*), +-c_max), c_max = 0.1 log points per month. The clip
  includes expected inflation, so monthly inflation above about 10.5% is impossible however
  long it is expected, and so are faster falls. Both happen in the data: hyperinflations
  (Zimbabwe 2007-08, and in P5 any monetary collapse) and commodity crashes (oil fell about 30%
  in one month in late 2008).
- **Money width.** The ledger holds money as i64 with 1 minor unit = 1e-4 numeraire. ADR 0089's
  as-built notes report the mint overflowing near a nominal scale of 800 relative to 2000.
  At 5% nominal growth that is about 140 years; with high inflation in P5, within the
  simulation horizon.
- **Uniqueness.** Uniqueness of the Sinkhorn stage is shown per good. With cross-good
  linkages, several fixed points may exist; determinism always selects the same one, which
  hides the question rather than answering it.

## Options

Clip: (a) keep; (b) clip only the real gap terms; (c) remove the clip with an implicit step.
Width: (a) keep i64; (b) i128 for accounts that aggregate; (c) redenomination events.

## Decision

Clip (b); width (b).

- Price rule (ADR 0090):
  dlnp_o = pi^e + clip((kappa_r / E_o)(l_o - lbar) + (kappa_n / h) lbar
                       - (kappa2 / E_o)(S'_o - S*_o)/S*_o, +-c_max).
  Wage rule (ADR 0089): dlnw = pi^e + clip(kappa_w (x_i + u*_i), +-c_max), times (1 - delta_w)
  when the step is negative.
  pi^e itself stays bounded by its own adaptive rule, so the stability region of the trade
  SPEC is unchanged at pi^e = 0. Storable commodities (goods 0, 1, 2, 5 under ADR 0091) take
  c_max = 0.3.
- Ledger accounts for pools, the mint, markets and the world transfer account are i128; pop
  and enterprise accounts stay i64 and are summed into i128. Snapshot codec bumps its version.
- A CI test solves the calibration from 8 perturbed starts (prices +-50%, deterministic seeds)
  and asserts the same fixed point to 1e-9; a failure is a finding, not a flake.

## Migration

- trade and econ: move the clip inside; `clip_ticks` counts the gap clip.
- core ledger: i128 aggregate accounts; codec version bump (ADR 0002 rules).
- python tests: the multi-start uniqueness check joins the P2 gate.

## Tests

- With pi^e = 0, prices and hashes are identical to the current golden.
- A toy market with pi^e fixed at 0.2 per month inflates at 0.2 per month.
- A scale-10,000 run of the calibration completes without overflow.

## Consequences

- Inflation and price collapses are bounded by behaviour, not by a numerical guard.
- Aggregate accounts double in width; their count is small, so memory change is negligible.
