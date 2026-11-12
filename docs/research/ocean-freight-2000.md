# Ocean freight per tonne-km around 2000, against road (ADR 0269, P04c)

2026-10-31. Reading only. Road = BTS 2000 truck, 0.094 USD per tonne-km (ADR 0269).

## Findings

1. **Liner (container) cargo, classes 0 to 2, is not much cheaper per tonne-km than barge.**
   All-in port-to-port rates give 0.04 to 0.25 of road; the four-route central value is about
   0.09. P04c at 0.121 is inside the range, near the transpacific headhaul. The intuition that
   ocean undercuts barge holds for bulk, not for boxes.
2. **Bulk, class 3, is two orders below road**, and is already sourced in ADR 0247: marginal
   slope 2.05e-4 USD per tonne-km (0.0022 of road), all-in Tubarao-Rotterdam 7.4e-4 (0.0078).
3. **0269 moves 0247's slope by accident.** 0247 defines `bulk_sea = s / P04c` with P04c 0.008.
   0269 raises P04c to 0.0121 and keeps `bulk_sea` 0.026, so class 3's sea slope becomes
   3.1e-4, 1.51 times the sourced 2.05e-4. To keep 0247's sourced rate, `bulk_sea` becomes
   2.05e-4 / 0.0121 = **0.017** (the port multiplier 0.40 is unaffected).
4. **Per-km cost falls with length.** Distance elasticities of ocean freight per tonne:
   0.17 to 0.25 (Clark, Dollar and Micco 2004, Tables 1 and 3; 0.245 for 2000), 0.151
   (Hummels 2007, Table 5 col. 3). A linear per-km rate therefore overprices long legs against
   short ones; that is ADR 0264's log-distance leg cost (P9), not a P04c level question.

## Container rates converted

Rates per TEU, port to port, divided by tonnes per TEU and sea km (Hong Kong-Los Angeles about
11,800 km, Hong Kong-Rotterdam via Suez about 18,000 km; nautical distances, not the pack's
graph).

| Route, direction | USD/TEU | Source | USD/t-km at 8 / 10 / 12 t per TEU | Per road at 10 t |
|---|---|---|---|---|
| Asia to US (headhaul) | 1,561 | CDM 2004 fn. 15, citing UNCTAD RMT 1999 | 0.0165 / 0.0132 / 0.0110 | 0.141 |
| Asia to US, 1999-2000 peak | 2,203 | Hummels 2009 (ITF), Containerisation International | 0.0233 / 0.0187 / 0.0156 | 0.199 |
| US to Asia (backhaul) | 999 | CDM 2004 fn. 15 | 0.0106 / 0.0085 / 0.0071 | 0.090 |
| Asia to Europe | 1,353 | CDM 2004 fn. 15 | 0.0094 / 0.0075 / 0.0063 | 0.080 |
| Europe to Asia | 873 | CDM 2004 fn. 15 | 0.0061 / 0.0049 / 0.0040 | 0.052 |

Range over the table and tonnage band: 0.043 to 0.248 of road. Mean of the four CDM routes at
10 t: 0.091.

Caveats:
- **Tonnes per TEU is not sourced.** 8 to 12 t is my assumption. RMT 2015 Fig. 1.2 gives 598 Mt
  of containerised trade in 2000; dividing by a global loaded-TEU count for 2000 would pin it,
  but I could not read that figure (RMT 2008 Fig. 7 / Table 56 did not extract).
- CDM's rates are from RMT 1999; the year they describe is not stated (1998 or 1999 likely).
- These rates include terminal and port charges. P04m charges 8 USD/t per listed port
  (16 USD/t a trip, about 160 USD/TEU at 10 t). Netting that out lowers each per-km figure by
  about 10 to 20% (Asia to US at 10 t: 0.0119, 0.127 of road).
- Stopford (2002, via Rodrigue, *Geography of Transport Systems*) puts the ship itself at 23%
  of door-to-door container cost, containers 18%, ports 21%, inland 25%, other 13%: most of a
  liner rate is not distance-proportional.

## Suggested register entry for P04c

Value 0.121 can stay (or 0.09, the four-route mean). Range for P9: **0.05 to 0.20**, closing
the open lower end: 0.05 is the Asia-Europe backhaul all-in at 10 t, 0.20 the 1999-2000
transpacific peak. Bulk stays in ADR 0247 (P154 rescaled to 0.017, finding 3).

## Sources and how they were read

Network access here is limited to a fetch tool that extracts text through a summariser; PDF
downloads were blocked. None was read cover to cover.

- Clark, Dollar and Micco (2004), "Port efficiency, maritime transport costs, and bilateral
  trade", NBER WP 10353 (J. Dev. Econ. 75(2)). Read in extraction, targeted at data, rates and
  elasticities. https://www.nber.org/papers/w10353.pdf
- Hummels (2007), "Transportation costs and international trade in the second era of
  globalization", JEP 21(3). Most of the text in extraction; no per tonne-km ocean figure in it.
  https://business.purdue.edu/faculty/hummelsd/papers/hummels%20jep%20rewrite%20final%20with%20tables.pdf
- Hummels (2009), "Globalisation and freight transport costs in maritime shipping and
  aviation", ITF Forum Paper 2009-3. First 100k of 117k characters; Table 3-1 values did not
  extract. https://www.itf-oecd.org/sites/default/files/docs/09fp03.pdf
- UNCTAD RMT 2000 and 2002: summaries only; the rate tables (2000 Tables 34, 39-41; 2002
  Tables 31, 32, 36, 41) did not extract. Freight factor 2000: 6.21% world, 5.21% developed,
  8.83% developing (RMT 2002 p. ix). RMT 2001 failed to fetch.
- UNCTAD RMT 2015, Fig. 1.2 (598 Mt containerised, 2000) and Fig. 1.6(b) (East-West flows).
- ADR 0247 (local branch claude/sleepy-sinoussi-a80fd4): its 8-route bulk table from RMT 2001
  and 2002, read in full.
- FAO Food Outlook Oct 2001: US Gulf-Japan grain 17.60 to 19.50 USD (2001, unit not stated,
  presumably per tonne). No 2000 grain rate found.
- Baltic indices: only BDI points (1,320 to 1,684 in 2000, FAO Food Outlook), no per-tonne
  route rates; not used.
- Golub and Tomasik (2008), OECD WP 609: costs per kg from RMT per-TEU rates, charts only.
