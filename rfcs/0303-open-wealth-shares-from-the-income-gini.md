# RFC 0303. Wealth-bracket shares from the income Gini, WID as a check only (amends ADR 0308)

Status: accepted
Date: 2026-11-04
Design sections: 9.1, 17; econ SPEC (Pack additions, Factor shares)
Research: [Open substitute for WID 2000 wealth-bracket shares](/mnt/project-files/research/wealth-shares-open-2026-11-04.md)

## Summary

ADR 0308 pays each country's profit pool to its pops by the WID 2000 shares of net personal
wealth of the bottom 50%, middle 40% and top 10%. WID states no licence, so its entry is
`closed_source_only` and a release pack cannot read it (`sources.rs`, release profile; owner
ruling 2026-11-04). No open source has these shares for 2000 outside the US: UBS's databook is
copyrighted, the Davies, Sandstrom, Shorrocks and Wolff (2011) estimates are UNU-WIDER
non-commercial, OECD WDD and ECB HFCS start about 2010 (ADR 0241 allows only 2000 data), LIS is
registered access. The global sets impute most countries from income distributions themselves.
This RFC replaces WID by a lognormal proxy built from the pack's Gini 2000 (ADR 0098) and one
constant fitted to the US Distributional Financial Accounts (Federal Reserve, a US government
work), and keeps WID only as a research-profile check. The rank mapping of ADR 0308 is
unchanged.

## Decisions

### D1. Which open proxy gives the 2000 wealth-bracket shares?

Found: with income lognormal, sigma_y = sqrt(2) Phi^-1((G + 1) / 2); a lognormal wealth
distribution with sigma_w gives the bottom 50% Phi(-sigma_w) and the top 10%
1 - Phi(z_0.9 - sigma_w). DFA 2000Q1 net worth: bottom 50% 3.2%, 50-90 33.0%, top 10% 63.8%,
so sigma_w,US = 1.635.

Options:

- A. Variance-additive: sigma_w^2 = sigma_y^2 + tau^2 (log wealth is log income plus an
  independent shock), tau^2 = sigma_w,US^2 - sigma_y,US^2; top 10% 61% to 73% over Gini 0.24
  to 0.60 (illustrative Ginis).
- B. Multiplicative: sigma_w = k sigma_y, k = sigma_w,US / sigma_y,US (about 2.4); top 10% 40%
  to 95% over the same range.
- C. One curve: every country takes the US fit (5 / 31 / 64).

Recommendation: A, because it keeps a plausible cross-country spread and the weak correlation
of wealth with income inequality; B puts 95% of wealth in the top 10% at Gini 0.6 and C ignores
the income data.

Decision: A, Antonio, 2026-11-04.
Row: ADR 0311

### D2. Does WID stay in research packs?

Options:

- A. Proxy in every profile; where WID is in a research fetch cache, the C6 gate report lists
  the proxy's top-10% wealth share against WID's per country, not asserted.
- B. WID in research packs, proxy in release packs: the profiles calibrate differently.

Recommendation: A, because one model across profiles keeps runs and goldens comparable, and WID
still measures the proxy's error.

Decision: A, Antonio, 2026-11-04.
Row: ADR 0312

## Spec changes

- 9.1: the pool is paid by the proxy shares (ADR 0311), not WID's.
- 17: P161, the DFA 2000Q1 top-10% share that fixes tau.
- Econ SPEC, Pack additions: `wealth_0` to `wealth_2` from the proxy for every country;
  `wealth_source` and `wealth_clipped` dropped; WID out of the table's sources.
- Econ SPEC, Factor shares: the proxy equations; WID as a check (ADR 0312).

## Tests

- `wealth_proxy_fits_the_us_top_decile`: the US top-10% share equals P161 (1e-12).
- `wealth_proxy_shares_sum_to_one` (1e-12), each share in (0, 1) and the top share increasing
  in the Gini (`crates/ceteris-pack/tests/factors.rs`).

## Consequences

- Release packs get wealth dividends; no new fetched source (P161 is one published number).
- Code (later step): `econ/factors.rs` computes the shares from the Gini column; `read/wid.rs`
  moves to the C6 report; the `wid` entry in `sources.toml` becomes a research-only check and the
  DFA is cited there.
- Lognormal wealth has no negative net worth: the US bottom 50% comes out 5.1% against the DFA's
  3.2%.
- The installed pack's wealth columns are rebuilt at C6 with the rest; hashes without them are
  unchanged.
