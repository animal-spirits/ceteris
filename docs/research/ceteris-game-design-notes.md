# Ceteris game design notes

Oct 24, 2026 · @Antonio

The simulator becomes a game when the player has a seat with limited power, a goal, fast feedback on each decision, and a pace set by events. The architecture (determinism, forks, ensemble lanes, attribution) makes several of these cheaper and better than in any existing strategy game. The one feature no competitor can copy is scoring the player's decisions against the counterfactual and across the ensemble.

Status: speculative and non-binding. Nothing here is decided, and none of it becomes an ADR before 2.0.

## Simulation versus game

A simulation answers "what happens if"; a game asks the player to make something happen under constraints. Four ingredients separate them.

| Ingredient | Simulation (today) | Game |
| --- | --- | --- |
| Role | Observer holding every lever | A seat with a few levers, each with a cost |
| Goal | None; outcomes are reported | Objectives, a score, ways to lose |
| Feedback | Results at the end of a run | A visible response within days; causes explained later |
| Pacing | Fixed tick rate | Time compresses in quiet years and stops in crises |

Rules that keep the game layer from damaging the research model:

1. The game writes causal state only through the intervention list, the same path as lab interventions, so every session is a reproducible branch.
2. No scripted outcomes. Scenarios set starting conditions and objectives; events emerge from the model.
3. Presentation may hide things (fog of statistics, storyteller selection), but the truth is always recoverable in lab mode.
4. Difficulty comes from constraints on the player (political capital, information, delegation), never from bending the model.

&#91;embedded content: core loop · 4 steps, 2 on forks\]

The loop repeats at each decision point on the seat's calendar. The baseline and the post-mortem run on forked branches, so they never touch the player's world.

## Roles and agency

The player takes a seat, not the god view: each seat sees part of the world, pulls a few levers, and pays for every pull.

| Seat | Sees | Levers | Main constraint |
| --- | --- | --- | --- |
| Head of government | National dossiers, polls, staff forecasts | Budget, reforms, appointments, diplomacy, war | Political capital, coalition partners |
| Finance minister | Fiscal accounts, bond market | Taxes, spending, debt issuance, IMF talks | The head of government, the bond market |
| Central bank governor | Reported inflation, expectations, banks | Policy rate, balance sheet, FX intervention, macroprudential rules | Mandate, credibility, legal independence |
| Opposition or protest leader | Approval, networks, repression | Mobilization, coalitions, platform | The state's capacity to repress |
| General or theatre commander | Intelligence dossiers, logistics | Deployments, operations, officer appointments | Civilian orders, supply |
| Macro fund | Prices, data releases, rumours | Positions in bonds, FX, commodities, equities | Capital, leverage, liquidity; a price taker by default |

**Political capital.** One stock per seat, earned through approval, wins and coalition strength, and spent on reforms, appointments and unpopular moves. Policy implementation (ADR 0083) turns a decree into a process: it needs administrative capacity and time, and compliance depends on trust.

**Delegation everywhere.** The command spectrum planned for war applies to every domain: micromanage, set directives, or delegate. Delegates are characters with competence and biases, so a hawkish governor you appointed keeps rates high on his own. Appointments become the main decisions, and 200 countries stay playable.

**Seats in sandbox.** Sandbox allows switching seats or playing a whole state with every lever (the classic grand-strategy mode); scenarios fix the seat.

## Goals, scenarios and scoring

Score the player against the world that would have happened without them. Forks and ensemble lanes (ADR 0024) make this nearly free, and no existing strategy game can do it.

**Scenarios** are a start date, a seat, an objective and a deadline. Examples:

- Argentina, December 2001, finance minister: end the crisis without hyperinflation.
- Greece, 2010, head of government: restore market access without leaving the euro.
- Brazil, mid-2002, central bank governor: hold the currency through an election-driven sell-off.
- A North African autocracy, January 2011, either side: stay in power, or force a transition without civil war.
- Nigeria, 2000, head of government: turn the oil boom into non-oil output by 2030.

