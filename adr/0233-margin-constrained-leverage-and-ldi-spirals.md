# 0233. Margin-constrained leverage and LDI spirals

Status: proposed (owner chose to record it, 2026-10-29)
Date: 2026-10-29
Design sections: 9.7, 9.8, 9.11, 9.13, 15.4, 17; ADRs 0227, 0229
Phase: P5b, behind a flag until it passes the ablation test (15.7)

## Context

Derivatives are not instruments (9.8). Two nonbank spirals in the 2011 to 2024 holdout ran
through derivative margins: UK pension funds' liability-driven investment (LDI) after the
September 2022 fiscal statement, where swap and repo collateral calls forced gilt sales that
raised yields further until the Bank of England bought gilts, and the March 2020 dash for cash,
where leveraged Treasury basis positions were unwound. DESIGN 9.7 says losses hit "capital and
margins" and 9.8 has a leveraged-funds class, but no rule says which classes carry margin, how
pension hedges are represented, or what the central bank does when forced sales break a
government bond market. Both episodes were short and reversed within weeks, so their effect on
monthly scored targets is small; their value is a realistic tail for fiscal-shock
counterfactuals.

## Options

1. Status quo: no margin rule beyond bank capital.
2. A note in the finance SPEC only.
3. This ADR: margin constraints on leveraged classes, LDI leverage for pension funds, and a
   market-functioning purchase lever.

## Decision

Option 3 (owner, 2026-10-29).

- **Synthetic leverage.** A class may carry duration exposure above its assets: exposure =
  L_i times its holdings of long government bonds, with L_i and a collateral buffer B_i (the
  yield rise it can absorb) per state from data. Swaps and repo stay out of the instrument list;
  only their exposure and margin enter. Classes: pension funds and insurers in states with LDI
  (UK, Netherlands, Denmark), and leveraged funds (basis trades).
- **Margin call.** In the inner loop of 9.7, a yield rise beyond B_i creates a collateral call
  equal to the mark-to-market loss on the excess exposure. The class meets a share m of the call
  from liquid assets within the month (register: one entry, the last free slot under 0227's
  cap); the rest is met by selling bonds into the demand system, which re-clears prices.
- **Market-functioning purchases.** A variant of 0229's asset-purchase lever: it fires when
  forced sales by margin-constrained classes move the long yield past a threshold within a tick,
  buys up to the forced sales, and unwinds over the following months. The Bank of England's
  September to October 2022 operation and the Fed's March 2020 purchases are the references.
- **Regulation on the tape.** Buffer requirements set after 2022 (about 250bp for UK LDI funds)
  enter the policy tape and are held after 2024.

## Migration

- P5b: in `ceteris-finance`, with the inner loop. Data to audit for the manifest: Pensions
  Regulator and ONS pension liability and hedging data, Bank of England Financial Stability
  Reports (2022, 2023), Federal Reserve Z.1 hedge-fund Treasury positions.

## Tests

- With L_i = 0 for every class, golden hashes are unchanged.
- Given the observed September 2022 fiscal shock, the 30-year gilt yield rise with the spiral
  exceeds the rise without it by about the share the Bank of England attributed to forced sales.
- The market-functioning purchase caps that rise and unwinds without a lasting yield effect.

## Consequences

Fiscal-credibility counterfactuals gain the gilt-spiral tail, and the same rule covers the 2020
basis-trade unwind. One register entry (m); the rest is data. The finance budget reaches its cap
of 12.
