## 23. Beyond v1: World Builder and Strategy-Game Depth

Version 1 is a calibrated economic laboratory; this section specifies what turns it into a world builder and a Paradox-depth strategy game without weakening that laboratory. Nothing here is scheduled before 1.0, and every item lands behind a feature flag (roadmap, release process). Version numbers follow ADR 0136: a major relaxes one ceteris paribus, and everything else is a minor. Seats, scoring, narration and flavour are 2.0 or later (ADRs 0239, 0240); 1.x minors are simulator, lab, editor and view work.

The gaps fall in three groups: building worlds other than the real 2000 start (23.2-23.3), giving the player something to do and be judged on (23.4), and adding the discrete texture of characters, commands, actions and stories that aggregate mechanisms lack (23.5-23.8).

### 23.1 Lab and extended profiles

Most additions here add free parameters faster than they add out-of-sample skill, so they would fail the ablation rule of 15.7. They ship anyway, under a second profile.

|  | Lab profile | Extended profile |
| --- | --- | --- |
| Purpose | Calibrated counterfactuals, scorecard, citable results | Play, exploration, worlds without data |
| Mechanisms | Only those that pass ablation | Lab mechanisms plus the additions in this section |
| Free parameters | At most 60 (Section 17) | Lab set plus an X-register (X01, X02, ...) with priors and provenance |
| Scoring | Scorecard (15.4), natural experiments (15.5) | Pattern tests only; never in the scorecard |
| UI label | None | "Exploratory" banner, as for 2.0 politics in lab mode |
| Determinism and invariants | Required | Required |

Rules: an extended mechanism must reduce to its lab counterpart when switched off (equilibrium nesting applied to profiles), and a mechanism that later passes ablation may join the lab profile as opt-in in a minor release; it becomes the lab default only at the next major (ADR 0136). Branches record their profile in the run manifest, and lab and extended runs are never compared as parent and child.

### 23.2 World authoring and the consistency solver

A consistency solver is the prerequisite for every authoring feature: it takes an edited or hand-made world and returns the nearest world that satisfies the invariants of 4.6. Without it, a large ex-ante edit (move a province, double a population, invent a state) either halts the run or silently breaks stock-flow consistency.

**Solver stages**, run in order on the edited state, each minimizing distance to the user's targets subject to the invariants:

1. **Pops and land.** Rescale pop cells so province totals, age structure and land use match the edit; integer cohorts by largest-remainder rounding.
2. **Production and IO.** Rebalance input-output and social accounting matrices by GRAS (Junius and Oosterhaven 2003) or cross-entropy (Golan, Judge and Robinson 1994), holding user-locked cells fixed.
3. **Trade.** Re-solve the entropic transport of 8.3 at the new costs and endowments; the result is the gravity-consistent flow set.
4. **Balance sheets.** Close the who-to-whom matrices of 9.8 by adjusting the least-locked counterparty (default: rest of world, then the central bank).
5. **Institutions and politics.** Recompute interest-group memberships, party bases and derived labels from the new pops.

Every adjustment is logged with its size, so the user sees how far the solver moved their edit. A null edit must return the input bit for bit (a CI test).

**Editors** (desktop app, extended and lab profiles):

- **Map:** split, merge and redraw provinces on the H3 base; reassign owner and controller; edit terrain, resources and infrastructure edges.
- **Polities:** create, rename or dissolve states, monetary unions and supranational bodies; set the institution vector by sliders with V-Dem anchors shown.
- **Peoples:** create ethnic, religious and language groups; paint settlement areas; set EPR status.
- **Economy:** lock or edit sector capacity, productivity gaps and balance-sheet lines.

Editors write an intervention list, not raw state, so every authored world is a branch of a data pack and stays reproducible (14.1).

### 23.3 Synthetic worlds, earlier eras and modding

**Synthetic worlds** (extended profile). A generator builds a fictional world in the same pack schema, then hands it to the consistency solver:

1. Terrain from noise plus plate-style uplift and erosion; rivers by flow accumulation.
2. Climate normals from latitude, elevation and prevailing winds; crop suitability from a GAEZ-style lookup on those normals.
3. Resources by geology priors; settlement by suitability and access, with city sizes drawn to Zipf's law (a V2 target already).
4. Provinces by Voronoi on population, states by seeded flood fill with border noise.
5. IO coefficients, institutions, education and balance sheets drawn from empirical priors conditional on income per head, estimated on the 2000 pack.

