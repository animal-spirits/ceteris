//! Province markets and the movement of goods between them: goods, trade parameters, the
//! graph-independent network, the two-level trade kernel, destination sourcing and the
//! per-buyer pipeline of goods on order. See `specs/trade/SPEC.md` and ADRs 0015 and 0016.

pub mod goods;
pub mod kernel;
pub mod market;
pub mod network;
pub mod params;
pub mod pipeline;
pub mod source;

pub use goods::N_GOODS;
pub use kernel::{Access, Kernel};
pub use market::{Markets, Rationing};
pub use network::{Leg, Network, Wedges};
pub use params::TradeParams;
pub use pipeline::Pipeline;
pub use source::{Sourced, dispatch, source};
