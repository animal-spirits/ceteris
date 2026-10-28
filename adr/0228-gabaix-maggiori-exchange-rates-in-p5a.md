# 0228. Gabaix-Maggiori exchange rates in P5a

Status: proposed (owner chose the option, 2026-10-28)
Date: 2026-10-28
Design sections: 8.8, 9.2, 9.9, 9.12, 21.1; ADRs 0100, 0210, 0227
Phase: P5a; nesting test in P5b

## Context

The P5a gate asks a policy-rate shock to move the exchange rate, but floating rates clear only in
the global demand system of 9.9, which is built in P5b. DESIGN 9.2 already names Gabaix and
Maggiori (2015) as that system's special case with one class of financiers.

## Options

1. Gabaix-Maggiori: financiers with limited risk-bearing capacity absorb net currency flows.
2. Uncovered interest parity with a debt-elastic premium (Schmitt-Grohe and Uribe 2003).
3. Build the demand system in P5a.

## Decision

Option 1 (owner, 2026-10-28).

Each month, for each floating currency c against the numeraire:

```
q_c(t)   = q_c(t-1) + F_c(t)                       net position financiers must hold
E[e_c(t+1)] / e_c(t) * (1 + i*_t) - (1 + i_c,t) = Gamma_c * q_c(t)
```

- F_c is the net currency demand of the month: trade balance, income flows, the ADR 0100
  transfer (booked as a capital inflow while it exists) and central-bank intervention.
- Gamma_c is the inverse risk-bearing capacity, one pooled register entry with a currency-size
  scaling, its prior from the Gabaix-Maggiori calibration and calibrated to FX volatility.
- Expectations of e use the heuristic rules of 12.2.
- Pegs and bands: the central bank absorbs F_c from reserves at the parity (ADR 0210 rules);
  unions have one currency; dollarized states have none.

## Migration

- P5a: the rule in `ceteris-finance`, with the policy rules of 0210.
- P5b: replaced by the demand system; the rule stays as a switch for the nesting test.

## Tests

- With Gamma_c = 0, UIP holds exactly.
- A policy-rate rise appreciates the currency on impact, then depreciates it along the rate
  differential.
- P5b nesting: with one financier class and all other classes inelastic in currency, the demand
  system of 9.9 reproduces this rule within tolerance.

## Consequences

P5a passes its exchange-rate gate without pulling the demand system forward, and P5b gets a
nesting test for free. Register: one entry (Gamma).