Because step 5 reuses 2000 cross-country relationships, synthetic worlds inherit modern-economy behaviour. That is intended; the generator is for alternate geographies, not alternate eras.

**Earlier eras.** The pipeline stops at 1995 because OECD ICIO and BACI do. A pre-1995 path needs:

- Long-run sources: Maddison Project (output, population), HYDE (gridded population and land use), Clio-Infra (education, wages, institutions), CShapes 2.0 (borders since 1886); licences to check for each.
- IO tables imputed from structural-change priors (Section 7.6) conditioned on income and sector shares.
- Capability milestones (23.7), because without them nothing distinguishes a railway economy from a container economy. This revisits the no-named-tech decision for earlier eras only.

Pre-1995 packs are extended-profile until a historical hindcast exists to score them.

**Modding.** The data-pack schema, scenario and intervention formats are already public API (roadmap, versioning policy). Modding adds:

- A documented mod manifest that layers packs, intervention lists and X-register parameter overrides, with load order and conflict reporting.
- Module hooks: a module ABI so a third-party crate can register systems at a fixed pipeline slot, declare its state components and parameters, and pass the V0 invariants.
- Mods run only in the extended profile unless they change data alone and pass the scorecard.

### 23.4 Player loop

The player loop lands in 2.0-alpha with Steward mode (ADR 0239). The player is scored by how much better their state does than the AI would have done with the same world and the same luck. The engine already computes exactly that: a paired effect under common random numbers (14.4-14.5), with the Tier 1a run as the baseline branch.

**Objectives.** At start the player picks or writes an objective as a weighted outcome vector over the state's own metrics, for example real income of the bottom 40%, debt to GDP, regime survival, territorial control or emissions. Presets cover common ambitions (catch up to the frontier, survive a hostile neighbour, keep the peg). The weights are fixed at start and shown on the scoreboard.

**Score.** Score = mean over K seeds of the objective in the player's branch minus the objective in the AI baseline, with a bootstrap interval. A shadow ensemble of the AI baseline runs in the background at low priority; the score is final only when both ensembles reach the end date. Single-seed play shows a provisional score with a wide band.

**Mandate and fail states.** The player is the government, not the state, so power can be lost:

- Democracies: losing an election hands the levers to the winning coalition's AI until the next election, unless the player chooses opposition play (proposing bills, campaigning, coalition offers).
- Autocracies: a coup, revolution or loss of the winning coalition ends the run, or continues it as the successor regime if the player opts in.
- Hard failures: sovereign default, state collapse, or loss of the capital province, each optional per scenario.

**Difficulty** changes information and frictions, never cheats: observation lag and noise (A13), the AI's planner horizon (23.8), and how much of the score breakdown the player sees during play.

**Ironman.** An Ironman run disables god-mode edits and branch forking, locks the seed list, and records only in-world edits (14.2), so its score is comparable across players.

### 23.5 Characters and the military command layer

**Characters** replace the stochastic leader attributes of 2.4 in the extended profile. People in office and in public life on 1 January 2000 carry their real names and public facts; everyone who enters later is generated from the population (ADRs 0183-0186, decision of 28 Oct 2026). The lab shows role labels, and a run-level switch replaces real people with generated stand-ins.

| Role | Latent traits | What the player sees | Main effect |
| --- | --- | --- | --- |
| Head of government | Competence, risk aversion, ideology, loyalty base, health | Record in office, platform, age | Planner weights and noise (12.3) |
| Ministers (finance, defence, foreign, interior) | Competence, faction, corruption | Faction, past posts | Implementation slippage on their policy module |
| Central bank governor | Hawkishness, independence | Speeches as noisy signals | Taylor-rule coefficients within the regime's band |
| Party leaders | Charisma, ideology, ambition | Polling, platform | Vote shares, coalition bargaining |
| Generals | See below | Dossier signals only | Combat, logistics, initiative |

