//! Province markets and the movement of goods between them: goods, trade parameters,
//! the graph-independent network, the two-level trade kernel, merchant routing and the
//! transit wheel. See `specs/trade/SPEC.md`.

pub mod goods;
pub mod kernel;
pub mod market;
pub mod network;
pub mod params;
pub mod route;
pub mod transit;

pub use goods::N_GOODS;
pub use kernel::{Access, Kernel};
pub use market::Markets;
pub use network::{Leg, Network, Wedges};
pub use params::TradeParams;
pub use route::{Routed, route};
pub use transit::TransitWheel;
