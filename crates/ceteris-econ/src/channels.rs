//! Channel shutdown (spec 14.7, ADR 0049): a run that records the paths of the wage and price
//! channels, and a run that reads them back instead of its own dynamics. A shut channel holds
//! the baseline's value at every tick, so the effect of an intervention that has to pass through
//! it is removed. Off by default; with no channel shut and no recording a run is unchanged.
//! Trade shares (the sourcing kernel) are not yet a channel.

use std::sync::Arc;

use crate::world::EconWorld;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Prices,
    Wages,
}

/// What a baseline run posted at the end of each tick it recorded: wages after E1 and origin
/// and province market prices after E4's repricing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    pub first_tick: u32,
    pub wage: Vec<Vec<f64>>,
    pub price: Vec<Vec<f64>>,
}

impl Path {
    fn at(rows: &[Vec<f64>], first: u32, tick: u32) -> Option<&[f64]> {
        let i = tick.checked_sub(first)? as usize;
        rows.get(i).map(Vec::as_slice)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Channels {
    record: Option<Path>,
    shut: Option<(Arc<Path>, Vec<Channel>)>,
}

impl Channels {
    pub fn record_from(&mut self, tick: u32) {
        self.record = Some(Path {
            first_tick: tick,
            ..Path::default()
        });
    }

    pub fn take_record(&mut self) -> Option<Path> {
        self.record.take()
    }

    pub fn shut(&mut self, baseline: Arc<Path>, channels: &[Channel]) {
        self.shut = Some((baseline, channels.to_vec()));
    }

    fn is_shut(&self, c: Channel) -> bool {
        self.shut.as_ref().is_some_and(|(_, s)| s.contains(&c))
    }
}

/// After E1: holds the wages of a shut wage channel at the baseline's, then records them.
pub fn after_labour(w: &mut EconWorld, tick: u32) {
    let ch = &mut w.scratch.channels;
    if ch.is_shut(Channel::Wages)
        && let Some((path, _)) = &ch.shut
        && let Some(row) = Path::at(&path.wage, path.first_tick, tick)
    {
        w.wage.copy_from_slice(row);
    }
    if let Some(rec) = &mut w.scratch.channels.record {
        rec.wage.push(w.wage.clone());
    }
}

/// After the repricing: the same for the posted prices.
pub fn after_reprice(w: &mut EconWorld, tick: u32) {
    let ch = &w.scratch.channels;
    if ch.is_shut(Channel::Prices)
        && let Some((path, _)) = &ch.shut
        && let Some(row) = Path::at(&path.price, path.first_tick, tick)
    {
        w.markets.price.copy_from_slice(row);
    }
    if let Some(rec) = &mut w.scratch.channels.record {
        rec.price.push(w.markets.price.clone());
    }
}
