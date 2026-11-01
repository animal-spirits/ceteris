# 0100. Trade imbalances as exogenous transfers until finance

Status: proposed
Date: 2026-10-26
Design sections: 3.3, 8.3, 9.5, 15.4; econ SPEC Calibration; amends ADRs 0014, 0089

## Context

P2 has balanced trade (0014). ADR 0089 reconciles 2000 world supply and spending of each good by
rescaling each good's origin values with one scalar r_g. The imbalance is a financial fact (the
US deficit was about 4% of GDP in 2000; China, Japan, Germany and oil exporters ran surpluses),
but r_g assigns it to goods. Every bilateral flow of a good with a large r_g is biased, and
2000-2008, the core of the calibration window, is defined by widening imbalances that the
model cannot carry.

## Options

1. Keep r_g until P5.
2. Exogenous nominal transfers D_c with sum zero, held at 2000 shares of world GDP (Dekle, Eaton
   and Kortum 2007).
3. Option 2 with D_c following observed current accounts through 2000-2010 as an exogenous
   path (lab profile only), fading to zero in projections.
4. Endogenous current accounts (P5 finance).

## Decision

Options 2 and 3. Option 4 replaces them in P5.

- Each country receives D_c each tick, in numeraire, from a world transfer account: D_c is the
  country's 2000 current-account deficit as a share of world GDP, times world nominal GDP of the
  previous tick. Transfers sum to zero by construction (the residual goes to the largest
  economy and is reported).
- D_c is paid to pops in proportion to income and is spent through all three final demanders
  (ADR 0099), so absorption exceeds income by D_c.
- r_g is removed. 0089 stage 1 solves with absorption = income + D_c.
- Switch (new entry in 17.2): transfers off (balanced), fixed shares (default), or observed path
  (WDI BN.CAB.XOKA.CD, 2000-2024, lab profile; projections fade with a 10-year half-life).

## Migration

- Pack: `econ_countries.arrow` gains `ca_2000` and the CA path.
- Ledger: one world transfer account; it nets to zero every tick, asserted with the other
  accounting invariants (4.6).
- Remove `r_g` from calibration outputs; report the residual of the old r_g at the solution for
  comparison.

## Tests

- Sum of transfers is zero every tick (integer ledger, exact).
- t = 0 bilateral flows match BACI 2000 at least as well as with r_g (PPML residual).
- With transfers off, results equal a balanced-trade run.

## Consequences

- US and surplus-country absorption and sector composition match 2000 data.
- The money and trade-balance loop of the price rule sees a persistent imbalance; the
  stability region must be rechecked with transfers on.
- The observed path is an exogenous input, so holdout scores in that mode are conditional
  forecasts (ADR 0052, conditional-forecast mode) and must be labelled as such.
