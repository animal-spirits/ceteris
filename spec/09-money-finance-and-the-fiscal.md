## 9. Money, Finance and the Fiscal State

The financial system is a stock-flow-consistent network of balance sheets in which central banks set policy, commercial banks create money by lending, asset prices move with expectations, and crises propagate through bank, housing and sovereign exposures.

### 9.1 Sectoral balance sheets

Each economy has households, non-financial enterprises, banks, government, central bank and the rest of the world (all other states, endogenous). The balance-sheet and transactions-flow matrices of Godley and Lavoie (2007) are computed every tick and must sum to zero across rows and columns. Agent-level claims (loans, mortgages, bonds, deposits, equity, reserves) aggregate exactly to the sector matrices, the standard SFC-ABM construction (Caverzasi and Godin 2015). Initial balance sheets come from national financial accounts where available and are imputed elsewhere. Until these balance sheets exist (P5), profits are owned nationally and paid by wealth: each country's pool goes to its pops by the 2000 shares of net personal wealth of its bottom 50%, middle 40% and top 10%, income and wealth perfectly rank-correlated over the mixture of the pops' income distributions at base incomes, fixed at load (ADR 0308). The shares come from a lognormal wealth distribution whose log variance is the income Gini's plus one constant fitted to the US Distributional Financial Accounts of 2000 (ADR 0311; P161); WID, which states no licence, is only a research check (ADR 0312). A foreign-owned share of mining, oil and gas and manufacturing profits, the OECD Analytical AMNE 2000 foreign share of value added capped at 0.8, is paid to the parent countries' pools by AAMNE parent weights (ADR 0309) through one world dividend account in the same tick, as income (ADR 0310).

### 9.2 Monetary and exchange-rate regimes

| Regime | Examples in 2000 | Policy rule |
| --- | --- | --- |
| Floating, inflation targeting | United States, United Kingdom, Brazil | Taylor-type rule on inflation and output gap, effective lower bound, asset purchases when at the bound |
| Managed float / crawling peg | China, many emerging markets | Exchange-rate band defended with reserves and capital controls |
| Hard peg / currency board | Hong Kong, Gulf states | Rate set to defend the peg; reserves constrain |
| Monetary union | Euro area, CFA franc zones | One central bank for many states, no national devaluation |
| Dollarization | Ecuador, Panama | No independent monetary policy |

Regimes are initialized from the de facto classification of Ilzetzki, Reinhart and Rogoff and change through transition hazards on inflation, reserve cover and recent peg breaks; entering or leaving a monetary union stays at t0 until 2.0 (ADR 0242). Floating exchange rates are cleared in one place, the global demand system of 9.9: current-account and income flows enter it as net currency demand absorbed by financiers with limited risk-bearing capacity (Gabaix and Maggiori 2015 is that special case), and central banks intervene from reserves as one more investor class.

### 9.3 Banks and asset markets

Banks hold reserves, loans, mortgages, bonds and interbank claims against deposits, wholesale funding and equity. Lending creates deposits up to capital (Basel-style) and liquidity constraints; loan rates are the policy rate plus a risk premium from borrower leverage. Bank runs follow a threshold rule on depositor and wholesale-funder confidence, softened by deposit insurance; contagion runs through interbank exposures (Gai-Kapadia). Housing and equity prices are formed by agents valuing expected rents and earnings with heuristic expectations (Section 12), so leveraged housing booms and busts, the core of 2008, can emerge.

### 9.4 Sovereign debt, default and the IMF

States issue bonds in local or foreign currency. Spreads use a reduced-form default-risk function inspired by Eaton-Gersovitz and Arellano (2008):

```latex
s_{j,t} = f\left( \frac{\text{debt}}{\text{revenue}}, \; \text{deficit}, \; \text{foreign-currency share}, \; \text{reserves}, \; \text{institutions}, \; \text{default history}, \; \text{lender of last resort} \right)
```

Membership in a monetary union without a lender of last resort raises spreads under stress, the mechanism of the euro crisis. Default is a strategic choice of the state AI; consequences are market exclusion, restructuring with haircuts, and financial-sector losses wherever the bonds are held. The IMF is a stylized agent that lends with conditionality (fiscal targets) to states that request it.

