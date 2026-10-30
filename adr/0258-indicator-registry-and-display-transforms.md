# 0258. Indicator registry, display transforms and the headline set

Status: proposed
Date: 2026-10-30
Design sections: 14.1, 20.1, 20.2; ADRs 0065, 0098, 0137, 0155, 0161, 0188, 0191, 0212, 0238, 0239, 0240, 0241
Phase: P3 (registry, numeraire transforms, headline set); P5a (currency and PPP transforms, money rows); 2.0+ (source stage, seat dashboards, objectives)

## Context

The owner asked which economic indicators the app shows, whether values switch between nominal,
real, per capita and PPP, which KPIs a player watches, and whether those are imposed or chosen
from an open set.

Already decided:

- DESIGN 20.1 lists economy map modes (GDP per capita, sector shares, wages, price of a good,
  price gap, inventories, market access) and finance modes (regimes, spreads, bank stress, capital
  flows). The UI catalogue (2026-10-29) adds 1.0 modes for inflation, policy rate, exchange rate,
  public debt, reserve cover, credit gap, labour share and others.
- Owner, 2026-10-28: tooltips and tables default to nominal local currency, with real and per head
  a toggle away. The mocked unit chip cycles nominal, real, per head and hours of local median work.
- 0137: every number carries a unit; chart subtitles give units.
- 0155: every series has a window toggle (month, year, decade, run); peer radar shows "headline
  series" as percentiles against a saved peer set. The headline series are never listed.
- 0188: one query language with checked units, `rank`, `pct(column, peers)` and time qualifiers;
  monitors are saved tables with windowed columns.
- 0098: country price levels at t = 0 come from ICP, so PPP is defined at the start.
- 0065 and 0238: the published-statistics layer and revisions are 2.0+. In 1.0 everyone reads
  true state.
- 0161 and 0239: objectives and scores belong to seats, which start at 2.0-alpha.

Missing: a list of what counts as an indicator, how a value aggregates from provinces to states
and the world, and definitions of real, PPP after t = 0, per capita, index and growth. Without
them each panel would invent its own, and 0188's checked units would have no unit system to
check against.

## Options

1. A fixed set of curated indicators, each hard-coded with its units.
2. An open registry of base indicators plus typed transforms that compose, with a curated
   headline set as the default layout.

## Decision

Option 2. Indicators and transforms are presentation: nothing here writes state.

### Base indicators

A registry entry holds: id, definition over state, dimension (money flow, money stock, quantity
of a good, people, hours, rate, ratio, index), stock or flow, subject grains (cell, province,
state, currency area, region, world; good, sector, pop group), aggregation rule (sum for
extensive; weighted mean with a named weight for intensive; none), phase, and a model or data
badge. Flows are stored per tick and shown annualized unless the window says otherwise.

Each system adds its indicators in the same task that adds the state (21.2 rule 3). Mods add
indicators through the same registry.

### Transforms

Typed operators, applied in a fixed stage order. A transform not valid for the dimension is
rejected by the unit checker, not hidden.

