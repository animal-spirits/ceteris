# Economy assessment: how far from a complete GE world, 12 November 2026

Snapshot of `main` at 032d7e4. Sources: `crates/ceteris-econ/src/systems.rs` (system table),
`reports/econ-numbers-vs-literature-2026-11-04.md`, `reports/retrospective-2026-11-12.md`,
`reports/modelling-research-2026-11-11.md`, `reports/architecture-overview-2026-11-04.md`,
`docs/free-moment-rerun-results.md`, `specs/p4..p9/PLAN.md`, open issues. Nothing was rerun for
this report. Effort sizes are relative estimates, not measurements.

## 0. Short answer

The **real economy of 2000 is built and runs**: a province-level, multi-good, Eaton-Kortum trade
and production model with PIGL demand, stock-flow prices, wages, transport lags and a 2000
steady-state calibration that the reference solver checks. Around it, population, money, the
state and energy have **kernels and wiring, all off by default**; investment, firm dynamics and
structural change are **unwired**; currencies, banks, housing, sovereign debt and growth are
**missing**. Validation is weak where it matters: most passes are by construction, and the free
moments (distance, Brexit size, x10 shock) miss. A living, calibrated, closed world is several
large steps away; the next step is not more mechanisms but making the trade core right.

## 1. What exists, by component

Status: **on** (default path), **wired/off** (in the tick behind a switch), **kernel** (tested
library, no path from a run), **missing** (spec or plan only).

