# 0143. State names: localized, dynamic and editable

Status: proposed
Date: 2026-10-28
Design sections: 14.2, 20; ADRs 0078, 0125, 0140, 0141, 0142
Phase: P3 (real names by date, game-language exonyms), P6 (new and renamed states)

## Context

The owner asked for localized names, and for dynamic state names the user can change, drafted
by AI or by a procedural fallback. Real countries change names (Zaire 1971, Burma to Myanmar
1989, Eswatini 2018, Türkiye 2022) and new states need names in every member language and every
game language. Working names panel in the proposal (section "Names").

## Options

1. English names only, fixed per state.
2. Hand-written name tables per state.
3. Real names from data for real states; for new states a per-language grammar, with AI
   alternatives and hand edits on top.

## Decision

Option 3. Names are presentation, except that a chosen name is stored as an edit.

- Real states: endonyms and names in every game language from Unicode CLDR (Unicode licence) and
  Wikidata (CC0), with dated changes; the name shown is the one in use on the game date. Real
  renames apply only when the run reaches them, or their cause.
- Who says what: a government's own documents use its official form; foreign press in the
  chronicle uses the form its country prefers, so Burma and Myanmar can both appear, each from a
  source that would say it.
- New states: Fluent templates per language for each form (union, federal republic, people's
  republic, kingdom, state, extendable), plus a root lexicon per state and language (adjective
  forms by gender, the "of" phrase with its article, a short name). Templates handle agreement,
  articles, definite suffixes and compounds (Nordiska förbundsrepubliken, الجمهورية المغاربية
  الاتحادية, Maghrebinisches Königreich).
- Dynamic: event types map to forms, so a founding treaty, a new constitution, a revolution, a
  restoration or a coup proposes a new name on an event card: accept, keep the old name, or type
  one. Accepted names form a dated name history in the chronicle.
- AI and edits: with a model connected (ADR 0142's provider adapter, off by default), the model
  proposes alternatives with formal and short forms in every member and game language. Every cell
  is editable by hand. A language without templates or lexicon falls back to the game language.
  Forms in languages nobody on the project speaks are marked as drafts until a native speaker
  checks them.
- Storage: a chosen name set is an edit in the run file (14.2), so replays never call a model.
  Lab runs take the generated name.

## Tests

- The generated name for every pack state and form exists in every shipped language.
- A run with an AI-chosen name replays to the same state hash offline.
- On a fixed date list, real names match CLDR and Wikidata (Myanmar after 1989, Eswatini after
  2018).

## Consequences

States read naturally in each language and the name itself carries politics. Cost about 2-3
part-time weeks, sharing the provider and edit plumbing of ADR 0142; lexicon entries for new
pack states are small but need native review.
