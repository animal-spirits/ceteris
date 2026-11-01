# 0162. Commitment devices

Status: proposed; amended by ADR 0239 (seats and scoring to 2.0-alpha)
Date: 2026-10-28
Design sections: 9.2, 9.6, 9.13, 12.6, 14.2, 17; ADRs 0083, 0123
Phase: P5 (monetary and fiscal), 2.0-alpha (as a Steward verb, ADR 0239)

## Context

Frostpunk's laws cannot be repealed and Stellaris locks a policy for years after a change, which
gives choices weight. The real counterpart is the trade between flexibility and credibility
(Kydland and Prescott 1977): fiscal rules, currency boards, central-bank independence, WTO tariff
bindings. ADR 0083 lets announcements move expectations before implementation, but nothing lets a
government bind its future levers, and the expectations of 9.13 do not depend on whether a rule is
binding. Survey of other games' mechanics (2026-10-28): https://claude.ai/artifact/LcDMgZuczpSefSVeadiZWT

## Options

1. No binding; every lever stays free every tick.
2. Binding as a lever with an exit cost and an expectations payoff.

## Decision

Option 2.

- A commitment is an edit (new type in 14.2) that binds a lever: a fiscal rule (deficit or debt
  ceiling), a currency board (0123), central-bank independence, a tariff binding. It removes
  levers from the action space or bounds them for a stated term.
- Exit is allowed at a cost: a jump in risk premia and a fall in the weight expectations put on
  later announcements, decaying over time.
- While a commitment holds, expectations in 9.13 put more weight on the announced rule.
- The payoff weight and exit penalty are new section 17 entries, estimated from evidence on fiscal
  rules and spreads (Iara and Wolff 2014) and on central-bank independence and inflation
  expectations. Until they pass ablation (15.7) they live in the X-register (23.1).
- The AI planner can use commitments too (12.6).

## Tests

- With the payoff weight and exit penalty at zero, a committed branch matches an uncommitted
  branch that follows the same lever path, bit for bit.
- Breaking a commitment raises the issuer's spread in the tick it is broken.

## Consequences

Choices can bind, as real ones do, and the credibility literature becomes playable. Adds two to
four free parameters. Cost about 4-6 part-time weeks after P5.
