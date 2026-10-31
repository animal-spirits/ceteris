# 0269. P04 mode cost ratios borrowed from freight rates

Status: accepted (owner, 2026-10-31)
Date: 2026-10-31
Design sections: 5.3, 17.1 (P04), 21 (pre-P3); amends ADR 0022 (direct estimation of P04);
ADRs 0247, 0257, 0264

## Context

P04a-d are the costs per tonne-km of road, rail, sea and air (geo SPEC, Parameters): 0.10, 0.04,
0.008 and 0.80 USD, placeholders with only the ordering imposed (DESIGN 5.3). ADR 0022 estimates
P04 by PPML gravity on 2000 BACI along S05 paths. The estimate is not identified: rail/road 52.6
(s.e. 24.1) and sea/road 115.3 (46.3) on the split pack, 36.6 (19.1) and 84.6 (39.5) on the four
theta classes, air on 3 country pairs (geo SPEC; ADR 0022, Amendment 2026-10-29). Country-pair
flows see only the least-cost envelope of legs, so they say little about the cost of a mode
that is rarely on it; the fit puts rail and sea above road, against every price source.

The literature identifies mode costs from mode-specific prices or moments: freight rates,
Donaldson's (2018) salt price gaps along the network, Allen and Arkolakis's (2014) mode shares.
The owner ruled (2026-10-31, "B a a a", ruling 3; C7 groundwork section 3): borrow the ratios
from BTS 2000 and Donaldson (2018) now, registered with ranges for P9; the mode-share fit goes
to P9 with ADR 0264's option F, one refit of the transport form instead of two.

### Sources, as read

**BTS, National Transportation Statistics, Table 3-21**, Average freight revenue per ton-mile,
current cents (archived edition of 3 February 2017; Table 3-17 in editions to 2010). 2000 values,
converted at 1.45997 tonne-km per ton-mile:

| Mode (BTS row) | cents per ton-mile | USD per tonne-km | per road |
|---|---|---|---|
| Truck (general freight common carriers, mostly LTL, note b) | 13.75 | 0.0942 | 1 |
| Class I rail | 2.26 | 0.0155 | 0.164 |
| Barge | 1.67 | 0.0114 | 0.121 |
| Air carrier, domestic (cargo and mail) | 73.85 | 0.5058 | 5.37 |

Over 1990 to 2004, the years with a truck value, rail/truck runs 0.164 to 0.208, barge/truck
0.103 to 0.133 and air/truck 3.87 to 5.37 (2000 is the air maximum). Revenue per ton-mile
mixes haul length and cargo: rail and barge carry bulk farther, so their ratios understate the
cost gap for like cargo, and the truck row is less-than-truckload, so road is high against
truckload rates and every ratio to it low.

**Donaldson (2018), AER 108(4-5)**, Table 2, p. 917: costs per unit distance relative to rail.
Historical freight rates (p. 905: rail 4 to 5, 2 to 4 and 1.5 to 3 times cheaper than road,
river and coast) at their midpoints road 4.5, river 3.0, coast 2.25 (column 1). The NLS estimate
from salt price gaps (column 2, preferred): road 2.375 [1.750, 10.000], river 2.250
[1.500, 6.250], coast 6.188 [5.875, 10.000], bootstrapped 95% intervals, India 1861 to 1930.
These replace the ranges in the C7 groundwork (4.5 to 7.9, 3 to 3.8, 2.25 to 3.9), which came
from working-paper slides and do not match the published table.

Donaldson's NLS ratios are full trade costs: they load the time, risk and information
advantages of rail onto the per-km cost (p. 916). The model prices those separately, in speed
(P04i-l) and handling time (P04n) valued through lambda T (S05). P04a-d are therefore the
freight-rate concept, which BTS measures for the 2000 world; Donaldson bounds how far full cost
ratios sit from freight-rate ratios. His coastal ratio is 19th-century coasting trade, not 2000
ocean shipping, and is not used for sea.

## Options

- **A. BTS 2000 ratios as values, ranges from BTS 1990 to 2004 and Donaldson; road scale kept at
  0.10, nothing fitted in C7.** Sourced, converges, honest about identification.
- **B. As A, with the road scale fitted by PPML** (one parameter). The flat likelihood that
  defeats the ratios also leaves the scale weakly identified once nu absorbs border costs
  (ADR 0092); the BTS truck level, 0.094, is within 6% of the prior already.
