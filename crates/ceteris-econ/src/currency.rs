//! Currencies of the external-balance closure (ADR 0228, RFC 0366, ADRs 0403 to 0407): one
//! currency per country (union members share one), a mint and a financier account per currency,
//! and the exchange rate e_c and its anchoring expectation m_c. Off by default
//! (`EconWorld::with_currencies`); this layer holds every e_c at 1 and moves no real quantity.

use ceteris_core::ledger::{AccountId, Flow, Ledger};
use ceteris_core::snapshot::{DecodeError, Decoder, Encoder};

use crate::world::{Accounts, MINT};

/// Opens the currencies section of a snapshot, which follows the capital one.
pub const SNAPSHOT_TAG: u32 = 0x3A6E_6F05;

/// The country of each pop, enterprise and province.
pub struct CountryOf {
    pub pop: Vec<usize>,
    pub enterprise: Vec<usize>,
    pub province: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Currencies {
    /// Index of the numeraire currency: that of the country with the largest base nominal GDP.
    pub numeraire: u32,
    /// Currency of each country.
    pub of_country: Vec<u32>,
    /// Currency of each ledger account open when the layer was switched on; world accounts
    /// (transfer, foreign, transit) are numeraire.
    pub of_account: Vec<u32>,
    /// Per currency: the account holding minus all of it ever issued. The numeraire's is
    /// `Accounts::mint`.
    pub mint: Vec<AccountId>,
    /// Per currency: the financier taking the net currency sold across borders (F_c).
    pub financier: Vec<AccountId>,
    /// Per currency: exchange rate, numeraire per unit; 1 at the base state.
    pub e: Vec<f64>,
    /// Per currency: the anchoring expectation m_c of the rate.
    pub m: Vec<f64>,
}

impl Currencies {
    /// Opens the currency accounts on `ledger` after every existing account and moves each
    /// non-numeraire currency's balances off the numeraire mint onto its own, so every
    /// currency's accounts sum to zero. `gdp` is each country's base nominal GDP,
    /// `gateway_country` the country of each gateway.
    pub fn open(
        ledger: &mut Ledger,
        accounts: &Accounts,
        of_country: Vec<u32>,
        gdp: &[f64],
        gateway_country: &[u16],
        countries: &CountryOf,
    ) -> Self {
        assert_eq!(of_country.len(), gdp.len());
        let n = of_country.iter().max().map_or(0, |&c| c as usize + 1);
        assert!(
            (0..n as u32).all(|c| of_country.contains(&c)),
            "currency indices must be dense"
        );
        let largest = (0..gdp.len()).fold(0, |a, c| if gdp[c] > gdp[a] { c } else { a });
        let numeraire = of_country[largest];

        let mut of_account = vec![numeraire; ledger.n_accounts()];
        let mut set = |ids: &[AccountId], country: &dyn Fn(usize) -> usize| {
            for (i, a) in ids.iter().enumerate() {
                of_account[a.0 as usize] = of_country[country(i)];
            }
        };
        let by_country = |i: usize| i;
        set(&accounts.rebate, &by_country);
        set(&accounts.dividend, &by_country);
        set(&accounts.government, &by_country);
        set(&accounts.gateway, &|g| usize::from(gateway_country[g]));
        set(&accounts.pop, &|k| countries.pop[k]);
        set(&accounts.enterprise, &|e| countries.enterprise[e]);
        set(&accounts.market, &|i| countries.province[i]);

        let mint: Vec<AccountId> = (0..n as u32)
            .map(|c| {
                if c == numeraire {
                    accounts.mint
                } else {
                    ledger.open_wide()
                }
            })
            .collect();
        let financier: Vec<AccountId> = (0..n).map(|_| ledger.open_wide()).collect();
        of_account.resize(ledger.n_accounts(), numeraire);
        for c in 0..n {
            of_account[financier[c].0 as usize] = c as u32;
            of_account[mint[c].0 as usize] = c as u32;
        }
        let this = Self {
            numeraire,
            of_country,
            of_account,
            mint,
            financier,
            e: vec![1.0; n],
            m: vec![1.0; n],
        };
        for a in 0..ledger.n_accounts() {
            let c = this.of_account[a] as usize;
            if c as u32 == numeraire || this.mint[c].0 as usize == a {
                continue;
            }
            let mut left = ledger.money_wide(AccountId(a as u32));
            while left != 0 {
                let step = left.clamp(-i128::from(i64::MAX), i128::from(i64::MAX));
                ledger.transfer(MINT, this.mint[c], accounts.mint, step as i64);
                left -= step;
            }
        }
        this
    }

    /// Sum of the balances of each currency's accounts; zero for every currency while payments
    /// stay in their own currency (RFC 0366, Invariants).
    pub fn balances(&self, ledger: &Ledger) -> Vec<i128> {
        let mut sums = vec![0i128; self.e.len()];
        for (a, &c) in self.of_account.iter().enumerate() {
            sums[c as usize] += ledger.money_wide(AccountId(a as u32));
        }
        sums
    }

    /// `amount` of currency `from` in currency `to`, rounded half to even; exact when the
    /// rates are equal.
    pub fn convert(&self, from: u32, to: u32, amount: i64) -> i64 {
        let (ef, et) = (self.e[from as usize], self.e[to as usize]);
        if ef == et {
            return amount;
        }
        (amount as f64 * ef / et).round_ties_even() as i64
    }

    /// Pays `amount` of `from`'s currency to `to`. Across currencies it goes through the
    /// financiers (RFC 0366, Conversion at the border): `from` pays its currency's financier,
    /// the financier of `to`'s currency pays `to` the converted amount, rounding staying with
    /// the financiers. Returns the amount `to` received, in its currency.
    pub fn cross(
        &self,
        ledger: &mut Ledger,
        flow: Flow,
        from: AccountId,
        to: AccountId,
        amount: i64,
    ) -> i64 {
        let (cf, ct) = (
            self.of_account[from.0 as usize],
            self.of_account[to.0 as usize],
        );
        if cf == ct {
            if amount != 0 {
                ledger.transfer(flow, from, to, amount);
            }
            return amount;
        }
        let got = self.convert(cf, ct, amount);
        if amount != 0 {
            ledger.transfer(flow, from, self.financier[cf as usize], amount);
        }
        if got != 0 {
            ledger.transfer(flow, self.financier[ct as usize], to, got);
        }
        got
    }

    pub fn encode(&self, e: &mut Encoder) {
        e.u32(SNAPSHOT_TAG);
        e.u32(self.numeraire);
        e.u32s(&self.of_country);
        e.u32s(&self.of_account);
        e.u32s(&self.mint.iter().map(|a| a.0).collect::<Vec<_>>());
        e.u32s(&self.financier.iter().map(|a| a.0).collect::<Vec<_>>());
        e.f64s(&self.e);
        e.f64s(&self.m);
    }

    pub fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        if d.u32()? != SNAPSHOT_TAG {
            return Err(DecodeError("currencies section tag".into()));
        }
        let ids = |v: Vec<u32>| v.into_iter().map(AccountId).collect::<Vec<_>>();
        let this = Self {
            numeraire: d.u32()?,
            of_country: d.u32s()?,
            of_account: d.u32s()?,
            mint: ids(d.u32s()?),
            financier: ids(d.u32s()?),
            e: d.f64s()?,
            m: d.f64s()?,
        };
        let n = this.e.len();
        if this.m.len() != n || this.mint.len() != n || this.financier.len() != n {
            return Err(DecodeError("currencies lengths".into()));
        }
        Ok(this)
    }
}
