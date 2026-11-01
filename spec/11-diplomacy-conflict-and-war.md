## 11. Diplomacy, Conflict and War

Interstate war is modeled as a failure of bargaining under private information and commitment problems, made rare by nuclear deterrence and economic interdependence; civil conflict, the dominant form of organized violence since 2000, grows from group grievance, weak state capacity and opportunity.

### 11.1 Diplomatic state

Per dyad: relations, trust (track record of honoured commitments), rivalry flag, territorial claims, and treaties: defence alliances (NATO-style collective defence), non-aggression and security partnerships, free trade agreements, customs and monetary unions, sanctions regimes, arms-supply relationships. Membership of international organizations (UN Security Council permanent seats, WTO, IMF) is an attribute with procedural effects (vetoes, dispute settlement, lending access).

### 11.2 Crisis bargaining

Disputes arise from territorial claims, ethnic kin across borders, resource disputes, security dilemmas, regime-change motives or alliance obligations. Each side holds a noisy estimate of its probability of winning p and of costs; the bargaining range follows Fearon (1995):

```latex
x \in \left[ \hat{p}_A - \hat{c}_A, \; \hat{p}_A + \hat{c}_B \right]
```

War occurs when private estimates leave no acceptable offer, when a projected power shift gives the declining side a preventive motive (Powell 2006), or when the stake is treated as indivisible. Costs include casualties, economic losses through trade and finance, sanctions, and for nuclear dyads an expected catastrophic-escalation term that makes direct war between nuclear powers extremely rare but lets proxy conflicts and limited wars occur. Power-estimate noise is calibrated to war-onset frequencies.

### 11.3 Alliances and interdependence

Alliance formation trades autonomy for security (Morrow 1991); reliability is learned from behaviour. Economic interdependence enters the cost of war: bilateral trade raises the opportunity cost of fighting a partner, while broad multilateral openness reduces dependence on any one partner (Martin, Mayer and Thoenig 2008). Both channels are in the model and tested (Section 18).

### 11.4 Military capability

Capability is computed in-model from military personnel, military spending, defence-industrial output (arms, aerospace, shipbuilding sectors), energy and population, and compared to Correlates of War capability data and military-spending series. Forces: land formations, air power, navies, missiles and air defence as aggregate capability stocks with readiness, equipment bought from domestic or foreign arms producers, and recruitment by law (conscription or volunteer). Nuclear status is a state attribute.

### 11.5 Combat, logistics and occupation

- **Battles**: Lanchester attrition with modifiers for terrain, fortification, air superiority, precision munitions and morale; loss-exchange ratios calibrated to modern battle data where available.
- **Logistics**: forces draw fuel, ammunition and food over the transport graph; interdicted supply lines raise attrition.
- **Occupation**: transfers control and taxation, disrupts production and trade, raises local grievance and insurgency risk.
- **Naval and air**: sea control and air superiority set blockade and strike risk on transport edges and infrastructure.

### 11.6 Civil conflict and insurgency

Civil war risk per province rises with group exclusion and horizontal inequality (Cederman, Gleditsch and Buhaug 2013), weak state capacity and rough terrain (Fearon and Laitin 2003), lootable resources, neighbouring conflict, and youth unemployment. Rebel factions are proto-states controlling provinces, financed by resources, diaspora and foreign sponsors. Conflict intensity is validated against georeferenced conflict-event data (UCDP GED) at province level, which is unusually fine-grained for a simulator.

### 11.7 The war economy and displacement

Mobilization removes workers; procurement shifts demand to arms, fuel and transport; war finance runs through debt and money creation; casualties reduce cohorts; capital and infrastructure are destroyed in contested provinces; refugees flow to neighbours (Section 6.5). War weariness feeds approval.

### 11.8 Termination, peace and coercion

Bargaining continues during war (Wagner; Slantchev 2003): battle outcomes update beliefs, and settlements occur when the range reopens; frozen conflicts are a possible outcome. Terms: territory, demilitarization, autonomy, reparations, regime change. Economic coercion (embargoes, financial sanctions, reserve freezes, export controls, exclusion from payment systems) uses the trade and finance modules directly; the 2022 sanctions on Russia are a validation case for trade rerouting and price effects.