Pricing and default are tied: the default probability that prices sovereign debt in 9.10 is the hazard implied by the state AI's own default rule, evaluated on the same state variables (Eaton-Gersovitz consistency), so spreads and realized defaults cannot drift apart. The function above is the prior for that rule, not a second pricing mechanism.

### 9.5 Capital flows and reserves

Savers and banks allocate across domestic and foreign assets through the demand system of Section 9.9, with home bias; central banks accumulate reserves in safe assets, which creates the demand for dollar and euro bonds. Sudden stops occur when risk appetite drops and foreign holders sell. Targets: current-account and net foreign-asset paths, and capital-flow reversals in 2008 and 2013.

Until this finance module exists (P5), the observed imbalances are exogenous transfers (ADR 0100): each country receives D_c each month from one world transfer account, its current-account deficit as a share of its own GDP times its national income of the previous month, less its income share of the world discrepancy (ADR 0315); the transfers net to zero every tick, in the integer ledger. The shares are the 2000 ones (default) or the observed path 2000 to 2024, fading to zero with a 10-year half-life after it (lab profile); a current account WDI does not report is 0 and flagged, and the world discrepancy is spread pro rata to income (ADRs 0296, 0300, 0315). Households receive D_c in proportion to income and spend it with their income through the three final demanders (ADRs 0297, 0303). With r_g gone, calibration balances each good's world sales and spending by a biproportional rescale of each country's traded final demand, rows its traded absorption at income + D_c, columns the good's world sales less its intermediate use; output and value added stay at the data (ADR 0295; the fixture's value added is scaled by its former r_g so that it is balanced at D_c = 0, ADR 0299).

### 9.6 Fiscal state

Revenue instruments: personal income tax, payroll and social contributions, VAT and sales taxes, corporate tax, tariffs, excises (fuel, tobacco, carbon), resource royalties and state-enterprise profits, seigniorage. Collected revenue is

```latex
R_{j,i} = \tau_{j,i} \times \text{Base}_{j,i} \times \kappa_{j,i} \times \text{compliance}_{j}
```

where kappa is fiscal capacity per instrument (income tax needs administration and formal employment; tariffs and royalties need little), built by investment and depreciating slowly, following Besley and Persson (2009). Informality shrinks the base. Spending: pensions, health, education, social transfers, defence, infrastructure, energy subsidies, interest. Initial tax rates, revenue and spending come from international government revenue and finance statistics. Deficits are financed by borrowing or monetary financing; budgets are set by the state AI under institutional rules (Section 10).

### 9.7 Financial markets: design stance

Asset prices are set once a month by clearing the portfolio demands of heterogeneous, constrained investor classes against the stock of every instrument in the balance sheets of 9.1; there are no order books, named firms or daily markets (consistent with 15.7). Fidelity comes from three sources: an exact who-to-whom map of claims, demand curves estimated from holdings data, and balance-sheet constraints that turn price falls into forced sales. Sections 9.8 to 9.14 refine 9.3 to 9.5.

Within a tick, clearing and constraints iterate to a fixed point: prices clear demands, losses hit capital and margins, constrained holders sell, and prices clear again. The loop runs a fixed number of rounds in a fixed order, so it obeys the determinism rules of 4.5. This inner loop, not a daily time step, carries fire sales, runs and sudden stops.

&#91;embedded content: one month of financial clearing · 6 steps, 1 feedback loop\]

Prices clear, losses hit balance sheets, and binding constraints force sales that re-enter the demand system; only when no constraint binds does policy act and the month close.

### 9.8 Instruments and holders

Each state carries about ten instrument classes, each split by currency of denomination, held by about ten investor classes at home and abroad. Every asset is someone's liability, so the who-to-whom matrix sums to zero each tick (a V0 invariant).

