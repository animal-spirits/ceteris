## 10. Institutions, Politics and Government

Politics runs in three layers: interest groups hold resources and preferences, parties assemble them into electoral coalitions, and institutions decide how much elections, money and arms each count for. Government types emerge from that institutional vector rather than being picked from a menu.

### 10.1 Institutional state vector

Each state carries continuous indices in \[0, 1\], initialized directly from V-Dem for 2000:

| Dimension | Meaning | V-Dem / other anchor |
| --- | --- | --- |
| Electoral competition | Free and fair elections, office changes hands | Clean elections, electoral democracy index |
| Franchise and participation | Who can vote | Suffrage |
| Executive constraints | Legislative and judicial limits | Judicial and legislative constraints |
| Rule of law / property security | Expropriation risk, contract enforcement | Rule of law |
| Bureaucratic quality | Merit vs patronage administration | Rigorous and impartial public administration |
| Civil liberties | Press, association, protest rights | Freedom of expression, association |
| Centralization | Unitary vs federal authority, regional autonomy | Regional autonomy indicators |
| Corruption | Diversion of public resources | Political corruption index |
| Minority inclusion | Equal treatment across groups | Ethnic Power Relations status |

Laws and policies (tariffs, taxes, minimum wage, immigration rules, central bank independence) are separate variables governed by these institutions.

### 10.2 Government types as derived labels

A classifier maps the vector to Regimes of the World categories (Luhrmann, Tannenberg and Lindberg 2018): closed autocracy, electoral autocracy, electoral democracy, liberal democracy; plus head-of-state form and executive type (presidential, parliamentary, semi-presidential, monarchy, military, one-party). Labels never drive mechanics.

### 10.3 Political structure

States contain first-level provinces with autonomy levels (federal units, autonomous regions), and may belong to supranational bodies with delegated powers: the EU holds trade policy and, for euro members, monetary policy; other unions hold narrower competences. Dependencies and de facto states (for example Taiwan, Kosovo after 2008) are represented with their actual status as an attribute.

### 10.4 Interest groups and power

Interest groups (business by sector, finance, organized labour, public-sector workers, farmers, pensioners, security forces, religious organizations, ethnic blocs, urban professionals) draw members from pops by labour status, education, sector, identity and ideology. Political power shares come from a contest success function:

```latex
\pi_g = \frac{w_g \, x_g^{m}}{\sum_h w_h \, x_h^{m}}, \qquad x_g = \text{wealth}_g^{a} \cdot (\text{members}_g \cdot \text{organization}_g)^{b} \cdot \text{arms}_g^{c}
```

Institutional weights w translate resources into influence: votes count through elections, money through lobbying and patronage (heavier with high corruption and weak constraints), arms through the security forces' capacity to intervene.

### 10.5 Parties and elections

- **Parties** are platforms in a policy space (economic left-right, cultural liberal-conservative, nationalism and sovereignty, ethnic or religious appeal) backed by a coalition of interest groups. Initial parties, positions and vote shares come from open party datasets (ParlGov, V-Party), with stable generic names rather than real ones in v1.
- **Voting**: pops vote by spatial proximity to platforms, party attachment, and retrospective economic voting (incumbents punished for unemployment and inflation), with turnout tied to education and mobilization.
- **Electoral systems**: plurality, two-round, proportional (D'Hondt, Sainte-Lague) and mixed, initialized per country, converting votes to seats.
- **Government formation**: majority winners govern; otherwise minimal connected winning coalitions form by bargaining over portfolios and policy.
- **Party system dynamics**: parties shift platforms toward vote-rich positions with inertia; new parties enter when a large ideological mass is unrepresented (the channel for populist entry); parties split and merge.
- **Calendars**: election dates follow constitutional terms from 2000 data; snap elections occur when governments lose confidence.

### 10.6 Policy choice

| Setting | Mechanism | Source |
| --- | --- | --- |
| Democracies | Governing party or coalition maximizes its vote and coalition-partner support, subject to veto players | Probabilistic voting (Persson-Tabellini 2000); veto players |
| Autocracies | Leader maximizes survival given winning coalition W and selectorate S; small W favours private goods | Bueno de Mesquita et al. (2003) |
| Legislatures | Proposer-veto bargaining; bills progress monthly and may fail | Design choice |

### 10.7 Legitimacy, ideology and grievance

Pop approval combines material conditions relative to expectations, policy distance from preferred platforms, group exclusion, corruption, war weariness and repression. Ideology evolves with education, age, economic conditions and media freedom, and diffuses through bounded-confidence opinion dynamics (Hegselmann-Krause), which can generate polarization.

### 10.8 Unrest and regime change

- **Protest**: mobilization follows a threshold model (Granovetter 1978) with preference falsification (Kuran 1991), so protest waves can cascade across neighbouring states, the pattern of the Arab Spring.
- **Structural pressure**: a Political Stress Index in the Goldstone-Turchin tradition (mass mobilization potential, elite competition, state fiscal distress) raises instability hazards.
- **Outcomes**: concession and reform, repression, democratic breakdown or backsliding (executive aggrandizement eroding constraints gradually), coups (calibrated to coup datasets), revolution, civil war (Section 11), secession (by referendum or conflict).
- **Calibration targets**: V-Dem index paths 2000-2024, frequencies of coups, mass protest campaigns and regime transitions.

