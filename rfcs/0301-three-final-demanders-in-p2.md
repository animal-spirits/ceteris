# RFC 0301. Three final demanders in P2: who buys, inventory change, shares, balance, composition and the accelerator (accepts and amends ADR 0099)

Status: accepted
Date: 2026-11-03
Design sections: 7.4, 8.6, 9.6, 17.2; econ SPEC (Pack additions, Final demand, Calibration, Parameters)
Research: none separate; the reading of ADR 0099 against the code at the C2 head (b7d824c).

## Summary

ADR 0099 (proposed) splits the one final demander of each pop into households (PIGL on HFCE),
government (GGFC plus NPISH, a fixed share) and investment (GFCF, a fixed share, with an optional
accelerator), and amends ADR 0089's stage 4 so that PIGL is inverted on household demand only.
Building it (pre-P3 Task C3) leaves six choices open. ADR 0099 puts the government and
investment demanders in country accounts, but buyers in the core sit in provinces (D1). It says
inventory change "stays dropped, as now", but the pack's shares include it (D2). It writes g_c
and s_c as shares of GDP, while the transfers of ADR 0100 make absorption differ from income
(D3). RFC 0308 D1 balances each country's final demand against world sales, and ADR 0099 holds
government and investment fixed in those targets (D4). "Fixed composition" can be fixed nominal
shares or a fixed basket (D5). The accelerator's smoothed income has no rule (D6). This RFC
accepts ADR 0099 with these answers (ADRs 0301 to 0306).

## Decisions

### D1. Who buys government and investment goods?

Found: every buyer of the core is a pop or an enterprise in a province (pipeline, rationing
classes, delivery). Construction and public services are local goods, bought only in the
province. A proportional tax at the country's rate, spent in full the next tick in each province
in proportion to what was collected there, buys exactly what each pop would buy if it spent the
same shares of its own money at the same compositions.

Options:

- A. Pops spend the three shares: each pop spends (1 - g_c - s_c) of its money by PIGL and g_c,
  s_c at the government and investment compositions at its province's prices; no new accounts
  or buyers until 9.6 and 7.4 bring the fiscal and capital blocks. Spending is tallied by
  demander.
- B. Country accounts as ADR 0099 writes: one fiscal and one investment account per country tax
  pop income at E4 and buy in each province by the tax collected there, as new buyers in the
  pipeline, the rationing classes and the snapshots.

Recommendation: A, because the goods flows are the same in P2 and the accounts are the hooks of
phases that replace these rules anyway.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0301

### D2. What happens to inventory change (ICIO INVNT)?

Found: `read::icio::FINAL_DEMAND` sums HFCE, NPISH, GGFC, GFCF and INVNT into the shares, and a
negative share stops the build. INVNT is negative for some goods of some economies.

Options:

- A. Spread each good's INVNT over the three demanders in proportion to their purchases of the
  good: final demand by good stays as today, and no entry turns negative.
- B. Add INVNT to investment (gross capital formation), clipping negative goods to 0 with a flag.
- C. Drop INVNT and rescale the three columns to final demand.

Recommendation: A, because final demand by good is unchanged, which ADR 0099's t = 0 test
checks, and nothing can go negative.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0302

### D3. Are g_c and s_c shares of absorption or of GDP?

Options:

- A. Shares of final demand (absorption, income + D_c): h_c, g_c, s_c from the three ICIO
  columns, summing to 1; every pop spends its money (income and transfer) at these shares, so
  the transfer D_c is spent by the three demanders, as RFC 0308 D3 foresaw.
- B. Shares of GDP as ADR 0099 writes; households spend 1 - g_c - s_c of income and all of D_c.

Recommendation: A, because it is the data's own split of final demand and needs no
household-only rule for the transfer.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0303

### D4. Which final demand does the balance of RFC 0308 D1 (ADR 0318) rescale?

Found: on the installed pack the balance's column factors run from 0.48 to 3.24 (data state)
and 1.27 to 3.17 (pass 1). Machinery and vehicles are bought mostly by investment.

Options:

- A. All final demand: one factor per country and good over the three demanders, the seed
  (h_c HFCE + g_c GOV + s_c INV) times absorption; the government and investment compositions
  are calibrated with the household one; PIGL is inverted on the household part only.
- B. Households only (ADR 0099 as written): government and investment at their data values in
  the targets, so the households' targets carry all of the discrepancy, and a capital good whose
  household target is not positive stops the calibration (`Balanced::held`).

Recommendation: A, because B moves every discrepancy onto the smallest buyer of capital goods.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0304

### D5. How do government and investment compose their spending?

Options:

- A. Fixed nominal shares (Cobb-Douglas): each good's share of the demander's spending is its
  calibrated value at any prices; a good with no reachable origin gets nothing and its share
  goes to the other goods in proportion.
- B. A fixed real basket at the calibrated prices (Leontief): nominal shares move with prices,
  and an unreachable good needs a rule of its own.

Recommendation: A, because ADR 0099 calls them fixed shares and A holds them exactly with no
inversion in stage 4.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0305

### D6. Is the accelerator built now, and with what smoothing?

Options:

- A. Build it: the investment share is s_c (Y_c / Ybar_c)^(beta - 1), Y_c the country's
  spending of the tick, Ybar_c its average with a 12-month half-life (snapshot state), capped at
  1 - g_c; households take the rest. beta is a register entry (prior 1 to 3, default 1, which is
  the fixed share and leaves the state unchanged), and so is the half-life.
- B. Defer it to P4, where investment follows capital (ADR 0099 option 3).

Recommendation: A, because the pre-P3 plan lists it with default 1, which moves no golden.

Decision: B, Antonio, 2026-11-03.
Row: ADR 0306

## Spec changes

- 8.6: final demand is three demanders per country (households, government, investment), spent
  through the pops in P2; PIGL on households' part only; the accelerator waits for P4 (ADRs 0301
  to 0306).
- 9.6: the fiscal and investment accounts come with the fiscal block; in P2 the pops spend the
  government and investment shares (ADRs 0301, 0303).
- Econ SPEC: Pack additions (`hfce_g`, `gov_g`, `inv_g`, `gov_share`, `inv_share` in
  `econ_countries.arrow`); Final demand; Calibration stages 1 and 4; Parameters.
