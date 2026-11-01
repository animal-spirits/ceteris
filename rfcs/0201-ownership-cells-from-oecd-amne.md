# 0201. Ownership cells from OECD Analytical AMNE

Status: proposed
Date: 2026-10-28
Design sections: 3.2, 7.4, 7.5, 9.1, 15.4; ADRs 0041, 0104, 0171
Phase: P5

## Context

DESIGN 3.2 aggregates enterprises per (province, sector, ownership type: domestic private,
state, foreign), but the engine has one enterprise per (province, good). ADR 0104 pays a fixed
foreign-owned share of profits abroad, ADR 0041 shifts foreign affiliates' profits to havens,
and ADR 0171 gives state firms their own rule. All three assume an ownership dimension that
does not exist. Foreign affiliates are also more productive than domestic firms: they come from
the top of the productivity distribution (Helpman, Melitz and Yeaple 2004).

The OECD Analytical AMNE database splits every row and column of the ICIO into domestic-owned
and foreign-owned firms, with the parent country, for 76 economies plus the rest of the world,
41 industries and the years 2000-2019. It sits on the ICIO grid Ceteris already uses, starts in
the start year, and is CC BY 4.0 (OECD open by default, July 2024).
Deep dive: https://claude.ai/artifact/J7jLsKygA6qyLfLmvNXhNa

## Options

1. Status quo: ownership as fixed shares in the income accounts (0104).
2. One cell per (province, sector, ownership, parent country).
3. Up to three cells per (province, sector) - domestic private, state, foreign - with the
   foreign cell carrying a parent-country share vector.

## Decision

Option 3. Option 2 multiplies cells by the number of parents for little gain.

- A cell splits only where its ownership type holds at least 5% of the (province, sector) VA in
  2000 (register entry). Domestic and foreign shares come from AAMNE; state shares from OECD
  state-owned enterprise data and the IMF Fiscal Monitor (April 2020) published tables.
- The foreign cell carries the shares of its top five parent countries plus the rest of the
  world. Dividends (0104), profit shifting (0041) and asset freezes route by these shares.
- Foreign cells sit higher in the productivity distribution: their productivity premium is the
  2000 ratio of foreign to domestic VA per worker (AAMNE VA, OECD AMNE employment). Catch-up
  (7.5) reads the foreign share, as DESIGN already states.
- The foreign share evolves through the 7.4 investment logit. Foreign owners see host provinces
  through an FDI barrier wedge from the OECD FDI Regulatory Restrictiveness Index.
- State cells follow ADR 0171. Nationalization and privatization (10.4, 2.0) move mass between
  cells of one (province, sector).
- This replaces 0104's fixed foreign share f_c,g with state that evolves.

## Migration

- Pack: AAMNE 2000 ownership shares by host, sector and parent; SOE shares; FDI RRI. Add to
  the source manifest (ADRs 0005, 0050).
- Econ: cell key gains an ownership field; per-province slices stay contiguous.
- Scorecard: foreign share of output by host and sector in 2019 as a 15.4 holdout target.

## Tests

- With every cell domestic and the premium at 1, hashes equal the single-cell path.
- Profits of a foreign cell reach the pops of its parent countries in proportion to the vector.
- 2000 foreign shares of output match AAMNE within rounding.

## Consequences

Cells grow by at most three times, and far less where ownership is concentrated. 0041, 0104 and
0171 get the cells they assume. FDI becomes a choice with a cost, and sanctions on foreign
assets have something to act on. Register: 2 entries (split threshold, FDI wedge scale).
