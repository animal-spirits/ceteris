# Population SPEC

Status: draft (P4 Branch 0; nothing here is built; ADRs 0031, 0032, 0033, 0074, 0105, 0109 and 0167 accepted by RFC 0335)
Phase: P4
Design sections: 6.1 to 6.3, 6.5, 8.6, 8.7, 15.4, 17.1, 19.2, 19.4; ADRs 0031, 0032, 0033, 0074, 0095, 0105, 0109, 0167, 0236, 0241; plan `specs/p4/PLAN.md`

## Purpose

`ceteris-pop` (a module of `ceteris-econ` until the first branch decides otherwise) owns people:
how many there are by cohort cell, how they age, die, are born, learn, switch sector, and move
between provinces and countries. It supplies labour supply by skill to `specs/econ/SPEC.md`
(Labour) and receives wages, prices, spending and fiscal levers from it. It does not own
ethnicity, religion, ideology or grievance (P6), pensions and fiscal rules (P5a), conflict and
climate hazards (P7, P8), or the moving-equilibrium calibration (Branch F, `specs/econ/SPEC.md`
Calibration). Population is conserved by construction: every flow is a pair of integer
transfers between cells.

## State components

Cell key `c = (province p, age band a, sex s, education e)`. Age bands are five years, `a = 0..20`
(0-4 to 95-99) plus `a = 21` for 100 and over: 22 bands. Education `e` in {none or in school,
below secondary, secondary, tertiary}: 4 levels (attainment is carried by the cohort for life
once reached, ADR 0032). Counts are `u64` persons, never `f64`, so conservation is exact.

| Component | Entity | Type | Units | Initialized from |
|---|---|---|---|---|
| Cohort count N | cell (p, a, s, e) | `u64` | persons | WIC reconstruction (`wic-reconstruction`) for age, sex and education by country, downscaled to provinces by the econ pack's population shares; UN WPP 2000 for totals; largest-remainder apportionment so totals match WPP exactly |
| Age residue | cell | `u16` | months into band (0..60) | zero; ageing moves a fixed 1/60 of the band each month, whole persons by apportionment (see Equations, ageing) |
| Brass level alpha | (country, education) | `f64` | logit units | WCDE life tables, one level per education group; per province offset zero |
| Brass slope beta | country | `f64` | | WCDE life tables |
| Fertility quantum TFR | (province, education) | `f64` | births per woman | ADR 0033/0032 schedule, WPP 2000 |
| Fertility tempo mean age mu | (province, education) | `f64` | years | WPP mean age at childbearing 2000 |
| Progression ratio rho_e | (country, sex, cohort band 15..34) | `f64` | share | WCDE schedules |
| Headship rate h | (country, age band, sex, education) | `f64` | households per person | census and UN household data, WCDE for education |
| Cause-group alpha_k (Branch B) | (country, cause group) | `f64` | logit units | research pack only (NC data, ADR 0050) |
| Prevalence by disease (Branch B2) | (province, age band, disease) | `f64` | share | UNAIDS, MAP, WHO TB (ADR 0182) |
| Skill and sector assignment (Branch C) | (province, sector, skill) | `u64` | persons | calibration table; skill high = tertiary |
| Settlement class split (Branch D3) | (province, class) | `u64` | persons | GHSL, rural/town/city |
| Admission account (Branch D2) | country | `u64` | persons this year | UN DESA stocks 2000-2005 |
| Diaspora stock | (origin country, destination country) | `u64` | persons | UN DESA bilateral stocks |

