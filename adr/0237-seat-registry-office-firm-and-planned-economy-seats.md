# 0237. Seat registry: office, firm and planned-economy seats

Status: proposed
Date: 2026-10-29
Design sections: 2.1, 12.6, 23.4, 23.9; ADRs 0084, 0126, 0130, 0136, 0160, 0162, 0195, 0203, 0206, 0210
Phase: 1.3 (registry, Steward, office seats); 1.x after 1.3 (planned-economy seat, trade-core seats); extended profile (firm seat)

## Context

DESIGN defines one seat, Steward (12.6): a whole state's levers. Other ADRs assume more. 0126
puts a trade-ministry and a central-bank seat in the agent gym, 0130 says "seats so far are
offices" and lists firms, 0203 builds a firm seat on granular firms, and 0206 has a
planned-economy seat place state capacity. None of them says what those seats control. The
0130 test, that every seat's action space is a subset of the Steward's, cannot hold for the
idea, founder, mediator, citizen or commodity-trader seats, which do not act for a state. The
owner chose (2026-10-29) to define seats once, including the firm and planned-economy seats, and
to ship no seat before 1.3 Steward.

## Options

1. Define each seat when its phase starts.
2. One registry: a seat is a typed record, and every seat ADR fills it in.

## Decision

Option 2.

**Record.** A seat names:

- Actor: a state office, a firm, a movement, a person, or an outside party (0130, 0160).
- Action space: a subset of that actor's levers. For a state office, a subset of the Steward
  action space (12.6).
- Observations: what the actor's AI sees, with the fog of 0065 where it is on.
- Calendar: decision points (rate meetings, budgets, board meetings), as auto-pause triggers.
- Objective presets (23.4, 0161) and the AI baseline it is scored against under CRN.
- Remainder: who holds the actor's other levers. In 1.x that is the 0210 rule or tape; from P6 it
  is the AI holder of the body that owns the competence (0195).

**No seat before 1.3** (owner, 2026-10-29). Steward (economic levers) is the first seat. The
commodity trader and corridor planner of 0160, the agent gym of 0126 and every seat below follow
it. 0160's "P3" for the trader becomes "after 1.3".

**Office seats (1.3).** Restrictions of the Steward action space to one office; the rest of the
state runs on its 0210 rules.

| Seat | Levers | Remainder |
| --- | --- | --- |
| Head of government | All Steward levers except those of an independent central bank | Central bank on its rule |
| Finance minister | Taxes, spending, transfers, debt issuance and maturity (9.6), IMF requests (9.4), fiscal commitment devices (0162) | Monetary, trade and safety-net levers on rules |
| Central bank governor | Policy rate, balance sheet, FX intervention and regime (9.2), lender of last resort and macroprudential tools (9.11), monetary commitment devices (0162) | Fiscal and trade levers on rules |
| Trade minister | Tariffs, quotas, export controls, trade agreements offered (8.5) | Fiscal and monetary levers on rules |

Where the central bank is not independent in the 2000 data, the head of government holds its
levers. From P6 the office-to-lever map is read from the body competences of 0195, so a reform
that moves a competence moves the lever between seats; until then it is this table.

**Planned-economy seat (1.x after 1.3, needs 0084 and 0206).** A state seat for a state whose
SOE share or 0084 price controls pass a threshold in the 2000 data. Levers: the Steward levers,
plus direct placement of SOE capacity (0206), output targets for SOE cells, administered prices
and rationing (0084), and directed credit through state banks. Output targets enter the SOE
cells' production decision as a quantity floor; the shortfall shows as rationing (8.2) and in
SOE losses on the fiscal accounts (9.6). Presets: plan fulfilment, real income of the bottom
40%, regime survival. Private capacity still only responds to prices and credit, as 0206 says.

**Firm seat (extended profile, after 0203's granular firms).** One explicit top firm of 0203.
Levers: investment in the firm's existing cells as private investment orders (0204, 0206), entry
into a new (province, sector) cell, its markup where Atkeson-Burstein markups are on (0030),
sourcing and inventory, leverage and payout. The firm stays part of its cells (0203): it never
adds output, a modifier or a bonus. Below the 0160 size threshold it is a price taker. Presets:
firm value (discounted payouts), market share, survival. Baseline: the firm's own decision rule.
This amends 0206 in one respect: a firm seat places the investment its rule would have
placed, under the same credit and construction-capacity limits.

**0130 test.** "Each seat's action space is a subset of its actor's action space"; for state
offices that actor is the Steward.

**Sandbox and Ironman.** Sandbox may switch seats; scenarios and Ironman fix one. Several seats in
one state share the lockstep multiplayer of 0130.

## Migration

- DESIGN 12.6 and 23.4 gain a sentence pointing here when accepted; 23.9 gains rows for office
  seats (1.3), the planned-economy seat and the firm seat.
- 0126's agent gym starts at 1.3 with the trade-minister and central-bank-governor seats.
- 0160's phase line reads "after 1.3 (commodity trader), P8 and after 1.3 (corridor planner)".

## Tests

- A seat that takes no action reproduces its baseline branch bit for bit.
- The office seats' action spaces cover the Steward action space with no lever in two offices
  (before P6).
- With a planned-economy seat setting every output target to the rule's own output, the state
  hash equals the rule run.
- A firm seat that chooses its rule's orders reproduces the granular-firm run.

## Consequences

Every seat is one record, so a new seat is a short ADR and the agent gym, scoring and chrome
(0140) read seats generically. Office seats cost little once Steward exists, about 1-2
part-time weeks for the four. The planned-economy seat waits for 0084 and 0206 and costs about
2-3 weeks; the firm seat waits for granular firms and costs about 3-4 weeks.