**Counterfactual score.** Each scenario runs a baseline branch beside the player's, with the seat held by its AI policy rule. The score is the player's outcome minus the baseline's on the objective's metrics: "you beat history by 2.1 points of GDP." The project's name says exactly this.

**Ensemble decision score.** At each major decision, run the player's choice and a few alternatives (the delegate's default, the AI's choice) across N lanes from that point. The post-mortem scores the decision on the distribution, not the realized path: "your 2008 stimulus raised median 2012 GDP across 64 worlds; the recession you got was bad luck." This separates skill from luck, the main frustration in strategy games with heavy randomness. The cost is N lanes times the remaining horizon, so it runs in the background or only at the post-mortem.

**Losing.** Removal from office (election, impeachment, coup, revolution), state collapse, or a missed deadline. Losing a seat can continue as opposition or exile (see Stakes).

**Sandbox goals.** Players set their own objectives, tracked like a scenario's; achievements reward hard outcomes, such as Argentina without a default from 2000 to 2030.

## Feedback and legibility

Macro effects take quarters to years; a game needs a visible response within days. Feedback comes in three layers:

1. **Immediate (days).** Markets reprice (spreads, exchange rate, equities), polls move, the press and rivals react, crowds gather. These are already model outputs; after a decision the UI shows them first.
2. **Lagged (months to years).** The real effects arrive as "consequences" notices that link back to the decision behind them.
3. **Explained (on demand).** Shapley attribution of any outcome, including the player's own decisions: "this recession: 40% your 2004 rate path, 35% the oil shock, 25% your main trading partner." These are Victoria 3-style tooltips with real causes behind them.

**Telegraph the baseline.** Before a decision, show what the staff expects if the player does nothing, as Into the Breach shows enemy moves. The baseline branch already computes this.

**Fog of statistics (ADR 0065).** The player sees reported numbers, with revisions and manipulation, not the truth. The post-mortem reveals the true series.

**Advisers.** Each seat has a small staff (economist, pollster, intelligence chief) who brief, forecast and disagree. Their forecasts are fan charts biased by their competence, so the player learns whom to trust, as with staff reports in Football Manager.

**UI rules.**

- A map mode for every variable.
- The decision screen shows the levers, the staff forecast for each option, the baseline, and the political cost.
- Any number is one click from its causal breakdown.
- The event chronicle is terse (ADR 0078); players read meaning into sparse cues.

## Pacing

Most real years are quiet, so pacing should run them fast and stop for what matters.

- **Institutional calendars as turns.** Budgets once a year, eight rate meetings, elections on their dates, summits, IMF reviews. Each seat's calendar sets its decision points, so play has a rhythm without artificial turns.
- **Auto-pause** on crises the seat cares about: a bank run, a coup attempt, a border incident, a market crash.
- **Time compression** between decision points, with the storyteller (ADR 0078) choosing which events reach the player and when.
- **Storyteller personas**, as in RimWorld: a steady narrator surfaces slow trends, a dramatic one leads with the crises the model produces anyway. Personas change presentation, never state.
- **Session length.** A scenario fits an evening; a 2000 to 2100 sandbox is a campaign over weeks.

## Stakes, failure and replay

Forks are free in the lab, so stakes have to be designed in.

- **Ironman versus lab.** Ironman means one save, no forks, no rewinds; lab means fork anything and inspect the truth. Scores and achievements count only in ironman.
- **Post-mortem.** After an ironman run, the game shows the branches not taken at the player's five biggest decisions, and the true statistics behind the reported ones.
- **Seeds.** Each playthrough perturbs the seed and, optionally, draws parameters from their posterior, so history cannot be memorized. The 2008 crisis can come early, late, milder or not at all, as the model allows. A historical mode keeps the calibrated path for players who want it.
- **Asymmetric starts.** Every state is a different puzzle: commodity exporter, ageing creditor, fragile coalition, frozen conflict.
- **Failure as content.** Losing office continues as opposition or exile, with a path back. Default, hyperinflation and collapse are long arcs with recovery (ADR 0086), not game-over screens; Paradox players enjoy recovering from disaster as much as avoiding it.

