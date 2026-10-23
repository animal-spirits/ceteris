//! Append-only log of discrete decisions (Section 4.1). Part of the snapshot, so
//! replay reproduces it exactly.

use crate::entity::EntityId;
use crate::schedule::SystemId;
use crate::snapshot::{Decode, DecodeError, Decoder, Encode, Encoder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventKind(pub u16);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub tick: u32,
    pub system: SystemId,
    pub kind: EventKind,
    pub subject: EntityId,
    pub value: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventLog {
    events: Vec<Event>,
}

impl EventLog {
    pub fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }
}

impl Encode for EventLog {
    fn encode(&self, e: &mut Encoder) {
        e.count(self.events.len());
        for ev in &self.events {
            e.u32(ev.tick);
            e.u16(ev.system.0);
            e.u16(ev.kind.0);
            e.u32(ev.subject.0);
            e.f64(ev.value);
        }
    }
}

impl Decode for EventLog {
    fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        let events = (0..d.count()?)
            .map(|_| {
                Ok(Event {
                    tick: d.u32()?,
                    system: SystemId(d.u16()?),
                    kind: EventKind(d.u16()?),
                    subject: EntityId(d.u32()?),
                    value: d.f64()?,
                })
            })
            .collect::<Result<_, DecodeError>>()?;
        Ok(Self { events })
    }
}