Leader effects are bounded by evidence: growth effects are calibrated to the leader-death natural experiment of Jones and Olken (2005), tenure and exit hazards to Archigos (Goemans, Gleditsch and Chiozza 2009). Autocrats choose between loyal and competent appointees, the trade-off of Egorov and Sonin (2011), so small winning coalitions produce weaker cabinets and officer corps.

**Command layer** (extended profile; the lab keeps the aggregate Lanchester model of 11.5). It implements the war direction set in October 2026: HOI3-style but very realistic, simulation first, no unlockable technology, and a command spectrum from micromanagement to delegation.

- **Order of battle.** The capability stocks of 11.4 are organized into formations (brigade, division, corps, army, front) with HQs. Templates set the mix of manoeuvre, fires, air defence, engineers and logistics; restructuring costs readiness for months.
- **Command spectrum.** The player can order a single brigade, draw a front line with an objective, or hand a theatre to a general with a goal and a risk tolerance. Any level can be delegated.
- **Generals.** Latent operational skill, logistics skill, initiative and risk aversion. The player sees only a dossier of signals: staff-college scores, training results, combat record, peer reports and medals. Medals inflate with patronage and corruption (10.1), so they are gameable signals. Performance in battle updates a visible estimate.
- **Doctrine and training.** Force employment, after Biddle (2004), is a stock built by training spending and exercise time. It multiplies effectiveness more than equipment does, which is how quality differs without a tech tree.
- **Equipment.** Quality comes from defence-sector productivity (7.5) and the supplier state. Production lines follow Wright's law per equipment type; Wright (1936) first measured it on aircraft. Switching lines resets learning.
- **Logistics and fronts.** Supply hubs sit on the transport graph (5.3); fronts are contact lines between controlled provinces on the daily sub-tick. Encirclement cuts supply and raises surrender hazards.
- **Fog of war.** Enemy formations are seen through reconnaissance and intelligence (23.6), with the noise of A13.

Nesting test: a theatre delegated to an average general with default doctrine must reproduce the aggregate Lanchester outcome distribution of the lab profile. Battle-level coefficients are fitted to historical battle datasets such as CDB90 (licence to check).

### 23.6 Diplomacy as actions, intelligence and covert action

Section 11 models why states fight and settle; this section gives the player and the AI the same explicit verbs. Every verb is an action in the Tier 1a action space (12.6), so the AI uses them too.

**Diplomatic actions**

| Action | Mechanism it acts through | Data anchor |
| --- | --- | --- |
| Demand (territory, policy, basing, debt terms) | Opens a crisis in the bargaining model of 11.2 with a stated stake | Militarized dispute frequencies |
| Casus belli and war goals | Sets the indivisible-stake flag and the terms a settlement must meet; legitimacy cost scales with how unprovoked it looks | Interstate war onsets |
| Peace conference | Interactive bargaining over the terms list of 11.8 while beliefs update from the front | War termination terms |
| Client states and spheres | Hierarchy contracts trading autonomy for protection (Lake 2009): basing, foreign-policy alignment, currency or trade preference | Alliance and basing data |
| Arms deals | Transfers equipment stocks and creates dependence on spare parts from the supplier | Arms transfer series (licence to check) |
| International organizations | UN Security Council and General Assembly votes, WTO disputes, IMF programmes; EU accession as an endogenous process gated by thresholds on the institution vector | UN voting ideal points (Bailey, Strezhnev and Voeten 2017) |

A great-power ranking (capability, economy, financial centrality) is a derived view for the UI only, like regime labels in 10.2.

**Intelligence.** Each state has an intelligence service, a stock built by spending and bureaucratic quality. It reduces the noise of A13 on chosen targets: military capability, reserves, internal stability, true bargaining resolve. Better intelligence narrows the bargaining range of 11.2, which lowers war risk from private information.

**Covert action** (extended profile), each with a success hazard, an exposure hazard and blowback when exposed:

