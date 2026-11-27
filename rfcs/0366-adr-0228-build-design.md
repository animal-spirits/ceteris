# RFC 0366. Build design for the ADR 0228 external-balance closure in P5a

Status: accepted; not built
Date: 2026-11-26
Design sections: 8.8, 9.2, 9.9, 12.2, 17; money SPEC (P5a rule); econ SPEC E2 to E4; trade SPEC Money-anchored rule
Release: 1.0, P5a
Research: [P5a external-balance price spike](../docs/research/p5-bop-price-spike-2026-11-26.md); [RFC 0365](0365-accept-adr-0228-external-balance-closure.md); [RFC 0350 bands at the fitted values](../docs/research/rfc0350-bands-fitted.md); issues #561, #667, #668, #673

## Summary

RFC 0365 accepted ADR 0228 for P5a as the external-balance closure with the E3 credit limit,
spike-gated. The spike (PR 678) tested only a proxy, a one-sided import wedge on cumulative money
change, which collapsed the world twice; the E3 package alone reaches 1.0712 at 1,500 ticks
against the 1.07 gate, a slow mainland drift between countries 0 and 1. The full rule was not
tested because the model has no currency layer: one numeraire, no financier, no exchange rate.

This RFC is the build design for ADR 0228 in full: currencies per country, a financier ledger
account per currency holding q_c, the rate rule, conversion at the border, and local-currency
wages and prices. The credit limit is already built (E3 package, P148, ADRs 0344, 0345) and is
kept unchanged, now bounding spending in local currency. The kernel ships off by default behind
one switch, is verified against the x10 gate fixed below, and then the RFC 0350 bands are
rescored once, unchanged. No kernel code is written by this RFC.

## Decisions

### D1. After the proxy no-go, how is the external deficit closed?

Options:

- A. Build ADR 0228 in full (symmetric rate, financier account, local-currency wages) under the
  RFC 0365 freeze exception, design first.
- B. Accept the 0.0012 gap with a written reason, pinning test and owner.
- C. Design and spike a lighter currency closure.

Recommendation: A, because it is the root-cause fix the calibration rule asks for.

Decision: A, Antonio, 2026-11-26 (card answer, project chat, 19:17Z).
Row: ADR 0403

### D2. How does the ledger hold money in several currencies?

Options:

- A. Per-currency ledger. Every account belongs to one currency (its country's, or the
  numeraire for world accounts); one mint and one financier account per currency. Money within a
  currency is conserved exactly in integers; cross-currency payments go only through the
  financier at e_c. q_c is the financier's balance in currency c, an exact stock.
- B. Numeraire ledger with a conversion layer. Money stays in one unit; e_c rescales wages and
  prices, and q_c is a float state outside the ledger.

Recommendation: A, because ADR 0228 names a financier position and local-currency wages, and only
A makes q_c a ledger stock covered by the conservation invariant (V0, 9.8); B leaves the very
deficit stock this closes outside the books.

Decision: A, Antonio, 2026-11-27 (card answer "Accept all recommended", 01:28Z).
Row: ADR 0404

### D3. Which expectations does the rate rule use in P5a?

Options:

- A. The anchoring heuristic of 12.2 alone: E_t[e_c(t+1)] is an exponential mean of past e_c
  with a fixed half-life; full heuristic switching (Brock and Hommes 1997) is added when 12.2 is
  built, tracked by an issue, as the nesting of D2 already allows.
- B. Full 12.2 heuristic switching (adaptive, trend, anchor) now, built for this rule.

Recommendation: A, because 12.2 is not built anywhere and B would add a second kernel inside the
one freeze exception; trend-following rules are a known source of endogenous cycles (12.2) that
would confound the gate.

Decision: A, Antonio, 2026-11-27 (card answer "Accept all recommended", 01:28Z).
Row: ADR 0405

### D4. Where does the rule live?

ADR 0228 Migration puts it in `ceteris-finance`, which does not exist; the monetary rules of 9.2
live in `crates/ceteris-econ/src/monetary.rs`.

Options:

- A. A module `crates/ceteris-econ/src/fx.rs` next to `monetary.rs`; ADR 0228's crate name is
  amended by this RFC, and the module moves when `ceteris-finance` is created in P5b.