| Component | Where | Status | Note |
|---|---|---|---|
| Structural gravity trade, two-level kernel, gateways | `ceteris-trade`, systems 205-207 | on | ADRs 0015, 0092, 0200 |
| Transit pipeline (1-4 month lags), dispatch | system 207 | on | |
| Origin markets, rationing, perishing | system 206 | on | |
| Stock-flow price rule, gain caps | system 208 | on | kappa1 0.05, kappa2 0 (design 0.2 / 0.1) |
| Production: CES VA, Leontief ICIO inputs, local chain | systems 202-203 | on | one putty-clay vintage |
| PIGL demand over pops, lognormal income | system 204 | on | Alder-Boppart-Mueller |
| Distribution: profits, dividends, transfers, write-off, wealth brackets | system 209 | on | world account closes imbalances |
| Steady-state 2000 calibration, reference solver, V0/V1/H2 | `ceteris-calibrate`, `ceteris-reference` | on | balanced growth (0243, #211) missing |
| Log-distance border friction (0272, crossing only) | trade kernel | on | leg-cost component (#212) missing |
| Competitive storage of storables | trade | wired/off | ADR 0091; storage-on calibration fails (Sinkhorn) |
| Per-good path choice | geo | wired/off | ADR 0279 |
| E1/E2/E3 price package (money anchor, pay on sales, credit line) | `PriceAnchor::e3` | wired/off | P5 price-collapse fix |
| Demography, disease | system 220 | wired/off | P4 A, B |
| Wage grid by province, sector, skill | system 225 | wired/off | P4 C |
| Migration | system 227 | wired/off | P4 D |
| Settlement | system 229 | wired/off | |
| Monetary policy, HLW neutral rate, regime hazards | system 230 | wired/off | sets policy-rate state only; no feedback yet (#543) |
| Fiscal, government ledger accounts | system 232 | wired/off | budgets, public capital pending (#546) |
| Energy policy wedges | system 233 | wired/off | |
| State energy cells | in distribute | wired/off | #550 merged, #533 still open |
| Sector balance sheets, SFC matrices | `sfc.rs` | kernel (partly used) | P5 A1 |
| Investment pipeline | `investment.rs` | kernel | #534 |
| Firm entry, exit, zombie band | `entry_exit.rs` | kernel | #535 |
| Structural change | `structural.rs` | kernel | #536, blocked by #173 |
| Short-run trade adjustment (rho, 0027) | | missing | #200; rho unidentified |
| Currencies, FX, pegs | money SPEC | missing | single numeraire today |
| Banks, housing, equity, sovereign bonds, default, IMF | P5 F, G | missing | #217 |
| HANK reference | P5 H | missing | #218 |
| Traded services | P5 J | missing | #158 |
| Frontier growth, catch-up, tech jumps | P8 A | missing | #220, #173 |
| Oil and gas, power sector, emissions, hazards | P8 B-F | missing | #221, #222 |
| 50-goods pack | P3 | missing | #206; 10 goods today |
| Hindcast, SBI, held-out scorer | P9 | missing | #223-#227, #246 |

Count: 8 components on, 13 wired/off, 4 kernels, 13 missing. ADR 0353 caps unwired kernels at 3;
there are 4 (counting `sfc.rs`), or 3 once #533 closes.

## 2. Validation evidence

**Reproduces real data, on evidence (not fitted):**
- ACR-IO welfare median 1.077 against the theory's 1 (bar [0.8, 1.25]).
- Plain vs IO ACR ratio about 2.2, the IO multiplier the literature predicts.
- Signs of Japan and EU27 welfare under border and Brexit shocks (default mode).
- 2000 price levels for 31 of 31 ICP benchmark economies within 10%.
- US-China 2018 pass-through 0.97, welfare -0.04% (28 Oct; untested since ADR 0200, #245).

**Agrees only by construction** (borrowed or fitted to the target, so tests the code): trade
thetas per class, PIGL parameters, openness vs ICIO (border nu fitted), US top-10% wealth share,
sector labour shares, education returns, mode-cost ratios.

**Does not reproduce:**

| Moment | Model | Data / literature |
|---|---|---|
| H2 distance elasticity | -0.472 (s.e. 0.011), identical off/e1/e3 | -1.1 to -0.7 |
| Hard / soft Brexit, UK welfare | -0.63% / -0.24% | -2.66% / -1.34% (Dhingra et al. 2017) |
| ACR-IO correlation | 0.862 | bar 0.9 |
| PPML theta on BACI | 10.9-14.1, no spread | class spread 2.9-16.9 (#241) |
| Short-run trade elasticity | long-run theta every month | about 0.8 short run, 2 long run (BLP-N 2023) |
| x10 border shock | default run dies; no autarky number | ACR gains of a few to tens of percent |

**Benchmark harness, 11-12 Nov rerun (16 tests):** off 16 pass (x10 case panics in
`world.rs:89`, reported as a failed case); e1 13 pass, wrong-sign welfare for Japan and EU27 and 8
countries in the ACR check; e3 13 pass, openness outside 1.5x of ICIO and a usize underflow in two
tests.

**Calibration failures:** `P_NU` (no border fit: PIGL inversion for country 222) and `P_ST`
(storage on: Sinkhorn residual 1.6e-13) failed; #553 stops both at their rounding floors, not yet
re-run. Untested: price, wage and employment speeds (#242), WID shares outside the US (#243), the
drift half-life (#172).

Pattern: each fitted wedge (border nu, buyer price index, anchor) hits its target while moving a
free moment away. The literature baseline points at the fitted border nu as the lead suspect.

## 3. Gap to a complete GE world

**Missing feedback loops.**
- Money to prices: policy rate is state only; no rate into investment, saving or FX (#543).
- Fiscal to demand: government ledger exists, no budget rule, debt or public capital (#546).
- Investment to capital to output: kernel unwired; capital is fixed at the 2000 vintage.
- External closure: imbalances held through a world account; no price closure (B1), no
  financing bound (B5), no target rule (F1).
- Firm entry and exit: unwired; zero-output cells cascade through Leontief inputs (C1, C2).
- Growth: no frontier growth or catch-up; the 2000 state is a steady state by construction.
- Short-run trade adjustment: long-run theta applies monthly.

**Missing sectors, agents, markets.** Currencies and FX; banks and credit; housing; equity;
sovereign bonds and default; traded services; oil, gas and power as distinct sectors; 40 of the
planned 50 goods. Agents are enterprises per (province, sector) and pops; no banks, no
government agent beyond the ledger, no central-bank balance sheet.

**Data gaps.** 50-goods pack (#206, SFI steel #175); SEA and AAMNE fetch (#182); Eurostat 2000
manufacturing gaps (#190); EIA sha256 (#499); HMD, MAP, ADB MRIO (#500-#502); origin packs for
2005 and 2010 for hindcast (#226). The best rho identification relies on NC sources (TRAINS,
Teti, MAcMap) that cannot ship in the paid build.

**Performance.** 10 goods: about 34 ms vs 35 ms budget. Pack (45-goods) tick 198-212 ms vs 160 ms
ceiling. Dispatch dominates. Under the laws this waits until Right is green; the plan is snapshot
encode off the tick, then a goods-innermost spike, then a budget ADR (#189, #236-#239).

## 4. Critical path

Ordered by dependency. S, M, L are relative effort; XL is several L.

| # | Step | Size | Depends on |
|---|---|---|---|
| 1 | Re-run `P_NU`, `P_ST` and the missing free-moment columns after #553 | S | Mac |
| 2 | Fix the e1/e3 benchmark failures (wrong-sign welfare, usize underflow, x10 panic) | M | 1 |
| 3 | Fix pass bands for the free moments before results (RFC) | S | none |
| 4 | Build 0272 leg-cost distance friction with recalibration (#212); test border-nu revert | M | 3 |
| 5 | x10 survival: C2 hysteresis band, C1 critical inputs | M | 2 |
| 6 | Turn E3 on by default, re-bless goldens, add to the matrix | M | 2, 5 |
| 7 | Balanced-growth calibration and moving reference (#211) | L | 4 |
| 8 | 50-goods pack (#206) and data fetches | L | Mac, data |
| 9 | Monetary and fiscal feedback loops (#543, #546), then on by default | M | 6 |
| 10 | Wire investment (#534), entry/exit (#535); capital accumulation | M | 7, 9 |
| 11 | External closure: T1 spike, then B1/F1 | M | 6 |
| 12 | Currencies and FX | L | 9, 11 |
| 13 | Short-run trade adjustment (#200) | M | 8 |
| 14 | Structural change (#536) and growth (#173, #220) | L | 7 |
| 15 | P4 population on by default (demography, migration, wage grid) | M | 7 |
| 16 | Sovereign debt and flows; banks, housing, HANK (P5b) | XL | 12 |
| 17 | Energy sectors, emissions, hazards (P8) | L | 14 |
| 18 | Tick speed to 160 ms on 50 goods | M | 8, Right green |
| 19 | P9 hindcast 2011-2024 out of sample | L | all above |

Steps 1-6 make the existing real economy right. Steps 7-15 make it a closed, growing, monetary
GE economy. Steps 16-19 make it the complete world of the design and test it against history.

## 5. Decisions (question cards, answered 12 Nov)

1. Trade core vs more wiring: **both in parallel**. Wiring (#534-#536) continues while one thread
   works steps 1-5.
2. First switch to default on: **E3 price package** (step 6).
3. P5b scope: **all of P5b stays in 1.0** (banks, housing, equity, sovereign, IMF, HANK).
4. Free moments: **hard gates, bands fixed now** by RFC, for the P4 and P5 gates (step 3).
5. B1 timing: **T1 Hume two-islands spike first**, then decide.

These answers still need recording in RFCs and the ADR rows; this report is not the record.
