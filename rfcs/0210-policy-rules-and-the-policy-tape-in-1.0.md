# 0210. Policy in 1.0: estimated rules per lever and the policy tape

Status: proposed (owner chose the option, 2026-10-28); amended per ADR 0241, 0242 (2026-10-29); amended by ADR 0239 (seats and scoring to 2.0-alpha)
Date: 2026-10-28
Design sections: 2.4, 8.5, 9.2, 9.4, 9.6, 12.3, 12.6, 15.7, 17, 19.3, 23.4; ADRs 0136, 0162, 0207
Phase: P5a (spec, monetary and fiscal rules, tape loader), P5b (default rule), P9 (estimation and ablation)

## Context

DESIGN 2.4 and ADR 0136 say that in 1.x "policy comes from estimated reaction functions or the
historical policy tape". Neither is designed. Budgets (9.6) are "set by the state AI under
institutional rules (Section 10)" and default (9.4) is "a strategic choice of the state AI", but
the Tier 1a planner of 12.3 weighs coalition power shares that only exist with P6, which is 2.0.
The tape has one bullet in 15.7: no levers, sources or format. Without a policy agent P5a cannot
set taxes and spending, P5b cannot decide a default, the P8 run to 2150 has no policy after the
tape ends, and the P9 hindcast cannot separate mechanism error from policy error. The 2.0-alpha player
score (23.4) is measured against "the AI", which in 1.x is this agent. Audit:
https://claude.ai/artifact/QxYA4tkRQQ4mCx6g8YKg6Q (gap 1).

## Options

1. Estimated rules per lever, fed by the tape to 2024 and running alone after it.
2. The tape to 2024, then every lever frozen at its last value.
3. A lean Tier 1a planner with fixed weights pulled into P5a.

## Decision

Option 1 (owner, 2026-10-28).

**Levers and rules.** One rule per lever, each a short equation with country parameters:

| Lever | Rule | Source of the form |
| --- | --- | --- |
| Policy rate, floating regimes | Taylor rule with smoothing and the effective lower bound | 9.2 |
| Policy rate and reserves, pegs and managed floats | Defend the band from reserves; the peg breaks under the reserve-cover rule of 9.12 | 9.2, 9.12 |
| Primary balance | Bohn-type reaction to lagged debt over GDP and the output gap | Bohn (1998); Mendoza and Ostry (2008) |
| Spending composition and tax rates | 2000 shares held; rates scaled together to deliver the primary balance, within fiscal capacity (9.6) | 9.6 |
| Tariffs, trade wedges, capital controls | Tape to 2024, held at the last value after | 8.5 |
| Sovereign default | Hazard on debt service over revenue, reserves cover and market access; the same hazard prices spreads (Eaton-Gersovitz consistency, 9.4) | 9.4 |
| Regime changes (pegs abandoned, unions joined) | Tape to 2024; after it only the peg-break rule above | 9.2 |

Monetary unions run one rule on union aggregates. Dollarized states have no rate rule.

**Estimation.** Parameters are estimated on 2000 to 2010 country by country where the series
allow, with partial pooling toward income-group priors elsewhere, and enter the register (17) with
IDs assigned in the SPEC. Rules are fitted to the tape, never to simulated outcomes.

**The tape.** A versioned pack table, one row per (state, lever, date, value, source), in the same
file family as the shock tape of 13.1 and loaded by the same reader. Sources to audit and add to
the manifest before use: BIS central bank policy rates, IMF GFS and WEO fiscal series, WITS TRAINS
or WTO tariff profiles, Ilzetzki-Reinhart-Rogoff regimes, Chinn-Ito or Fernandez et al. capital
controls, and a sovereign default database (Bank of Canada and Bank of England, or
Cruces-Trebesch). Non-commercial sources are acceptable (owner, 28 Oct 2026).

**Modes.** A scenario sets policy per state to `tape`, `rules` or `tape-then-rules` (default). A
Steward seat (2.0-alpha, ADR 0239) replaces the rules for its state; a commitment device (ADR 0162) binds a rule's
lever. Each rule's terms show in the explainer (20.2), as 12.3 promises for the planner.

**Code.** The rules live in `ceteris-ai` (19.3) as the 1.x policy interface that Tier 1a later
implements; `specs/policy/SPEC.md` is written and owner-approved before P5a code (21.2, P5 rule).

## Migration

- P5a: SPEC, tape table and loader, monetary and fiscal rules. P5b: default hazard and spreads.
- P9: estimation, and the ablation below in the scorecard.
- DESIGN 2.4, 9.4, 9.6 and 12 gain a sentence pointing to this ADR when it is accepted.

## Tests

- In `tape` mode the hindcast reproduces the observed lever paths exactly.
- Rules fitted on 2000 to 2010 forecast 2011 to 2019 policy rates and primary balances better than
  holding the 2010 value (out-of-sample RMSE).
- The scorecard reports skill under `tape` and under `rules`; the gap is the policy error.
- A 2150 run under `rules` keeps debt over GDP bounded in every state (P8 gate).

## Consequences

1.0 has a policy agent with few parameters that validates on the hindcast and gives Steward a
defensible baseline. Policy after 2024 is mechanical and does not respond to politics; that is
what 1.0 holds fixed (ADR 0136). Tariffs never move after 2024 without a user edit.