- B. Create `ceteris-finance` now with this module only.

Recommendation: A, because the rule needs the ledger, layout and monetary state that live in
`ceteris-econ`, and a crate holding one module is premature (KISS; ADR 0353 counts kernels, not
crates).

Decision: A, Antonio, 2026-11-27 (card answer "Accept all recommended", 01:28Z).
Row: ADR 0406

### D5. What is the gate, fixed before any run?

Options:

- A. As RFC 0365 D2, made exact: `small_world`, `EconParams::frictionless()`, E3 package and
  closure on, ntb 10 on every pair, 1,500 ticks; world price index (chained, trade SPEC, buyer
  prices converted to the numeraire at e_c, base consumption weights) at most 1.07 of base at
  every tick, output at least 0.9 of base after tick 20; the ntb 1 run holds its steady state
  (e_c within 1e-9 of 1, q_c zero). Gamma and the anchoring half-life are set from their
  literature priors in the spec, before the run. One run; a miss is a recorded no-go and the
  structure returns to Antonio. No Gamma, half-life, band or go-number change after seeing it.
- B. The same, with the index measured in each country's local currency and the gate on the
  largest country deviation.

Recommendation: A, because it is the number Antonio fixed on the structure card and the spike
baseline (1.0712) was measured the same way, so the comparison is like for like; local indices
are reported as a diagnostic.

Decision: A, Antonio, 2026-11-27 (card answer "Accept all recommended", 01:28Z).
Row: ADR 0407

## Design (applies the recommended options)

### Currencies and accounts

- Each country has a currency; union members share one (9.2, ADR 0210); dollarized countries use
  the anchor's. The numeraire is the currency of the country with the largest base nominal GDP
  (the United States in the pack; the largest fixture country on `small_world`). Every fixture
  country floats.
- Base parity e_c(0) = 1, numeraire per unit of c: pack prices are in USD at 2000 market rates,
  so the base state is unchanged by the layer.
- `Accounts` gains `currency_of` (per account), one mint per currency and one `financier`
  account per currency. World accounts (`transfer`, `foreign`) are numeraire. Opened only with
  the switch on, after the existing accounts, so a world without the closure keeps its ledger and
  hash.

### The rule (ADR 0228, monthly tick)

```
q_c(t)  = financier balance in currency c after the tick's payments (F_c accumulated)
Q_c(t)  = q_c(t) e_c(t-1)                                   numeraire value, explicit in e
e_c(t)  = E_t[e_c(t+1)] (1 + i*_t) / (1 + i_c,t + Gamma_c Q_c(t))
E_t[e_c(t+1)] = m_c(t-1),  m_c(t) = m_c(t-1) + (1 - 2^(-1/h)) (e_c(t) - m_c(t-1))
Gamma_c = Gamma / Y_c(0)                                    currency-size scaling, Y_c(0) base nominal GDP
```

- F_c is the net currency sold to the financier: international goods legs, the ADR 0310 foreign
  dividends, the ADR 0100 transfer (a capital flow while it exists) and intervention. A deficit
  country sells its currency, q_c rises and e_c falls; a surplus country's q_c falls and e_c
  rises, so the rule is symmetric, which the proxy was not.
- i_c from the policy rule (system 230) when monetary is on; otherwise i_c = i* = 0.
- Pegs: the central bank buys or sells its currency against the financier at the parity from
  reserves (`CentralBank::defend`), keeping q_c at zero while reserves last, then floats on its
  fallback (ADR 0210). Unions run one q for the area.
- Financier valuation gains stay on its account in P5a (reported, not distributed); distribution
  to owners comes with the 9.9 demand system in P5b. Shortcut, tracked by an issue.
- New register entries (Section 17, codes from `next-code P` in this PR): Gamma (P184, prior from
  Gabaix and Maggiori 2015, the value and source written in the row) and the anchoring half-life
  h (P185, prior from the anchoring estimates of Anufriev and Hommes 2012). Neither is fitted in P5a.

### Conversion at the border

- Within a country every payment is in its currency: wages (E2), local goods, pop spending,
  dividends, rebates, the credit limit cash + T_e (E3, P148), the write-off (E4).
