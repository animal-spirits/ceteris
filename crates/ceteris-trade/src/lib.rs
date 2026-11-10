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

pub use goods::Goods;
pub use kernel::{Access, Kernel};
pub use market::{Markets, Rationing};
pub use network::{
    BulkLegs, GoodLegs, Leg, LegSet, LegTaus, Network, Wedges, equal_gateway_mass, kappa,
};
pub use params::{N_CLASSES, PriceAnchor, TradeParams};
pub use pipeline::Pipeline;
pub use source::{LaneInput, Sourced, arrivals, dispatch, source, source_lanes, source_weighted};
