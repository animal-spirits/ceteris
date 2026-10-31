# 0278. Energy scope for 1.0: grid zones, plant capacity, state energy cells, frozen energy policy, gas markets

Status: accepted (owner chose the options, 2026-10-31)
Date: 2026-10-31
Design sections: 5.3, 7.1, 7.7, 9.6, 13.1, 16.1; amends ADRs 0028, 0034, 0035, 0095, 0201, 0207, 0241

## Context

Electricity is good 7 (ICIO D and E) in the ten-goods pack and D under ADR 0271: a local good
cleared per province in the same-tick chain, with national VA split by population. ADR 0034 runs
residual load duration curves per state or market zone in P8 and asked P2 to carry generation
capacity by class per state; the pack does not. ADR 0095 requires a grid market before P8, and no
phase schedules it.

ADR 0207 moved ADRs 0201 (ownership cells) and 0171 (SOE behaviour) to 1.x, so in 1.0 the profits
of state oil companies and utilities go to pops by population (ADR 0089), and the state sees them
only through taxes (9.6). It also kept only the crude part of ADR 0028 in 1.0, so gas prices move
with crude and 0034's 2021-22 gas test cannot run.

ADR 0241 makes default runs calendar-blind, and ADR 0242 holds trade policy at its 2000 values
until 2.0. Nothing says what sets carbon prices, fuel taxes and fuel subsidies after t0.

## Options

1. Grid: (a) market zones with interconnector capacity; (b) trade D within a state only; (c) keep
   province-local.
2. Plants: (a) GEM plants per province by class; (b) capacity per state only; (c) named plants
   (ADR 0206) in 1.0.
3. State rents: (a) a royalty per country calibrated to the 2000 government take; (b) state
   ownership cells for energy goods only; (c) leave to 1.x.
4. Energy policy: (a) 2000 fuel tax and subsidy wedges held, carbon price zero; (b) a fitted policy
   rule; (c) the replay set only.
5. Gas: (a) split from crude in P8 as a high-friction good without capacity limits; (b) split with
   ADR 0028's full gas markets; (c) keep merged to 1.x.

## Decision

1(a), 2(a), 3(b), 4(a), 5(b) (owner, 2026-10-31).

- **Grid zones (P8, before RLDC).** A pack table assigns each province to one electricity zone;
  by default a zone is a state, and listed multi-state zones (for example a coupled continental
  European zone) are pack data. Within a zone, D clears in one pool: the same-tick chain runs over
  the zone, not the province, with no friction between its provinces. Between zones, D flows up to
  each interconnector's capacity (MW times hours in the month), toward the higher price; the flow
  is set before the zone pools clear, and congestion rent goes to the interconnector's owner. The
  RLDC of ADR 0034 runs per zone. This is the grid market ADR 0095 asked for. Zone and
  interconnector sources are chosen and their licences checked under ADR 0005 when the table is
  built.
- **Plant capacity (P4 pack, with ADR 0271 downscaling).** Generation capacity in MW per province
  and ADR 0034 class comes from the Global Energy Monitor power trackers (CC BY 4.0, in the
  manifest): plants with a start year up to 2000 and not retired before it, placed by coordinates.
  The gap to national 2000 capacity (tracker size cutoffs) is spread over the country's tracked
  plants of that class, or by population where a class has none. D's province VA follows capacity
  weighted by class capacity factor, not population; E keeps population. On the ten-goods pack
  good 7 is unchanged. The table replaces 0034's P2 line (capacity per state), and P8 sums it to
  zones.
- **State energy cells (P5a).** ADR 0201's ownership split applies in 1.0 to energy goods only:
  crude oil and gas extraction (B06; good 1 on the ten-goods pack), and D. State shares come from
  0201's sources (OECD SOE data, IMF Fiscal Monitor April 2020 tables), with its 5% VA rule. State
  cells' profits go to the state's account (9.6); their investment follows the 7.4 logit like any
  owner. ADR 0171's objective and soft budget stay in 1.x, so state cells behave as private ones.
  No foreign cells and no other goods are split in 1.0.
- **Energy policy held at t0 (P5a wedges, P8 carbon).** Fuel excises and fuel subsidies are per
  unit wedges on goods B06, C19 and D at their 2000 rates per state, held for the run like trade
  policy (ADR 0242). A subsidy is a fiscal cost at the wedge times quantity; ADR 0084's ceilings
  and parallel markets stay in 1.x. Carbon prices, ETS caps and CBAM start at zero. All of them can
  be changed by a lab edit (14.2) or by the replay set (ADR 0241, which can carry the EU ETS from
  2005); ADR 0035's instruments act on levelized costs as written. Sources for the 2000 rates are
  chosen under ADR 0005.
- **Gas markets (P8, in 1.0).** ADR 0028's gas part comes back into 1.0. B06 splits into crude
  oil and natural gas (a physical split, as ADR 0271 leaves to its own ADR). Gas clears in
  regional markets linked by pipelines and LNG shipping with capacity limits; liquefaction,
  regasification and pipeline throughput are capital stocks (ADR 0204), and the pipeline mode of
  5.3 is built. 2000 capacities come from the Global Energy Monitor gas trackers. ADR 0034's
  2021-22 gas shock test becomes a 1.0 test.

## Consequences

- ADR 0207's table changes: 0201 (energy cells only) moves into P5a, and 0028 (gas, LNG,
  pipelines) moves into P8 whole. Neither is in ADR 0248's estimate; P5a and P8 grow, to be
  estimated in their plans.
- The pack gains a zone table, an interconnector table, a plant capacity table and gas capacity
  tables; D's province VA moves on the 50-goods pack, so its goldens move with ADR 0271's.
- Petrostate budgets carry their 2000 state share of rents from P5a; utility profits in France,
  China and other state-heavy systems go to the state.
- Default runs keep 2000 fuel taxes and subsidies forever and have no carbon price, so the energy
  transition in a default run comes from learning curves alone (H12's no-pricing branch). Carbon
  pricing is a lab question.
- Goods become 51 in P8 (50 plus gas from B06).
