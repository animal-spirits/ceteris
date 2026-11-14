# RFC 0353. A small calibrated pack in CI and a per-class fit cache

Status: accepted
Date: 2026-11-14
Design sections: 5.1, 21.1; ADRs 0005, 0050, 0200, 0207, 0257, 0272
Release: 1.0 (P4 and P5 gates)
Research: none; follows PR #571 (`calibrate --fit-cache`, `scripts/cloud/`) and RFC 0350

## Summary

Calibration feedback is slow: a full-pack `calibrate --pack` is about 2 hours, the benchmark
(`crates/ceteris-calibrate/tests/benchmark.rs`) skips in CI because `CETERIS_PACK` is unset, and
every full run needs the owner's Mac or a rented box. PR #571 made the epsilon trial cache
bit-identical and left two items open because both can change results: a per-class cache and a
reduced-pack quick-fit. This RFC proposes (a) a small calibrated pack the benchmark runs on in CI
and (b) a faster fit for the development loop, and fixes beforehand what each keeps bit-identical,
what may move, and how the small pack is shown to predict the full-pack outcomes against the RFC
0350 bands. The full pack stays authoritative at every phase gate (ADR 0257); nothing here changes
a full-pack golden. Docs only: no code until accepted and an issue is filed per work item.

## Facts this rests on

- Full pack: 4,796 provinces, 10 goods, about 190 countries. `pack.toml` already has a `dev_slice`
  flag (44 European economies) that marks provinces; it holds no US, China or Japan, so it cannot
  run the benchmark moments of RFC 0350 (Brexit, EU27, Japan, ACR-IO, openness, x10, US-China).
- The benchmark reads from the pack the ICIO-derived shares, BACI trade, CEPII gravity and the
  tables written by `calibrate`. The ICIO zips are `manual = true` in `fetch.toml`, so a hosted
  runner cannot fetch them; the pack build needs GEOS 3.15 and tippecanoe and is owner-side.
- `border::fit_border_nu` fits one nu per friction group by Brent's method; the groups share every
  evaluation and each scaling starts from nu 0, so the result does not depend on evaluation order,
  and a change to one group's target moves every trial (the reason #571 declined a per-class cache).
- Main CI skips `rfcs/**`, `spec/**`, `specs/**` and `*.md`; hosted minutes are limited (they ran
  out on 2026-10-27) and macOS counts 10x, so any new job is Linux only.

## Bit-identical and may-change, stated up front

