# 0167. Endemic disease: HIV/AIDS, malaria and tuberculosis

Status: proposed
Date: 2026-10-28
Design sections: 6.2, 9.6, 13.3, 15.4; ADRs 0033, 0061, 0070
Phase: P4

## Context

Neither DESIGN.md nor any ADR models HIV/AIDS, malaria or tuberculosis. ADR 0033's
communicable cause group trends with income, health spending and urbanization, and 13.3's SEIR
model covers epidemics that arrive and burn out. Endemic diseases do neither: they persist at a
prevalence that treatment and prevention move.

This matters inside the calibration window. Life expectancy in southern Africa fell by more than
ten years between 1990 and the mid-2000s and recovered once antiretroviral therapy scaled up
after 2004; in KwaZulu-Natal adult life expectancy rose 11.3 years in eight years (Bor et al.
2013). Insecticide-treated nets averted most of the malaria cases prevented in Africa between
2000 and 2015 (Bhatt et al. 2015). A Preston-curve Brass level cannot produce either path, so the
P4 acceptance test (mortality within tolerance of UN data for 2000-2010) is at risk for about 20
states. Treatment also restores labour supply: ART raised the labour supply of treated workers
by about 20% within six months in western Kenya (Thirumurthy, Graff Zivin and Goldstein 2008).
Gap audit (2026-10-28): https://claude.ai/artifact/HRDQVbjJz8MrcDuPphPyYs

## Options

1. Status quo: endemic disease is absorbed by 0033's communicable trend.
2. A scenario hazard per country, like 0033's deaths-of-despair lane.
3. Endemic prevalence stocks per province with incidence, treatment coverage and prevention as
   state, feeding mortality and morbidity.

## Decision

Option 3.

- State: prevalence of HIV, malaria (Plasmodium falciparum) and TB per province and age band,
  updated monthly by a compartmental incidence-and-exit rule calibrated to UNAIDS Spectrum
  outputs, Malaria Atlas Project surfaces and WHO TB estimates. Transmission coefficients are data
  fitted offline per region (ADR 0108 window), not free parameters.
- Levers: ART coverage, bednet and indoor-spraying coverage and TB treatment coverage. Each is
  funded from the health budget (9.6) and from donor programmes (0070), and implemented at the
  rate set by health-system capacity (0061).
- Mortality: disease deaths replace the part of 0033's communicable component these diseases
  carry, so totals are not double counted.
- Morbidity: untreated prevalence lowers effective labour per worker in the pop (register entry,
  prior from Thirumurthy et al. 2008) and raises care need (0064).
- Malaria transmission scales with temperature and rainfall from the climate emulator (0037),
  which links it to the damage lanes without a separate damage term.

## Migration

- P4: three prevalence arrays per province and age band in the demography module; coverage
  levers in the fiscal and donor accounts.
- Pack: initial prevalence and coverage for 2000; regional transmission fits.
- Data: see ADR 0182 (UNAIDS estimates, Malaria Atlas Project, WHO TB).

## Tests

- With prevalence fixed at zero, mortality equals 0033's path and golden hashes are unchanged.
- 2000-2010 life expectancy in Botswana, South Africa, Zimbabwe and Malawi falls and recovers
  within the UN WPP band when the historical coverage path is fed.
- An ART scale-up in a toy province raises labour supply within the published range.

## Consequences

Southern African demography and labour supply follow the observed path in the calibration
window, and aid and health spending gain a measurable channel. Three regional transmission fits
enter the pack as data. Register: 1 entry (morbidity labour loss).
