use std::fmt;

/// A broken accounting or state invariant. Any violation halts the run (Section 4.6).
#[derive(Debug, Clone, PartialEq)]
pub struct Violation {
    pub tick: u32,
    pub invariant: &'static str,
    pub detail: String,
}

impl Violation {
    pub fn new(tick: u32, invariant: &'static str, detail: String) -> Self {
        Self {
            tick,
            invariant,
            detail,
        }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "tick {}: {} invariant violated: {}",
            self.tick, self.invariant, self.detail
        )
    }
}

impl std::error::Error for Violation {}