| Item | Class |
|---|---|
| Epsilon trial cache (#571) | bit-identical to uncached, already tested |
| Building the small pack twice from the same inputs and config | bit-identical (hash checked) |
| `calibrate` and the benchmark on the small pack, any thread count | bit-identical across runs, by the same gates as the full pack |
| Small-pack numbers against full-pack numbers | differ by construction (rest-of-world aggregation, fewer provinces); never compared bitwise |
| Full-pack goldens, `tests/golden/`, hash domains, file formats | unchanged |
| Warm-started or decoupled fit (D4 B, C) | not bit-identical to the cold joint fit; within the fit tolerance (`NU_TOL`) at best |

A fit that is not bit-identical never writes a golden, an `epsilon.json` or a pack table that a
full-pack run reads (option C), or does so only after the golden re-bless review (option B).

## Decisions

### D1. How does the small pack get to CI?

The pack is cut from the full pack owner-side (or on the cloud box) by a deterministic subset
command: a fixed country list, with the others aggregated to rest of world on the ICIO grid; all
10 goods and the friction groups are kept, because the per-class openness and nu structure are what
the benchmark tests. Countries: every country the benchmark and shock files name (EU27, GBR, USA,
CHN, JPN and the rest of `shocks/*.txt`), fixed in the config before validation (D3).

Options:

- A. Commit the calibrated small pack under `tests/fixtures/pack-ci/`, with a manifest of sources.
  No infrastructure; CI reads it as `CETERIS_PACK`. History grows with each regeneration, and a
  history purge before going public is parked, so every committed regeneration is also a purge
  cost. Cap: 25 MB per regeneration, regenerated only by a reviewed PR.
- B. Store the pack as a content-addressed object (B2 bucket, already chosen for backups) and fetch
  it in CI by sha256 like `ceteris pack fetch`. History stays clean; needs the bucket, keys and a
  CI secret, and a fork PR cannot read it.
- C. Build the small pack in CI from fetched inputs. Needs the ICIO zips (manual), GEOS and
  tippecanoe on the runner, and the full input download per run; slowest and the most minutes.

Recommendation: A while one regeneration stays under 25 MB, because it needs nothing the project
has not got and a fork PR can run it; move to B if the size cap or the purge cost bites.

Decision: A, Antonio, 2026-11-14 (question card; "Pick these options").
Row: ADR 0370

### D2. Does CI calibrate the small pack, or read stored tables?

Options:

- A. Commit the pack uncalibrated; CI runs `calibrate --pack` on it (with `--fit-cache` keyed by
  git tree hash) and then the benchmark. A change to calibration code is tested by running it.
  Estimated minutes, not measured: the small pack is about a quarter of the provinces.
- B. Commit the calibrated tables too; CI runs only the benchmark. Faster, but a calibration change
  is not exercised until someone regenerates the tables by hand.

Recommendation: A, because calibration is where the regressions arrive; the job is path-filtered
to `crates/ceteris-{calibrate,trade,econ,pack}` and `spec/` parameter files, Linux only, with a
budget of 10 minutes per run (to be measured in the spike, ADR 0257 style go/no-go).

Decision: A, Antonio, 2026-11-14 (question card; "Pick these options").
Row: ADR 0371

### D3. How is the small pack shown to predict the full pack, and what may CI gate on?

Pre-registered here; no number below moves after results are seen without a new RFC.

Validation set: the full pack and the small pack both run on the same settings, namely the three
price-anchor rules (`off`, `e1`, `e3`), `P_NU`, `P_ST` (`docs/free-moment-rerun.md`), and the
epsilon on and off runs of ADR 0272. Compared moments: the RFC 0350 table (H2 distance, hard and
soft Brexit welfare, sign of EU27 and Japan, ACR-IO median, correlation and wrong-sign count,
openness per class, x10 survival, index and welfare ratio).

Predictive means all of: (1) every moment's pass or fail against its RFC 0350 band agrees in every
setting; (2) for each continuous moment, the ranking of the settings by the small pack equals the
ranking by the full pack (Spearman 1.0 over these six); (3) no sign differs. A moment that fails
this stays report-only on the small pack and keeps its full-pack gate.

Options for what CI does with the result:

- A. Hard gate on the moments that validated, against the RFC 0350 band widened to the largest
  small-minus-full gap seen in validation (the gap is recorded, not tuned); report-only on the
  rest. Small-pack goldens (state, journal hashes) are its own set, moved only by reviewed
  re-bless.
- B. Report-only for 4 weeks of merged PRs, then A for moments with no disagreement.
- C. Report-only permanently; the full pack is the only gate.

Recommendation: B, because a gate that has never been seen red or green on real PRs risks blocking
merges on noise while the hosted minutes are scarce.

Decision: B, Antonio, 2026-11-14 (question card; "Pick these options").
Row: ADR 0372

### D4. Per-class or quick fit for the development loop

Options:

- A. Neither. The bit-identical trial cache plus the small pack (D1, D2) are the whole change; a
  full fit still costs about 2 hours, on the cloud box.
- B. Decoupled per-class fit with a cache per class: fit each group's nu holding the others at
  their inputs, store per class keyed by that class's inputs and target, then one joint
  verification pass; fall back to the joint fit if any group share misses its target by more than
  `NU_TOL`. Intermediates (Leontief) couple the classes, so the result differs from the joint fit
  within tolerance at best; changes full-pack results, needs a golden re-bless and a rerun of the
  RFC 0350 bands before it is authoritative.
- C. Warm start for the dev loop only: seed each group's Brent bracket from the last accepted fit
  (a tight bracket around the cached nu, widened on failure), same objective and tolerance. Output
  is labelled non-authoritative, never written to a golden or to `epsilon.json`, and a cold joint
  fit is still what a phase gate and a golden use. Gain unmeasured; a one-day spike counts
  evaluations warm against cold on the full pack, go if at least 3x fewer (the law's spike rule).

Recommendation: C, because it speeds the loop without any result that feeds a gate or a golden,
and B's coupling error would have to be judged against RFC 0350 bands that the current trade-core
work (#559 to #561) is still moving.

Decision: C, Antonio, 2026-11-14 (question card; "Pick these options").
Row: ADR 0373

### D5. Data and licence conditions of a committed or stored small pack

Options:

- A. Ship only primary sources whose licences allow redistribution in a derived table
  (OECD ICIO terms, BACI and CEPII Etalab 2.0, CC BY series: WDI, PWT, FAOSTAT, Natural Earth,
  public domain), with attribution carried in the manifest, and a CI step runs
  `pack check-sources --profile research` on the manifest. EXIOBASE (validation-only,
  user-fetch) and the ETH/EPR family (CC BY-NC-SA 4.0, share-alike) are excluded from the
  subset by construction; the benchmark reads neither.
- B. As A but allow EPR, accepting that share-alike attaches to the fixture.

Recommendation: A, because a share-alike file in the repository would constrain the simulator
licence (#514, RFC 0344) for no benchmark moment. The repository is private today, but ADR 0005
treats every committed byte as shippable.

Decision: A, Antonio, 2026-11-14 (question card; "Pick these options").
Row: ADR 0374

## Cost

Estimates, not measurements: subset command a few minutes on the cloud box; CI job under 10
minutes per touched PR on Linux (D2 budget); one validation campaign is twelve full-pack runs,
about 2 hours each on the cloud box, once. Owner effort: none beyond approving the CI workflow,
which an agent may not edit (`.github/` changes come from a non-`claude/*` branch Antonio approves).

## Work items after acceptance

1. Subset command and config, deterministic, with a hash test (D1).
2. Validation campaign and report with the D3 numbers filled in.
3. CI job and small-pack goldens (D2, D3), workflow file PR from Antonio's branch.
4. Spike for D4 C.

No code, no `.github/` change and no number is taken before acceptance.

## Spec changes

- 21.1: the CI small-pack benchmark, report-only for 4 weeks, then a gate on validated moments (ADR 0370 to 0372, 0374).
- 5.1: the small-pack subset rule (ADR 0370).
- 19: warm-start fit for the dev loop, non-authoritative (ADR 0373).

ADR 0257 is cited, not changed.