//! Trade flows by country pair over the last 12 ticks, recorded at T2 for the run export.
//! Not state: a restored world starts with an empty window, like the rest of `Scratch`.
//!
//! Units are origin units at dispatch, before any iceberg or border loss; value is what the
//! origin receives: the delivered value (destination price times arriving units) net of
//! tariff. A shipment between two provinces of one country is a domestic pair (exporter =
//! importer); international value and units are the crossing columns of `Routed`,
//! attributed to the countries of the two gateways of the crossing. Supply routed home is
//! valued at the origin's own price.

use ceteris_trade::{Kernel, N_GOODS, Routed};

/// Ticks averaged by the export.
pub const WINDOW: usize = 12;

#[derive(Clone, Debug)]
struct Slot {
    /// `[(exporter * n_countries + importer) * N_GOODS + g]`.
    pair_value: Vec<f64>,
    pair_units: Vec<f64>,
    /// Supply routed to stay in its origin province, `[province * N_GOODS + g]`.
    home_value: Vec<f64>,
    home_units: Vec<f64>,
}

impl Slot {
    fn new(n_countries: usize, n_provinces: usize) -> Self {
        let pairs = n_countries * n_countries * N_GOODS;
        let cells = n_provinces * N_GOODS;
        Self {
            pair_value: vec![0.0; pairs],
            pair_units: vec![0.0; pairs],
            home_value: vec![0.0; cells],
            home_units: vec![0.0; cells],
        }
    }

    fn clear(&mut self) {
        for v in [
            &mut self.pair_value,
            &mut self.pair_units,
            &mut self.home_value,
            &mut self.home_units,
        ] {
            v.fill(0.0);
        }
    }
}

/// Means over the recorded ticks, same layouts as the slots.
#[derive(Clone, Debug, PartialEq)]
pub struct MeanFlows {
    pub pair_value: Vec<f64>,
    pub pair_units: Vec<f64>,
    pub home_value: Vec<f64>,
    pub home_units: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct FlowWindow {
    n_countries: usize,
    slots: Vec<Slot>,
    /// Slot of the current tick.
    head: usize,
    ticks: usize,
}

impl FlowWindow {
    pub fn new(n_countries: usize, n_provinces: usize) -> Self {
        Self {
            n_countries,
            slots: vec![Slot::new(n_countries, n_provinces); WINDOW],
            head: WINDOW - 1,
            ticks: 0,
        }
    }

    /// Ticks recorded, at most [`WINDOW`].
    pub fn ticks(&self) -> usize {
        self.ticks
    }

    /// Starts a tick: the oldest slot is cleared and becomes the current one.
    pub fn advance(&mut self) {
        self.head = (self.head + 1) % WINDOW;
        self.slots[self.head].clear();
        self.ticks = (self.ticks + 1).min(WINDOW);
    }

    /// Adds the flows of traded good `g` routed this tick. `price` is per province, as
    /// given to `route`; `province_country` and `gateway_country` come from the network.
    pub fn record(
        &mut self,
        kernel: &Kernel,
        province_country: &[u16],
        gateway_country: &[u16],
        g: usize,
        price: &[f64],
        r: &Routed,
    ) {
        let nc = self.n_countries;
        let gk = kernel.good(g);
        let slot = &mut self.slots[self.head];
        let pair = |ex: u16, im: u16| (usize::from(ex) * nc + usize::from(im)) * N_GOODS + g;

        for o in 0..kernel.n_provinces {
            let cell = o * N_GOODS + g;
            slot.home_value[cell] += price[o] * r.home[o];
            slot.home_units[cell] += r.home[o];
            let at = pair(province_country[o], province_country[o]);
            let legs = gk.domestic.offsets[o] as usize..gk.domestic.offsets[o + 1] as usize;
            for j in legs {
                slot.pair_value[at] += r.domestic_value[j];
                slot.pair_units[at] += r.domestic_units[j];
            }
        }
        for a in 0..kernel.n_gateways {
            let lo = gk.crossings.offsets[a] as usize;
            for (j, e) in gk.crossings.row(a).iter().enumerate() {
                let at = pair(gateway_country[a], gateway_country[e.col as usize]);
                slot.pair_value[at] += r.crossing_value[lo + j];
                slot.pair_units[at] += r.crossing_units[lo + j];
            }
        }
    }

