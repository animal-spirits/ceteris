# 0193. Vote intention, turnout and polls

Status: proposed
Date: 2026-10-28
Design sections: 10.5, 10.6, 14.4; ADRs 0046, 0055, 0065, 0068, 0184, 0189, 0190, 0192
Phase: P6 (2.0-alpha)

## Context

Section 10.5 says pops vote by spatial proximity, party attachment and retrospective economic
voting, with turnout tied to education and mobilization. It does not give a functional form, a
model of abstention, strategic voting in single-member districts, or polls between elections.
The unified model of Adams, Merrill and Grofman (2005) puts distance, partisanship and
non-policy factors in one conditional logit and is estimated across many elections with the CSES
micro data. Strategic desertion of non-viable parties (Cox 1997) is what makes Duverger's law
hold. Polls track vote intention with an error that shrinks toward election day (Jennings and
Wlezien 2016, 2018, about 30,000 polls in 45 countries).

## Options

1. Status quo: the 10.5 list, with the form left to implementation.
2. A conditional logit over parties plus abstention for every pop every month, strategic
   discounting under majoritarian rules, and polls in the observed layer.

## Decision

Option 2.

**Utility** of pop i for party j:

```
U_ij = - sum_k s_ik (x_ik - p_jk)^2 + g PID_ij + d 1[j in gov] (A_i - A_ref_i)
       + v_j + c_j tau_ij + e E_ij + eps_ij
```

- s, x, p: salience-weighted distance on the 10.5 axes; ADR 0046 shocks raise salience of the
  nationalism and anti-establishment axes.
- PID: attachment, strengthening with age and with years voting for the party (Converse 1969;
  Achen 1992 running tally), inherited by new cohorts with a parental weight (ADR 0055).
- The approval bridge d (A - A_ref) carries ADR 0192's approval into the vote of governing
  parties.
- v_j: valence from the leader's traits (ADR 0184), scandals and competence reputation.
- c_j tau_ij: clientelist transfers targeted to the pop (ADR 0068).
- E_ij: ethnic or religious tie to the party's base (EPR; Horowitz 1985).
- eps: type I extreme value, so shares are closed form.

**Abstention** is option zero. Its utility rises with education, age, compulsory voting,
mobilization by ADR 0189 organizations and PID strength, and with alienation (every party far)
and indifference (parties alike), as in Adams, Dow and Merrill (2006).

**Strategic voting.** In single-member districts and runoffs, each party's utility is
discounted by its expected viability from current polls (Cox 1997).

**Elections.** Shares are expected values, not sampled ballots. On election day they pass
through the 10.5 formula on the electoral partition of ADR 0190, with one keyed draw per
(state, election) for a national and regional swing whose variance matches election-eve poll
error.

**Polls.** Pollsters observe intention through ADR 0065: sampling error, house effects, an error
that shrinks toward election day, and in autocracies a falsification bias rising with
repression. State AI, the player and the whip count read polls, never true intention.

**Referendums.** The same model with yes and no as the options, for reforms routed to a
referendum gate (ADRs 0195, 0194).

**Parameters.** PID weight, approval bridge, viability discount and turnout loadings are
estimated from CSES and enter as priors; P34 (new-party entry) is unchanged.

## Migration

- P6, in ceteris-polity, after ADR 0192.
- Data: CSES modules 1-5 (terms to audit); Manifesto Project (CC BY-NC-SA, acceptable under the
  non-commercial decision); V-Party (CC BY-SA 4.0); CHES and ParlGov for platforms and results;
  CLEA for district results; International IDEA for turnout (terms to audit for each, ADR 0050).

## Tests

- Shares including abstention sum to one for every pop.
- With the approval bridge, valence, clientelism, ethnic tie and strategic discount off, shares
  equal the 10.5 spatial-plus-attachment baseline bit for bit.
- A three-party plurality toy converges to two viable parties per district; the same toy under
  proportional representation does not.
- Simulated 2000-2010 vote shares track ParlGov within the P6 acceptance tolerance; H7
  (Funke, Schularick and Trebesch 2016) and the China-shock polarization event study (Autor,
  Dorn, Hanson and Majlesi 2020) recover the published signs.
- Simulated poll error by days to election matches the Jennings-Wlezien funnel.
- Polls never change a state hash.

## Consequences

Elections become the month expected shares turn into seats, and polls exist in between, so
governments, players and the whip count act on noisy forecasts as real ones do. The cost is
about 900,000 logit terms a month (0.3 ms) and about 4-6 part-time weeks.
