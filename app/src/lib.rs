//! Native desktop app (ADR 0088): Slint panels over a wgpu map drawn into a texture.

pub mod bench;
pub mod branches;
pub mod camera;
pub mod chunks;
pub mod clock;
pub mod commands;
pub mod diffmap;
pub mod econ;
pub mod fan;
pub mod fill;
pub mod freight;
pub mod inspect;
pub mod labels;
pub mod map;
pub mod modes;
pub mod ramps;
pub mod session;
pub mod sim;
pub mod tokens;
pub mod web;
pub mod why;

/// A Tracy zone over the rest of the enclosing scope with the `profile` feature; nothing
/// without it.
#[macro_export]
macro_rules! zone {
    ($name:literal) => {
        #[cfg(feature = "profile")]
        let _zone = tracy_client::span!($name);
    };
}
