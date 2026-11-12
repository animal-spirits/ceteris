# Ceteris: what else could we add

Oct 24, 2026 · speculative brainstorm, non-binding.

Scope: ideas **not** already in `specs/DESIGN.md` (incl. sec. 23), `ceteris-game-design-notes.md` or `ceteris-narrative-and-explanation.md`. Checked by reading those and grepping the specs; where an idea extends something existing, the anchor is named.

Cost for a solo dev: **S** under 2 weeks, **M** 1-2 months, **L** a quarter or more. Profile: lab (must pass ablation, 15.7) or ext (extended profile, 23.1).

## Shortlist

The ones with the best ratio of novelty to cost, or that make an existing module playable early.

| # | Idea | Why | Cost |
| --- | --- | --- | --- |
| 1 | Supply-chain manager seat | Playable on the trade core, which is the most mature module; no politics needed | M |
| 2 | Forecast game (Brier vs. ensemble) | Nearly free given ensembles; teaches calibration; unique | S |
| 3 | Minimal-intervention puzzles | "Smallest edit that prevents 2008"; deterministic, verifiable leaderboards | M |
| 4 | Lineage view | One family's life across decades, sampled from pop hazards; humanizes macro, view-only | S-M |
| 5 | Live-world mode | Assimilate real data monthly so "today" is always a start date | L |
| 6 | Automation as task displacement | The 2000-2050 labour story is missing; frontier AI milestone has no labour channel | M |
| 7 | Housing and land | Biggest household asset and a crisis channel (2008); currently thin | M |
| 8 | Profit shifting and tax havens | Real fiscal lever with good data; a natural diplomacy game | M |
| 9 | Climate tipping points + solar geoengineering lever | Unilateral SRM is a great strategic dilemma | M |
| 10 | Data-quality map mode | Honest about imputed vs. measured; cheap | S |

## Simulation additions

### Economy and finance

**Automation as task displacement** (ext, then lab if it passes). Acemoglu-Restrepo task model: sectors are task bundles; automation moves tasks from labour to capital, new tasks reinstate labour. Gives labour share, skill premium and regional inequality dynamics; gives the frontier-AI milestone (23.7) a labour-market channel instead of only a TFP shock. Refs: Acemoglu and Restrepo (2018 AER; 2022 Econometrica). Data: occupation task content (O*NET), robot density (IFR, licence to check). **M**.

**Housing, land and mortgages** (lab candidate). Housing is a stock in pop balance sheets with supply elasticity per province (zoning, terrain), house prices from user cost, mortgage credit from banks (9.3). Channels: wealth effects, collateral constraints, construction cycles, spatial misallocation when supply is tight. Refs: Jordà, Schularick and Taylor (2016, "The great mortgaging"); Hsieh and Moretti (2019 AEJ Macro); Saiz (2010 QJE) for supply elasticities. Opens a land-value tax lever (Henry George, a classic player request). **M**.

**Profit shifting and tax havens** (lab candidate). Multinational profits booked where taxes are low; a per-state corporate rate and enforcement; havens as small states with a fiscal stake. Levers: rate competition, global minimum tax (Pillar Two), anti-avoidance rules, sanctions on havens. Refs: Tørsløv, Wier and Zucman (2023 REStud), with country-level data. **M**.

**Granular firms and production networks** (ext). The few largest firms per state as explicit agents with idiosyncratic shocks, so a single firm's collapse moves GDP (Gabaix 2011) and propagates through the IO network (Acemoglu et al. 2012; Carvalho and Tahbaz-Salehi 2019). Extends the conglomerates of 23.7. **M**.

**Weaponized interdependence** (lab, rides on P7). Hub positions in networks (payment rails, chokepoint goods, undersea cables) give states panopticon and chokepoint power. A derived network-centrality metric per state, plus actions: cut off from payments, export-control a chokepoint good. Refs: Farrell and Newman (2019 International Security). Extends sanctions; makes critical minerals and semiconductors matter without new sectors, as a concentration index on existing ones. **S-M**.

**Private money: stablecoins and crypto** (ext). Dollar stablecoins as a dollarization channel in high-inflation states; runs on stablecoin reserves as a contagion path into T-bill markets; capital-control evasion. Refs: Gorton and Zhang (2023, "Taming wildcat stablecoins"). Low priority for realism, high for player curiosity. **S-M**.

