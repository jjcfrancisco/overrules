//! gers-lr — geodesic linear referencing and scoped-rule resolution for
//! Overture Maps transportation segments.

pub mod api;
pub mod index;
pub mod lr;
pub mod model;
pub mod pack;
pub mod query;
#[cfg(feature = "native")]
pub mod reader;
pub mod rules;
#[cfg(feature = "native")]
pub mod server;
#[cfg(feature = "wasm")]
pub mod wasm;

pub use index::Network;
pub use model::Segment;
pub use rules::Situation;
