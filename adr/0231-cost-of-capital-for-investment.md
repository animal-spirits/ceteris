# 0231. Cost of capital for investment

Status: proposed (owner chose the option, 2026-10-28)
Date: 2026-10-28
Design sections: 7.4, 9.10, 9.11; ADRs 0202, 0204
Phase: P5a (rule with policy-rate spreads), P5b (credit spreads and equity returns)

## Context

DESIGN 7.4 has owners compare expected returns to "the cost of capital (a Tobin-q style rule)",
financed from retained earnings, bank credit, bonds and equity, but sets neither the price nor the
mix. ADR 0204's cash calls need credit. This is the main link from finance to the real economy.

## Options

1. Weighted cost from simulated prices, with a collateral cap on borrowing.
2. Tobin's q from simulated equity prices.
3. Loan rate only.

## Decision

Option 1 (owner, 2026-10-28).

For an enterprise cell c and asset class a:

```
r_c   = w_D,c * (i_loan,c + s_credit,c) * (1 - tau_c) + w_E,c * r_E,c
u_c,a = p_a * (r_c + delta_a - E[dp_a / p_a])        user cost (Hall-Jorgenson)
```

- i_loan is the bank lending rate of 9.3, s_credit the credit spread of 9.10 (Merton default
  probability plus excess bond premium), tau the corporate tax rate, r_E the required equity return
  of the equity block, delta_a and p_a the depreciation and bundle price of 0204.
- w_D and w_E are each cell's 2000 debt and equity shares: country non-financial corporate leverage
  from financial accounts, spread across sectors by the Merton leverage of 9.10.
- The 7.4 logit compares expected return to u_c,a.
- Funding order for orders and cash calls: retained earnings, then loans up to a collateral cap
  (debt at most lambda times the value of the cell's capital, lambda per country from credit to
  non-financial corporations over their capital stock in 2000), then equity issued at r_E. When the
  cap binds, orders shrink.
- In P5a, before the finance blocks, s_credit is a fixed spread over the policy rate from 2000 data
  and r_E is the risk-free rate plus a fixed premium.

## Tests

- With spreads fixed and the cap slack, results equal a rule on the loan rate alone.
- A rise in the excess bond premium lowers investment with the sign and lag of Gilchrist and
  Zakrajsek (2012).
- A fall in collateral values binds the cap and cuts investment more in high-leverage cells.

## Consequences

Rates, credit spreads and equity markets all reach investment, and the collateral cap gives the
2008 credit crunch its channel. No new free parameters: weights and lambda are data.
