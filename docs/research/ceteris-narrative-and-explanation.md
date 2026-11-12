# Narrative and explanation in Ceteris

Oct 24, 2026 · design proposal, non-binding. Extends the storyteller (ADR 0078), Shapley attribution (0049) and fog of statistics (0065) referenced in [ceteris-game-design-notes.md](ceteris-game-design-notes.md).

## Thesis

Dwarf Fortress does not write stories. It records structured facts with actors and causes, then *queries* them and renders them through grammars. Ceteris should do the same, and add something DF cannot: real causal attribution from accounting identities, decision traces and forks. Stories and explanations are one pipeline with two front ends.

```
systems (tick) ──emit──> Event log (+cause links) ──> Causal DAG
      │                                            │
      └──> DecisionTrace, identity decompositions  ├──> Legends / chronicles / biographies (queries + arcs)
                                                   ├──> Explanations (3 tiers)
forks ──────────────────> branch diff ─────────────┴──> Counterfactual narrative
                                                       │
                                     Renderer: grammar (canonical) | LLM (optional, cached)
```

## 1. Event log

Append-only, per branch, emitted inside the tick, deterministic.

```rust
struct Event {
    id: EventId,            // (tick, system_id, seq): stable across replays
    kind: EventKind,        // Default, CoupAttempt, PriceSpike, Appointment, Intervention, ...
    actors: SmallVec<[(Role, EntityId); 4]>,
    place: Option<ProvinceId>,
    magnitude: f32,         // kind-specific, normalized (z-score vs. history)
    causes: SmallVec<[CauseRef; 4]>,
    payload: EventPayload,  // typed facts for rendering
}
enum CauseRef { Event(EventId), StateCrossing { var: VarId, threshold: f64 }, Decision(DecisionId), Intervention(InterventionId) }
```

- **Emit on change of regime, not every tick.** Each system owns *detectors*: threshold crossings, sign changes, rank changes, structural breaks, agent decisions. A price drifting 0.1% is not an event; a 3-sigma move is.
- **Causes are recorded by the emitter**, which knows its proximate inputs: a sovereign default cites the debt/GDP crossing, the spread spike event and the failed IMF-talks event. This is local and cheap, and the union of links is the causal DAG. No global inference.
- **Ex-post edits are events.** Every intervention (lab or player) enters the log as an `Intervention` root, so chronicles and attribution can point at it ("after the 2004 rate edit...").
- **Forks share the prefix.** A branch stores events from its fork tick and references the parent's log for earlier ticks (structural sharing, same as state).
- **Compaction.** Low-salience events older than N years roll up into aggregates ("43 strikes in Lagos province, 2003-2007"); anything referenced by a cause link or a legend survives.

Determinism: events are pure functions of state, so the log is reproducible and can join the state hash for desync checks. Agent *memories* (rivals remembering sanctions) are separate state that may reference event ids.

## 2. Legends (DF-style history)

Legends are queries over the log, rendered lazily.

- **Entities with histories.** Leaders, ministers, generals, parties, firms, institutions, provinces, wars, crises. A biography is `events where actor == e`, ordered, filtered by salience.
- **Named things.** Wars, crises, eras and booms get names from a grammar seeded by `hash(event_id)`: "the Real Crisis of 2002", "the Second Sahel War", "the Lagos Boom". Deterministic, so every replay and every client agrees.
- **Eras.** Structural-break detection on key national series (growth, inflation, regime type) segments history into named periods ("the Lost Decade", "the Commodity Supercycle").
- **Arc detection.** DF-style patterns as DAG queries: rise and fall, rivalry (repeated hostile events between a pair), revenge (hostile event whose cause chain contains an earlier hostile event by the target), reversal of fortune, prophecy fulfilled (forecast vs. outcome), protege surpassing patron. Arcs are first-class objects the storyteller can surface.
- **Salience.** `magnitude × novelty × fame(actors) × relevance(seat)`. Fame accrues from appearing in salient events. Relevance is the player's seat and attention; the storyteller persona reweights it (presentation only, never state).
- **Views.** Chronicle (terse feed, ADR 0078), Legends browser (entities, wars, eras, a timeline), biographies, a province history, an "almanac" year page.

## 3. Explanations (why did X change)

Three tiers, cheapest first. The UI always shows the cheapest tier that answers the question, with a click through to the next.

**Tier 1: identity decomposition (exact, free, every tick).** The model's equations already decompose changes; record the terms.
- Price: `Δln p = Σ cost_share_i · Δln input_i + Δmarkup + Δtrade_cost + Δdemand shifter`, using the same CES/gravity structure the trade core solves.
- Trade flows: gravity decomposition into productivity, trade costs (tariffs, distance, conflict), and multilateral resistance.
- GDP: production side (sectors), expenditure side (C, I, G, NX), growth accounting (K, L, TFP).
- Fiscal and external balances: their accounting identities.
These are Victoria 3 tooltips, but exact. Only first-order linearizations; the residual is shown as "interactions".

