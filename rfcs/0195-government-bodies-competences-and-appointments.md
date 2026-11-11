# 0195. Government as bodies with scopes and competences

Status: proposed; amended by ADR 0253 (succession laws for heredity)
Date: 2026-10-28
Design sections: 9.6, 9.8, 10.1, 10.3, 10.5, 10.6, 14.2, 20; amends ADRs 0068 and 0129; see ADRs 0047, 0067, 0083, 0140, 0163, 0186, 0187, 0189, 0190
Phase: P6 (2.0-alpha)

## Context

ADR 0190 makes administrative divisions an open, editable registry and defers the government
side to P6. The owner wants to: give units different powers (special economic zones, directly
administered cities, autonomous regions); appoint regional and local leaders from the centre with
a few parameters (Chinese cadre rotation, Saudi emirates given to princes); join other states such
as Jordan or the UAE while they keep their own succession; see it all as an org chart with
demographic and economic figures; and pass changes during a run as bills that the relevant bodies
approve or reject.

Today, 10.3 gives provinces an autonomy level, 0068 splits revenue and spending authority between
two tiers, and 0129 has constitution modules with no territorial scope. The government-simulation
design of 2026-10-23 (original project chat) proposed composable bodies; it was never recorded.

Design doc with diagrams

## Options

1. Status quo: autonomy levels and a two-tier fiscal split.
2. A fixed menu of government forms per tier.
3. One body primitive with a scope and competences; tiers, statuses, unions and reforms are data
   over it.

## Decision

Option 3. Option 2 cannot express uneven or invented arrangements.

**Body.** A body is membership rule (election, appointment by a named body, heredity, lot,
seizure), decision rule (single office, majority, supermajority, weighted, unanimity), agenda
(bodies that may propose to it), competences, enforcement (which coercive and administrative
capacity of 0067 obeys it) and a budget account in the government sector of 9.8 (central, state
and local as in the SNA). Its scope is a unit of a 0190 partition, a pop filter (confessional
courts, language communities), or both. A tier is a partition plus a body template stamped on each
unit; any unit may override its instance.

**Competence.** A competence names a lever of the policy catalogue (23.7) and how this body's
setting composes with others' in a cell:

| Composition | Effective value |
| --- | --- |
| Add | sum over bodies covering the cell (income tax layers) |
| Floor | max (minimum wage) |
| Ceiling | min (debt brakes, state-aid caps) |
| Override | lowest body that sets it, unless a higher body preempts |
| Exclusive | one body only (currency, tariffs) |
| Delegated | held by delegation, revocable by the delegator |

Residual powers go to the top or the bottom body, a per-constitution switch. Effective values per
economic unit are computed through a sparse overlap matrix (economic units x admin units,
population-weighted) rebuilt only on edits, so the monthly tick reads flat arrays as now. The EU
of 10.3 is a body whose scope is a partition over owners; no separate mechanism.

**Statuses.** The statuses of 0190 gain their effects here: each is a named bundle of competence
overrides and tree changes (special economic zone, direct administration, autonomous region,
special administrative region, military zone, traditional authority), editable and extendable.

**Fiscal tiers.** Each body with a budget collects the instruments it holds under 9.6. Transfers
between tiers are rules owned by a body (formula equalization, conditional grants, revenue
sharing). Subnational borrowing and bailout expectations are a competence, so a soft budget
constraint (Rodden 2006) can be switched.

**Appointment policy.** A membership rule "appointed by body X" takes a policy: candidate pool
(0186 career pools: cadres, royal family, military, technocrats, local notables), weights on
loyalty, competence and seniority, a balance rule across factions or family branches, avoidance
of home regions, term and rotation, and an evaluation rule for promotion and removal. At each
vacancy or rotation date posts are filled by a deterministic assignment on those scores (stream
keyed by body and tick); the player can lock or hand-pick any post. Each fill is an in-world event
with causes. The appointee's traits enter implementation (0083), corruption, reach (0163) and
defection risk; loyalty trades off against competence (Egorov and Sonin 2011), and growth-target
evaluation raises growth and statistical inflation (Li and Zhou 2005; 0065).

**Self-constitution and unions.** The right to set one's own bodies' membership rules is a
competence. A member that holds it keeps its succession whatever the centre's appointment policy
(Jordan's monarchy, the UAE's rulers and Federal Supreme Council); one that does not falls under
it. Customs union, confederation, federation, personal union, protectorate, annexation with
autonomy and full annexation are presets of which competences move to the union, set by god-mode
edit or by treaty (0047) accepted by each member's bodies. Trade costs at an internal boundary
follow who holds tariffs, currency and regulation, not the owner label; the owner label changes
only where the economic partition does (0115). Secession is a competence with its own rule (10.8,
11.6).

**Reforms as bills.** An in-world edit of partitions, statuses, bodies or competences is a bill: a
bundle of the same edit objects as god mode, drafted on a scratch branch with the consistency
solver's preview (seats under new districts, transfers, winners and losers). It is routed through
every body whose competence it touches, each a veto gate with its own decision rule, including
referendums in affected units and regional consent where the constitution requires them, and
tabled only by a body with agenda rights. A court with judicial review can strike it down. It
applies at its effective date if it passes. The vote model inside each gate (members' yes
probability, whip counts, referendum vote intention) belongs to the legislative-votes design of
the same date, which takes self-interest from the bill's preview as an input. God-mode edits still
bypass gates and are logged apart (14.2).

**Views.** The org chart draws the body graph as a tree; each node shows aggregated figures
(population, age, urban share, GDP and per head, growth, sector mix, ethnic and religious shares,
budget and net transfers, approval, reach) and its office holder's dossier line (0187), coloured
by who fills the post. Treemap, sunburst and a unit-by-competence matrix are other layouts. Edits
from the chart are the same intervention types as on the map.

**Initial state.** Bodies and competences for 2000 from the Regional Authority Index (self-rule
and shared rule by tier, about 90 countries); unitary with deconcentrated provinces elsewhere.
Subnational accounts from IMF GFS and the OECD subnational finance data; 0068's federal structure.
Licences to audit.

## Tests

- With one national body holding every competence, results equal the current single-government
  model bit for bit.
- The default 2000 bodies reproduce each state's 2000 revenue split by tier within tolerance of
  GFS.
- A null bill and a failed bill change no state hash.
- An appointment policy with all weights zero and a fixed seed fills posts identically across
  runs.
- A member holding self-constitution is never assigned by the centre's appointment policy.

## Consequences

Any constitution, real or invented, is data, and reforms, power grabs, unions and appointments
become play with logged causes. The cost is a policy catalogue in which every lever names its
composition rule, and calibration of appointment effects. 0068's two-tier split and 0129's
modules become presets over this primitive. Cost about 8-12 part-time weeks after P6's bodies and
pops, overlapping 0129; appointment policies need the 0186 pools.
