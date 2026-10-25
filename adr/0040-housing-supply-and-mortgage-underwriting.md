# 0040. Housing supply elasticity and mortgage underwriting

Status: proposed
Date: 2026-10-24
Design sections: 8.7, 9.3, 9.10, 9.11, 9.13
Phase: P5 (supply elasticity data in P2)

## Context

Mortgage credit growth is the best single predictor of financial crises (Schularick and Taylor
2012; Jorda, Schularick and Taylor 2016), and housing busts were the core of 2008 in the United
States, Spain and Ireland. The design prices housing per province with credit limits and supply
elasticity "from land and terrain" (9.10) but does not specify either. Saiz (2010) shows that
geographic constraints (steep slopes, water) and regulation explain most of the variation in
housing supply elasticity across US metro areas. The housing ABM of Baptista et al. (2016, Bank
of England) shows that loan-to-value and loan-to-income limits, and buy-to-let investors,
change boom-bust dynamics materially.

## Options

1. Status quo: unspecified elasticity and limits.
2. Supply elasticity per province from terrain and land availability on the H3 raster, plus a
   regulation index; mortgage underwriting with loan-to-value, loan-to-income and
   debt-service-to-income caps; owner-occupiers and investors as separate housing demand
   classes.
3. Individual household mortgage microsimulation.

## Decision

Option 2.

- Supply elasticity per province from the share of land unavailable for development (slope
  above a threshold, water, protected area) following Saiz, scaled by a regulation index where
  available (P74 maps both to the elasticity).
- Mortgage demand per pop wealth bracket is capped by the binding of LTV, LTI and DSTI limits;
  caps are macroprudential levers (9.11) and their binding share is reported.
- Investor demand (buy-to-let) responds to expected capital gains under the extrapolative
  expectations of 9.13, with its share of purchases P75; it is the channel through which
  extrapolation amplifies booms.

## Migration

- P2: compute the land-availability share per province in the pipeline (data only).
- P5: underwriting constraints and investor demand in the housing block.
- Validation: house price momentum at one year and mean reversion at five (Glaeser et al.
  2014); credit-to-GDP and house-price paths (BIS, already in the manifest); macroprudential
  episodes (UK 2014, Ireland 2015 caps).

## Tests

- With caps non-binding and investor share zero, results equal the status quo.
- A tighter LTV cap in a toy boom lowers the peak of prices and credit, with the direction
  found by Baptista et al.

## Consequences

Housing-credit booms, the main crisis channel, get their mechanism and their policy levers.
