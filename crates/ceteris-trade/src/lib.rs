//! Province markets and the movement of goods between them: goods, trade parameters,
//! the graph-independent network and the two-level trade kernel. See `specs/trade/SPEC.md`.

pub mod goods;
pub mod kernel;
pub mod network;
pub mod params;

pub use goods::N_GOODS;
pub use kernel::{Access, Kernel};
pub use network::{Leg, Network, Wedges};
pub use params::TradeParams;
