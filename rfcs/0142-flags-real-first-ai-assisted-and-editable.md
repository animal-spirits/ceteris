# 0142. Flags: real first, then AI-assisted and editable

Status: proposed
Date: 2026-10-28
Design sections: 14.2 (edits), 20; ADRs 0050, 0125, 0128, 0132, 0140
Phase: P6 (when states can form and split); tiers 1-2 data in the P3 pack pipeline

## Context

States can form, merge and secede, so the game needs flags for states that never existed. The
owner dislikes procedural flags and rejected a rule-based generator and hand-designed options
as generic. He proposed AI-generated flags (a free model or the user's own credentials) that the
user can edit by hand, replace by upload, or refine in a chat with the model. A working studio
(draft four, revise by request, recolour, edit SVG, upload, undo, lint) is in the proposal (section "Flag studio").

## Options

1. Rule-based generator (union device, geography motif, inherited emblem).
2. Flags designed ahead of time for plausible unions, shipped in the pack.
3. AI drafting with manual editing, upload and chat revisions, with the first two as fallbacks
   when no model is connected.
4. Image-generation models.

## Decision

Option 3, in three tiers.

1. Real flags with real dates: every state's flag in 2000 and the real changes to 2025, each
   applied only when its cause happens in the run.
2. Real flags that already exist for regions and movements (Catalonia, Bavaria, Kurdistan,
   Somaliland), from Wikimedia Commons with licences checked per file.
3. New flags, only when nothing real exists, through the flag studio:
   - Draft: a model receives a brief built from the state (parents' flags and colours, members,
     geography, founding year, form of government) and design rules (two to four colours, no
     lettering or seals, legible at 36 by 24 px, no copies of real flags, no religious symbols
     unless a parent carries them, nothing resembling extremist symbols), and returns four SVG
     options with a name and rationale.
   - Revise: the user asks for changes in plain words; the model returns the revised SVG and a
     one-line note. Recolour pickers, an SVG editor, undo and upload (SVG, or PNG/JPG traced to
     vectors with `vtracer`) work without any model.
   - Without a model: the rule generator and pack-designed options fill the studio.
- Providers: off by default (ADR 0125). The user brings a key for Anthropic, OpenAI, Google or
  OpenRouter, kept in the OS keychain, or points the app at a local model (Ollama,
  llama.cpp). Free hosted tiers may be listed but are not promised. One provider adapter serves
  this ADR and ADR 0143.
- Format: SVG in a 300 by 200 viewBox. Image-generation output is for inspiration only; it blurs
  at small sizes and garbles detail.
- Safety: every SVG, generated or uploaded, is parsed by `usvg` into a safe subset (no scripts,
  links, external images or fonts) and rendered by `resvg`. Lint checks colour count, text,
  complexity and frame. A perceptual hash against the real-flag set warns on near copies.
  Shared timelines (ADR 0132) carry a report action for hate symbols.
- Determinism: a flag is an asset, not state. The chosen SVG is stored in the run file under its
  content hash as an ordinary edit (14.2), so replays, forks and shared runs (ADR 0128) reuse it
  and never call a model. Lab runs and runs without a pick take the first fallback option.
- Excluded: LLM output in simulation state; shipping model weights.

## Tests

- A run with an AI-drafted flag replays to the same state hash with networking disabled.
- The sanitiser strips a planted script, `foreignObject`, external `href` and `url()` reference.
- The perceptual-hash check flags a recoloured real flag.

## Consequences

Players get flags they choose and can shape, and the project avoids a generator the owner finds
generic. Cost about 3-5 part-time weeks: provider adapters and keychain (1), studio panel in
Slint (2), sanitising, lint and perceptual hash (1), tracing (0.5). Quality depends on the
model the user connects; the fallbacks keep the feature usable offline.