## Opponents and characters

Opponents that form expectations remove the dominant strategies that break most strategy games.

- **Expectations as anti-exploit.** Markets and rival AIs learn the player's policy rule (the Lucas critique, as gameplay). A trick that works once, such as surprise inflation or a peg defended at any cost, stops working once it is predictable.
- **Rival personalities and memory.** AI leaders (ADR 0066) carry traits (risk aversion, ideology, time horizon, paranoia) and remember: the leader you sanctioned in 2005 remembers it in 2015. Reputation is a stock per pair of leaders and per state.
- **Characters with careers.** Governors, ministers, generals and dissidents rise and fall and enter the legends (ADR 0078). Players remember the governor who saved the currency and the general who lost the war.
- **Dossiers, not stat sheets.** Quality shows as signals (record, training, rumours, gameable medals), as already planned for officers. Players learn competence by watching performance, as with scouting in Football Manager.
- **Difficulty** comes from rival quality, from rule-following AIs to forward-looking planners, and from how much each side knows.

## Multiplayer, challenges and community

Determinism turns several expensive features into cheap ones, because a session is fully described by its seed, packs and input log.

- **Lockstep multiplayer.** Only player inputs are synced; every client runs the same simulation, and the existing state-hash check detects desyncs. Red-team mode (ADR 0079) becomes multiplayer, including several seats in one state: president, governor and opposition leader.
- **Asynchronous play.** Decision points on institutional calendars suit sessions spread over days.
- **Verifiable leaderboards.** A score is submitted with its input log, and a server replays it to verify. To cheat, a player would need a valid input sequence that actually scores.
- **Daily and weekly challenges.** One seed and one scenario for everyone, as in Spelunky 2 and Slay the Spire.
- **Replays and take-overs.** A session file is its input log: anyone can replay it, watch it, or fork it at any point and try their own choice.
- **Modding.** Parameter and data mods are packs, so modded runs stay reproducible. Mods that change mechanics need a plugin story (an open question).

## Example sessions

Five sketches of what an evening of play looks like in different seats.

**Central bank governor, Brazil, mid-2002.** The leading candidate's rhetoric spooks markets, and the real and dollar-linked debt are both sliding. Levers are the Selic, reserves and an IMF line; the staff disagree on whether a hike signals strength or panic. Each meeting telegraphs the baseline, and the post-mortem shows how the ensemble would have scored a bigger hike.

**Finance minister, Greece, 2010.** The reported deficit is revised up in the first month (fog of statistics). Every austerity measure costs political capital, moves the next poll and the spread on the same day, and arrives in output over two years. Losing means a disorderly exit or a fall from office; the score compares the player's unemployment and debt path with the baseline branch.

**Opposition leader, a North African autocracy, January 2011.** The levers are calls to protest, coalition offers to unions and the army, and a platform. Mobile penetration and a downturn lower protest thresholds (ADR 0071); repression capacity and elite splits decide the response. Winning is a transition without civil war, and the post-mortem reveals the regime's true intentions.

**Macro fund, September 2008.** The player trades on reported data, rumours and staff research, never the truth. Because the fund is a price taker, the world's path is the same in every run; skill is reading it early. The ensemble post-mortem separates good calls from lucky ones.

**Head of government, Nigeria, 2000 to 2040 (sandbox).** Oil revenue, federal bargaining, insurgency, urbanization and a young population. Appointments matter most: a competent finance minister and central banker run their domains while the player handles coalitions and security.

## Pitfalls

The biggest risk is a game that is correct and tedious; most of these fixes reuse machinery the simulator already has.

