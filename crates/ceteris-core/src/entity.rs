//! Stable entity IDs and structure-of-arrays helpers (Sections 4.2, 19.4).

use crate::snapshot::{Decode, DecodeError, Decoder, Encode, Encoder};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(pub u32);

/// Entity kind tag; separates random streams of kinds that share numeric IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kind(pub u16);

/// Allocates IDs in creation order and never reuses them, so random streams keyed by
/// ID stay aligned across branches whose histories agree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IdAlloc {
    next: u32,
}

impl IdAlloc {
    pub fn alloc(&mut self) -> EntityId {
        let id = EntityId(self.next);
        self.next = self.next.checked_add(1).expect("entity ID space exhausted");
        id
    }
}

impl Encode for IdAlloc {
    fn encode(&self, e: &mut Encoder) {
        e.u32(self.next);
    }
}

impl Decode for IdAlloc {
    fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        Ok(Self { next: d.u32()? })
    }
}

/// Drops rows whose `keep` flag is false, preserving order. Apply the same mask to
/// every column of a table.
pub fn compact<T>(column: &mut Vec<T>, keep: &[bool]) {
    assert_eq!(column.len(), keep.len());
    let mut flags = keep.iter();
    column.retain(|_| *flags.next().unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn compact_keeps_flagged_rows_in_order(keep in proptest::collection::vec(any::<bool>(), 0..200)) {
            let mut col: Vec<usize> = (0..keep.len()).collect();
            compact(&mut col, &keep);
            let expected: Vec<usize> = (0..keep.len()).filter(|&i| keep[i]).collect();
            prop_assert_eq!(col, expected);
        }
    }

    #[test]
    fn ids_are_never_reused() {
        let mut ids = IdAlloc::default();
        let a = ids.alloc();
        let b = ids.alloc();
        assert!(b > a);
    }
}