- Election interference, calibrated to partisan electoral interventions (Levin 2016).
- Coup sponsorship and support for rebels, raising the hazards of 10.8 and 11.6 in the target.
- Covert regime change, whose poor long-run record (O'Rourke 2018) is a validation target.
- Sabotage of infrastructure edges and cyber attacks on payment systems or utilities.
- Propaganda: shifts ideology diffusion (10.7) in the target, weighted by its civil-liberties index.

Exposure damages trust (11.1) with the target and third parties, so covert action trades short-run gain for long-run isolation.

### 23.7 Policy catalog, capability milestones, emerging sectors and non-state actors

**Policy catalog.** Section 10.1 says laws are separate variables; this is the list. Every law passes through the legislative process of 10.6, takes effect after a lag scaled by bureaucratic quality, and draws a stance from each interest group in proportion to its members' payoff.

| Domain | Levers | Acts through | Initial values from |
| --- | --- | --- | --- |
| Labour | Minimum wage, employment protection, union rules, retirement age | 8.7, 6.3 | OECD EPL; Botero et al. (2004) |
| Social protection | Pension design (pay-as-you-go or funded, replacement rate), unemployment insurance, cash transfers | 9.6, household income | OECD SOCX, ILO social protection data |
| Health | Public share of spending, coverage | Mortality level (6.2), SEIR response (13.3) | WHO health expenditure data |
| Education | Spending, compulsory years, tertiary access | Enrollment (6.3) | UNESCO data |
| Property and ownership | Land reform, nationalization, privatization | Ownership type (3.2), rule of law | V-Dem, privatization records |
| Markets | Price controls, subsidies, export bans, industrial policy, local content | Rationing (8.2), wedges (8.5) | Tariff and subsidy datasets |
| Finance | Capital controls, central bank independence, macroprudential tools | 9.11, 9.12 | Fernandez et al. (2016) |
| Citizenship and identity | Immigration law, naturalization, language and autonomy policy | 6.4, 6.5 | Migration policy indices |
| Information | Censorship, state media share | Civil liberties, ideology diffusion (10.7) | V-Dem |

**Capability milestones.** Discrete capabilities emerge from continuous investment; nothing is picked from a tree. A capability switches on when sector productivity and cumulative investment cross thresholds, and it unlocks actions or modifiers:

- Nuclear weapons, gated by nuclear latency (Fuhrmann and Tkach 2015) and a state decision with a sanctions response.
- Satellites and space launch: raise intelligence quality (23.6).
- Precision munitions and drones: raise Lanchester modifiers (11.5).
- Rapid vaccine platforms: shorten the vaccination lag in 13.3.
- Frontier AI: a services productivity shock plus cyber capability.

The 2000 state of each capability is data. This keeps the no-named-technology decision of 2.4 for 1.x while giving players milestones to race toward.

**Emerging sectors.** With 45 fixed sectors, a run to 2150 keeps the product space of 2000. Latent sectors exist from the start with zero output, an income elasticity above one in the demand system (8.6), and a parent sector. A latent sector activates when its frontier productivity crosses a viability threshold; provinces enter it in order of relatedness to their current output (Hidalgo et al. 2007). Extended profile only until a hindcast can test it.

**Non-state actors**

- **Criminal organizations.** Illicit production and trafficking as sectors with routes on the transport graph; crackdowns fragment groups and raise violence, the mechanism of Dell (2015). Initialized from UNODC estimates.
- **Armed and terrorist groups.** Extend the rebel factions of 11.6 with transnational networks that recruit from grievance and raise the attack hazards of 13.5.
- **Media.** An outlet layer per state (state, private, foreign ownership; reach) that weights ideology diffusion.
- **Conglomerates.** A few generated business groups per state as lobbying interest groups (10.4) that own foreign capital across borders.

### 23.8 Narrative layer and strategic AI

**Narrative layer.** Stories come from the event log, not from scripts, so "mechanism over script" (2.5) holds. The layer is a view: it never changes state, except through choices the player makes, which are recorded as in-world edits.

- **Event cards.** A card fires when a logged event or threshold crossing touches the player's state or a watched one: a bank run, a coup attempt, a protest cascade, a spread spike. It shows what happened, the decomposition of its drivers (explainability, 2.5) and, where relevant, two to four responses. Every response is an ordinary action from the action space, so a card adds no effect a menu could not.
- **Chronicle.** A per-branch history written from the event log with deterministic templates, readable as a timeline and diffable against the parent branch.
- **News feed and outliner.** World events ranked by size and proximity; the outliner lists the player's formations, crises, bills in progress and active covert operations.
- **Alerts.** User-set thresholds on any metric, such as reserves below three months of imports.

**Strategic AI.** The Tier 1a planner looks one step ahead, which is enough for budgets but too myopic for an opponent: it cannot prepare a war, build an alliance or save for a crisis. A strategic layer sits above it:

1. **Goals**, chosen quarterly from the coalition objective of 12.3: build fiscal capacity, deter neighbour X, join union Y, take province Z.
2. **Plans**, decomposed from goals by a hierarchical task network into constraints and targets for the one-step planner over several years (mobilize by a date, hold reserves above a floor, court a partner).
3. **Evaluation** by rollouts on the neural surrogate of 12.4, with a fixed rollout count and seeds keyed by (state, tick) so decisions stay deterministic.
4. **War planning** sets theatre objectives, force allocation and generals' risk tolerance for the command layer (23.5).

The rollout budget is set so the strategic layer stays within 10% of the tick budget of 19.2. Planning horizon and rollout count are the AI difficulty settings of 23.4. The layer runs in the extended profile until it passes the policy-forecast gate (roadmap, 0.6.0), at which point it may replace the one-step planner in the lab profile.

### 23.9 Placement in the roadmap

The first addition in this section is the consistency solver in 1.4 (climate, 1.1, is outside it); seats, scoring and most game depth arrive with 2.0 politics and war (ADR 0239); pre-1995 eras enter the lab at 3.0. Releases follow the roadmap's After 1.0 table, its versioning rules and ADR 0136; a 2.x or 3.x label below on an extended-profile item means its dependencies land then, and the item ships as a minor.

| Item | Section | Release | Depends on | Profile |
| --- | --- | --- | --- | --- |
| Event cards, chronicle, outliner, alerts | 23.8 | 1.3 | Event log | View only |
| Policy catalog: labour, social, health, education, markets, finance | 23.7 | 1.3 | God-mode edits (14.2) | Lab |
| Objectives, score against the AI baseline, Ironman | 23.4 | 2.0-alpha | Steward mode, ensembles | Lab and extended |
| Steward mode and office seats (ADR 0237) | 12.6, 23.4 | 2.0-alpha | P6 | Lab and extended |
| Planned-economy seat (ADR 0237) | 23.4 | 2.0-alpha or later | Steward mode, ADRs 0084, 0206 | Lab and extended |
| Firm seat (ADR 0237) | 23.4 | 2.0-alpha or later | Steward mode | Extended |
| Consistency solver and editors | 23.2 | 1.4 | P2, P4, P5 complete | Lab and extended |
| Mod manifest (data and parameters only) | 23.3 | 1.4 | Solver | Extended |
| Policy catalog: property, citizenship, information | 23.7 | 2.0-alpha | P6 legislative process | Lab if it passes pattern tests |
| Characters | 23.5 | 2.0-alpha | P6 | Extended |
| Neural surrogate (pulled forward from P10) and strategic AI | 23.8 | 2.0-beta | P5 policy gate | Extended until the gate |
| Diplomatic actions and intelligence | 23.6 | 2.0-beta | P7 | Lab if H7-H9 hold |
| Command layer | 23.5 | 2.0-beta | P7, characters | Extended |
| Capability milestones | 23.7 | 2.0 | P7, P8 | Extended; lab at 3.0 |
| Covert action, non-state actors | 23.6, 23.7 | 2.x | Intelligence, P6, P7 | Extended |
| Emerging sectors | 23.7 | 2.x | P8 | Extended; lab at 3.0 |
| Synthetic worlds, module ABI | 23.3 | 2.x | Solver; Rust crates published | Extended |
| Pre-1995 packs | 23.3 | 2.x (extended), 3.0 (lab) | Capabilities; reanalysis hindcast (ADR 0131) for lab | Extended until 3.0 |

Open decisions for this section:

- [ ] Size of the X-register, and whether it gets its own table in Section 17.
- [ ] Whether a lost mandate ends the run or hands control to the AI by default (23.4).
- [x] Whether the roadmap's After 1.0 table is updated to the releases above: yes, under ADR 0136 (28 Oct 2026).

