# 0253. Royal houses, claimants and emergent restorations

Status: accepted (owner, 2026-10-30)
Date: 2026-10-30
Design sections: 10.2, 10.5, 10.7, 10.8, 13.5, 23.5; ADRs 0066, 0136, 0139, 0183, 0185, 0186, 0187, 0189, 0190, 0192, 0195, 0198, 0222, 0237, 0239, 0241
Phase: P6 (2.0-alpha) for houses and mechanics; P7 (2.0-beta) for the claimant seat and installation by an occupier

## Context

The owner asked (2026-10-30) for real royal houses with their people and family trees as of 1
January 2000, deposed houses and pretenders included, and for a way to play a restoration.
ADR 0183 gives real people a character record but no kinship; ADR 0195 has a membership rule
"heredity" without a succession law; ADR 0066 has succession crises but no claimants. 1.0 has no
politics (0136) and no seats (0239), and nothing after t0 may be scheduled (0241). Design page

Four questions: phasing (D1), which houses (D2), how far real trees and living relatives go (D3),
and how a restoration comes about (D4).

## Options

D1, phasing:

1. Houses and mechanics at P6 (2.0-alpha), claimant seat at 2.0-beta, nothing in 1.0.
2. Option 1 plus the house tables in a 1.x pack as a read-only dynasty view.
3. Houses at P6, no claimant seat; god mode only.

D2, which houses:

1. Sovereign reigning houses, subnational rulers with statutory powers, deposed houses with a
   live claim in 2000.
2. Sovereign reigning houses only.
3. Option 1 plus every traditional authority and mediatised house.

D3, real trees and living relatives:

1. A notability gate; ancestors to 1800 for display; spouses and children after 2000 generated;
   no marriage between two real people.
2. Full Wikidata trees at any depth; real people may marry each other in a run.
3. Heads and first heirs real, the rest generated.

D4, restoration:

1. A royalism axis in the policy space, with restorations only through paths already in the model.
2. Option 1 plus a dedicated restoration hazard.
3. God mode only.

## Decision

D1 option 1, D2 option 3, D3 option 2, D4 option 1 (owner, 2026-10-30). There is no separate
monarchy mode: every run from P6 carries the houses.

**Objects.**

| Object | Fields | Reuses |
| --- | --- | --- |
| Character (0183) | + father, mother, spouses with dates, house, eligibility flags (renounced, unequal marriage, religion rule) | Kin edges are data, unlike 0185 relations, which only logged events create |
| House | Wikidata id, head, cadet-of, succession law, residence state, exile flag, wealth stock for sovereign houses (household sector, 9.8) | An 0189 organization with kinship membership |
| Claim | Title, target (state or 0190 partition unit), basis, legal status in the target (barred, tolerated, recognised), salience | Separate from 0222 territorial claims; one title may carry several rival claims |
| Succession law | A pure function on the kin graph returning the ordered line | Parameterises 0195 "heredity" |

Laws: absolute primogeniture, male-preference primogeniture, agnatic primogeniture, agnatic
seniority with designation, designation by the ruler within the house (family-council fallback),
election or rotation among eligible houses. A reigning house changes its law by bill through its
0195 bodies; a deposed head changes the house law by personal act, at a hazard rising with age
and with the absence of an eligible heir under the current law. Abdication is a hazard on the
head's age, health (13.5) and the house's own abdication record before 2000; a minor heir opens a
regency body; no eligible heir or rival lines open the 0066 crisis state.

**Houses in the 2000 data (D2).** Sovereign reigning houses; subnational rulers with statutory
powers (Malay rulers, UAE emirs, Yogyakarta), as 0195 bodies with heredity and the
self-constitution competence; deposed houses with a live claim; and every traditional authority
and mediatised house. Where Wikidata documents a house thinly, as for most chieftaincies, the
documented members are kept and the rest of the house, up to a fixed size per house, is
generated from its region's pop cells and naming tables (0186), as 0183 fills every bench to the
same size. Traditional authorities hold the 0195 "traditional authority" status where they have
statutory roles and act only as 0189 organizations where they do not.

