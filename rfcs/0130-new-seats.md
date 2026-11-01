# 0130. New seats: idea, founder, coalition partner, mediator, quartermaster, citizen

Status: proposed
Date: 2026-10-28
Design sections: 10.5, 10.7, 11.2, 11.6 to 11.8, 12.6, 23.4, 23.5; ADRs 0067, 0115
Phase: P6, 2.0-alpha (idea, coalition); P7, 2.0-beta (founder, mediator, quartermaster); citizen as a minor once its dependencies land (ADR 0136)

## Context

Seats so far are offices: heads of government, ministers, central bankers, the IMF, firms. Six
roles use the model in ways no office does. Brainstorm:
https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (ideas 36, 38 to 41 and 52).

## Options

1. Status quo.
2. Six seats, each a restricted action space over existing mechanisms with its own objective
   and baseline.

## Decision

Option 2. Each seat is scored against its AI baseline under CRN (23.4).

- Idea: an ideology or movement with no territory. Levers: which groups to court, framing,
  party alliances, protest or electoral strategy. Wins by changing laws across countries. Acts
  through ideology diffusion (10.7) and ADRs 0055 and 0071.
- Founder: a separatist or independence movement on a few cells that builds a state:
  recognition, currency, tax capacity, a constitution (ADR 0129). Territory moves by cell
  labels (ADR 0115).
- Coalition partner: several players share one government with separate voter bases; each can
  bring it down. Turns are cabinet meetings. Scored on vote share and policy wins. Uses
  lockstep multiplayer.
- Mediator: a third party in a war or frozen conflict. Levers: proposals, guarantees,
  monitoring missions, donor side payments, timing. Scored on duration, deaths and recurrence
  within ten years, through 11.2 and 11.8.
- Quartermaster: the war economy. Mobilization through the input-output table, rationing,
  labour conscription, import substitution under blockade, war finance; the front is delegated
  (23.5).
- Citizen: one person inside the world. Levers: study, migrate, work, save, vote, protest. The
  lineage view becomes playable; the world barely notices, which is the point.

## Migration

- P6 (2.0-alpha): idea and coalition seats. P7 (2.0-beta): founder, mediator and quartermaster. Citizen: a minor once its dependencies land (ADR 0136).

## Tests

- Each seat's action space is a subset of the Steward action space; a seat that takes no
  actions reproduces its baseline.

## Consequences

The game gains roles that make peace, production and ordinary life playable, without new
mechanisms.
