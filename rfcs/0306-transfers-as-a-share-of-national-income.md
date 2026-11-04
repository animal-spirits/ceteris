# RFC 0306. Transfers as the current account's share of national income (amends ADR 0100)

Status: accepted
Date: 2026-11-04
Design sections: 9.5; econ SPEC (Pack additions, Transfers; Transfers; Calibration, Stage 1)
Research: none (diagnosis of the C6 pack gate, below)

## Summary

ADR 0100, as amended by ADRs 0296, 0298 and 0300, pays each country D_c = s_c Y, Y world income,
with s_c minus the country's current account less its GDP share of the world discrepancy over
world GDP. The transfer so scales with the country's GDP in the data, not with its income in the
model. Where the model's national income is far below GDP, D_c can exceed it. On the C6 pack
this happens in Aruba: tourism, most of its GDP, sits in local services the model cannot export,
so the calibration shrinks local output to local demand and household income is 7% of GDP
(median 0.90); its 2000 surplus, 11% of GDP, makes D_c -1.59 times income, spending is negative,
and a pop with negative spending buys nothing. World spending then exceeds world sales by 2.3e-5
(`balance_mu`), the calibrated point misses the reference by the same, and the pack gate fails.
Bermuda, Andorra, Antigua, Palau and Macao have low income against GDP for the same reason, but
none goes negative; Kuwait is next at -0.45.

This RFC scales each country's current account by its own national income: D_c is its ratio of
the current account to GDP, times its income, less its income share of the world discrepancy.

## Decisions

### D1. How does D_c scale with the country's size?

Options:

- 1. As a share of national income: rho_c = -CA_c / GDP_c (0 where either is missing), D_c =
  (rho_c - rho_bar) Y_c, Y_c the country's income of the previous tick and rho_bar = sum_c rho_c
  Y_c / sum_c Y_c, so the transfers net to zero every tick (the discrepancy spread pro rata to
  income, as ADR 0298 spreads it to GDP). Spending is (1 + rho_c - rho_bar) Y_c, positive while
  the current account stays below national income. The path (ADR 0300) takes each year's CA and
  GDP.
- 2. Floor absorption at zero: cap D_c >= -Y_c and spread the excess over the other surplus
  countries; the shares stay world-GDP shares.
- 3. Traded services (tourism exports), so that a tourism economy earns its surplus: a P3
  change.

Recommendation: 1 now, 3 filed for P3, because 1 keeps every country's 2000 current-account
ratio, treats every country alike and cannot make spending negative; 2 is a patch on the one
country.

Decision: 1, and 3 filed for P3, Antonio, 2026-11-04.
Row: ADR 0315

## Spec changes

- 9.5: D_c is the country's current-account ratio times its national income, less its income
  share of the world discrepancy (ADR 0315).
- Econ SPEC, Pack additions: `transfer` and `transfer_2000` to `transfer_2024` are -CA_c / GDP_c.
- Econ SPEC, Transfers: D_c = (rho_c - rho_bar) Y_c.
- Econ SPEC, Calibration, Stage 1: the transfer at the pass's incomes.

## Tests

- `transfer_ratios_are_minus_the_current_account_over_gdp` (pack `econ/transfers.rs`): the ratio,
  0 where the current account or GDP is missing.
- `transfers_net_to_zero_every_tick` (econ): unchanged, the account ends every tick at zero.
- `transfers_scale_with_national_income` (reference `model.rs`): spending (1 + rho_c - rho_bar)
  m, the transfers summing to zero (1e-12 of world income).

## Consequences

- No country's spending can go negative while |CA_c| < GDP_c; Aruba's transfer becomes about
  -0.11 of its income.
- The calibration's world balance closes (mu = 1 at the fixed point), so the C6 pack gate's
  reference check can hold.
- The `transfer` columns change meaning; the pack format, the econ tables' hashes and the
  calibration change; worlds without transfers are unchanged.
