# 0203. Named firms: view, granular opt-in, political actors

Status: proposed
Date: 2026-10-28
Design sections: 2.5, 10.4, 15.7, 20.2, 23.7; ADRs 0026, 0030, 0130, 0136, 0186, 0189
Phase: view 1.x (minor); granular firms extended profile; political actors P6 (2.0)

## Context

Players expect firms with names: Victoria 3's companies (1.5) and charters, headquarters and
executives (1.9) are read as named, ownable, political things. In Ceteris firms are a Pareto mass
per cell (0026), which carries no names. ADR 0026 also notes that granular shocks from the
largest firms (Gabaix 2011; di Giovanni, Levchenko and Mejean 2014) are absent, Atkeson-Burstein
markups (an option in 0030) need explicit top firms, ADR 0130 lists firms as seats, and DESIGN
23.7 has conglomerates as lobbies. Victoria 3 layers companies on top of the building economy as
bonuses; Ceteris must not, since a named firm that adds output would break the cell accounts.
Deep dive: https://claude.ai/artifact/J7jLsKygA6qyLfLmvNXhNa

## Options

1. No named firms.
2. Names only, as labels on the tail of each cell.
3. Explicit top firms in the lab default.
4. Staged: labels first, explicit top firms as an opt-in, political roles with 2.0.

## Decision

Option 4. Option 3 adds agents that 15.7 excludes until they earn their place.

- **View (1.x minor).** Wikidata (CC0) supplies the real firms of 2000 per country and sector:
  name, headquarters, inception, parent and industry. The firm of rank r in a cell shows the
  expected r-th order statistic of the cell's Pareto tail. Reported 2000 revenues are a check
  on the tail, never an input. Named firms change no state, so this is a minor under 0136.
- **Granular firms (extended profile).** The top N firms per cell become explicit agents with
  idiosyncratic productivity shocks, and the closed-form tail is truncated above them, so cell
  aggregates stay consistent. Atkeson-Burstein markups and a firm seat (0130) build on this
  layer. It enters the lab only if it improves out-of-sample GDP volatility and its other
  scorecard targets (15.7), and then only at a major.
- **Political actors (2.0).** Large firms and business groups become ADR 0189 organizations of
  the lobby kind, with resources from tail profits and leaders from the 0186 business-owner
  pool. Protection for sale (Grossman and Helpman 1994) and state capture act through the 2.0
  policy levers. Executives with causal traits follow 0136's 4.0 rule for characters.
- A named firm is always part of a cell. It never adds output, a modifier or a bonus.
- Proprietary firm data (Orbis, Compustat) may feed the extended profile only through a
  user-side importer under the user's own licence. The lab and the release pack use open
  sources only.

## Migration

- 1.x: a name table per (country, sector) in the pack from Wikidata; inspector and chronicle
  read it.
- Extended: an explicit-firm array per cell with tail truncation; a switch, default off.
- 2.0: the lobby kind of 0189 reads firm profits.

## Tests

- View: with names loaded or absent, state hashes are equal.
- Granular: with N = 0, hashes equal the closed-form path; the sum of explicit firms and the
  truncated tail equals the cell aggregate every tick.
- The top-firm sales of 2000 sit inside the tail's order-statistic bands for most cells (Zipf
  check).

## Consequences

The map, inspector and chronicle can say "Toyota" without the model pretending to know Toyota's
books. Granular volatility and oligopoly pricing have a path in, gated by ablation. Firms join
politics in 2.0 through an existing primitive. Register: none in the lab; N per cell in the
extended profile.