| Risk | Why it happens | Mitigation |
| --- | --- | --- |
| Spreadsheet fatigue | Thousands of pops, goods and prices | Delegation; default views with a few levers; depth one click away |
| Quiet decades | Real history is mostly slow | Scenarios, time compression, storyteller |
| Solving the model | A deterministic world with fixed parameters | Perturbed seeds and posterior draws; adaptive expectations |
| Save-scumming | Forks are free | Ironman for scores; lab mode for exploration |
| Opaque causes | Rich models feel random | Attribution panels; consequences linked to decisions |
| Slow turns | Heavy computation per tick | Fast systems in the tick; ensemble scoring in the background |
| Harsh realism | Real constraints leave few good options | Difficulty from information and capital, never from bending the model |
| Sensitive content | Real states, leaders, wars and atrocities | Generic party labels (already planned); a policy on real people and recent wars |

## Mapping to the architecture

Most game features are a UI and a thin rule layer over mechanisms already in the design.

| Game feature | Builds on | New work |
| --- | --- | --- |
| Seats and levers | Intervention list, Steward action space | Lever set per seat; seat UI |
| Political capital | Approval, coalitions (P6) | The stock, its sources and its costs |
| Delegation | Steward planners, generals (P7) | Delegate characters for every domain |
| Policy as a process | Implementation and diffusion (0083) | Decision screen showing progress |
| Counterfactual score | Branches and forks | Baseline branch per scenario; score metrics |
| Ensemble decision score | Ensemble lanes (0024) | Alternative-choice runs; post-mortem UI |
| Telegraphed baseline | Baseline branch, staff forecasts | Forecast display per option |
| Attribution feedback | Shapley attribution, channel shutdown (0049) | Attribution to player decisions |
| Fog of statistics | True and reported statistics (0065) | Reveal at the post-mortem |
| Pacing | Storyteller (0078), institutional calendars | Auto-pause rules; personas |
| Adaptive opponents | Expectations (0082), leaders (0066) | AI difficulty levels |
| Multiplayer | Determinism, state hashes, red-team mode (0079) | Input-sync netcode; lobby |
| Leaderboards | Determinism, input logs | Replay verification service |
| Replays and take-overs | Branch metadata | Take-over UI |
| Modding | Packs and the sources manifest (0005) | Scenario format; mechanic plugins |

## Open questions

- [ ] Which seat ships first? A central bank governor or finance minister has the fewest levers and sits on the most mature module.
- [ ] Real leaders: are post-2000 leaders named, or generated from the start date on?
- [ ] Recent wars: are they playable scenarios, or only reachable through emergent play?
- [ ] How many lanes can ensemble scoring afford per decision on a MacBook, and does it run live or only at the post-mortem?
- [ ] Can the player switch seats in ironman?
- [ ] Multiplayer scope: co-op seats in one state, competitive states, or both?
- [ ] Mechanic mods: how do they coexist with reproducibility and verified leaderboards?
- [ ] Does any game feature change the licence picture (ADR 0050), for example a store release?

## Games worth studying

Each of these solves one of the problems above well.

| Game | What to take |
| --- | --- |
| Victoria 3 | Pops and markets UI; tooltips that explain prices |
| Hearts of Iron III and IV | Command spectrum, front lines, logistics, AI generals under orders |
| Crusader Kings III | Characters with traits, memory and dynasties; stories come from people |
| Democracy 4 | The policy web: visible links from policies to voter groups |
| Suzerain | A president with limited power; ministers with their own agendas |
| Football Manager | Delegation to staff; scouting with uncertain attributes; staff reports |
| RimWorld | Storyteller pacing and personas |
| Dwarf Fortress | Legends mode; emergent history |
| Frostpunk | Laws as hard trade-offs; discontent and hope as visible stocks |
| Into the Breach | Telegraphing what happens if you do nothing |
| Slay the Spire, Spelunky 2 | Daily seeded challenges with shared leaderboards |
