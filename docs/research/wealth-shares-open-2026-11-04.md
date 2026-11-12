# Open substitute for WID 2000 wealth-bracket shares (ADR 0308)

2026-11-04. Need: per country, 2000 shares of net personal wealth held by the bottom 50%,
middle 40% and top 10% (the three W_b in ADR 0308), readable by a release pack (commercial use,
not closed-source-only; `sources.rs` fails both on the release profile). Only 1 Jan 2000 data
(ADR 0241/0242), so post-2000 surveys cannot stand in.

## Candidate sources

| Source | Licence | 2000 coverage | Brackets | Verdict |
|---|---|---|---|---|
| WID.world (`shweal992j`) | none stated | ~all | any percentile | current; closed-source-only, fails release |
| UBS / Credit Suisse Global Wealth Databook | copyright UBS | 2000 onward, ~200 countries | deciles, top 1/5% | not open; most countries imputed from income anyway |
| Davies, Sandstrom, Shorrocks, Wolff (2011), UNU-WIDER | CC BY-NC-SA 3.0 IGO (site-wide, as WIID) | 2000, 229 countries | deciles | non-commercial, fails release; also imputed |
| datahub `global-wealth-distribution` | PDDL | world pyramid 2010+, 20 countries 2023 | USD bands | wrong years, from UBS |
| OECD Wealth Distribution Database | CC BY 4.0 | ~2010+, ~28-40 OECD | top 1/5/10%, bottom 60% | no 2000 |
| ECB HFCS / Distributional Wealth Accounts | ECB reuse with attribution | 2009+ euro area | deciles | no 2000 |
| LIS Luxembourg Wealth Study | registered microdata access | sparse ~2000 | microdata | not open |
| Fed Distributional Financial Accounts | US government work | US, 1989Q3+ | 0-50, 50-90, 90-99, 99-100 | open, US only |

No open source covers the 2000 brackets beyond the US. The global sets (UBS, Davies et al.)
impute most countries from income distributions themselves, so a proxy from income is what
they would give us anyway.

US DFA 2000Q1 net worth: bottom 50% 3.2%, 50-90 33.0%, top 10% 63.8% (90-99 35.9 + top 1 27.9).

## Proxy

Income per adult lognormal with sigma_y from the SWIID 2000 Gini (CC0, already primary):
sigma_y = sqrt(2) Phi^-1((G+1)/2). Wealth lognormal with sigma_w, bracket shares
bottom 50% = Phi(-sigma_w), top 10% = 1 - Phi(1.2816 - sigma_w), middle the rest.

Two ways to link sigma_w to sigma_y, one constant each, fitted to the US DFA top-10% share
(sigma_w,US = 1.635; US Gini 0.36 to 0.38 illustrative, the PR uses the pack's SWIID value):

- Variance-additive: sigma_w^2 = sigma_y^2 + tau^2 (log wealth = log income + independent
  shock from returns, inheritance, life cycle). tau ~ 1.48 to 1.50.
- Multiplicative: sigma_w = k sigma_y, k ~ 2.3 to 2.5.

Implied shares (bottom 50 / middle 40 / top 10), Gini illustrative:

| Gini | variance-additive | multiplicative |
|---|---|---|
| 0.24 | 6 / 33 / 61 | 14-16 / 44-45 / 39-42 |
| 0.33 | 5 / 32 / 63 | 7-8 / 35-37 / 55-58 |
| 0.40 | 5 / 30 / 65 | 3-4 / 26-29 / 67-71 |
| 0.52 | 4 / 27 / 69 | 1 / 11-14 / 85-88 |
| 0.60 | 3 / 24 / 73 | 0 / 5-7 / 93-95 |

Multiplicative overshoots at high Gini (top 10% above 90%) and undershoots egalitarian-income
countries, where measured wealth concentration is known to be high (Nordics). Variance-additive
keeps the cross-country spread narrow, consistent with the weak correlation between income and
wealth inequality in the literature. Lognormal misses negative net worth: US bottom 50% comes out
5.1% against DFA 3.2%.

The constant tau goes into the spec with its derivation (DFA 2000Q1 top 10% and the pack's SWIID
US 2000 Gini); no new source in sources.toml. WID can stay as a research-only check in the C6
gate report.
