# RFC 0314. Held-out sources register and the C7 guard

Status: draft
Date: 2026-11-06
Design sections: 15.3, 15.7, 16
Release: 1.0, pre-P3 (register and guard); P9 (scorer, blocking tolerances)
Research: `research/held-out-validation-sources-2026-11-04.md` (project files; the inventory, status classes and starting tolerances are taken from it)

## Summary

Calibration must not be scored against its own inputs. C7's "openness within 1.5 of ICIO" passes
by construction (C7-LITERATURE section 3), so it is a fit check, not validation. This RFC builds
the held-out register and a guard that fails when a pack build or calibration reader opens a
held-out file, now rather than at P9. It supersedes the 2026-11-04 note "report-only, no RFC,
register or guard until P9" (owner, 2026-11-06, issue #174). It builds on ADR 0050 (the
`validation.toml` file list), ADR 0107 (holdout split and sealed period) and ADR 0241 (only 1
January 2000 data in a run). Tolerances stay report-only until pre-registered (D3). The P9
scorer crate (#246) is unaffected.

Nothing is fetched by this RFC. Licence status of the six unverified sources is in the table
below; D4 and D5 decide what happens next.

## Decisions

### D1. How is a source marked held out?

Today `use = "validation"` means "scoring only", and `validation.toml` lists files only tests
read, but nothing says which of the 12 validation sources calibration must never see, and
report-only sources that share an upstream with a calibration source (WIOD with ICIO) are mixed
in with independent ones.

Options:

- A. Add `holdout = "H" | "R"` to validation sources in `sources.toml`. H is independent of every
  calibration source; R is a report-only cross-check that shares upstream with one and is never
  cited as an independent pass (protocol section 3, rule 5). Each `validation.toml` file gains
  `read_by`, the test or crate path allowed to open it.
- B. A boolean `holdout = true` for H only; R is inferred as validation without the flag.
- C. A separate `holdout.toml` register beside `sources.toml`.

Recommendation: A, because it is one manifest, two classes stated explicitly, and `check-sources`
already parses both files.

### D2. What does the guard check, and where?

Options:

- A. Static, in `ceteris pack check-sources`, three rules: (1) no H or R source appears in
  `fetch.toml` (the pack build); (2) no file named in `validation.toml` with `holdout` set appears,
  by file name or source id, in `crates/ceteris-pack/src` or `crates/ceteris-calibrate/src`
  (library code), and in tests only at its `read_by` path; (3) moving a source from H to C needs
  a `replaces_holdout` field naming the replacement H source (protocol rule 5), so the diff shows
  an RFC was written. C7 gets one test, `holdout_guard_clean`, that runs the same check and fails
  on any problem, so the pre-P3 gate cannot pass with a leak.
- B. A only for rule 1, deferring rules 2 and 3 to P9.
- C. A runtime guard: readers open held-out files only through a `holdout::open(id)` function that
  the calibration crate cannot link.

Recommendation: A. A grep-level scan is crude but needs no new crate and fails on the mistake that
matters; C is the right end state and belongs with the P9 scorer.

### D3. When are tolerances pre-registered, and when do they block?

Options:

- A. A `tests/holdout/prereg.toml` per source, committed before the first score: metric,
  tolerance, target-file sha256 (ADR 0107 sealed mechanic). `check-sources` fails if a scored
  source has no entry. Scores are report-only at C6/C7 and from the pre-P3 gate to P9; blocking
  from P9. Starting tolerances (T0 static 2000 equilibrium): shares in [1/1.5, 1.5] for at least
  70% of countries with median, p10 and p90 reported; wide-spread levels at Spearman at least
  0.7 with log RMSE reported; signs and rank of welfare responses as in C7 via ACR. A miss goes to
  Antonio with the table; no tolerance or parameter changes after seeing a score.
- B. Report-only with no pre-registration file until P9.

Recommendation: A, because the hash and tolerance are cheap to commit now and cannot be added
honestly after a score exists.

### D4. Which sources enter the register first, and what is deferred?

Options:

- A. First tier is the five that are independent of calibration, cover the econ core and score at
  T0: Eora 2000 (foreign input shares), WID wealth non-US (already in the manifest), IEA energy
  balances 2000, Gennaioli regional GDP, LIS income deciles. ACLED and Polity5 are registered
  (licence only, no tolerance) for S/Z hazards and wait for P3+ paths. Pink Sheet, IFS, WEO, BIS,
  crises and EM-DAT stay as already listed.
- B. First tier minus Eora until D5 is settled.
- C. Register everything in the protocol inventory at once.

Recommendation: B. Eora's terms are the only first-tier risk (D5); the other four score at T0 now.

### D5. What clears a source for fetching, and how is Eora handled?

Licence reading from the provider's primary page and a `checked` date are required before a
source enters `sources.toml` (ADR 0005). Result of the reads made for this RFC (2026-11-06, no
data fetched):

| Source | Finding | Status |
| --- | --- | --- |
| Eora | worldmrio.com/licensing.jsp: "free for academic use at degree-granting academic institutions. All other users must license the data." Redistribution, publication not addressed; cite Lenzen et al. 2012, 2013. | Not usable unless Antonio has an academic affiliation or buys a licence |
| IEA balances | Product page sells the data ("Purchase licence"); no free tier or CC BY-NC-SA 4.0 IGO found; licence page did not load. | Unverified; the "free tier" in the protocol is unconfirmed |
| LIS | LISSY "exclusively for non-commercial purposes", registered researchers only, remote execution; Key Figures open to the public, no licence named; citation required. Publication of aggregates not on the page; terms page did not load. | Likely usable for aggregate scores; unverified |
| ACLED | Content Usage Terms bar competing or substitute datasets, services to third parties, attributing analysis to ACLED, and ML/LLM use "subject to strict limitations"; they require stating how data were manipulated. EULA and attribution policy not read. | Unverified; scoring a non-ML simulator looks compatible |
| Gennaioli et al. | No licence statement found; the dataset site blocks automated reads. | Unverified; treat as no licence stated, so closed only |
| Polity5 | systemicpeace.org/inscrdata.html: copyrighted by the Center for Systemic Peace; citation required; "Reproduction or redistribution ... is prohibited" without written permission. | Fetch-your-own only; never bundled |

Options:

- A. Each source clears when Antonio (or a thread he names) reads the licence text in the browser
  and records it in `sources.toml` with a `checked` date; only then does a fetch task start.
  Eora is deferred until Antonio confirms an eligible licence. Users fetch their own copies; the
  repository ships only file names, hashes and aggregate scores.
- B. Drop Eora now and make EXIOBASE 3.9 (also unverified, listed as NC) the IO held-out source.
- C. Drop Eora and score IO structure against WIOD as R only, accepting that the 40-economy WIOD
  shares an upstream with ICIO and so is not an independent pass.

Recommendation: A, because it costs nothing now and keeps Eora's 190-country coverage of the
non-ICIO economies the model imputes; if Antonio has no eligible licence, fall back to B.

## Spec changes

Filled in when accepted, in the same PR as the rows:

- 16 (data licences): the `holdout` field, `read_by`, and the three guard rules.
- 15.3 and 15.7: held-out classes H and R, pre-registration file, report-only until P9.
- `specs/pre-p3/PLAN.md` C7: `holdout_guard_clean` added to the acceptance list.
- `crates/ceteris-pack/config/sources.toml` and `validation.toml`: entries and `read_by` for the
  first-tier sources, added by the implementation tasks, not by this RFC.
