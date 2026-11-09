## 17. Degrees of Freedom Register

The register lists every parameter that is not pinned by data, with its prior and how it is identified; ranges below are starting priors to be refined in Phase 9, not final values.

### 17.1 Continuous parameters

| ID | Parameter | Module | Starting prior | Identification / source | Class |
| --- | --- | --- | --- | --- | --- |
| P01 | Trade elasticity theta\_g by sector | Trade | 4 classes (ADR 0101, trade-weighted harmonic means): class 0 (food; chemicals and metals) 2.91, class 1 (machinery, electronics and vehicles) 4.14, class 2 (agriculture; light manufacturing) 7.68, class 3 (crude oil, gas and refined fuels; coal and other mining) 16.91; local goods 0 (3 classes 2.62, 5.19 and 10.57 before, ADR 0023) | Caliendo and Parro (2015); estimated directly by PPML structural gravity on 2000 BACI flows (ADR 0022); sets the gravity kernel | Literature |
| P02 | Merchant inventory target coverage | Markets | 1 to 6 months by good | Storage and perishability; price volatility | Free |
| P03a | Price adjustment speed kappa1 | Markets | 0.05 to 0.5 per month, start 0.05 | Price volatility and autocorrelation | Free |
| P03b | Inventory term kappa2 of the price rule | Markets | 0.05 to 0.5 per month when on; start 0 (off) | Price volatility and autocorrelation; stability region (ADR 0019) | Free |
| P03c | Step clip c\_max of the price and wage rules | Markets | 0.1 in log per month | Bounds one tick's move; binding ticks counted per province | Free |
| P03d | Share m of each market's stability limit used by its price gain (ADR 0263), and of its coupled block's, the country's linearized tick (ADR 0267) | Markets | 0 to 1, start 0.5 | Frictionless stability (V1); per-market speed; a capped block keeps the same margin of its own limit | Free |
| P04 | Mode cost ratios (water, rail, road, track) | Transport | Road 0.10 USD per tonne-km; rail, sea, air 0.164, 0.121, 5.37 per road (BTS 2000); P9 ranges per ADR 0269 | Freight rates (BTS NTS Table 3-21), price gaps (Donaldson 2018) | Literature (borrowed, ADR 0269); calibrated in P9 |
| P05 | Non-tariff border frictions | Trade | Log-uniform | Gravity border effects; fitted per theta class to ICIO 2000 cross-border shares (ADR 0092), per good in class 3 (ADR 0276, proposed), a good below its own share at nu 0 clamped at 0 | Calibrated |
| P06 | Goods substitution sigma within a PIGL category | Demand | 0.3 to 0.9, start 0.6 | Comin-Lashkari-Mestieri (2021) | Literature |
| P07 | Income elasticities by category | Demand | Food below 1, services above 1 | Comin-Lashkari-Mestieri; budget surveys | Literature |
| P08 | Capital-labour substitution | Production | 0.5 to 1.2 | Oberfield and Raval (2021) and related | Literature |
| P09 | Agglomeration elasticity | Production | 0.02 to 0.10 | Rosenthal and Strange (2004) | Literature |
| P10 | Depreciation rates | Production | 2 to 8% per year by asset | Accounting conventions | Literature |
| P11 | Investment logit dispersion | Investment | Free | Capital allocation dispersion | Free |
| P12 | Ideas production lambda, phi | Technology | lambda 0.3 to 1, phi below 1 | Jones (1995) | Literature + calibrated |
| P13 | Catch-up rate and its determinants (education, trade exposure, FDI, institutions) | Technology | A few percent of the gap per year | Conditional-convergence evidence; Buera and Oberfield (2020) | Calibrated |
| P14 | Fertility response gammas | Demography | Signed priors | Crude birth rates, transition timing | Calibrated |
| P15 | Mortality level mapping (Preston curve) | Demography | Fitted curve | Life tables, crude death rates | Data + calibrated |
| P16 | Migration dispersion nu and cost weights | Migration | Literature range | Artuc et al. (2010); Caliendo et al. (2019); flow data | Literature + calibrated |
| P17 | Diaspora network weight | Migration | Positive | Chain-migration flows | Calibrated |
| P18 | Cultural transmission rates | Culture | Free | Assimilation and language shift rates | Calibrated |
| P19 | Heuristic-switching intensity of choice | Expectations | Laboratory estimates | Anufriev and Hommes (2012) | Literature |
| P20 | Bank leverage and reserve limits | Finance | By era | Historical bank balance sheets | Data |
| P21 | Sovereign spread function coefficients | Finance | Signed priors | Historical spreads | Calibrated |
| P22 | Fiscal capacity accumulation and decay | Fiscal | Free | Revenue/GDP paths | Calibrated |
| P23 | Contest success exponents m, a, b, c | Politics | Around 1 | Policy outcomes vs group strength | Free |
| P24 | Mobilization threshold distribution | Unrest | Free | Frequency of revolts and revolutions | Calibrated |
| P25 | Bounded-confidence threshold | Ideology | Free | Polarization paths | Free |
| P26 | Power-estimate noise in bargaining | War | Free | War onset frequencies (COW) | Calibrated |
| P27 | Lanchester coefficients and modifiers | War | By technology era | Battle datasets | Calibrated |
| P28 | Supply attrition curve | War | Free | Historical campaign attrition | Calibrated |
| P29 | State planner inertia and noise | AI | Free | Policy change frequency | Free |
| P30 | Discount factor | Several | 0.95 to 0.97 per year | Standard | Literature |

