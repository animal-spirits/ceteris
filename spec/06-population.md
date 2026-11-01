## 6. Population: Pops, Demography, Identity and Migration

Population is both the labour supply and the political substrate, so each pop carries economic state (education, labour status, income, wealth) and political state (ethnic identity, ideology, party attachment, grievance) in one record.

### 6.1 Pop definition

Key: (province, ethnic group, education, labour status), about 100,000 pops after dropping empty cells.

| Key dimension | Categories | Initialization source |
| --- | --- | --- |
| Ethnic group | Politically relevant groups per country, plus 'other' | GeoEPR settlement areas and group sizes |
| Education | Below secondary, secondary, tertiary | Barro-Lee / Wittgenstein Centre attainment, subnational where available |
| Labour status | Formal wage employee, informal worker, self-employed / smallholder, business owner / employer, public sector, unemployed, inactive (students, retirees, carers) | ILO labour statistics, household surveys |

Religion is a share vector within each pop rather than a key, initialized from available national and subnational religion data, to keep the pop count bounded. Components per pop: size by age band and sex (five-year bands), income mean and log-variance, wealth by asset type, consumption basket, employer links, ideology vector, party attachment, grievance, language and religious shares.

### 6.2 Fertility and mortality

Mortality uses the Brass relational logit life table: a standard schedule shifted by a level parameter and tilted by a shape parameter.

```latex
\operatorname{logit}\left(1 - l_x\right) = \alpha_{p,t} + \beta \, \operatorname{logit}\left(1 - l_x^{std}\right)
```

The level alpha responds to real income (a Preston-curve relation), health spending, epidemics, conflict and climate damages (heat). Fertility is a pop-level total fertility rate driven mainly by female education, income, urbanization and child survival, with a floor well below replacement so that sustained low-fertility regimes are reachable:

```latex
TFR_{p,t} = TFR^{min} + (TFR^{max} - TFR^{min}) \cdot \sigma\!\left(\gamma_0 - \gamma_e \, e^{f}_{p,t} - \gamma_y \ln y_{p,t} - \gamma_u \, \text{urban}_{p,t} + \gamma_m \, \text{child mortality}_{p,t}\right)
```

Rates are initialized to match UN World Population Prospects for 2000; 2000-2023 estimates are the validation target. UN projections beyond 2024 are a comparison benchmark, not a target. Aging feeds directly into labour supply and pension spending, which matters for open-ended runs.

### 6.3 Education and labour status

Cohorts move up education levels at enrollment rates set by public and private education spending and household income. Workers move between labour statuses each month: matching in formal labour markets, entry to informality when formal jobs are scarce, business formation and failure, retirement by age and pension law. Skill premia emerge from sector demand for educated labour.

### 6.4 Ethnicity, religion and identity politics

Ethnic groups carry a political status from the Ethnic Power Relations data (dominant, senior or junior partner, powerless, discriminated). Horizontal inequality between groups (income and political exclusion) feeds grievance, the mechanism documented by Cederman, Gleditsch and Buhaug (2013) for civil conflict. Language and religiosity shift through the cultural transmission model of Bisin and Verdier (2001), with state policy (schooling language, discrimination, autonomy) as levers.

### 6.5 Migration

Internal and international migration share one dynamic discrete-choice model in the Artuc-Chaudhuri-McLaren / Caliendo-Dvorkin-Parro tradition. Each month a share of each pop reconsiders location; destinations are chosen by logit over expected values net of moving costs:

```latex
\mu_{od} = \frac{\exp\left( (\beta \, \mathbb{E} V_d - m_{od}) / \nu \right)}{\sum_k \exp\left( (\beta \, \mathbb{E} V_k - m_{ok}) / \nu \right)}
```

Moving cost m depends on distance and travel cost, immigration law and visa regimes (free movement inside unions such as the EU), language and cultural distance, credit constraints, and diaspora networks. Expected value uses Harris-Todaro expected wages (wage times employment probability), amenities, conflict risk and climate damages. Forced displacement from conflict and disasters follows a separate hazard-driven flow toward safe neighbours. Remittances flow back as household transfers, a large financial flow for many economies. The dispersion parameter nu is calibrated to bilateral migration stocks and flow estimates; internal urbanization rates are a second target.

**Computation.** A dense logit is 100,000 pops x 4,594 destinations, about 5 x 10^8 exponentials a month, far over budget. Choice probabilities depend on a pop only through its origin province and the attributes that enter V (education, ethnic group), so they are computed per (origin province, education, group) cell over a sparse choice set: provinces within a cost radius, the largest cities, and foreign gateways, with remaining destinations pooled in one nest per country. Expected values follow the Caliendo-Dvorkin-Parro recursion, updated once per tick:

```latex
V_{o,t+1} = u_{o,t} + \nu \ln \sum_{d} \exp\left( \left( \beta V_{d,t} - m_{od} \right) / \nu \right)
```

so value iteration is spread over ticks like trade clearing (8.3), and its fixed point is the CDP steady state.