| Instrument | Issuer | Main holders | Initial stocks from |
| --- | --- | --- | --- |
| Currency, bank reserves | Central bank | Households, banks | Central bank balance sheets, IMF IFS |
| Deposits | Banks | Households, enterprises, government | National financial accounts (OECD, ECB, Fed Z.1) |
| Loans, mortgages | Households, enterprises | Banks, nonbank lenders | BIS credit to the non-financial sector |
| Sovereign bills and bonds (local and foreign currency, by maturity bucket) | Governments | Domestic banks, central banks, pensions and insurers, funds, foreign reserve managers | BIS debt securities; IMF sovereign investor base (Arslanalp and Tsuda) |
| Corporate and bank bonds | Enterprises, banks | Insurers, pensions, funds, foreign investors | BIS debt securities |
| Equity (listed and unlisted) | Enterprises, banks | Households, funds, pensions, foreign investors | Financial accounts, market capitalization series |
| Official FX reserves | Reserve-currency sovereigns and banks | Central banks | IMF IFS, currency shares from IMF COFER |
| Cross-border claims by currency | All sectors | Banks, portfolio investors, direct investors | BIS locational and consolidated banking statistics, IMF CPIS and CDIS, Lane and Milesi-Ferretti (2018) |
| Housing (real asset) | Not a claim | Owner-occupiers, landlords | Residential property price series, WID wealth shares |

Investor classes per state: households (by pop wealth bracket, portfolio shares from household finance surveys), enterprises, banks (archetypes in 9.11), insurers and pension funds, investment funds, leveraged funds, central bank and reserve manager, government and sovereign wealth fund. Foreign investors are the same classes of other states. Derivatives are not instruments; FX swaps enter only as short-term dollar funding of non-US banks, and credit derivatives are omitted.

### 9.9 Price formation: a global demand system

Prices clear a characteristics-based demand system (Koijen and Yogo 2019), extended to many currencies as in Koijen and Yogo (2020), so exchange rates and asset prices clear jointly. Investor class i with assets A puts portfolio weight w on asset a:

```latex
w_{i,a,t} = \frac{\exp(\delta_{i,a,t})}{1 + \sum_{b \in \mathcal{A}_i} \exp(\delta_{i,b,t})}, \qquad \delta_{i,a,t} = \gamma_i \, \mathbb{E}_{i,t}\left[ r_{a,t+1} \right] + \beta_i' x_{a,t} + \eta_{i,a}
```

The 1 in the denominator is the outside asset (deposits or cash). Characteristics x are currency, home or foreign issuer, default probability, duration, liquidity and mandate eligibility; an ineligible asset (a junk bond for a constrained insurer) gets zero weight. Expected returns come from the asset blocks of 9.10 and the expectations of 9.13. Each month, prices P solve:

```latex
P_{a,t} \, Q_{a,t} = \sum_i A_{i,t} \, w_{i,a,t}(P_t) \quad \text{for every asset } a
```

- **Inelastic markets.** Demand elasticities follow the inelastic markets hypothesis (Gabaix and Koijen 2021): a dollar of flow into equity moves market value by several dollars. Flows from QE, reserve managers, pension rebalancing or fire sales therefore move prices realistically.
- **Exact initialization.** The latent demand term eta is backed out so that 2000 holdings are reproduced exactly, as in Koijen and Yogo; it is data-determined, not a free parameter.
- **QE and reserve demand for free.** Central-bank purchases and reserve accumulation are just another investor's demand, so their price effects need no special mechanism; euro-area QE estimates (Koijen, Koulischer, Nguyen and Yogo 2021) are a validation target.
- **Solver.** Damped Newton on log prices with a block-sparse Jacobian (home bias keeps most weights within a state), a fixed iteration count and fixed reduction order. Each investor class's demand is restricted to slope downward in the asset's own price, as Koijen and Yogo impose, since extrapolative expectations (9.13) could otherwise make it slope upward and leave clearing without a unique solution. A residual counter per market, like the clip-binding counters, records every tick where the fixed iteration count leaves clearing error above tolerance. Cost scales with roughly 2,000 assets and 2,000 investor classes worldwide, small next to trade clearing (8.3).

### 9.10 Asset-class blocks

Each asset class supplies the demand system with a fundamental anchor and a premium driver, so prices stay tied to the simulated real economy while flows and constraints move them around that anchor.