**Tier 2: causal chain (recorded, free).** Walk the DAG backwards from an event or a variable's threshold crossing: "Default ← spread spike ← oil price collapse ← OPEC quota event; ← failed IMF talks ← parliament rejected austerity". Decisions contribute their `DecisionTrace`.

**DecisionTrace.** Every agent decision (elite, central bank, government, firm above a size) returns the scored options and the top-k utility terms behind the choice. With additive utilities this is a by-product, not extra work. It makes policy changes explicable by construction: "The governor raised rates 200bp: inflation expectations +3.1, currency defence +1.4, outweighing growth -2.2; his hawkish trait doubled the inflation weight."

**Tier 3: counterfactual attribution (forks, expensive, on demand).** For "how much of this recession was the oil shock vs. my rate path": fork at the relevant tick, switch off candidate causes (channel shutdown, ADR 0049), compute Shapley values. Cost is 2^k branches for k causes, so cap k (3 to 5 grouped causes), sample permutations for more, run in the background, cache by `(branch, tick, target, cause set)`. Ensemble lanes give uncertainty bands on the shares.

Tier 1 and 2 are correlational-by-construction (local mechanics); only tier 3 is a true counterfactual. The UI labels which tier produced a sentence.

## 4. Fork comparison

Determinism makes "what changed between worlds" a diff.

1. **Align.** Both branches share history up to the fork tick. After it, match events by `(kind, primary actors, place)` within a tick window.
2. **Classify.** Events only in A, only in B, matched but shifted in time, matched but different magnitude.
3. **Find the first divergence.** Walk forward from the fork to the earliest unmatched salient event in each branch; that is the story's hinge. With the intervention as root, the DAG of branch B shows the path from edit to hinge.
4. **Series deltas.** Key national series compared with Tier 3 attribution against the intervention list.
5. **Render.** "In the world where Brazil raised the Selic to 25% in October 2002, the real stabilized by January; the 2003 banking scare never happened, and Finance Minister X was not replaced. Median 2005 GDP: +1.8% (64 lanes)." This feeds the game's post-mortem and counterfactual score directly.

## 5. Text rendering

**Canonical: deterministic grammars.**
- Typed templates per `EventKind` and explanation shape, with slots filled from the payload. Tracery-style alternatives chosen by `hash(event_id, template_id)`, so text is stable across replays and multiplayer clients.
- Use Project Fluent (`fluent-rs`) for i18n: plural, gender and case rules per language. Templates live in packs, so modders and translators edit text without code.
- Aggregation rules collapse repeats ("third coup attempt in five years") and choose register by persona (dry statistical, journalistic, chronicler).
- Number formatting respects fog of statistics: the chronicle quotes *reported* figures; lab mode and the post-mortem quote true ones.

**Optional: LLM layer (presentation only).**
- Input is a structured *fact bundle* (events, cause chains, decision traces, attribution shares, all with ids), never raw state.
- Output must cite fact ids; a validator checks every number and name against the bundle and drops sentences that fail.
- Never writes state, never feeds agents, so determinism and lockstep are untouched. Cache keyed by `(bundle hash, model id, prompt version)` so a replay shows the same text when the cache is shared; the grammar text is always the fallback and the source of truth.
- Good uses: long-form legends, era summaries, newspaper front pages, adviser briefings, the post-mortem essay. Keep it out of tooltips and the chronicle, which must be instant, offline and exact.
- Local model or API, off by default; the licence and Steam store question (ADR 0050) applies.

## 6. Build order

1. Event struct, detectors for 10 to 20 core kinds, cause links, persistence with branch sharing.
2. Tier 1 decompositions for price, trade flow and GDP; tooltips.
3. DecisionTrace on central bank and government agents.
4. Grammar renderer with Fluent; chronicle feed; salience.
5. Legends browser: entities, biographies, named crises and wars.
6. Fork diff and counterfactual narrative; Tier 3 Shapley in the background.
7. Arc detection; eras.
8. Optional LLM renderer.

## Open questions

- [ ] Event volume budget per tick at full world scale, and the compaction horizon.
- [ ] Should the event log enter the state hash, or only a digest?
- [ ] How many grouped causes can Tier 3 afford per query on a laptop?
- [ ] Event matching across forks: is a tick window plus actor key enough, or do we need fuzzy matching on payload?
- [ ] Real people: does naming policy for leaders also cover generated legends about them?
- [ ] Is the LLM layer in scope for 1.0, 2.0, or a mod?