**Trees and living people (D3).** People and kin come from Wikidata at any depth the source links
them; claims and their disputes come from a curated file in `crates/ceteris-pack/config/` naming
Wikidata ids and the basis of each claim. In a run, real people may age, die by life tables and
the existing hazards, abdicate, renounce, be deposed, restored or elected, and marry, including
each other. Children born in a run are generated people. The guards of 0183 hold: the run-level
"Real names" switch, role labels in the lab, no likeness (0187), and every event touching a real
person worded as an event of this run. Fields stay minimal: birth and death dates, sex, kin,
titles, positions, residence; religion only as a constitutional eligibility flag; no health,
sexuality or private wealth. A denylist of Wikidata ids in `sources.toml` removes a person from
every build on request.

**Restoration (D4).** In each state with a live claim, the 10.5 policy space gains one axis,
support for restoring house h. Each pop cell's position moves by the 10.7 opinion dynamics
toward a target set by lived memory of the monarchy (0139 kernel), alignment with the house's
church and ethnic group, the current regime's legitimacy deficit (0192) and the claimant's
salience (residence, public acts). There is no restoration hazard and no new event. A restoration
happens only by paths already in the model:

- **Vote.** Latent support becomes a movement (0189) and a party plank or a new party (10.5);
  a bill changing the head-of-state body to heredity in house h passes the 0195 veto gates,
  usually a referendum.
- **Transition.** After a revolution, civil war or occupation, the constituent body chooses a
  head-of-state form; a claimant is one option, scored by the bargaining parties for neutrality,
  royalism and patrons' preferences.
- **Force.** A personalist ruler designates a claimant as successor (0066), or a military faction
  or occupier (P7) installs one.

The same paths run in reverse: abolition by referendum or revolution, a realm becoming a republic.
A claimant may also stand for office as any character can. God mode can install or depose a house
at any tick, logged as an intervention (14.2).

**Claimant seat (2.0-beta).** Under 0237 and 0239. The seat holds the head of a deposed house and
passes to the heir on death (0198 continuation); it is scored against the same seat played by the
AI. Its verbs are actions every AI claimant has: return from or stay in exile under the state's
entry law, found or endorse a movement, court interest groups, fund them from house wealth, raise
salience, stand for office, negotiate in a transition, designate an heir, change the house law,
settle with a rival line, arrange marriages. On a restoration the seat becomes the monarch's
office under the restored constitution, an office seat of 0237 (2.x).

**Facts after t0.** No post-2000 royal event is an input. Simeon II as prime minister (2001),
Nepal's abolition (2008), house-law changes (Romania 2007, United Kingdom 2013), Barbados becoming
a republic (2021) and the abdications are validation patterns (0107) or members of the optional
`replay-2000-2024` set (0241).

## Tests

- Each law function reproduces the published 2000 line of succession, to depth ten, for the United
  Kingdom, Spain, the Netherlands, Sweden, Norway, Belgium, Denmark and Japan.
- Every house with a live claim has a head and a non-empty line or an open crisis on the start
  date.
- Switching "Real names" off changes no state hash (0183).
- Unconditional 2000-2024 runs (0241): sovereign restorations per decade, abolitions and
  abdications within the reference-class rates fitted on 1900-1999; a run producing several
  restorations per decade fails.
- The royalism axis has three free parameters (memory, legitimacy, salience weights), fitted on
  monarchy referendum shares before 2000 (Italy 1946, Greece 1974, Brazil 1993, Albania 1997,
  Australia 1999) and recorded in the register (DESIGN 17).

## Consequences

The 2000 start carries its real houses and pretenders, and a restoration is a possible outcome of
simulated politics rather than a scripted event. The full-tree and all-chiefs choices make this
the largest set of real living people in the pack, many of them private; the switch, the wording
rule and the denylist are the guards, and the risk of fiction about real reigning monarchs under
lèse-majesté laws stays with the user's choice of the switch. Data are uneven across houses, so
chieftaincies lean on generated members. Cost about 9-14 part-time weeks: import with
chieftaincy fallback 3-5, laws and hazards 1-1.5, royalism and paths 2-3, dossier family section
1-1.5, claimant seat 2-3. Sources: Wikidata (CC0); Wikipedia lists (CC BY-SA 4.0) for
cross-checks only; constitutions and house laws; Archigos and REIGN (0066, licences to verify
under 0050); Kokkonen, Møller and Sundell's European monarchs data for crisis priors (licence to
verify). The Peerage, Geni, WikiTree, FamilySearch, the Almanach de Gotha and Burke's are not used.
