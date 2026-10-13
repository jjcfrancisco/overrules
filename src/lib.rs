//! gers-lr — geodesic linear referencing and scoped-rule resolution for
//! Overture Maps transportation segments.

pub mod index;
pub mod lr;
pub mod model;
pub mod reader;
pub mod rules;

pub use index::Network;
pub use model::Segment;
pub use rules::Situation;