- **C. Keep the placeholders and the PPML diagnostic.** No source for the values; the PPML fit
  stays unconverged.

## Decision

Option A (owner ruling 3, 2026-10-31; nothing fitted is this ADR's proposal within it).

| ID | Mode | Ratio to road | Range for P9 | Source of value; of range | USD per tonne-km |
|---|---|---|---|---|---|
| P04a | Road | 1 (scale) | 0.094 to 0.10 USD | prior kept; BTS 2000 truck 0.094 | 0.10 |
| P04b | Rail | 0.164 | 0.10 to 0.57 | BTS 2000; Donaldson (2018) NLS interval for 1 / road, 1 / 10 to 1 / 1.75, which spans BTS 1990 to 2004 (0.164 to 0.208) and Donaldson's historical 0.20 to 0.25 | 0.0164 |
| P04c | Sea | 0.121 | 0.103 to 0.133, lower end open | BTS 2000 barge; BTS 1990 to 2004. Ocean rates sit below barge; no ocean source is in this register | 0.0121 |
| P04d | Air | 5.37 | 3.87 to 5.37 | BTS 2000; BTS 1990 to 2004 | 0.537 |

- The values replace P04a-d's placeholders in `TransportParams`. Circuity, speeds, handling
  fees and times, the canal fee and terrain (P04e-q) are unchanged. The ordering sea < rail <
  road < air of DESIGN 5.3 holds (0.0121, 0.0164, 0.10, 0.537).
- ADR 0247's bulk freight rate multiplies the new P04c, as it multiplied the old one. P154 is
  a ratio to P04c, so it is rescaled to keep class 3's sourced slope (see Revisions).
- The PPML estimate of ADR 0022 stays computed and recorded as a diagnostic; it no longer sets
  P04. The register class of P04 becomes "Literature (borrowed, ADR 0269); calibrated in P9".
- P9 replaces the borrowed ratios with a fit to mode-specific moments (Allen and Arkolakis
  2014's strategy: value shares by mode from US Census imports and Eurostat Comext, a new
  source under `sources.toml`), jointly with ADR 0264's log-distance leg cost and ADR 0022's
  joint refit, within the ranges above unless the fit gives reason to leave them.

## Consequences

- Against the placeholders rail costs fall to 0.41 times, sea rises to 1.51 times and air falls
  to 0.67 times per tonne-km; road is unchanged. Least-cost paths shift towards rail. Not yet
  measured: C7 recalibrates nu (ADR 0092) and the steady state on the new costs and records H2,
  openness and the ACR criterion after.
- Golden hashes move: `tests/golden/p1-geo.txt` (cost bits) and every econ golden that prices
  legs. The commit says why and the CHANGELOG carries `### BEHAVIOUR: P04 mode cost ratios
  borrowed from BTS 2000 (ADR 0269)`.
- No silent tuning (DESIGN 21.2, rule 5): the values come from the table above, not from a
  test; a later change to them is a register entry with its justification.
- Sea's lower end is open: ocean freight per tonne-km is below the barge rate and the register
  has no source for it. At 0.121 long ocean legs are likely overpriced; the direction of the
  error is known, its size is not. P9 names the source.
- Under ADR 0257 tier 2 the mode costs become "no moment identifies it, borrowed and recorded",
  not an open fit.
- The geo SPEC (Parameters), DESIGN 17.1 (P04 row) and ADR 0022 (Amendment) state the values
  and the class with C7's code.

## Revisions (owner ruling 2026-10-31, "Go with your recommendations")

Two findings from the ocean freight research for C7, adopted by the owner:

- **Sea range.** Ocean rates bound P04c on both sides: register the P9 range for sea as 0.05 to
  0.20 per road, in place of 0.103 to 0.133 with the lower end open. 0.121 sits inside it. The
  research's sources are entered in the register with the ruling.
- **P154 rescaled.** ADR 0247 sets class 3's sea multiplier to its sourced slope over P04c,
  2.05e-4 / 0.008 = 0.026. Kept at 0.026 on the new P04c, class 3's sea cost per tonne-km
  becomes 3.1e-4, 1.51 times the sourced 2.05e-4. P154 class 3 becomes 2.05e-4 / 0.0121 =
  0.017, which keeps the slope; P155 (0.40) is a ratio to P04m, which does not change.