Added for the modern scenario:

| ID | Parameter | Module | Starting prior | Identification / source | Class |
| --- | --- | --- | --- | --- | --- |
| P31 | Climate damage coefficients (yields, labour, mortality) | Climate | Published dose-response curves | Schlenker and Roberts; Burke et al. (2015); Carleton et al. (2022) | Literature |
| P32 | Wright's-law learning rates for energy sectors | Technology | 10 to 30% cost decline per doubling | Way et al. (2022) | Literature |
| P33 | Economic-voting weight | Parties | Positive | Incumbent vote response to growth and inflation | Calibrated |
| P34 | New-party entry threshold | Parties | Free | Frequency of new parties reaching parliament | Calibrated |
| P35 | Taylor-rule coefficients by central bank | Monetary | Inflation above 1, output positive | Estimated policy rules | Literature + calibrated |
| P36 | Nuclear escalation cost term | War | Large | Absence of direct war between nuclear powers | Calibrated |
| P37 | Consumption propensity m, wealth correction lambda, target wealth ratio w\* | Households | m 0.5 to 1 by wealth bracket; lambda 0.01 to 0.05 per month | Intertemporal MPCs (Fagereng, Holm and Natvik 2021; Auclert, Rognlie and Straub 2024); wealth-to-income by bracket (WID) | Calibrated |
| P38 | Long-run input substitution elasticity (putty-clay) | Production | 0.3 to 1 | Drift of input-output coefficients across ICIO years 1995-2022 | Literature + calibrated |
| P39 | Wage adjustment speed kappa\_w | Labour | 0.05 to 0.5 per month, start 0.1 | Wage inflation persistence (P2 econ spec, ADR 0019) | Free |
| P40 | Downward wage rigidity delta\_w | Labour | 0 to 1, start 0.5 | Mass of nominal wage changes at zero | Free |
| P41 | Employment adjustment speed eta | Labour | 0.1 to 1 per month, start 0.3 | Job creation and destruction rates | Free |
| P42 | Education efficiency e\_k | Labour | Below secondary 1; secondary 1.4 to 1.6; tertiary 2.0 to 2.6 | Mincer returns of about 8 to 12% per year | Literature |
| P43 | Time cost of transit lambda\_g, ad valorem per day | Trade | Manufactures 0.006 to 0.021; food 0.003 to 0.010; primary 0 to 0.006 | Hummels and Schaur (2013); ADR 0013 | Literature + free |
| P44 | Route-choice dispersion theta\_r | Trade | 41 to 182, value 111.5; unused in P2 (route choice deferred, ADR 0017) | Fan, Lu and Luo (2021), routing elasticity from exporters' port choice | Literature |
| P45 | Expectation gain g of pi^e | Markets | 0.004 to 0.65 per month, start 0.006 | Learning gain of Poledna et al. (2023) to the adaptive heuristic of Anufriev and Hommes (2012); ADR 0019 | Literature |
| P46 | PIGL price parameters sigma\_B, phi, gamma, with the Dt weights theta | Demand | Start sigma\_B 0.26, phi 0, gamma 0.71, theta 0.634, 0.082, 0.284 | Alder, Boppart and Muller (2022), pooled PIGL; ADR 0021 | Literature |
| P47 | PIGL Engel parameter epsilon | Demand | Start 0.71; country estimates 0.34 to 0.90 | Alder, Boppart and Muller (2022); ADR 0021 | Literature |
| P50 | Within-cell elasticity of substitution sigma\_g | Production | 3 to 8, below theta\_g + 1 | Markup estimates; Broda and Weinstein (2006) | Literature |
| P51 | Fixed export costs by border crossing | Trade | Log-uniform | Exporter shares by sector and country | Calibrated |
| P52 | Entry costs by sector | Production | Log-uniform | Firm counts by size class; entry and exit rates | Calibrated |
| P56 | Storage loss delta\_g and money cost k\_g per month of the competitively stored goods 0, 1, 2, 5 (ADR 0091) | Markets | Storage off by default (owner ruling 2026-10-31: the approved values destabilize the fixture, ADR 0091 As built), so delta defaults to 0; approved values, set by `TradeParams::with_storage`: delta: agriculture 0.003 (0.001 to 0.01), fuels 0.0005 (0 to 0.002), coal and mining 0.001 (0 to 0.005), chemicals and metals 0 (0 to 0.001). k: 0.009 (0.0055 to 0.02), 0.011 (0.0075 to 0.02), 0.007 (0.005 to 0.012), 0.0067 (0.0055 to 0.01); k includes the interest part 0.0047, the 2000 mean of the US 3-month bill (FRED TB3MS, 5.82% a year) | delta: Hofstrand (ISU A2-33, 1995) and APHLIS post-harvest losses for grain (medium); judgement for fuels and coal (weak); no deterioration of base metals, Cafiero et al. (2011) (medium). k: interest (strong) plus physical storage: ISU extension 2017 grain rates, Cushing tank leases, LME rents (weak, not 2000-dated). Owner approval 2026-10-30T22:51Z; refit in P9 | Literature |
| P144 | Speculation intensity psi\_g of competitive storage (ADR 0091), months | Markets | 0 by default (storage off, owner ruling 2026-10-31); approved values: agriculture 20 (14 to 37), fuels 1.0 (0.6 to 2), coal and mining 3.571, chemicals and metals 5.714 | Derived, not estimated: psi = lambda s\_g / (zeta (k + delta)), lambda the monthly persistence target 1 - rho of de-trended log(I / I\*) in Gorton, Hayashi and Rouwenhorst (2013, Table 2): 0.14, 0.16, and 0.05 and 0.065 for the last two (weak). Goods 2 and 5 are the source sheet's 25 and 40, derived at zeta 0.5, rescaled to the approved zeta 3.5 (psi x 0.5 / 3.5): for coal and mining that is the formula's value at zeta 3.5, for chemicals and metals the formula gives 5.54 (0.065 x 2 / (3.5 x 0.0067)), so 5.714 is the rescaled sheet value, as approved. Owner approval 2026-10-30T22:51Z; refit in P9. Since ADR 0280 (owner ruling 2026-10-31) the storage order is a partial adjustment at lambda = psi zeta (k + delta) / s, the persistence rate itself: 0.144, 0.15, 0.05, 0.067 a month at the approved psi (the targets above, rounded through psi, and good 5's rescaled value); lambda is derived from P144, P145, P56 and P02, not a new parameter | Free |
| P145 | Convenience-yield elasticity zeta\_g (ADR 0091) | Markets | Agriculture 1.8 (0.7 to 2.7), fuels 13 (8 to 22), coal and mining 3.5, chemicals and metals 3.5 | Slope of the basis in inventory at I / I\* = 1, Gorton, Hayashi and Rouwenhorst (2013, Table 3) over k + delta: grains -0.255 a year, energies -1.861 (medium); 3.5 for goods 2 and 5 is the owner's choice between the level route (0.4 to 0.8) and the curvature route (3.2 to 4.3) of the metals group (weak). Owner approval 2026-10-30T22:51Z; refit in P9 | Free |
| P146 | Gain g\_s of the per-market expected price change pi^e\_og (ADR 0091) | Markets | 0.006 per month for all four goods (0.004 to 0.65, the P45 prior) | P45's start: the constant learning gain of Milani (2007, JME 54), 0.0183 a quarter. Was 0.08 (a 12-month window, Gorton, Hayashi and Rouwenhorst 2013), at which agriculture's storage loop leaves the region (psi limit 14.7 against 20); at 0.006 the limit is 43.5 (owner approval 2026-10-31, B7 follow-up); refit in P9 | Free |
| P148 | Loss write-off threshold M | Production | 3 months of base sales | Limited liability of enterprises (ADR 0112) | Free |
| P149 | Input stock cover | Production | 1 month | The cover of the input order rule (owner decision, 24 Oct 2026) | Free |
| P150 | Theta scale: multiplies every theta\_g where the kernel is built (ADR 0101) | Trade | 1; tariff experiments report the lane {0.6, 1, 1.4} until direct estimates replace the literature values | Experiment lane for the uncertainty of P01 (trade SPEC) | Free (experiment lane) |
| P151 | Price step clip c\_max\_storable of the storable commodities, goods 0, 1, 2 and 5 (ADR 0106) | Markets | 0.3 in log per month | Oil fell about 30% in one month in late 2008 (trade SPEC) | Free (numerical) |
| P153 | Half-life h of the initial TFP and capital drifts (ADR 0103) | Production | 3 to 15 years, start 7 | 2000-2010 holdout with drift on against off | Free |
| P154 | Bulk freight rate: multiplier on the sea cost per tonne-km, per theta class (ADR 0247) | Transport | Class 3 (fuels, mining) 0.017, the sourced slope over P04c (0.026 over the old P04c; rescaled by ADR 0269, owner 2026-10-31), classes 0 to 2 at 1 (on by default, owner ruling 2026-10-30) | Slope of 2000 tanker and dry-bulk voyage rates per tonne against sea km, UNCTAD Review of Maritime Transport 2001 and 2002 | Literature |
| P155 | Bulk freight rate: multiplier on the port handling fee, per theta class (ADR 0247) | Transport | Class 3 0.40, classes 0 to 2 at 1 (on, as P154) | Half the fixed part of the same rates over P04m's listed-port fee | Literature |
| P156 | Steady-state money tolerance per pop on the full pack (ADR 0089 acceptance, `p2_steady_state.rs`) | Validation | max(4 minor units, 2e-6 y_k), y_k the pop's steady monthly income (was 2e-6 of the quantity itself) | A balance's error accumulates from one tick's flows through it (wage, dividends and rebates in, spending out), each at the relative error eps of the real drift: \|dm\| <= eps (W + Div + Reb + S) ~ 2 eps y; a bound relative to the balance reads it against the non-wage share only (Albania 0.22, Pitcairn 0.32); the factor 2 is not taken, so the bound is the tighter of the two. Owner approval 2026-10-30T23:19Z | Free (numerical) |
| P157 | Block stability tolerance (ADR 0267, `block::TOLERANCE`): a block of markets counts as stable while the measured spectral radius of its linearized tick is at most 1 + this | Markets | 2e-3 (not tuned) | A market at its own per-market cap has radius 1 at gains / m; the growth-rate measurement over 2,000 ticks reads a radius to about 1e-3 (5.4e-4 against the Gelfand limit on the fixture's blocks); the tick grows at the map's radius to within it (`block_map_growth_matches_the_tick`) | Free (numerical) |
| P159 | Calibration pass tolerance (`steady::PASS_TOL`; P158 on this branch before, renumbered since main's ADR 0272 holds P158): the passes of stages 1, 2 and 4 stop when rebates, r, buyer prices and the PIGL weights change by at most this, relative (econ SPEC, Calibration) | Calibration | 1e-13 (was 1e-12; B8e, owner, 2026-10-31) | Derived from stage 5's check (1e-12): it compares Q, settled at the last pass's values, with D, the orders those values' spending gives at the settled prices, so its gap is about 3.8 times the last pass change (B8 pack: 1.34e-12 at a last change of 3.55e-13) plus about 3e-13 of rounding; the tolerance must be at most (1e-12 - 3e-13) / 3.8 = 1.8e-13, and 1e-13 is the decade below (5.25e-13 measured at it, B8d) | Free (numerical) |
| P160 | Real GDP check tolerance on the benchmark economies (ADR 0290, `econ_build.rs` `BENCHMARK_TOLERANCE`): the pack's GDP over p_c may differ from WDI's GDP at PPP by at most this share where PWT's `i_cig` of 2000 is Benchmark (31 ICIO economies) | Validation | 0.10 (ADR 0098's 5% before, never enforced, ADR 0287) | The measured agreement of two published vintages of the same benchmark PPPs (PWT 11 pl_da and WDI's level, mostly Eurostat-OECD): 31 of 31 within it once Cyprus is compared on totals, the largest gap the UK's 1.093; 23 of 31 within 5%. Owner ruling 2026-11-01 (RFC 0282 D8) | Free |
| P161 | DFA top-10% share T: the US 2000 top-decile net worth share that fixes tau, the log-wealth variance above the income Gini's, in the wealth-bracket proxy (ADR 0311; econ SPEC, Factor shares) | Households | 0.638 | Federal Reserve Distributional Financial Accounts, 2000Q1: 90th to 99th percentile 35.9% plus top 1% 27.9% (bottom 50% 3.2%, 50th to 90th 33.0%) | Data |
| P162 | Sector-class catch-up factor on the catch-up rate: manufacturing, tradable services, non-tradable services, agriculture (ADR 0031; provisionally P60 there) | Structural change | One per class, priors from the unconditional-convergence literature; set in P4 Branch E | Unconditional convergence in manufacturing (Rodrik 2013), aggregate non-convergence | Literature, free |
| P163 | Formalization transition scaled by enforcement capacity (ADR 0031; provisionally P61) | Structural change | Calibrated in P4 Branch E | ILO informality rates, Economic Transformation Database | Free |
| P164 | Education progression response to education spending and household income, cohorts 15 to 34 (ADR 0032; provisionally P62) | Population | WCDE progression ratios as prior; response calibrated in P4 Branch A | WCDE v3 transition schedules (K.C. et al. 2024) | Free |
| P165 | Fertility tempo response to female education and housing cost (ADR 0032; provisionally P63) | Population | Calibrated in P4 Branch A | Bongaarts and Feeney (1998), WPP mean age at childbearing | Free |
| P166 | Deaths-of-despair scenario hazard, default (ADR 0033; provisionally P65) | Population | 0 | A scenario lane until a mechanism passes ablation (ADR 0063) | Scenario |
| P167 | City congestion cost: housing, commuting and pollution, against agglomeration A5 (ADR 0074; provisionally P126) | Settlements | Calibrated in P4 Branch D3 | Urban Centre Database, GHSL; Zipf city sizes | Free |
| P168 | Slum share rise when housing supply lags urban growth under insecure tenure (ADR 0074; provisionally P127) | Settlements | Calibrated in P4 Branch D3 | UN-Habitat slum estimates (terms to audit) | Free |
| P169 | Effective labour lost per worker at untreated disease prevalence (ADR 0167) | Population | Prior from Thirumurthy et al. (2008); set in P4 Branch B2 | Thirumurthy, Graff Zivin and Goldstein (2008) | Literature, free |
| P170 | Irregular share z: fraction of rejected applicants who migrate anyway at an extra cost (ADR 0109) | Migration | 0 to 0.2 | Calibrated to Abel and Cohen (2019) bilateral flows | Calibrated |

Held or proposed, not built in P2: P143 kappa\_n (ADR 0090, proposed; its split price rule is parked on `p2-adr0090` and `pre-p3/b6-wip`, and deferred to P5 by owner ruling, 2026-10-30, ADR 0090 "Deferred to P5") and P147 M\_F (ADR 0112, second decision; reserved and held, `p2-r3-producer-stock`).

### 17.2 Structural switches (model uncertainty)

| ID | Switch | Options |
| --- | --- | --- |
| S01 | Node price formation | Stock-flow rule / Walrasian clearing |
| S02 | Trade solver | Gravity normalization cleared across ticks / full within-tick Sinkhorn (balanced or unbalanced) / Walrasian spatial price equilibrium |
| S03 | Demand system | PIGL / non-homothetic CES / Stone-Geary |
| S04 | Expectations | Heuristic switching / adaptive only / perfect foresight (reference) |
| S05 | Trade costs | Route choice / least-cost path; per-unit + iceberg / iceberg only |
| S06 | State AI | Tier 1a / 1b / 2 |
| S07 | Replay (ADR 0241) | Off / shocks / policy / all |
| S08 | System order within tick | Fixed / randomized (sensitivity) |
| S09 | Intermediate technology | Putty-clay vintages / fixed 2000 Leontief coefficients |
| S10 | Climate damages | Level effects / growth effects |
| S11 | Rationing | Priority / proportional / mixed (Pichler et al. 2022) |
| S23 | Initial drift (ADR 0103) | On (default) / off (steady-state tests, P2 gate) |
| S22 | Trade-imbalance transfers (ADR 0100) | Off (balanced trade) / fixed 2000 shares of world GDP (default) / observed current-account path 2000 to 2024, fading with a 10-year half-life (lab profile) |

A conclusion is reported as robust only if it survives the plausible settings of the relevant switches.

