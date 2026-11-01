# 0206. Named projects, the 2000 pipeline and the project tape

Status: proposed; amended per ADR 0241 (2026-10-29); amended by ADR 0239 (seats and scoring to 2.0-alpha)
Date: 2026-10-28
Design sections: 5.2, 5.3, 5.5, 12.6, 14.2, 15.4, 15.5, 20.1; ADRs 0034, 0136, 0160, 0163, 0178, 0204, 0205
Phase: P8 (named projects, tape); 2.0-alpha or later (seats and placement verbs, ADR 0239)

## Context

Ordinary capital is a continuous stock (0204), but some assets are lumpy and change the map or the
network: transport links and nodes, ports, power plants, dams, pipelines, LNG terminals, mines.
5.5 proposes projects scored by market-access gain and 0160 gives them reference-class delay and
overrun, but no ADR says what a named project is, which were under way in 2000, or who may place
one. EU4's great projects and Paradox building placement are the comparisons.

Design note: https://claude.ai/artifact/XmuftWfuBvehjp3ou7SYYd

## Options

1. Everything continuous, no named projects.
2. Named projects above a size cutoff per asset type, running through 0204's pipeline.

## Decision

Option 2 (owner, 28 Oct 2026).

- **Named project.** A project with a location (graph edge, node or H3 cell), type, size, owner and
  schedule; above a per-type cutoff in the pack it is named and drawn, below it it is part of the
  continuous stock. It runs through 0204's stages and 0205's capacity; overrun and delay are drawn
  from 0160's reference classes, keyed by the run seed.
- **Completion effects** by type: edge cost, speed and capacity (5.3); RLDC capacity (0034);
  waterway or pipeline edges; deposit development (0178); reach through trade costs (0163).
- **2000 pipeline.** Projects under construction on 1 January 2000 start with their real progress
  where open data exist (Global Energy Monitor trackers, CC BY 4.0; Three Gorges, for one). The
  rest of the pipeline is 0204's steady state.
- **Project tape.** For the 2000-2024 hindcast, projects started after 2000 (Golden Quadrilateral,
  Qinghai-Tibet railway, Nord Stream) can come from a tape, as policy does in 1.0 (0136), or be left
  to the endogenous proposer of 5.5. The scorecard reports both lanes.
- **Who places what** (owner). Private capacity is never placed by the player; it responds to
  prices, credit and incentives. Seats place only state capacity: the corridor planner (0160),
  Steward public investment, SOE investment and concessions (2.0-alpha, ADR 0239), and direct placement of state
  capacity in a planned-economy seat (2.0-alpha or later). The lab can add, cancel or reschedule any project as an
  edit (14.2).
- **Views.** A project map mode (pins with schedule bars) and a construction-capacity heat map.
- Learning on repeated projects of one type in one country (Wright's law, like HOI3's gearing) is a
  lab-off mechanism until it passes ablation (15.7).

## Migration

- P8, in `ceteris-geo` (project sites on the graph) and `ceteris-econ` (pipeline). Pack: per-type
  cutoffs, GEM project status in 2000, the project tape. IAEA PRIS reactor build times and
  reference-class overrun data with licences to check under ADR 0005.
- 1.x: seats and placement verbs.

## Tests

- A named project with zero overrun and delay equals 0160's plain infrastructure edit at
  completion.
- With the tape off and the proposer off, no named project starts after 2000.
- The Golden Quadrilateral from the tape reproduces the district-level effects of Ghani, Goswami
  and Kerr (2016) within their intervals; China's expressways against Faber (2014).

## Consequences

The map shows what is being built and when it will finish, the hindcast can separate the model's
building choices from history's, and the player governs rather than invests. Adds per-type cutoffs
and a tape to the pack.
