# 0238. Data revisions and real-time vintages

Status: proposed (owner chose the option, 2026-10-29)
Date: 2026-10-29
Design sections: 12.3, 12.6, 15.2, 15.4, 17, 19.2; ADRs 0052, 0065, 0161, 0176, 0207, 0210, 0212
Phase: P9 (with 0065, lean)

## Context

ADR 0065 says each statistical office publishes "with lags, revisions and noise", but specifies a
generative model only for noise and manipulation. Revisions are the part of the statistical fog
that most changes decisions: US 2008 Q4 GDP was first published at -3.8% annualized and later
revised beyond -8%; Greece's 2009 deficit went from 3.7% of GDP to 12.7% and then 15.4%; Ghana's
2010 and Nigeria's 2014 rebasings raised GDP by about 60% and 89%. Policy estimated on revised data
misstates what policymakers saw: Taylor rules fitted on real-time output gaps differ materially
from those fitted on final data (Orphanides 2001). ADR 0210 fits policy rules to the tape without
saying which vintage the rule inputs come from. ADR 0052 uses real-world vintages only for the
scorecard; ADR 0161 names revisions as an ascension rung with no mechanism behind it.

## Options

1. Revisions in 1.0 with 0065: each release converges toward truth, plus rebasings and the
   unwinding of manipulation; decisions read the vintage available at the time.
2. Noise only: published equals truth plus persistent noise and manipulation, never revised.
3. Defer revisions to 1.x behind a flag; 0210 rules fitted on revised data.

## Decision

Option 1 (owner, 2026-10-29), under switch S21 (off: published equals truth, golden hashes unchanged).

**Release model.** For state s, series x and reference period t, the release at age k is
truth plus an error e(k) = e(0) * rho^k, where e(0) has standard deviation scaled by statistical
capacity (0065) and a bias term, and k counts ticks since first release. Manipulation (0065) adds
to e(0) and does not decay on its own.

**Events.**
- Benchmark rebasing: a hazard that falls with statistical capacity; when it fires, the level of
  the whole back series moves by a draw scaled by accumulated unmeasured activity, and growth
  history is revised.
- Unwinding: when a regime change, an IMF programme or an audit (0065 independent measures) fires,
  accumulated manipulation is revised out of the back series.

**Who reads what.** The 0210 rules, the 0176 r* filter, the state AI and Steward seats read the
vintage published as of the current tick. Observer and lab modes show truth, first release and
latest release. The 0161 rung decides only whether the Steward player is shown latest vintage
or truth; AI reads vintages regardless when S21 is on.

**Estimation.** 0210 rules are estimated with real-time inputs: Philadelphia Fed Real-Time Data
Set (Croushore and Stark 2001) and ALFRED for the US, the OECD Original Release Data and Revisions
Database for OECD members, the ECB real-time database for the euro area, and IMF WEO archive
vintages elsewhere. Revision parameters (initial error scale by capacity, decay rho, bias,
rebasing hazard and jump scale; five, IDs assigned in the SPEC) are fitted on the same sources and
enter the register (17). Sources are audited into the manifest before use.

**Determinism and storage.** Errors are counter-based draws keyed by (seed, state, series, period,
age), never stream-ordered, so any vintage is recomputable from truth and the seed and a fork sees
the same releases as its parent up to the fork tick. The run store keeps first and latest release
per headline series (about 35 MB for a run to 2150); other vintages are recomputed on demand under
the tiers of 0212. Rebasing and unwinding events go to the event chronicle (0049).

## Migration

- P9: release model and events in the 0065 published layer; real-time estimation of 0210 rules.
- Pack: the four real-time sources above in the manifest.
- DESIGN 12.3 and 12.6 gain a sentence pointing here when accepted.

## Tests

- With S21 off, golden hashes are unchanged.
- A vintage recomputed on demand equals the one recorded, bit for bit, including across a fork.
- On simulated data, mean absolute revision by release age and capacity tier matches the OECD
  revisions database moments within tolerance.
- 0210 rules estimated on simulated real-time data recover the planted coefficients; estimated on
  final data they show the Orphanides bias.

## Consequences

Agents and the player face recognition lags: recessions are seen late and booms look bigger than
they were. The cost is about 1 to 1.5 owner-weeks in P9 and five register parameters. Ensembles and
replays are unaffected beyond a small store per run.