| Asset | Fundamental anchor | Premium or spread driver | References |
| --- | --- | --- | --- |
| Sovereign yields, local currency | Expected policy-rate path from the rules of 9.2 | Term premium from duration supply against price-insensitive holders | Vayanos and Vila (2021) |
| Sovereign spreads | Default probability from 9.4 (debt to revenue, r minus g, foreign-currency share, reserves) | Liquidity; redenomination risk in unions; self-fulfilling rollover runs when refinancing needs are high | Calvo (1988); De Grauwe and Ji (2013); Lorenzoni and Werning (2019) |
| Equity, per state and sector group | Dividends from simulated operating surplus by sector (Section 7) | Discount rate that rises as intermediary capital falls | He, Kelly and Manela (2017) |
| Corporate and bank credit | Merton default probability from sector leverage and earnings volatility | Excess bond premium tied to intermediary capital | Merton (1974); Gilchrist and Zakrajsek (2012) |
| Housing, per province | Rents from province income and population; user cost | Credit limits (LTV, DTI), mortgage rates; supply elasticity from land and terrain (5.1) | Mian and Sufi (2009); Saiz (2010) |
| Exchange rates | Cleared jointly in 9.9; pegs and bands defended per 9.2 | Limited risk-bearing capacity of financiers; dollar convenience yield | Gabaix and Maggiori (2015); Jiang, Krishnamurthy and Lustig (2021) |
| Commodities | Physical balance and competitive storage (15.7) | Small financial-investor demand | Deaton and Laroque (1992) |

Housing is the one market cleared outside the demand system: each province clears owner and landlord demand against a slowly adjusting stock, and mortgages link it back to bank balance sheets.

### 9.11 Banks, contagion and the safety net

Each state has up to five bank archetypes rather than named banks: a global systemic bank (holding the cross-border book), a domestic universal bank, regional savings and cooperative banks (weighted by province), a state-owned bank, and nonbank lenders. Balance sheets come from financial accounts and BIS banking statistics.

- **Constraints.** Risk-weighted capital, leverage and liquidity coverage ratios. As buffers shrink, lending tightens and the bank's eligibility terms in 9.9 push it to sell risky assets.
- **Funding.** Insured and uninsured deposits, wholesale funding, and short-term dollar funding of non-US banks through FX swaps (McGuire and von Peter 2009; Ivashina, Scharfstein and Stein 2015), the channel of the 2008 dollar shortage.

Contagion runs through four channels, all resolved inside the monthly fixed-point loop of 9.7:

1. Direct exposures on the interbank and cross-border network (Gai and Kapadia 2010, as in 9.3).
2. Fire sales on common holdings (Greenwood, Landier and Thesmar 2015).
3. Funding runs: uninsured and wholesale funders withdraw past a threshold (Diamond and Dybvig 1983), with seeded noise keyed by the rules of 14.4 so ensembles show run probabilities, not certainties.
4. The sovereign-bank loop: home-biased holdings of own-government debt tie bank capital to sovereign spreads and bailouts to public debt (Brunnermeier et al. 2016; Farhi and Tirole 2018).

Safety-net levers in the state and central-bank action space: lender-of-last-resort lending against haircut collateral, deposit insurance limits, guarantees, recapitalization, bail-in or resolution, asset purchases, swap lines, and macroprudential tools (countercyclical capital buffer, LTV and DTI caps). Their costs feed the fiscal state (9.6) and spreads (9.4). These levers make the scenarios 'Lehman rescued' and 'the euro without a lender of last resort' plain decision overrides (14.2). Levers without a 0210 rule hold their t0 values and move through tightening and loosening hazards driven by simulated crises and the credit gap (ADR 0242).

### 9.12 Global financial cycle, the dollar and capital controls

One global risk-appetite state, an analogue of the VIX, correlates every market, so stress spreads across borders the way the global financial cycle does (Rey 2013; Miranda-Agrippino and Rey 2020).

