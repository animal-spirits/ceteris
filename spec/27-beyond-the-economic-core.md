## 27. Beyond the economic core (ADRs 0055 to 0087)

A second research pass looked outside the economic core for missing mechanisms and missing couplings between modules. ADRs 0055 to 0080 come from that scan; 0081 to 0087 fill gaps between existing modules. Each PR adds only new files under adr/, so they merge in any order.

| ADR | Decision | Phase | PR |
| --- | --- | --- | --- |
| 0055 | Cohort values: secular and emancipative orientations | P4, P6 | 8 |
| 0056 | Generalized trust and civil society | P6 (P4 pandemic compliance) | 8 |
| 0057 | Gender norms, sex ratios at birth, marriage markets | P4 | 8 |
| 0058 | Religion switching and identity salience | P4, P6 | 8 |
| 0059 | Learning quality, not only years of schooling | P4 (P2 reads 2000 values) | 8 |
| 0060 | Crime, homicide and organized crime | P6, P7 | 8 |
| 0061 | Health-system capacity | P4, P8 | 9 |
| 0062 | Chronic disease risk and antimicrobial resistance | P4, P8 | 9 |
| 0063 | Addiction epidemics | P6 | 9 |
| 0064 | Child nutrition and the care burden of ageing | P4 | 9 |
| 0065 | True and reported statistics | P3, P5, P6, P9 | 10 |
| 0066 | Leaders, tenure and succession | P6 | 10 |
| 0067 | State capacity: building, eroding, collapsing | P6 | 10 |
| 0068 | Subnational governance, land tenure, clientelism, electoral integrity | P6 | 10 |
| 0069 | Non-state armed actors, insurgency finance, terrorism | P7 | 10 |
| 0070 | Development finance actors | P5, P6 | 10 |
| 0071 | Information technology and the information environment | P4, P6 | 11 |
| 0072 | International order, peace operations, courts, soft power | P6, P7 | 11 |
| 0073 | Proliferation, the Arctic, frozen conflicts, intelligence | P7, P8 | 11 |
| 0074 | Settlement hierarchy and urban systems | P4 | 12 |
| 0075 | Infrastructure interdependence and cascading failures | P7, P8 | 12 |
| 0076 | Groundwater, shared river basins, glaciers | P8 | 12 |
| 0077 | Heat, wildfire, fisheries, deforestation frontiers | P8 | 12 |
| 0078 | Chronicle narration, storyteller camera, legends | P3 onward | 13 |
| 0079 | Red-team branches and surrogate sliders | P3, P9 | 13 |
| 0080 | Polycrisis coupling audit | Gates from P5 | 13 |
| 0081 | Seasonality and weather-driven demand | P2, P8 | 14 |
| 0082 | Cohort experience effects on expectations and risk-taking | P5 | 14 |
| 0083 | Policy implementation and diffusion | P5, P6 | 14 |
| 0084 | Price controls, subsidies, parallel markets, currency substitution | P5 | 14 |
| 0085 | Strategic reserves and stockpiles | P5, P7, P8 | 14 |
| 0086 | Reconstruction and recovery | P7, P8 | 14 |
| 0087 | Synthetic observables for validation | P3, P9 | 14 |

Register: P93 to P142 (society P93 to P104, health P105 to P110, state P111 to P119, information and geopolitics P120 to P125, cities and environment P126 to P133, cross-cutting P134 to P142). Switches: S19 cohort values (0055), S20 religion switching (0058), S21 published statistics differ from truth (0065); with each off, golden hashes are unchanged. Amendments to earlier PRs: 0033 gains the WHO CC BY-NC-SA 3.0 IGO note (PR 3); 0050 adds the second licence scan, 0052 non-economic out-of-sample targets and a subnational satellite track, 0054 seven more exclusions (PR 7).

Highest leverage: 0065 changes the data layer from P3, so it should land before any calibration code reads observed series as truth; 0080's coupling register gates every phase from P5. Licence action now: GeoEPR carries no licence and WCDE a custom one, so request permission before P4 needs them (ADR 0050).
