# RFC 0349. Government ledger account for the fiscal system

Status: proposed
Date: 2026-11-12
Design sections: 9.6, 4.6
Release: 1.0 (P5a)
Research: none; follows the code of `crates/ceteris-econ/src/fiscal.rs` and PR #547

## Summary

The fiscal kernel (P5 C1, ADR 0042) is wired in PR #547 as system 232 with a government account
held beside the economy: revenue, spending, interest and debt are booked in the SFC matrices and
nothing moves on the money ledger, so taxes take no money from anyone and spending adds no
demand. Antonio chose on 2026-11-12 to give the government a real ledger account instead, so
that fiscal shocks reach the real economy and energy wedges (#532) and state energy cells (#533)
move money to and from it. New ledger accounts and Flows change the journal and its hash domain,
so they are decided here. Every item keeps the switch-off world bit-identical to the status quo
(ADR 0354): with the fiscal system off no account is opened and no new Flow is posted.

Today each country has a rebate account (tariff revenue paid back to pops by population) and a
profit pool (E4, ADR 0308), the world has the mint and the transfer account of ADR 0100, and
government final demand is a fixed share of each pop's spending (ADR 0099).

## Decisions

### D1. How is the government account opened?

Options:

- A. One wide (`i128`) account per country, opened after the foreign account and only when the
  fiscal system is on, as the transfer account of ADR 0100 is: a world without fiscal keeps its
  ledger and every golden.
- B. Always open one per country: every world's ledger layout and every golden move once, with a
  re-bless.

Recommendation: A, because it follows the ADR 0100 precedent and keeps the off switch free.

Decision: B, Antonio, 2026-11-12. Every world opens the accounts; the goldens move once, in a
reviewed re-bless commit of the wiring PR. With the fiscal system off the accounts stay at zero
and nothing posts to them, so off equals the re-blessed status quo.
Row: pending

### D2. Which new Flows does the journal get?

Options:

- A. Three, one per SFC flow row the government touches: TAX, GOV_TRANSFER, GOV_INTEREST
  (Flow 12, 13, 14). State-cell profits (#533) use the existing DIVIDEND flow; excises (#532)
  use TAX.
- B. Two: TAX and GOV_SPENDING, interest posted as spending; the SFC interest row is then
  rebuilt by the fiscal system, not from the journal.
- C. One FISCAL flow for everything, split by the fiscal system's own record.

Recommendation: A, because the transactions-flow matrix is built from journal postings
(`specs/money/SPEC.md`) and one Flow per row keeps that build mechanical.

Decision: A, Antonio, 2026-11-12.
Row: pending

### D3. Where does government spending go, given that ADR 0099 already puts government demand in pops' spending?

Options:

- A. Spending is paid to pops as transfers by population, and pops keep the ADR 0099 government
  share of their spending: government demand still comes out of pop spending, so nothing is
  counted twice. Taxes and transfers redistribute within a country and a deficit adds money.
- B. The government buys goods itself with the ADR 0099 government shares, and the government
  share leaves pop spending when the fiscal system is on. Closest to the national accounts, but
  the government becomes a buyer in E3 (pipeline, markets), a large change.
- C. A now, B as a later RFC once the government is a buyer for P6 budgets.

Recommendation: C, because A is small and correct in money, and the government as a buyer
belongs with budgets set by the state AI (Section 10, P6).

Decision: pending.
Row: pending

### D4. Where are taxes collected?

Options:

- A. At the source: income and payroll taxes as a share of each wage payment, VAT on each sale,
  corporate tax in the profit pool. Right incidence, but it touches the wage and sale paths in
  the hot loop.
- B. Once per tick in system 232, after E4: personal income and payroll from pops in proportion
  to their wage income, VAT from pops in proportion to their spending, corporate tax from the
  national profit pool before it pays out, each split by largest remainder in exact `i64`.
  Revenue is the kernel's `R_i` on the tick's bases.
- C. B now, A when the hot loop is reworked after the Make-it-fast stage.

Recommendation: B, because it keeps the tick's money paths untouched and the incidence close
enough for 1.0; A can come by its own RFC if validation asks for it.

Decision: pending.
Row: pending

### D5. How is the deficit financed on the ledger?

Options:

- A. The government account may go negative: its balance is minus the debt, matching the bonds
  position of the SFC balance sheet. Money is conserved on the ledger; no mint flow.
- B. Monetary financing through the mint (MINT flow): the deficit adds money the ADR 0338 anchor
  counts.
- C. A by default, B as a per-country switch for monetary financing (spec 9.6 allows both).

Recommendation: A, because banks and bonds are not ledger accounts yet and A keeps the money
anchor of ADR 0338 untouched; monetary financing waits for the central bank balance sheet.

Decision: pending.
Row: pending

## Spec changes

Filled in once the items are decided: `specs/money/SPEC.md` (state components, fiscal state,
system 232), spec 09 9.6, spec 04 4.6 (ledger accounts and Flows).
