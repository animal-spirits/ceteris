# 0230. Latent demand dynamics in the demand system

Status: proposed (owner chose the option, 2026-10-28); amended per ADR 0241 (2026-10-29)
Date: 2026-10-28
Design sections: 9.9, 9.14, 13.1, 14.4, 17; ADR 0111
Phase: P5b

## Context

Latent demand eta in 9.9 is backed out so that 2000 holdings are reproduced exactly. How it moves
after 2000 is unset; ADR 0111 left "mean-reverting latent demand" for P5 design. In Koijen and Yogo
(2019) latent demand explains most of the variance of returns, so a fixed eta leaves volatility to
fundamentals alone and would miss the equity and FX volatility targets of 9.14.

## Options

1. Fixed at 2000 values.
2. Mean-reverting by asset group, with shocks on the tape.
3. Refit yearly to observed holdings until 2024, then mean-reverting.

## Decision

Option 2 (owner, 2026-10-28).

```
eta(i, a, t) = eta0(i, a) + xi(i, g(a), t)
xi(i, g, t)  = rho * xi(i, g, t-1) + sigma * eps(i, g, t)
```

- g(a) is the asset group: issuing state times instrument class of 9.8 (sovereign, corporate
  credit, equity, currency).
- eps is drawn on the shock tape of 13.1 with common random numbers keyed by (class, group) per
  14.4. Global co-movement comes from risk appetite (9.12), not from correlated eps.
- rho and sigma are pooled across classes and groups: two entries in the finance budget. rho has a
  prior from Koijen-Yogo persistence estimates; sigma is calibrated in P9 on return volatility.

## Tests

- With sigma = 0, results equal fixed latent demand.
- The unconditional mean of eta equals eta0 over long runs.
- Simulated equity and FX volatility within the 9.14 target bands at the calibrated values.

## Consequences

Price volatility gets a source beyond fundamentals with two parameters, and hindcasts stay
unconditional forecasts.