| Stage | Transforms | Valid on |
| --- | --- | --- |
| Valuation | nominal; real (chain-linked Fisher volume, reference year default the run's start year); deflator | money |
| Currency | local; another currency at market rate; international dollars (PPP) | money, from P5a |
| Scale | per capita, per worker, per working-age person; share of GDP, of parent aggregate, of world; hours of local median work | extensive quantities |
| Time | level; change; growth (m/m, y/y, annualized, CAGR over window); index (base tick = 100); rolling mean | all; shares and rates change in points, not percent |
| Compare | difference or ratio vs a branch; percentile vs peer set; rank | all |

- **Real.** Volumes come from the model's own prices and quantities, so deflators are exact,
  not estimated. GDP deflator, CPI per pop group (the PIGL basket of 0021) and a household
  cost-of-living index are registry entries. Chained components do not sum to the chained total;
  tables say so.
- **PPP.** Recomputed each simulated year by GEKS-Fisher over states' final-expenditure baskets
  at simulated prices, US dollar as numeraire. At t = 0 it reproduces the 0098 price levels
  (test). Before P5a there is one numeraire money, so the currency stage is identity.
- **Reference dates are ticks.** "Start year", "fork tick" and "base tick" are relative, never a
  literal 2000, so the 0241 relabel test holds for every view.
- **Grammar.** A composed indicator has one text form, which is also a 0188 query column and a
  monitor column: `gdp | real | ppp | per_capita | yoy`. Saved views, pins and dashboards store
  that text in the run (owner decision 2026-10-28).
- **Determinism.** Transforms are pure functions with fixed summation order. A `:when` rule edit
  (0188) that reads an indicator replays bit for bit.
- **Retention.** Transforms read 0212 history. The headline set is retained monthly for every
  branch, as if it were a declared outcome metric (14.1); other series recompute from snapshots.

### Default units

- One subject (tooltip, country panel, ledger): nominal local currency, as decided 2026-10-28.
- Many subjects (map, ranking, peer radar, region and world aggregates): a comparable unit.
  Money levels default to real PPP international dollars at start-year prices; trade and capital
  flows to US dollars at market rates; prices of a good to US dollars per unit. The unit chip shows
  the active unit and cycles it.

### Headline set (default dashboard)

A curated default layout, not a restriction: any slot can be swapped for any registry expression,
and the layout is a saved view. Rows appear when their system lands.

| Group | State dashboard | Phase |
| --- | --- | --- |
| Output | real GDP growth y/y; GDP per capita, real PPP; real household consumption per capita | P3 |
| Labour | unemployment rate; real median wage growth y/y; employment rate | P3 |
| Prices | CPI inflation y/y; price level vs US (PPP over market rate) | P3, P5a |
| External | trade balance and current account, % of GDP; exchange rate vs USD, % change since base | P3, P5a |
| Money | policy rate; reserve cover, months of imports | P5a |
| Public | fiscal balance and public debt, % of GDP; sovereign spread | P5a, P5b |
| Distribution | Gini of disposable income; material welfare W (0191) | P3, 1.x |
| Risk | nearest cliff (0155 headroom, smallest across switches) | P5a |

World dashboard: world real GDP growth, world trade over GDP, real food and fuel price indices,
world inflation, emissions (1.1). Province card: output per capita, wage, unemployment, local CPI
vs state, market access, self-sufficiency in food and fuel. These are 0155's headline series and
the default peer-radar axes.

### 1.0 and 2.0+

- **1.0 (no seats, 0239).** Every value is true state. The user can view, map, chart, monitor and
  rank any indicator with any valid transform chain. Nothing is scored, and no indicator feeds
  any agent: model rules (0210) read their own state, not the registry.
- **2.0+ seat play** adds, without changing the registry:
  - A source stage before valuation: true, published first release, latest vintage, independent
    signal (night lights, mirror trade). In seat play the default and only source is published
    (0065, 0238 under S21); the true source is shown in observer and lab modes and in the run's
    post-mortem.
  - Availability follows the seat state's statistical office: a low-capacity state publishes
    fewer series, later and less often, so some cells read "not published".
  - Seat default dashboards by seat type (head of government: fiscal and growth; central bank:
    inflation, gap, rate, reserves; commodity trader: freight and prices) built from the same set.
  - Objectives and scores (0161, 0129) are written as registry expressions, so a score is one
    more indicator with a transform chain.

## Consequences

- One definition per number; panels, maps, monitors, queries and scores agree.
- The unit checker of 0188 gets its dimension system.
- Cost: about 1.5-2.5 owner-weeks in P3 (registry, checker, numeraire transforms, headline set,
  unit chip), 0.5-1 in P5a (currency stage, GEKS PPP). Part of the P3 monitor and query work in
  ADR 0248's estimate.
- Amends 0155 (defines the headline series) and 0212 (headline set retained for all branches).
- On acceptance DESIGN 20.2 gains an "Indicators and units" item pointing here.

## Tests

- Rendering any view or evaluating any indicator changes no state hash.
- Chained real GDP equals nominal GDP in the reference year; a world with constant prices has
  real = nominal everywhere.
- GEKS PPP at t = 0 reproduces 0098 price levels within 1e-9; it is transitive across any three
  states.
- Extensive indicators summed over provinces equal the state value; per capita of a sum equals
  the population-weighted mean of per-capita values.
- An invalid chain (`unemployment | per_capita`, `inflation | real`) fails the unit checker.
- Relabelling the start year (0241) leaves every indicator value unchanged.
- A `:when` rule reading an indicator replays bit for bit across machines.
