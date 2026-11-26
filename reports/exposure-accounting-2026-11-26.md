# Brexit and US-China band gaps: exposure accounting, 26 November 2026

Status: analysis inside the kernel freeze. No code, test, band or parameter changed; no run made.
Sources: `docs/research/rfc0350-bands-fitted.md` (PR #675, fitted run), RFC 0350,
`specs/pre-p3/C7-LITERATURE.md` ("Japan and Brexit"), econ SPEC "Pre-P3 benchmark",
`reports/trade-structure-decision-2026-11-26.md`. Numbers marked "estimate" are hand
arithmetic, "unverified" means no source in the repo or no run backs it.

Each gap is split into three parts: untraded services (until P8), base-year exposure (2000
model against the papers' 2014 and 2017 data), and shock mapping (what the shock file or band
compares against). Only shock mapping for Brexit can be quantified from the repo. Services and
base year cannot be separated by any run that exists, and no run for them is possible before P8
and a 2014 pack.

## Hard and soft Brexit, UK welfare

Model at the fitted epsilon and theta_2: -0.758% hard, -0.283% soft. Dhingra et al. 2017
headline: -2.66% and -1.34%. The band is 0.5x to 1.5x of the headline.

| Part | Soft (pts) | Hard (pts) | Basis |
|---|---|---|---|
| Total shortfall, headline minus model | 1.057 | 1.902 | -1.34 - (-0.283); -2.66 - (-0.758) |
| Shock mapping: foregone future EU integration, not in the shock files | 0.90 (85%) | 1.61 (85%) | Dhingra Table 4 as quoted in C7; the files carry NTB and, hard, MFN tariffs only |
| Like-for-like shortfall, services plus base year | 0.247 (23%) | 0.682 (36%) | like-for-like source -0.53% and -1.44% (NTB -1.31 plus tariff -0.13) minus model |

Shares are of the total shortfall and do not sum to 100%: the integration term in C7 (0.90 and
1.61) differs from headline minus like-for-like (0.81 and 1.22); the two Dhingra
decompositions in C7 do not reconcile (soft: 0.53 + 0.90 = 1.43 vs 1.34 headline; hard:
1.31 + 0.13 + 1.61 = 3.05 vs 2.66). Unverified which is right; the paper's table was not
re-read. Using the headline minus like-for-like instead gives integration 0.81 and 1.22 points
(77% and 64%), like-for-like shortfall as above. Either way integration is the largest part.

Like-for-like ratio, model over source: 0.534 soft, 0.526 hard (0.45 before the fit, C7). The
model transmits about half of the NTB and tariff shock to UK welfare.

Findings:
- Soft Brexit: the like-for-like source value, -0.53%, is outside the RFC 0350 band
  [-2.0%, -0.67%]. A model that matched Dhingra's NTB effect exactly would still miss the
  band. The band therefore tests the integration term the shock file does not carry. This is a
  shock-mapping miss by construction, not evidence about the trade core.
- Hard Brexit: the like-for-like value, -1.44%, is inside the band [-4.0%, -1.33%] only just.
  A model matching it would pass; the model at 0.526x does not.
- Services and base year, jointly 0.25 (soft) and 0.68 (hard) points. Not separable. Services:
  Dhingra apply the NTBs to goods and services (WIOD, 31 sectors) and the model has services
  local until P8 (trade SPEC scope table), so the service-sector NTB effect is absent entirely.
  Share of UK-EU trade that is services in the WIOD base: unverified. Base year: UK-EU trade
  exposure in 2000 against 2014: unverified, no figure in the repo. Direction is known (less
  exposure in 2000 gives a smaller loss); the size is not.
- Guards: ACR-IO passes (median 1.129, correlation 0.939) and openness per class passes
  (0.985 to 1.087), so the model's welfare follows its own trade-share changes. The shortfall
  is in the shock and exposure, not the transmission formula.

## US-China 2018 welfare

Model: USA +0.0044% of GDP, band [-0.1%, -0.02%], source Fajgelbaum et al. 2020: -0.04%. Pass-through
proxy 0.941, inside [0.8, 1.2].

| Part | Size | Basis |
|---|---|---|
| Untraded services | not a driver; tariffs hit goods only | tariff lists are goods; services enter only through input-output links, unquantified |
| Base-year exposure | scales the model's gain and loss terms by about 2.6x; sign unchanged | estimate: US goods imports from China about 1.0% of GDP in 2000, about 2.6% in 2017; from memory of Census and BEA series, unverified against the pack |
| Shock mapping and sign | the whole gap to the band | see below |

Why base year does not close this gap: at 2000 exposure the terms-of-trade gain is about
0.01 x 0.061 x (1 - 0.941) = 0.0036% of GDP (estimate), matching the measured +0.0044%. At
2017 exposure the same arithmetic gives about 0.0094%, still positive and further from the
band. Exposure scales the welfare number by roughly 2.6x but cannot flip its sign. Base-year
exposure explains why the model's magnitude would differ, not why it is positive.

What sets the sign is pass-through. China absorbs about 6% of the tariff at 0.941, which gives
a first-order gain that exceeds the second-order loss at this exposure. Fajgelbaum et al. find
full pass-through and therefore no terms-of-trade gain. The pass-through band [0.8, 1.2] admits
values on both sides of the sign flip, so the band for pass-through and the band for welfare
are not mutually consistent at 2000 exposure. The pass-through value at which the model's
welfare crosses zero is unverified; it needs a run (see below).

Further shock-mapping differences, none quantified:
- Fajgelbaum's -0.04% covers all 2018 US tariffs and all partners; the model case is China
  only, smaller by construction (econ SPEC). The band [-0.1%, -0.02%] centres on the
  all-partner number.
- The model has two tariff groups with equal rates (6.1 points US, 7.6 points China, no value
  weights); the real lists vary by product.
- Whether Fajgelbaum's figure includes the effect of Chinese retaliation on US export prices:
  unverified.

## What would settle the unverified parts

Not run here. Each needs the local pack (Mac) or data outside the repo, and none changes the
kernel:
1. US-China welfare against the pass-through assumption: rerun `us_china_2018_is_reported` with
   the China-side tariff scaled to move the proxy toward 1.0. Gives the crossing value.
   Report-only, outside the freeze, no tuning.
2. Base-year exposure: US imports from China over GDP in 2000 and UK-EU trade over GDP, 2000
   against 2014, from BACI and WDI already in the pack. No new data.
3. Services: not measurable until P8 builds traded services; the like-for-like shortfall in the
   Brexit table is the quantity P8 must close, 0.25 to 0.68 points.
4. Dhingra Table 4 decomposition: reconcile the 3.05 against 2.66 discrepancy above by reading
   the paper.

## Recommendation for the tier-2 ADR after the rescore

Name services trade (P8) and 2000 base exposure as the unbuilt mechanisms for the like-for-like
Brexit shortfall, with the integration term recorded as a shock-mapping difference that the
band includes and the shock files exclude. For US-China, name pass-through at 2000 exposure as
the owner of the sign miss. Any change to the soft-Brexit band is an RFC (development laws), and
this note makes the case that the band does not match the like-for-like shock.
