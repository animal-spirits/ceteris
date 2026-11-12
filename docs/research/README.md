# Research index

Every study behind a decision is listed here, newest first, the day it is written. A study may
live outside the repository (in the owner's private notes); the row is what
makes it findable. `cargo run -p ceteris-rows -- check` fails when `spec/` or `rfcs/`
links a study file under `docs/research/` that is not in this table. Studies outside the repository are listed here without a link.

Outcome names the rows the study fed. Rows before 2026-11-01 were rebuilt from agent memory at
the migration: the outcome lists the ADRs that memory mentions next to the study, which is a
superset of what the study decided.

| Date | Study | Outcome |
| --- | --- | --- |
| 2026-11-12 | [Electricity generation by country after Ember](electricity-source-after-ember-2026-11-12.md) | RFC 0348 (accepted); ADR 0358 |
| 2026-11-10 | [P5 E3: island money sink at x10](p5-e3-island-money-sink-2026-11-10.md) | RFC 0341 (accepted, spike no-go) |
| 2026-11-10 | [P5 E3: spike of RFC 0341 D1 and D2](p5-e3-d1d2-spike-2026-11-10.md) | no-go; RFC 0341 not built |
| 2026-11-10 | [P5 E3: why the mainland dies at x10 under the credit limit](p5-e3-mainland-deflation-2026-11-10.md) | root cause: anchor index; fix go; RFC 0342 (accepted) |
| 2026-11-10 | [P5 E2 spike: bounded price gap at zero output under the money anchor](p5-e2-zero-output-step-spike-2026-11-10.md) | ADR 0340, 0341 |
| 2026-11-10 | [P5 price-anchor spike: money anchor and good-level term on the fixture](p5-price-anchor-spike-2026-11-10.md) | ADR 0338, 0339 |
| 2026-11-04 | [Open substitute for WID 2000 wealth-bracket shares: candidate sources and licences, US DFA anchor, lognormal proxies from the income Gini](wealth-shares-open-2026-11-04.md) | ADR 0311, 0312 |
| 2026-11-04 | Tick speed profile and brainstorm: 148 ms single-thread against 43 ms at 14 threads, serial sections and T2b per-good imbalance, 23 ranked ideas by determinism class; project files tick-speed-brainstorm-2026-11-04.md | RFC 0287; ADR 0294 to 0298 |
| 2026-11-01 | Why PWT 11 and WDI disagree on 2000 price levels (ICP 2011 backcast vs PWT's 1996/2005 benchmarks), the corrupt archive PA.NUS.PPPC.RF file, and an enforceable variant of ADR 0098's real GDP check; project files research/pwt-wdi-price-levels-2026-11-01.md | none yet (feeds RFC 0282 D1, D7; ADR 0285) |
| 2026-11-01 | Storage-on frictionless V1 misses of ADR 0280: cause (Q = 0 bang-bang steps at stored origins), fix screen of six rounds, local-goods trap (project files, `storage-misses/storage-on-frictionless-v1-2026-11-01.md`) | RFC 0286 (deferred) |
| 2026-11-01 | [Filling the Eurostat 2000 manufacturing gaps for ADR 0275 (DK, CZ, HU, NO, BG, SI, FI, PT): coverage measured, replacement sources and licences](eurostat-2000-mfg-gaps-2026-11-01.md) | ADR 0287 |
| 2026-10-31 | 2026-10-31 explainer of how energy/electricity are modelled and tie to state, companies, construction | ADR 0028, 0034, 0035, 0084, 0095, 0122, 0171, 0178 and more |
| 2026-10-31 | Plan for the ADR 0271 50-goods pack (P4): B09 local, 7 theta classes, manufacturing spread by plants+census+RAS, goods count as pack value | ADR 0023, 0248, 0271, 0273, 0274, 0275 |
| 2026-10-31 | 2026-10-31 brainstorm of other Ceteris-like side projects for Antonio | none recorded |
| 2026-10-30 | 2026-10-30 thread "Make the app blazingly fast" (plus "fearlessly concurrent"): Mac probe numbers, ranked app/sim recs, doc link, open items | none recorded |
| 2026-10-30 | 2026-10-30 audit of econ core fidelity to its literature, all 41 alterations since the 23 Oct DESIGN snapshot, and design drift | ADR 0092, 0093, 0094, 0101, 0106, 0112, 0200, 0247 and more |
| 2026-10-30 | Which UI mocks the native Slint+wgpu app can build (built/easy/effort/hard), with Slint 1.18.1 limits found in its source | ADR 0137, 0244, 0248, 0250, 0255 |
| 2026-10-30 | Which UI mocks the native Slint+wgpu app can build (built/easy/effort/hard), with Slint 1.18.1 limits found in its source | ADR 0137, 0244, 0248, 0250, 0255 |
| 2026-10-30 | 2026-10-30 flavour round three: 28 Paradox visual/notice/tooltip/sound cues as patterns, tagged 1.0 vs 2.0+, effort, native feasibility | ADR 0118, 0137, 0188, 0242, 0244, 0251, 0252 |
| 2026-10-30 | 2026-10-30 Polish Bench artifact: 34 small UI/UX polish touches (micro-interactions, tickers, unit chips, tooltips, keys, toasts, sound limits) with Slint feasibility and cost | ADR 0137, 0252, 0258, 0260, 0261, 0262 |
| 2026-10-30 | ADR 0248 (PR #88, 2026-10-30) re-estimates 0207 after ADRs 0208-0247 | ADR 0207, 0208, 0227, 0233, 0241, 0242, 0247, 0248 and more |
| 2026-10-30 | Phone-friendly roadmap timeline artifact (2026-10-30): phases pre-P3 to R, gate windows, risks, 2.0+ | ADR 0248, 0252, 0254, 0255 |
| 2026-10-30 | 2026-10-30 design for real royal houses, claimants, emergent restorations and a claimant seat | ADR 0066, 0139, 0183, 0189, 0190, 0192, 0195, 0222 and more |
| 2026-10-30 | 2026-10-30 research on making the native Slint+wgpu app look polished: plan, crate verdicts, Slint gaps, open cards | ADR 0137, 0260, 0261 |
| 2026-10-30 | 2026-10-30 explainer of war and territory occupation (artifact link) and two gaps decided and recorded as ADR 0245 (#86) | ADR 0115, 0145, 0147, 0149, 0154, 0165, 0166, 0190 and more |
| 2026-10-30 | 2026-10-30 research on wars 2000-2025 and organic conflict-onset design | ADR 0110, 0253, 0254 |
| 2026-10-29 | 2026-10-29 feasibility of a Total-War-like battle/tactics companion for Ceteris war (2.0+) | ADR 0146, 0147, 0149, 0166 |
| 2026-10-29 | 2026-10-29 naming shortlist for the city, space and battle companion games | none recorded |
| 2026-10-29 | 2026-10-29 brainstorm on running forward from 1 Jan 2000 data only (no shock/policy tape in default runs), six open decisions D1-D6 | ADR 0035, 0051, 0052, 0062, 0179, 0206, 0210, 0229 and more |
| 2026-10-29 | 2026-10-29 flavour round two, "instrument" register: 39 ideas tagged 1.0/1.x/2.0+, artifact link, open picks | ADR 0240 |
| 2026-10-29 | 2026-10-29 space brainstorm (satellites as infrastructure, launch as granular industry, military space, debris commons) and proposed slotting | ADR 0075, 0136, 0153, 0203, 0206, 0215, 0239 |
| 2026-10-29 | 2026-10-29 brainstorm of "Hohmann", a hard-science space colonization companion game coupled to Ceteris | ADR 0223, 0225 |
| 2026-10-29 | Catalogue of all Ceteris UI surfaces (373 map modes, overlays, panels, screens), tagged decided/mocked/new and by version | ADR 0118, 0124, 0144, 0157, 0159, 0187, 0207 |
| 2026-10-28 | 2026-10-28 brainstorm on fully editable government administration and land division (open partition registry + body/competence primitive) | ADR 0068, 0115, 0129, 0136, 0190, 0191, 0194, 0195 |
| 2026-10-28 | 2026-10-28 art/sound direction proposal for Ceteris (artifact link, four options, recommendation, open choices) | ADR 0065, 0125, 0137, 0140, 0144 |
| 2026-10-28 | 2026-10-28 follow-ups on the Ceteris art page: pattern plates, atlas details, local standards (ADR 0141), UI interactions and sound default | ADR 0137, 0140, 0141 |
| 2026-10-28 | 2026-10-28 audit of non-economic design gaps (product, engineering, release, governance) ranked against lean 1.0 | ADR 0050, 0128, 0137, 0167, 0182, 0197, 0208, 0209 and more |
| 2026-10-28 | 2026-10-28 bold-ideas brainstorm doc (32 ideas, deduped vs DESIGN/ADRs/research docs, tagged cost and economy-only 1.0 fit) | ADR 0096, 0116, 0135 |
| 2026-10-28 | 2026-10-28 survey of 20 mechanics from other games (deduped vs DESIGN, ADRs 0001-0144, research docs) | ADR 0001, 0017, 0020, 0096, 0144, 0155, 0164 |
| 2026-10-28 | 2026-10-28 character system | ADR 0066, 0139, 0142, 0183, 0186, 0187, 0188 |
| 2026-10-28 | 2026-10-28 character system | ADR 0066, 0139, 0142, 0183, 0186, 0187, 0188 |
| 2026-10-28 | 2026-10-28 character system | ADR 0066, 0139, 0142, 0183, 0186, 0187, 0188 |
| 2026-10-28 | 2026-10-28 feasibility of a hardcore city-builder companion coupled to Ceteris | ADR 0074, 0115, 0137 |
| 2026-10-28 | 2026-10-28 feasibility of a hardcore city-builder companion coupled to Ceteris | ADR 0074, 0115, 0137 |
| 2026-10-28 | 2026-10-28 feasibility of a hardcore city-builder companion coupled to Ceteris | ADR 0074, 0115, 0137 |
| 2026-10-28 | 2026-10-28 deep dive on modelling companies/firms in Ceteris | ADR 0026, 0030, 0089, 0170, 0171, 0172, 0189, 0200 and more |
| 2026-10-28 | 2026-10-28 construction/capital-formation proposal (time to build, construction capacity, named projects) vs Vic3/EU/HOI | ADR 0020, 0112, 0160, 0196, 0198, 0200, 0203, 0204 and more |
| 2026-10-28 | 2026-10-28 assessment of Ceteris vs Dwarf Fortress (Antonio's goal: "the DF of grand strategy") | none recorded |
| 2026-10-28 | 2026-10-28 benchmark of P2 econ core vs trade papers and ICIO data | ADR 0092 |
| 2026-10-28 | 2026-10-28 exploratory brainstorm of HOI-style equipment designers / Vic3 production methods for war (2.0) | ADR 0146, 0165, 0166, 0201, 0203 |
| 2026-10-28 | 2026-10-28 review of how Ceteris models financial markets (DESIGN 9.7-9.14 vs lean 1.0) | ADR 0025, 0040, 0042, 0043, 0111, 0172, 0176, 0202 and more |
| 2026-10-28 | 2026-10-28 audit of what Ceteris does not model, what is exogenous but could be endogenous, and unused datasets (artifact link, top picks) | ADR 0001, 0089, 0111, 0144, 0167, 0182 |
| 2026-10-28 | 2026-10-28 design note on culture/language/religion change, assimilation and mass atrocities (artifact link, proposals, pending cards, ADR numbers) | ADR 0055, 0058, 0110, 0154, 0207, 0208, 0209, 0210 and more |
| 2026-10-28 | Keyboard-first interaction brainstorm (vim mode, standard keymap, palette, query language, terminal mnemonics, automation, natural language) | ADR 0113, 0125, 0155, 0188 |
| 2026-10-28 | 2026-10-28 proposed rule for Ceteris major versions (each relaxes one ceteris paribus), 1.0-4.0 ladder, and the open 1.0-scope conflict | ADR 0005, 0029, 0036, 0038, 0076, 0107, 0121, 0130 and more |
| 2026-10-28 | 2026-10-28 proposed rule for Ceteris major versions (each relaxes one ceteris paribus), 1.0-4.0 ladder, and the open 1.0-scope conflict | ADR 0005, 0029, 0036, 0038, 0076, 0107, 0121, 0130 and more |
| 2026-10-28 | 2026-10-28 proposed rule for Ceteris major versions (each relaxes one ceteris paribus), 1.0-4.0 ladder, and the open 1.0-scope conflict | ADR 0005, 0029, 0036, 0038, 0076, 0107, 0121, 0130 and more |
| 2026-10-28 | 2026-10-28 proposal for pop wellbeing, approval vs legitimacy, vote intention, legislative bill voting (doc link, recommended defaults, open decisions) | ADR 0065, 0129, 0139, 0164, 0183, 0189, 0191, 0194 and more |
| 2026-10-28 | 2026-10-28 answer to "won't dictatorships be easier/more fun than democracies" | ADR 0065, 0153, 0162, 0183, 0186, 0192, 0193, 0194 and more |
| 2026-10-28 | 2026-10-28 check of ms-per-tick targets vs Paradox/Civ late-game slowdown | ADR 0208, 0212, 0223, 0226, 0232, 0234, 0235, 0236 |
| 2026-10-28 | 2026-10-28 design note on hourly war ticks (HOI-style), other time mechanics, and sim/render decoupling for smooth speed and stable fps | ADR 0145, 0149, 0188, 0208, 0209 |
| 2026-10-28 | Ceteris UI mockup artifacts (Iberia main screen, 46-board screens canvas) and the design choices they assume | ADR 0049, 0116, 0117, 0118, 0119, 0122, 0124, 0137 and more |
| 2026-10-28 | Ceteris UI mockup artifacts (Iberia main screen, 46-board screens canvas) and the design choices they assume | ADR 0049, 0116, 0117, 0118, 0119, 0122, 0124, 0137 and more |
| 2026-10-28 | Ceteris UI mockup artifacts (Iberia main screen, 46-board screens canvas) and the design choices they assume | ADR 0049, 0116, 0117, 0118, 0119, 0122, 0124, 0137 and more |
| 2026-10-28 | 2026-10-28 HOI3-style-but-deeper war/military design breakdown for P7 (artifact link, key choices, build order W0-W10, open decisions) | ADR 0115, 0145, 0154, 0155, 0164, 0165, 0166 |
| 2026-10-28 | 2026-10-28 HOI3-style-but-deeper war/military design breakdown for P7 (artifact link, key choices, build order W0-W10, open decisions) | ADR 0115, 0145, 0154, 0155, 0164, 0165, 0166 |
| 2026-10-28 | Multi-window support and external/local API assessment (2026-10-28): doc link, recommendations, ADRs pending Antonio's go | ADR 0113, 0135, 0188, 0196, 0197 |
| 2026-10-27 | Provider-page licence check of ~60 ADR-planned sources and flagged pack items (2026-10-27) | none recorded |
| 2026-10-27 | Antonio wants sub-province territorial change (esp. for the game) | ADR 0113, 0114, 0115, 0190, 0249, 0251 |
| 2026-10-26 | World-model assumption audit report (all domains) for Ceteris, 2026-10-26, with top risks and cheap fixes | ADR 0027, 0033, 0036, 0091, 0098, 0111, 0112 |
| 2026-10-26 | 2026-10-26 audit of all 111 Ceteris ADRs for conflicts/duplicates | ADR 0015, 0022, 0023, 0026, 0027, 0028, 0029, 0032 and more |
| 2026-10-26 | 2026-10-26 follow-up survey ranking frontier speed/memory techniques against the Task 19 benchmark | ADR 0053 |
| 2026-10-26 | 2026-10-26 comparison of Ceteris's planned/built econ engine against Victoria 3 (patch 1.14) and EU5 | ADR 0049 |
| 2026-10-25 | Where the Ceteris P2 economic model lives (p2-econ branch) and the 2026-10-25 spec review's key findings | ADR 0023, 0026, 0027, 0029, 0088 |
| 2026-10-25 | 2026-10-25 survey of speed/memory techniques for the economic core vs determinism rules | ADR 0017, 0024 |
| 2026-10-24 | Antonio's framing of Ceteris as a deeper, less gamey Paradox game, and the sim/game boundary agreed in discussion | ADR 0089, 0103 |

## Other studies

Studies copied from the project's shared folder on 2026-11-12, by file:

- [C7-LITERATURE.md](C7-LITERATURE.md)
- [b7-storage-bounds.md](b7-storage-bounds.md)
- [0270-extraction-column-for-rent-economies.md](0270-extraction-column-for-rent-economies.md)
- [0263-price-gain-cap-draft.md](0263-price-gain-cap-draft.md)
- [kappa-study-README.md](kappa-study-README.md)
- [battle-companion-2026-10-29.html](battle-companion-2026-10-29.html)
- [blind-spots-2026-10-28.html](blind-spots-2026-10-28.html)
- [borrowed-mechanics-2026-10-28.html](borrowed-mechanics-2026-10-28.html)
- [ceteris-brainstorm-additions.md](ceteris-brainstorm-additions.md)
- [ceteris-game-design-notes.md](ceteris-game-design-notes.md)
- [ceteris-modular-2026-10-29.html](ceteris-modular-2026-10-29.html)
- [ceteris-narrative-and-explanation.md](ceteris-narrative-and-explanation.md)
- [character-flavour-2026-10-28.html](character-flavour-2026-10-28.html)
- [characters-2026-10-28.html](characters-2026-10-28.html)
- [companies-2026-10-28.html](companies-2026-10-28.html)
- [companion-names-2026-10-29.html](companion-names-2026-10-29.html)
- [construction-2026-10-28.html](construction-2026-10-28.html)
- [equipment-designers-2026-10-28.html](equipment-designers-2026-10-28.html)
- [f32-recheck-2026-10-29.md](f32-recheck-2026-10-29.md)
- [financial-markets-2026-10-28.html](financial-markets-2026-10-28.html)
- [held-out-validation-sources-2026-11-04.md](held-out-validation-sources-2026-11-04.md)
- [hohmann-2026-10-29.html](hohmann-2026-10-29.html)
- [identity-change-2026-10-28.html](identity-change-2026-10-28.html)
- [initial-conditions-only-2026-10-29.html](initial-conditions-only-2026-10-29.html)
- [instrument-charm-2026-10-29.html](instrument-charm-2026-10-29.html)
- [keyboard-interaction-2026-10-28.html](keyboard-interaction-2026-10-28.html)
- [leadership-dossiers-2026-10-28.html](leadership-dossiers-2026-10-28.html)
- [ocean-freight-2000.md](ocean-freight-2000.md)
- [paradox-cues-2026-10-30.html](paradox-cues-2026-10-30.html)
- [polish-bench-2026-10-30.html](polish-bench-2026-10-30.html)
- [pwt-wdi-price-levels-2026-11-01.md](pwt-wdi-price-levels-2026-11-01.md)
- [royal-houses-2026-10-30.html](royal-houses-2026-10-30.html)
- [space-2026-10-29.html](space-2026-10-29.html)
- [tick-speed-check-2026-10-28.html](tick-speed-check-2026-10-28.html)
- [time-and-war-clock-2026-10-28.html](time-and-war-clock-2026-10-28.html)
- [ui-surfaces-2026-10-29.html](ui-surfaces-2026-10-29.html)
- [war-and-occupation-2026-10-30.html](war-and-occupation-2026-10-30.html)
- [wars-since-2000-2026-10-30.html](wars-since-2000-2026-10-30.html)
- [sudan-frictionless-2026-10-31.md](sudan-frictionless-2026-10-31.md)