**Remittances and diaspora finance** (lab candidate). Migrants (6.5) send income home; remittances are countercyclical insurance and an FX inflow. Ref: Yang (2008 EJ). Already 1 mention in specs; worth a proper flow. **S**.

**Resource wealth management** (lab). Sovereign wealth funds and fiscal rules for commodity exporters; the Dutch-disease and voracity effects. Ref: van der Ploeg (2011 JEL). Natural lever for the Nigeria scenario. **S**.

**Insurance and the protection gap** (ext). Insurers price climate risk; repeated disasters raise premiums, then withdrawal, then house-price and mortgage stress in exposed provinces. Bridges 13.4 disasters to housing above. **M**.

### Demography and human capital

**Household formation, gender and female labour supply** (lab candidate). Fertility exists; marriage, household size and female participation do not ("gender" has zero hits). Female LFP drives growth and fertility jointly. Refs: Goldin (2006 AER); Doepke, Hannusch, Kindermann and Tertilt (2023, Handbook of the Economics of the Family). **M**.

**Early-life health and learning quality** (lab candidate). Education is enrolment; add quality (harmonized test scores) and early-childhood nutrition, since cognitive skills, not years, predict growth. Refs: Hanushek and Woessmann (2012 JEG); Almond and Currie (2011 JEP); Hoddinott et al. (2008 Lancet). Data: World Bank harmonized learning outcomes, stunting rates. Long lags make it a good "plant trees you won't sit under" lever. **M**.

**Brain drain and brain gain** (lab). Skill-selective emigration, with the incentive effect at home and diaspora networks raising trade and FDI. Refs: Docquier and Rapoport (2012 JEL). Extends 6.5. **S**.

**Deaths of despair and chronic disease** (ext). Mortality responding to local economic distress (Case and Deaton 2020) and NCD burden, so deindustrialization shows up in life expectancy. **S**.

### Environment and resources

**Climate tipping elements** (ext). AMOC slowdown, permafrost carbon, ice-sheet loss and Amazon dieback as threshold processes on global temperature with hysteresis. Refs: Lenton et al. (2008 PNAS); Armstrong McKay et al. (2022 Science). Fits the FaIR emulator (13.2) as extra state. **M**.

**Solar geoengineering as a unilateral lever** (ext). Cheap, fast cooling with regional precipitation side effects and termination shock. Any mid-size state can do it: the "free driver" problem (Weitzman 2015). A superb strategic dilemma: who sprays, who objects, who threatens war over it. **S-M** once tipping points exist.

**Climate clubs and carbon border taxes** (lab, P7). CBAM-style border adjustment and club tariffs on non-members. Ref: Nordhaus (2015 AER). Trade core already prices tariff wedges, so this is mostly policy plumbing. **S**.

**Water and groundwater** (ext). Aquifer stocks depleted by irrigation, river basins shared across borders (Nile, Indus, Mekong) as a conflict stake. Refs: Rodell, Velicogna and Famiglietti (2009 Nature); Hornbeck and Keskin (2014 AEJ Applied). **M**.

**Fisheries and ecosystem stocks** (ext). Open-access renewable stocks with collapse thresholds; EEZ enforcement as a naval task. Ref: Costello et al. (2016 PNAS). Teaches the tragedy of the commons in-game. **S-M**.

**Energy learning curves** (lab candidate; check econ SPEC overlap). Solar, batteries, electrolysers on Wright's law, so the transition's cost path is endogenous to cumulative deployment. Ref: Way et al. (2022 Joule). Wright's law is already used for equipment (23.5). **S**.

### Politics, culture and information

**Narratives as contagious state** (ext). Economic narratives ("this time is different", "the peg will hold") spread SIR-style through the media layer and shift expectations and asset demand. Refs: Shiller (2017 AER; 2019 book). Links ADR 0082 expectations to 23.7 media. **M**.

**Social media and informational autocracy** (ext). Platform penetration raises mobilization speed and polarization; autocrats shift from repression to manipulation. Refs: Zhuravskaya, Petrova and Enikolopov (2020 ARE); Guriev and Treisman (2019 JEP). Extends ADR 0071 mobile penetration. **M**.

**Slow culture: trust and kinship** (ext). Generalized trust and kinship intensity as slow-moving pop traits that shape state capacity, tax compliance and firm size. Refs: Algan and Cahuc (2010 AER); Enke (2019 QJE). Fits P18 cultural transmission. **S-M**.