- **Risk appetite.** Its innovations follow a stochastic-volatility process with fat tails, drawn from the run's random streams like weather and written to its event log (ADR 0241). Its level responds endogenously to US monetary policy and to intermediary losses, and it scales the return sensitivity of every investor class in 9.9. Crises themselves stay endogenous (13.6).
- **Dollar centrality.** Dollar invoicing, dollar debt of emerging-market sovereigns and firms, and reserve-manager demand for Treasuries give the dollar a convenience yield, so it appreciates in stress. Fed swap lines are a lever that relieves dollar funding of foreign banks.
- **Capital flows.** Gross flows emerge from portfolio rebalancing in 9.9; nothing is scripted. Sudden stops (Calvo 1998) and flow surges and retrenchments (Forbes and Warnock 2012) are validation targets, not mechanisms.
- **Capital controls and sanctions.** Controls are taxes or caps on foreign classes' weights in a state's assets, initialized from the capital-control measures of Fernandez et al. (2016). Sanctions, reserve freezes and payment-system exclusion (11.8) set eligibility to zero.
- **Pegs under attack.** A peg is defended with reserves and rates (9.2). As reserves fall relative to short-term external debt, investor demand for the currency weakens and the state AI weighs abandoning the peg; the Argentine and Turkish crises of 2001 are test cases.

### 9.13 Expectations and endogenous crises

Financial expectations reuse the heuristic switching of 12.2, with one addition: a diagnostic rule that over-weights recent good news (Bordalo, Gennaioli and Shleifer 2018), which lets credit booms build and reverse without scripted shocks. Weights are disciplined by survey data: return expectations that extrapolate past returns (Greenwood and Shleifer 2014), plus SPF and Michigan inflation expectations.

No crisis is scripted. Each type below emerges from the mechanisms above and is scored as a frequency across ensembles, never as a date.

| Crisis type | Built-in trigger | Propagation | Validation cases, 2000-2022 |
| --- | --- | --- | --- |
| Banking | Capital depletion after rapid credit growth (Schularick and Taylor 2012) | Runs, fire sales, credit crunch | United States, United Kingdom, Ireland, Iceland 2008; Cyprus 2013 |
| Sovereign debt | Spread spiral when rollover needs are high and no lender of last resort stands behind the debt | Bank losses through the sovereign-bank loop; austerity through 9.6 | Argentina 2001; Greece 2010-2012; Lebanon 2020; Sri Lanka 2022 |
| Currency | Reserves low against short-term external debt; dollar-debt mismatch | Depreciation raises the burden of dollar debt | Turkey 2001 and 2018; Russia 2014; Argentina 2018 |
| Sudden stop | Drop in global risk appetite; foreign investor classes exit | Credit crunch, current-account reversal | Emerging markets 2008-09; taper tantrum 2013 |
| Housing bust | Leveraged price boom with extrapolative expectations | Mortgage defaults; consumption falls through household balance sheets | United States 2006-2009; Spain and Ireland 2008 |

Crisis frequencies and types by region are scored against the systemic banking crisis database of Laeven and Valencia (2018).

### 9.14 Calibration, validation and build order

Finance gets at most 12 of the 60 free behavioural parameters (Section 17): run thresholds, fire-sale impact, the diagnostic weight and the risk-appetite process. Demand elasticities are literature-anchored, and latent demand and initial balance sheets are data-determined.

**Targets**

- Yield-curve levels and term premia (Adrian, Crump and Moench 2013), sovereign spread distributions.
- Equity and FX volatility, cross-country return correlations, exchange-rate pass-through.
- Credit-to-GDP and house-price paths; gross capital flows and net international investment positions.
- Price effects of QE and reserve accumulation; crisis frequencies (9.13).

**Holdings data.** Commercial fund-flow and holdings data is not free, so investor holdings come from IMF CPIS, ECB securities holdings statistics and US Treasury TIC data. The Jorda-Schularick-Taylor Macrohistory database is likely non-commercial; use it for priors only and never bundle it. BIS, IMF and ECB terms for a commercial build are to be confirmed (Steam release plan).

**Build order.** Each stage passes the ablation test of 15.7 before the next starts.

1. Balance sheets and who-to-whom matrices, international investment positions, policy rates and exchange-rate regimes, sovereign yields from expected policy paths.
2. The demand system and investor classes; equity, credit and FX clearing; province housing markets.
3. Bank archetypes, the monthly fixed-point loop, the four contagion channels and safety-net levers.
4. Global risk appetite, capital controls and sanctions, then validation of endogenous crises.

