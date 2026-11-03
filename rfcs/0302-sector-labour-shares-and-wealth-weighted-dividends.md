# RFC 0302. Sector labour shares, wealth-weighted and foreign dividends: sources, bounds, ranks and routing (accepts and amends ADR 0104)

Status: accepted
Date: 2026-11-03
Design sections: 6.1, 7.2, 8.7, 9.1, 16; econ SPEC (Pack additions, Production, Income, Calibration)
Research: none separate; the reading of ADR 0104 against the code at the C3 head (080b7f9).

## Summary

ADR 0104 (proposed) gives each sector its own labour share, rescaled so the VA-weighted mean is
the PWT national value, pays the domestic profit pool by WID wealth shares instead of per
person, and pays a fixed foreign-owned share of mining, oil and gas and manufacturing profits
abroad by IMF CDIS positions. Building it (pre-P3 Task C4) leaves four choices open. The
rescaling can push a sector's share outside (0, 1) and ADR 0104 has no rule for it (D1). The
mapping of pops' lognormal incomes to wealth brackets is circular, because a pop's base income
contains its dividend (D2). CDIS reports inward positions by source country only, with no
sector, and the pack has no capital stock, so f_c,g cannot be formed as ADR 0104 writes it;
OECD Analytical AMNE has the 2000 foreign-owned share by host, sector and parent on the ICIO
grid that ADR 0201 already chose for P5 (D3). The foreign share needs a route and a status in
income (D4). This RFC accepts ADR 0104 with these answers (ADRs 0307 to 0310).

Sources (terms checked 2026-11-03): WIOD Socio-Economic Accounts 2016 release, CC BY 4.0
(Timmer et al. 2015), one Excel file on DataverseNL (doi:10.34894/PJ2M1C), the host PWT is
already fetched from; OECD Analytical AMNE 2000-2019, 76 economies and the rest of the world, 41
industries, CC BY 4.0 (ADR 0201); WID.world, open access, licence not stated on its data page,
to be confirmed at fetch (non-commercial terms are acceptable, project principle). The cloud
network blocks all three hosts; fetch and `--record` run owner-side.

## Decisions

### D1. Where do sector labour shares come from, and what bounds them?

Found: SEA covers 43 economies in 2000, with LAB (labour compensation, self-employed included)
and VA by 56 industries, which map onto the 10 goods as ICIO's do. The relative share is r_g,c =
(LAB_g / VA_g) / (LAB / VA). Rescaling to PWT can give s_L,c,g >= 1 where PWT's share is high and
the sector mix is capital-light, and r near 0 where VA is mostly imputed rent (Services).

Options:

- A. SEA 2016 r_g,c; economies outside SEA take, good by good, the mean r of the 5 SEA
  economies nearest in log GDP per person (ADR 0104's income group; the pack has no
  income-group table), flagged; mining (WIOD B, one industry) counts in full for goods 1 and 2;
  s_L,c,g clipped to [0.05,
  0.95] and the unclipped goods rescaled until the VA-weighted mean equals PWT (at most 10 goods,
  so at most 10 rounds), flagged per country where a clip binds.
- B. As A, but a share outside (0, 1) stops the build.
- C. EU KLEMS instead of SEA: fewer economies outside Europe, a stricter licence per release.

Recommendation: A, because it keeps ADR 0104's exact PWT mean and every share interior, which
the CES production function needs.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0307

### D2. How are pops' incomes ranked against wealth brackets?

Found: each pop has a lognormal income per person (mean its base income per person, dispersion
s_w, ADR 0094), and s_w and the base incomes both contain the dividend this decision sets.

Options:

- A. Perfect rank correlation, one pass: brackets (bottom 50%, middle 40%, top 10%) from the
  country's income distribution (the mixture of its pops' lognormals) at today's base incomes
  (dividends per person); a pop's dividend share is sum_b W_b x (its population in bracket b) /
  (bracket b's population), W_b the WID 2000 net personal wealth share (nearest year 1995 to
  2005; a negative share 0, the rest rescaled, flagged; region mean population-weighted where
  WID has none, flagged). Then base incomes and s_w are recomputed with these shares. Fixed at
  load, as today.
- B. As A, iterated to a fixed point of shares, base incomes and s_w.
- C. Rank by labour income only (wages), with no recomputation.

Recommendation: A, because it is deterministic, closed and one step; B's fixed point may not
contract and C ranks owners of capital by the wrong income.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0308

### D3. Which source gives the foreign-owned share and its owners?

Options:

- A. AAMNE 2000: f_c,g the foreign-owned share of VA by host and good for goods 1 to 6 (oil and
  gas, mining, manufacturing, as ADR 0104), capped at 0.8; the parent vector from foreign-owned
  gross output by parent; hosts outside AAMNE take their pack donor's f and vector; AAMNE's rest
  of the world as a parent is spread over non-AAMNE pack countries by GDP 2000.
- B. CDIS 2009 as ADR 0104 writes: one inward share per host over all of goods 1 to 6, from
  inward stock over a capital stock the pack does not have (PWT rnna at 2000 prices, a new
  source column), parents by position.

Recommendation: A, because it is the start year, has the sector, sits on the ICIO grid and is the
data ADR 0201 brings in P5, so P5 extends the table instead of replacing it.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0309

### D4. How are foreign dividends paid and counted?

Options:

- A. At E4 each country pays f_c,g of each positive enterprise payout in goods 1 to 6 to one world
  dividend account, which splits its total over the parent countries' pools by the vectors
  (exact `split`); the pools then pay their pops by wealth share in the same tick. Foreign
  dividends are income (in Y, the transfer base), a new flow, and the account is opened only
  where some f is nonzero, so a world without them keeps its ledger and hashes.
- B. Through the transfer account of ADR 0100, as transfers (not income).

Recommendation: A, because they are primary income in the current account the transfers are
built from: with them, a country's modelled trade balance is its current account less its net
dividends, as in the data; as transfers they would leave income at GDP.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0310

## Spec changes

- 7.2: s_L per (country, good) from SEA, VA-weighted mean equal to PWT.
- 8.7, 9.1: the profit pool is paid by wealth share; a foreign-owned share abroad by AAMNE
  parents.
- 16: SEA, WID and AAMNE in the source manifest.
- Econ SPEC: Pack additions (`econ_labour_shares.arrow`, `econ_wealth.arrow`,
  `econ_foreign.arrow`); Production (s_L per enterprise); Income; Calibration (base point).

## Tests

- `labour_share_va_weighted_mean_equals_pwt` (1e-12), `missing_source_falls_back_with_flag`
  (`crates/ceteris-pack/tests/factors.rs`).
- `dividend_shares_integrate_wealth_over_income`, `foreign_dividends_net_to_zero_worldwide`
  (`p2_properties.rs`); every pop of a province without traded output keeps a positive dividend
  share.
- ADR 0104's top-10% income share against WID within 5 points is reported at C6 on the rebuilt
  pack, not asserted here (it needs the pack).

## Consequences

- Without the three tables (the fixture, the installed pack) every hash is unchanged.
- As built (C4, part 1): labour shares and dividends by wealth in the simulator, export, GE
  reference, calibration and pack builder (columns only where SEA and WID are in the fetch
  cache); foreign dividends (ADRs 0309, 0310) follow.
- The installed pack needs a rebuild (C6): w0, the profit pools, A and the balance of RFC 0287 D1
  all move.