    /// Means over the recorded ticks, summed from the oldest to the newest.
    pub fn mean(&self) -> MeanFlows {
        let first = &self.slots[0];
        let mut out = MeanFlows {
            pair_value: vec![0.0; first.pair_value.len()],
            pair_units: vec![0.0; first.pair_units.len()],
            home_value: vec![0.0; first.home_value.len()],
            home_units: vec![0.0; first.home_units.len()],
        };
        if self.ticks == 0 {
            return out;
        }
        for back in (0..self.ticks).rev() {
            let s = &self.slots[(self.head + WINDOW - back) % WINDOW];
            for (sum, add) in [
                (&mut out.pair_value, &s.pair_value),
                (&mut out.pair_units, &s.pair_units),
                (&mut out.home_value, &s.home_value),
                (&mut out.home_units, &s.home_units),
            ] {
                sum.iter_mut().zip(add).for_each(|(x, y)| *x += y);
            }
        }
        let n = self.ticks as f64;
        for v in [
            &mut out.pair_value,
            &mut out.pair_units,
            &mut out.home_value,
            &mut out.home_units,
        ] {
            v.iter_mut().for_each(|x| *x /= n);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::small_world;
    use ceteris_trade::{TradeParams, route};

    fn relative(a: f64, b: f64) -> f64 {
        (a - b).abs() / a.abs().max(b.abs()).max(1e-300)
    }

    #[test]
    fn pairs_account_for_everything_routed() {
        let w = small_world();
        let kernel = Kernel::build(&w.network, &w.weight, &w.wedges, &TradeParams::default());
        let np = w.n_provinces();
        let nc = w.n_countries();
        let g = 4;
        let supply: Vec<f64> = (0..np).map(|i| 10.0 + (i % 5) as f64).collect();
        let price: Vec<f64> = (0..np).map(|i| 1.0 + 0.05 * (i % 7) as f64).collect();
        let active = vec![true; np];
        let mut routed = Routed::new();
        route(&kernel, g, &supply, &price, &active, &mut routed);

        let mut window = FlowWindow::new(nc, np);
        window.advance();
        window.record(
            &kernel,
            &w.network.province_country,
            &w.network.gateway_country,
            g,
            &price,
            &routed,
        );
        let m = window.mean();

        let at = |ex: usize, im: usize| (ex * nc + im) * N_GOODS + g;
        let pairs = (0..nc).flat_map(|ex| (0..nc).map(move |im| (ex, im)));
        let value: f64 = pairs.clone().map(|(ex, im)| m.pair_value[at(ex, im)]).sum();
        let units: f64 = pairs.clone().map(|(ex, im)| m.pair_units[at(ex, im)]).sum();
        let received: f64 = routed.received.iter().sum();
        let shipped: f64 = routed.shipped.iter().sum();
        assert!(relative(value, received) < 1e-12, "{value} {received}");
        assert!(relative(units, shipped) < 1e-12, "{units} {shipped}");

        let home: f64 = (0..np).map(|i| m.home_units[i * N_GOODS + g]).sum();
        let total: f64 = supply.iter().sum();
        assert!(relative(home + shipped, total) < 1e-12);

        // only direct neighbours trade: the island ships to the two mainland ports' countries
        assert!(m.pair_value[at(3, 0)] > 0.0 && m.pair_value[at(3, 2)] > 0.0);
        // exporter-importer pairs match the domestic split
        let domestic: f64 = (0..nc).map(|c| m.pair_value[at(c, c)]).sum();
        let domestic_legs: f64 = routed.domestic_value.iter().sum();
        assert!(relative(domestic, domestic_legs) < 1e-12);
    }

    #[test]
    fn window_averages_the_last_twelve_ticks() {
        let (nc, np) = (1, 1);
        let mut window = FlowWindow::new(nc, np);
        assert_eq!(window.mean().home_units, vec![0.0; N_GOODS]);
        for t in 0..30 {
            window.advance();
            window.slots[window.head].home_units[0] = t as f64;
        }
        assert_eq!(window.ticks(), WINDOW);
        // ticks 18..30
        assert_eq!(
            window.mean().home_units[0],
            (18..30).sum::<usize>() as f64 / 12.0
        );
        let mut short = FlowWindow::new(nc, np);
        for t in 0..3 {
            short.advance();
            short.slots[short.head].home_units[0] = t as f64;
        }
        assert_eq!(short.mean().home_units[0], 1.0);
    }
}
