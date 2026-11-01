# 0171. State-owned enterprise behaviour

Status: proposed
Date: 2026-10-28
Design sections: 7.2, 7.3, 9.6, 10.4, 15.4; ADR 0105
Phase: P5

## Context

Enterprises carry an ownership type, and "state" is one value, but a state enterprise behaves as
a private one: it maximizes expected profit, exits when insolvent and borrows at its risk
premium. State firms produce about a quarter to a third of Chinese industrial output and dominate
energy and banking in Russia, Saudi Arabia, India and Vietnam. China's 2009 stimulus ran largely
through state-owned enterprises and local government vehicles funded by state banks, and capital
misallocation between state and private firms is a large part of Chinese TFP dispersion (Hsieh
and Klenow 2009; Song, Storesletten and Zilibotti 2011). The 2009-2015 Chinese investment and
credit paths are 15.4 targets.
Gap audit (2026-10-28): https://claude.ai/artifact/HRDQVbjJz8MrcDuPphPyYs

## Options

1. Status quo: ownership type is a label.
2. Scenario wedges on state firms per country.
3. A behavioural rule for state firms: an objective that weights employment and output, a soft
   budget constraint and a fiscal channel.

## Decision

Option 3.

- Objective: state firms maximize profit plus a weight on employment and output, set per country
  from the gap between state and private labour productivity in 2000 (register entry, one
  weight per region class).
- Soft budget: state firms do not exit on insolvency; state banks roll over their loans at the
  policy rate plus a fixed spread (Kornai 1986). Losses accumulate as contingent liabilities of
  the state and enter the fiscal account when written off (9.6).
- Fiscal channel: a state investment lever directs state firms' investment, which is how
  China 2009 is replayed.
- Privatization and nationalization (10.4) change the ownership type, so the rule moves with it.

## Migration

- P5: objective weight and soft-budget flag in the enterprise step; contingent liabilities
  in the state balance sheet.
- Pack: state ownership shares by country and sector (Orbis and OECD SOE data) and the 2000
  productivity gaps.

## Tests

- With the weight at zero and the soft budget off, results equal the status quo.
- State firms' capital-to-labour ratio rises above private firms' in a toy credit expansion.
- The 2009-2012 Chinese investment share of GDP within the 15.4 band when the stimulus lever
  is fed.

## Consequences

China, Russia and the Gulf states gain their main allocation distortion; misallocation and
hidden fiscal costs become outputs. Register: 2-3 entries.