**Courts and constitutional review** (2.0, lab if pattern tests pass). Judicial independence as a veto player that can block laws and executive action, and a target for court-packing. Lever for both seats and autocratic backsliding. **S**.

### Infrastructure of the model

**Live-world mode** (product feature). Extend the calibration-window data assimilation (DESIGN 15, EnKF or particle filter) past 2024: ingest monthly releases, re-anchor the state, publish "today" as a start date. Players start from last month; the lab gets a running forecast scored against reality. **L**, plus data-pipeline upkeep.

**Data-quality map mode** (view). Per province and variable, show measured vs. imputed vs. solver-adjusted. Cheap honesty; pairs with fog of statistics. **S**.

**Welfare accounting** (view and scoring). Consumption-equivalent welfare combining consumption, leisure, mortality and inequality (Jones and Klenow 2016 AER) as a map mode and an objective metric, so players are not just maximizing GDP. **S**.

## Game additions

### New seats

| Seat | Levers | Why it is good | Builds on | Cost |
| --- | --- | --- | --- | --- |
| Supply-chain manager | Sourcing, dual-sourcing, inventories, routes, plant location | Uses only the trade core; chokepoint shocks are natural crises; price taker, so the world stays exogenous | Trade (P2), chokepoints, shock tape | M |
| IMF mission chief | Programme size, conditionality, haircut demands | Multi-party bargaining with a government and creditors; counterfactual score is clean | 9.4, 23.6 | M |
| Econometrician | Choose data, design an identification strategy, estimate a parameter | Scored against the true parameter; the fog of statistics becomes the puzzle | 15.5, ADR 0065 | M |
| Mayor of a megacity province | Zoning, transit, housing permits, local taxes | Small scope, fast feedback; needs housing | Housing above | M |
| Journalist / pollster | What to investigate and publish | Reveals truth from reported stats; shifts narratives | Fog, narratives | M |

### New modes

**Forecast game.** Before running, the player forecasts outcomes ("Argentina defaults by 2003: p = ?"). Scored by Brier against the ensemble frequency, not the single realized path. Daily seeded rounds. Teaches calibration (Tetlock and Gardner 2015). Uses only ensemble lanes. **S**.

**Minimal-intervention puzzles.** "Prevent the 2008 crisis with the fewest, smallest edits." Score is intervention cost (count, size, how early); outcome verified by deterministic replay. Same idea as minimal counterfactual explanations in ML (Wachter, Mittelstadt and Russell 2017). Variant: **butterfly hunt**, the latest and smallest edit that flips an event, which directly exposes predictability horizons (14.6). **M** (needs a cost metric over edit types).

**Policy-bot tournaments.** Players submit policy rules as Python against the scripting API (14.8); a server runs them in a fixed seat over many seeds. Axelrod's tournaments, for central banks. Cheap once the API is stable. **S-M**.

**Classroom mode.** An instructor publishes a scenario and objective; students submit input logs; replay grades them and shows the class's decisions on one ensemble chart. A natural adoption path for an open-source econ sim. Also: papers as missions, where each natural experiment from 15.5 becomes a guided scenario. **M**.

**Lineage view.** Pick a pop cell and follow one generated family over decades: births, schooling, jobs, migration, deaths, drawn deterministically from that pop's transition rates. View-only, writes no state. The micro texture of This War of Mine or a CK3 dynasty over a macro model. Pairs with the legends browser. **S-M**.

**Spectator and chat-driven shocks.** A streamer exposes the shock tape to chat votes; each vote is a recorded intervention, so the stream is a replayable branch. **S** after lockstep inputs exist.

**Share codes.** A run as a short string (seed + pack hash + input log digest), like Factorio blueprints, opening as a take-over. **S**.

## Ideas considered and dropped

- Tech trees or named technologies: conflicts with the 23 Oct decision; capability milestones already cover the need.
- Scripted historical event chains: violates "no scripted outcomes".
- MMO or persistent shared world: cloud hosting is a non-goal, and lockstep plus async play covers multiplayer.
- Tourism as its own sector: small macro weight; a services demand shifter covers it.

## Open questions

- [ ] Is a non-government seat (supply-chain manager) an acceptable first playable for the 1.0 free demo, given politics lands in 2.0?
- [ ] Which of housing, automation and gender passes the 15.7 ablation rule, and which stays extended?
- [ ] Live-world mode: worth the ongoing data-pipeline cost for a solo dev?
- [ ] Policy-bot tournaments: is the scripting API stable enough to be a public game surface?