Derived, not stored: working-age population, labour force (age-band participation), households,
effective labour supply per province and skill (feeds econ's L^s).

**Bounds (ADR 0236).** With 4,594 provinces: 4,594 x 22 x 2 x 4 = 808,544 cells, 6.5 MB as `u64`
(three per-cell scratch arrays for flows double it). The size is constant to 2150: cells are
fixed, only counts change. Migration works on cell-by-destination flows over a sparse choice set
of at most `K_d` destinations (provinces within the cost radius, the largest cities, foreign
gateways, one pooled nest per country; `K_d <= 64`, to be confirmed by the D1 spike): flow
scratch is at most 808,544 x 64 x 8 B = 414 MB if materialized, so flows are computed per
(origin province, education) and consumed immediately, never stored: working set one origin's
row, about 64 x 22 x 2 floats. Snapshots add 6.5 MB per frame before compression.

## Equations

Register codes P162 to P170 are those of RFC 0335 (the ADRs wrote them provisionally as P60 to P65, P126 and P127; ADR 0167's labour loss is P169, ADR 0109's z is P170).

**Ageing (Branch A1).** Yearly cadence for band transfers, applied once per year on tick
`t mod 12 = 0` of the cohort calendar (the cohort tick count, not a date: ADR 0241). Persons in
band `a` move to `a + 1` for `a < 21`; band 21 is absorbing. Exactly `N_c / 5` per year move out of
band `a` for a five-year band at uniform age density; the integer transfer is
`floor(N_c / 5)` plus the largest-remainder share of residues, ties broken by cell index. The
age residue stores the unmoved fraction so ageing is not biased downward by repeated floors.

**Mortality (A2).** Per band `a`, sex `s`, education `e`, province `p`:

    logit(1 - l_x) = alpha_{c,e,t} + beta * logit(1 - l_x^std)

`l_x^std` is the WCDE standard schedule. alpha responds to real income (Preston curve, ADR 0033
option 2), health spending and, in Branch B, cause-group components that sum:
`alpha = sum_k alpha_k` plus additive hazards (epidemics, conflict, famine, heat) that are zero
until P7 and P8. Deaths are `D_c = round(N_c * q_a)` by Hamilton apportionment over the cell's
province-age row so that integer rounding does not accumulate bias; every death removes one from
`N_c` and adds nothing elsewhere, so conservation is `births - deaths + net migration`.

**Fertility with tempo (A2, ADR 0032).** Quantum:

    TFR = TFR^min + (TFR^max - TFR^min) * sigma(gamma_0 - gamma_e e^f - gamma_y ln y - gamma_u urban + gamma_m child mortality)

(spec 6.2). Age-specific rates follow a parametric schedule (Hadwiger or Gamma) whose mean age
`mu` is shifted by tempo (P165: responds to female education and housing cost). Births in period
`= round(sum_a ASFR_a(mu) N_{p,a,female,e})` by apportionment; newborns enter band 0, education
none, sex by the biological ratio 1.05 male per female (data constant, not free). The test fixes
a postponement and checks period TFR falls while cohort completed fertility does not
(Bongaarts and Feeney 1998).

**Education progression (A3).** For cohort bands 15..34 and each transition `e -> e + 1`:
`progress_e = rho_e * clamp(f(spending, income))`, with `rho_e` the WCDE schedule prior and
`f` the response of P164 to education spending per student and household income (ADR 0032).
Attainment above 34 is fixed. Headship: `households_p = sum h_{a,s,e} N_c`, used by housing
demand (8.7) and consumption's household size.

**Cause groups and disease (Branch B).** `alpha_{c,e,t} = alpha_comm + alpha_ncd + alpha_inj`
(ADR 0033); communicable component trends with income, health spending, urbanization;
non-communicable with income and ageing; injuries with conflict and roads. Disease state
(ADR 0167): prevalence `x_{p,a,d}` evolves by compartmental incidence-and-exit with coefficients
fitted offline per region; disease deaths replace the part of the communicable component they
carry (no double counting: test `zero_prevalence_equals_the_cause_group_path`). Untreated
prevalence lowers effective labour per worker.

**Skills and switching (Branch C, ADR 0105).** Two skill groups, high = tertiary, low = the rest:

    L = ( alpha L_h^{(s-1)/s} + (1 - alpha) L_l^{(s-1)/s} )^{s/(s-1)},  s = 1.6

alpha per country to the 2000 premium. Each month a share `m` of workers in each (province,
sector, skill) re-chooses sector by logit over sectors in the province on expected wages, with
dispersion `omega_s`; flows are integer by Hamilton apportionment over the logit shares. Both
switches off reproduce the current model exactly. P42's `e_k` weights education inside a group.

**Migration (Branch D, ADRs 0109, 0074, spec 6.5).** Monthly share `mu_move` of each (origin
province, education, age band, sex) reconsiders; destination shares by logit:

    mu_od = exp((beta V_d - m_od)/nu) / sum_k exp((beta V_k - m_ok)/nu)

`V` by the Caliendo-Dvorkin-Parro recursion, one update per tick. International admissions: if
`sum_o A_od > Q_d / 12` the destination rations proportionally to `A_od` weighted by a
diaspora preference `(1 + chi * stock_share_od)`; rejected applicants stay, and a fraction `z`
of them migrate irregularly at extra cost. Settlement classes (D3): within a province, class
shares move by the same logit with short distances; cities grow with agglomeration net of
congestion (P167), slum share rises when housing supply lags (P168). Flows are integer by
Hamilton over `(o, d)` rows; each person moves once per tick (no chain moves).

**Structural change (Branch E, ADR 0031).** The sector-class catch-up factor multiplies the
catch-up rate of ADR 0108 (P162); informal to formal transitions scale with enforcement
capacity (P163). Lives in `ceteris-econ`, not here; listed for the test plan.

## System order

System IDs 220 to 229 are reserved for population (econ holds 201 to 210). Cadence is
monthly unless noted. The tick pipeline position follows the econ order (`specs/econ/SPEC.md`):
population systems run after E4 (income is known) and before the next tick's E1.

| System ID | Name | Cadence | Reads | Writes | Position |
|---|---|---|---|---|---|
| 220 | age | yearly | N, age residue | N, residue | after E4, before P2xx |
| 221 | mortality | monthly | N, alpha, health spending, income | N (deaths), death counts | after 220 |
| 222 | fertility | monthly | female N, TFR, tempo | N (births) | after 221 |
| 223 | education | yearly | N, spending, income, rho | N (moves between education) | after 222 |
| 224 | disease (B2) | monthly | prevalence, coverage | prevalence, hazards read by 221 | before 221 |
| 225 | sector switch (C2) | monthly | wages, shares, omega | sector assignment | after E4 |
| 226 | internal migration (D1, D3) | monthly | V, costs, N | N (moves) | after 225 |
| 227 | international admissions (D2) | monthly | A_od, Q_d, diaspora | N, diaspora stock | after 226 |
| 228 | households | monthly | N, h | derived households read by econ E3 | after 227 |
| 229 | pop assert | monthly | all | | last |

RNG: no draws. Every rounding is by deterministic Hamilton apportionment, tie by cell index, so
results are bit-identical at any thread count. A system that later needs randomness takes its
stream by system ID (ADR 0241 forbids reading the date, not drawing).

## Invariants

| Invariant | Statement | Tolerance | Checked where |
|---|---|---|---|
| Conservation by cohort cell | `N_{t+1} = N_t + births - deaths + inflow - outflow` per cell and in aggregate, in integers | exact | 229 each tick; tests `population_is_conserved_by_cohort_cell`, `admissions_never_exceed_the_cap_and_shares_sum_to_one` |
| No negative cell | all `N_c >= 0` | exact | 229 |
| Ageing once a year | each person changes band at most once per year | exact | `ageing_shifts_bands_once_per_year` |
| Disease does not double count | deaths from disease plus cause-group deaths equal Brass deaths | exact | `zero_prevalence_equals_the_cause_group_path` |
| Switch-off equals status quo | each mechanism's switch off reproduces the golden hash | bit | `<mechanism>_off_equals_status_quo`, one per mechanism |
| Thread-count independence | state and journal hashes equal at 1, 3 and all threads with `fixed_sum` | bit | `tests/golden.rs` |
| Population 2000-2010 | fertility, mortality, urbanization within the gate tolerances of Branch 0 RFC | per RFC D2 | Branch H |
| WCDE SSP2 nesting | with WCDE scenario inputs fixed, population by age, sex and education to 2100 within tolerance | per RFC D2 | `wcde_ssp2_nesting` |

## Parameters

| ID | Name | Prior / value | Class |
|---|---|---|---|
| P164 | Education progression response to spending and income | WCDE prior | free, calibrated |
| P165 | Fertility tempo response to female education and housing cost | Bongaarts-Feeney range | free |
| P162, P163 | Sector-class catch-up factor; formalization scaled by enforcement | ADR 0031 priors | literature, free |
| P166 | Deaths-of-despair scenario hazard | 0 | scenario |
| P167, P168 | Congestion cost; slum share rise | ADR 0074 | free |
| s | Skill CES elasticity | 1.6 | literature (Katz and Murphy 1992; Ciccone and Peri 2005) |
| m, omega_s | Sector-switch share and dispersion | Artuc, Chaudhuri and McLaren (2010), monthly | literature |
| Q_d | Admission cap per country | UN DESA 2000-2005 mean | data |
| z | Irregular share of rejected applicants | 0 to 0.2 | calibrated to Abel and Cohen |
| nu, beta, m_od | Migration dispersion, discount, moving costs | calibrated to 2000-2010 bilateral stocks | calibrated |


## Equilibrium nesting

With mortality, fertility, migration, skills, switching and settlement switches off, counts are
static and the economy equals the P2 steady state (every golden holds). With WCDE scenario
inputs fixed, the module reproduces WCDE SSP2 population by age, sex and education to 2100.
With infinite admission caps, international flows equal the pure logit. With zero endemic
prevalence, mortality equals the cause-group path; with added hazards at zero, the Brass level.

## Test plan

- Unit: apportionment (Hamilton) preserves totals and is order-independent; one test per equation
  above, named as in `specs/p4/PLAN.md`.
- Property: conservation and non-negativity over 100 simulated years at 1, 3, all threads.
- Golden: pop-on 120-tick scenario with its hash file under `tests/golden/`; one golden move per
  branch, each a reviewed commit that says re-bless and why.
- Benchmark: tick cost of each system against 19.2, reported before each branch merges; no
  `HashMap` reaching state; flows computed per origin row, not stored (Bounds).
- Data: WIC, WPP, WCDE, DESA are open or CC BY; WHO GHE and IHME stay in the research pack
  (ADR 0050, issue #231).

## Open questions

1. Integer rounding: Hamilton apportionment per row versus a stochastic rounding stream.
   Default here: Hamilton (no RNG, trivially thread-independent). Open until the A1 spike shows
   whether the bias in small cells (a rural province with under 100 women in a band) matters.
2. Whether `ceteris-pop` becomes its own crate or a module of `ceteris-econ`. Default: module until
   Branch A, then split if the econ compile graph or the cell-count boundary justifies it.
3. Cells for provinces of under 1,000 inhabitants: pool with the parent country's smallest
   province or keep a floor of one person per band. Default: keep, no pooling; the D1 spike
   reports the cost.
4. The 22-band, 4-level cell count (808,544) rests on 4,594 provinces; a 50-goods pack
   (#206) does not change it.