- International goods leg (dispatch, T2b): the buyer pays its destination market in c_d; the
  destination gateway pays the c_d financier; the c_o financier pays the origin gateway
  `amount e_d / e_o`, rounded half to even per leg; gateways still net to zero per tick.
  Rounding stays on the financier, so per-currency conservation is exact.
- The trade kernel (`ceteris-trade`) is untouched: the econ side converts origin unit costs to
  the numeraire at e_o before the solve and delivered prices to local currency at e_d after it.
- Money-anchored price rule (trade SPEC): each currency anchors on its own money stock and local
  index; the world index of D5 is reported in the numeraire.

### Invariants (each with its test)

- Per-currency conservation: every currency's balances sum to its mint, exactly, every tick
  (`bop_money_conserved_per_currency`).
- Gateways net to zero per tick per currency (existing invariant, extended).
- With every e_c held at 1 and the switch on, the real allocation (goods, output, prices in
  numeraire) is bit-identical to the E3 run (`bop_at_unit_rates_matches_e3`).
- State and journal hashes identical across runs and thread counts; snapshot replays
  (`bop_snapshot_replays`).

### Tests (ADR 0228 Tests plus the gate)

- Gamma = 0, i_c = i*: e_c equals its expectation exactly (UIP).
- A policy-rate rise appreciates on impact, then depreciates along the rate differential.
- A one-country import shock: the deficit currency depreciates and the surplus currency
  appreciates; q_c returns toward zero.
- Peg absorbs F_c from reserves at parity; breaks to its fallback when reserves run out.
- Switch-on behaviour test, `bop_x10_border_shock_holds_1500_ticks` (gate D5) in
  `crates/ceteris-econ/tests/p5_bop.rs`: passes with the closure on; the same run with it off
  exceeds 1.07 (1.0712, spike baseline), so the test fails without the kernel.
- ntb 1 steady state holds for 1,500 ticks.
- Goldens: the switch is off by default, so no golden moves in the build PRs. Turning it on by
  default with the E3 package (#561) is its own PR with a reviewed golden move.

## Build plan (one task per PR, follow-on threads)

1. Spec: this RFC's spec changes and rows (money SPEC P5a rule, spec/09 9.2, econ SPEC E2 to E4
   currency notes, trade SPEC anchor per currency, Section 17 Gamma and h). This PR.
2. Currency layer: `currency_of`, numeraire, per-currency mint and financier accounts, e and m
   state with snapshot tag, per-currency conservation invariant; e held at 1, no behaviour
   change. Files: `world.rs`, `params.rs` (switch), `fixture.rs`, snapshot tests.
3. Border conversion: dispatch legs, transfer and foreign dividends through the financier; trade
   solve conversion in and out. Files: `systems.rs`, `world.rs`. Test `bop_at_unit_rates_matches_e3`.
4. Rate rule: `fx.rs` (q from the ledger, anchoring expectation, e update), new system after 230
   and E4, pegs and unions via `monetary.rs`; UIP, policy-rate, shock and peg tests.
5. Gate: `tests/p5_bop.rs` switch-on behaviour test and ntb 1 test, one run, study in
   `docs/research/` with the numbers; go or recorded no-go. Spike rule: one day.
6. On go: switch on by default with the E3 package (#561), golden move reviewed, ADR 0354 switch
   review entry.
7. Rescore (Mac, owner-built pack, fitted values of #667): every RFC 0350 band once, unchanged
   (#668), Brexit and US-China welfare as real consumption deflated by the local CPI, which the
   numeraire choice does not change. What still misses goes to a tier-2 ADR (ADR 0257) naming
   services trade (P8) and 2000 base exposure, with a pinning test and an owner (RFC 0365 D3).
   Pack: currency areas from the regime data the monetary system already reads (ADR 0210); if
   the pack lacks them, that PR stops and asks.

## Spec changes

- money SPEC Equations: P5a exchange rates (ADRs 0403 to 0407).
- spec/09 9.2: P5a rule and its replacement in P5b (ADRs 0403 to 0407).
- econ SPEC E2 to E4: currencies of payments and border conversion (ADRs 0403, 0404).
- trade SPEC: money anchor per currency (ADR 0404).
- Section 17: P184 Gamma, P185 h (ADRs 0403, 0405, 0407).
