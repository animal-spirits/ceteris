# 0178. Endogenous exploration and reserves

Status: proposed
Date: 2026-10-28
Design sections: 7.4, 13.4, 15.4, 15.5; ADRs 0028, 0091
Phase: P8 (with 0028)

## Context

Discoveries of oil, gas and minerals enter as tape shocks (13.4), so supply responds to prices
only through the use of known reserves. US shale output rose from about 0.5 to 4.5 million
barrels a day in 2008-2014 in response to high prices and falling drilling costs, and its supply
response shaped the 2014 price collapse. Giant discoveries are news shocks with large macro
effects (Arezki, Ramey and Sheng 2017).
Gap audit (2026-10-28)

## Options

1. Status quo: discoveries on the tape.
2. Exploration investment on expected prices and depletion, with a discovery draw.

## Decision

Option 2.

- Extraction enterprises invest in exploration from expected prices (their forecast rule) and
  the depletion of their reserves.
- Exploration cost falls with cumulative drilling by a learning curve (register entry,
  learning rate).
- Discoveries are drawn per basin from a size prior fitted to the historical distribution of
  field sizes, using the module's own RNG stream, so the result is deterministic
  and forkable.
- Unconventional resources (shale, oil sands) are basins with high cost and fast decline.
- Historical discoveries 2000-2010 remain available as a replay tape for the holdout.

## Migration

- P8: reserve and exploration state per basin, with 0028's resource stocks.
- Pack: basin priors from published field-size data; 2000 reserves.

## Tests

- With exploration off, the tape path reproduces the status quo.
- A sustained toy price rise raises discoveries and output with a lag of 2-5 years.
- 2008-2014 US tight-oil output within tolerance when historical prices are fed.

## Consequences

The supply side of commodity cycles becomes endogenous. Register: 2-3 entries.
