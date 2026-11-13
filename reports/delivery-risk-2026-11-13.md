# Delivery risk for 1.0, 13 November 2026

Question (Antonio): have we done the easy items and deferred the hard ones, or solved some of the biggest problems? Is the work at the limit of what agent models can do? Nothing was rerun. Sources: `reports/progress-2026-11-12.md`, `reports/economy-assessment-2026-11-12.md` (#554), `reports/pre-1.0-polish-plan-2026-11-09.md`, RFC 0350, `docs/free-moment-rerun-results.md`, merged PRs to #568. The epsilon fit (0.3723, 1 of 4 held-out classes passing) and the e3 benchmark failure are as relayed on 13 Nov; the epsilon figure is not yet in a repo record. Effort figures are estimates.

## Verdict

Mostly the first. Agents have shipped what is specifiable and testable by construction: kernels, wiring, data pins, ADRs. Of the one hard problem that decides whether the product is credible (a trade core that reproduces held-out facts), no part is solved. 32% of 1.0 issues are closed (39/122), but the closed ones skew to plumbing; P5a is 0/4 and kernel wiring 0/9 on the issue board, although several wirings have merged since.

## 1. Classification

| Item | Class | Evidence |
|---|---|---|
| Kernels (fiscal, monetary, HLW, regime, energy, capital) | done, off by default | #542-#550, #555, #558; none has been shown to move a free moment |
| Data pack, pins, licences | done | 583/583 sha256 pins; 31/31 ICP price levels within 10% |
| Determinism, goldens, hashes | hard-and-solved | bit-identical across threads; goldens green |
| Steady-state 2000 calibration | hard-and-solved (by construction) | passes, but trade thetas, PIGL, border nu are fitted to their targets |
| Free moments (H2, Brexit, ACR-IO corr., PPML spread) | hard-and-open | H2 -0.472 vs band [-1.1, -0.7]; Brexit -0.63%/-0.24% vs -2.66%/-1.34%; ACR-IO 0.862 vs 0.9; every fitted wedge hit its target while moving a free moment |
| Epsilon distance fit (ADR 0272, #212) | hard-and-open | code merged (#563-#565); fit 0.3723, 1 of 4 held-out classes passes; not yet in the tick by default |
| E3 price package / x10 survival | hard-and-open | e3 fails the benchmark (openness outside 1.5x, usize underflow); x10 panics at `world.rs:89`; E3 is the first planned default-on switch |
| Calibration under new switches | hard-and-open | `P_NU` and `P_ST` fail (PIGL inversion country 222; Sinkhorn 1.6e-13); #553 floors not yet rerun |
| Structural change #173, growth | hard-and-open, blocked | kernel exists, no design for the driver |
| Banks, housing, sovereign, IMF, HANK (P5b, still in 1.0) | hard-and-open, not started | RFC 0352 is the first record; no kernel in a run; rated XL |
| Currencies/FX, external closure | hard-and-open | single numeraire; T1 Hume spike first |
| Performance | hard-and-open, correctly deferred | 198-212 ms vs 160 ms on 45 goods; 50-goods pack missing; laws say speed waits for Right |
| P1, P2, P7, P8, RFC 0333, RFC 0334 | mostly open, mostly not agent-hard | RFCs still `proposed`; P1/P7 need the Mac and a clean and a 16 GB machine; P8 needs Antonio's Steam account; P2 needs the P3 gate and credible scenarios |
| P9 hindcast 2011-2024 | hard-and-open, unreachable | depends on all of the above plus 2005/2010 packs |

## 2. Where agent limits are the real risk

1. **Calibrating without overfitting.** Agents reliably hit a stated target and just as reliably move another. Recorded for border nu, buyer index and anchor (#554); epsilon and E3 look the same. RFC 0350 fixed the bands in advance, which is right, but the fix needs structural insight (which wedge is mis-specified), not more iteration.
2. **Validation by construction.** Most passing tests check code against its own fit. Agents optimise for green. The held-out count that matters is the 1 of 4.
3. **P5b.** Banks, housing, sovereign default and HANK are research-grade modelling with no agreed reference behaviour. Agents can write kernels; nobody can say when the result is right. Four kernels plus ADRs is not a model.
4. **Mac-bound feedback loop.** Real-pack runs need Antonio's Mac and take hours (GLC about 32 h, a 65-minute calibration that failed). Agents cannot iterate on the signal that matters; each loop costs a day.
5. **Process drift.** Kernel throughput outpaced the P3 gate. ADR 0353 exists, but wiring continues while Right is red.

Not an agent limit: Steam account, signing, licences, the Mac, the 16 GB run. Those are owner or access limits and are cheap to fix.

## 3. Recommendation

Options:

- **A. Narrow 1.0 to a trade-counterfactual product.** Ship the P3 workbench on the real-economy core with the free-moment gates (RFC 0350) as the release bar; P4 population on if it passes; P5a monetary and fiscal as labelled experimental; P5b, P8 energy, P9 hindcast, structural, FX move to 1.x. Keep P1, P2, P7, P8, RFCs 0333/0334 (accept them). Sell it as "what-if trade and tariffs, reproducible".
- B. Keep the full 1.0 scope and the schedule. Highest chance of a long slip with a model that fails its own gates.
- C. Keep scope, drop the gates to diagnostics. Ships on time with claims the model fails; contradicts ADR 0364.

Recommended: **A.** Concretely:

- Cut or defer: P5b entire (reverses the 12 Nov card, so it needs Antonio), P9 hindcast, #173 structural, currencies/FX, 50-goods, speed to 160 ms (budget ADR on 10 goods instead).
- Do differently: freeze new kernels and wiring until one flagship counterfactual (tariff or Brexit) passes its RFC 0350 bands on the pack; one Opus-class thread on root cause of the border nu / epsilon conflict rather than another fit; drop the e1 package if e3 is the default; schedule Mac runs in batches so each rerun answers a question fixed beforehand; accept RFCs 0333/0334 now, since the first-run and Linux work are the cheapest credible progress.
- Release bar: ship only the showcase scenarios whose pre-registered checks pass; drop the rest (as the polish plan already says).

If the flagship counterfactual cannot reach the bands after the border nu and epsilon work, that is the signal that the trade core itself, not agent speed, is the limit; the honest 1.0 is then a documented research preview.
