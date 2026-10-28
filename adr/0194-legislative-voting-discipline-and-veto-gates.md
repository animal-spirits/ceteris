# 0194. Legislative voting, discipline and veto gates

Status: proposed
Date: 2026-10-28
Design sections: 10.5, 10.6, 14.4, 23.4; ADRs 0068, 0096, 0129, 0130, 0156, 0164, 0183, 0186, 0189, 0190, 0192, 0193, 0195
Phase: P6 (2.0-alpha), after the bodies of ADR 0195

## Context

Section 10.6 gives legislatures one line: "proposer-veto bargaining; bills progress monthly and
may fail". ADR 0164 promises a whip count by party and group with intervals and "what would move
it", but nothing produces the votes it would count. Legislators already exist as characters on
the bench (ADR 0183), and ADR 0195 routes each reform to bodies with their own decision rules.
The literature gives each missing piece a tested form:
spatial roll-call voting (Poole and Rosenthal 1997), setter and pivot models of veto gates
(Romer and Rosenthal 1978; Krehbiel 1998; Tsebelis 2002), party discipline from confidence
fusion and nomination control (Diermeier and Feddersen 1998; Carey and Shugart 1995; Carey 2007;
Kam 2009), negative agenda control (Cox and McCubbins 2005; Doring 1995), vote buying
(Groseclose and Snyder 1996), coalitional presidentialism (Raile, Pereira and Power 2011;
Chaisty, Cheeseman and Power 2018) and coalition exit (Lupia and Strom 1995). Saiegh (2011)
compiles executive legislative success rates across about 800 country-years, a direct target.

## Options

1. Status quo: a bill passes when the government holds a majority, with a failure hazard.
2. Party blocs as voters with a spread of member positions.
3. Every legislator votes on every bill stage with a probability, under the whip, inside a
   sequence of veto gates; blocs are an aggregation view.

## Decision

Option 3. There are about 46,000 parliamentarians worldwide (IPU Parline), already modelled as
characters, so per-legislator voting costs microseconds per stage.

**Bills.** A bill is a bundle of lever changes or edit objects (ADR 0195's reform bills). Its
ideological face is the change projected onto the 10.5 axes. Its distributive face is the
derivative of each pop's life evaluation, each organization's demands gap and each district's
vote share with respect to the bill, from the tangents that build the influence web (ADRs 0096,
0156) and the bill's preview.

**A legislator's vote.**

```
Y_l = |x_l - q|^2_S - |x_l - b|^2_S + psi C_l + w O_l + d_p w_p + pi_l + nu_l
Pr(yes_l) = logistic(Y_l / sigma)
```

- Policy: the spatial gain of bill b over status quo q from the legislator's ideal point (party
  position plus a personal offset, ADR 0184).
- C_l, self-interest: the effect on the legislator's own re-election odds and office, read from
  the bill's preview (a district map that erases the seat, a tier or chamber abolished, a term
  limit), weighted more under candidate-centred rules.
- O_l: pressure from organizations that fund or mobilize for the legislator (ADR 0189), weighted
  by their 10.4 power.
- d_p w_p: the party line (yes, no or free vote) times discipline.
- pi_l: side payments received for this bill.

**Discipline d_p** is higher where the cabinet depends on confidence, where leaders control
nomination and list rank, and where the leader holds patronage; it falls with the leader's
approval (ADR 0192), the member's marginality, distance from the leadership and the end of the
term.

**Agenda.** Whoever holds the agenda under ADR 0195 picks which bills reach the floor and
when, using the whip count as its own forecast, so majority parties are rarely rolled. Doring's
index initializes government agenda control; floor time caps bills per session, calibrated to
Comparative Agendas Project counts.

**Veto gates.** A bill runs through the bodies its levers touch: committee, chambers with
shuttle rounds, executive veto and override, constitutional review weighted by judicial
independence, referendum (ADR 0193), and any external owner of the lever. In one dimension the
feasible set is the intersection of every pivot's winset of q and the proposer picks the point
nearest its ideal inside it; in several dimensions this is a small quadratic program over the
salient axes.

**Side payments.** When the count falls short, the proposer buys votes with typed, non-fungible
currencies: portfolios (shares near seat shares, Warwick and Druckman 2006), budget amendments,
policy concessions toward the pivot, and promises recorded as edits (ADR 0164). A legislator's
price is the utility shortfall for yes; the proposer buys the cheapest set reaching the
threshold, greedily by price per vote. No separate political-capital currency exists.

**Outcome.** The yes count is Poisson-binomial over legislator probabilities, computed exactly
by dynamic programming; this distribution is ADR 0164's whip count, and bloc voting intention is
its sum by party, faction, region or organization. One keyed draw per (state, bill, stage)
realizes the vote (14.4). Bills advance one stage a month and lapse at the end of the session.

**Confidence and cabinets.** Confidence and investiture votes use the same machinery. A partner
leaves when expected vote gain from exit plus policy gain exceeds the value of its portfolios, so
falling approval and approaching elections raise the hazard.

**Decrees.** Where the constitution grants decree power, the executive may bypass the
legislature at a legitimacy and judicial-challenge cost (Carey and Shugart 1998).

**Autocracies.** The same gates run with discipline near one and the agenda in a shadow body;
legislatures co-opt rather than decide (Gandhi 2008), and dissent is small but present (Truex
2016). The decisive vote moves to the selectorate body, whose members are characters, with the
same Poisson-binomial giving a leader's odds of losing it.

**Parameters.** Free: the discipline function and the side-payment price scale. Roll-call noise
sigma is estimated from roll-call archives.

## Migration

- P6, in ceteris-polity, after ADRs 0192-0193 and the bodies of ADR 0195.
- ADR 0164's whip count reads this model.
- Data: Saiegh (2011) executive success rates; party-unity data (Carey 2007), Voteview, EP roll
  calls (Hix, Noury and Roland 2007), Brazilian roll calls (Figueiredo and Limongi 2000); ParlGov
  and ERDDA cabinet durations; Doring agenda-control index; Comparative Agendas Project; IPU
  Parline; Comparative Constitutions Project for gate rules (terms to audit, ADR 0050).

## Tests

- With discipline one, full government agenda control and a single gate, a bill passes exactly
  when the government holds a majority.
- The exact Poisson-binomial matches a Monte Carlo of legislator votes to sampling error.
- In one dimension, the enacted point equals the analytic setter outcome inside the pivots'
  winsets.
- The whip count never changes a state hash; with no promises, a bill's vote matches the same
  tick in a branch without the view (ADR 0164).
- Simulated executive success rates fall for minority and divided governments and order
  presidential and parliamentary systems as in Saiegh (2011); unity scores by system type and
  cabinet survival curves within tolerance of the data above.

## Consequences

Passing a law becomes a count of real people under measurable pressures, so a minority
government, a fragmented congress or a rebellious backbench produces the gridlock, deals and
defeats seen in the data. The whip count and coalition-partner seat get a model to project. Cost
about 6-8 part-time weeks.
