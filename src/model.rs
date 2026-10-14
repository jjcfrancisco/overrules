//! Typed Overture transportation data: segments and their rule-based properties.
//!
//! Field names and enum spellings follow the Overture schema
//! (`schema/transportation/segment.yaml`), so these types (de)serialise straight from
//! Overture GeoJSON properties or Parquet rows rendered as JSON.

mod between;
mod rules;
mod scope;
mod segment;

pub use between::Between;
pub use rules::{
    AccessRestriction, AccessType, ProhibitedTransition, Speed, SpeedLimit, SpeedUnit,
    TransitionStep,
};
pub use scope::{
    Comparison, Dimension, Heading, Quantity, TravelMode, Unit, VehicleSelector, When,
};
pub(crate) use segment::Record;
pub use segment::{ConnectorRef, Route, Segment};

/// The two scoping properties every rule can carry, so rules of any kind can be
/// evaluated by the same code.
pub trait Scoped {
    /// The rule's `when` block, if any.
    fn when(&self) -> Option<&When>;
    /// The rule's `between` range, if any.
    fn between(&self) -> Option<&Between>;

    /// The rule's conditions in words: `when {heading: backward} between [0.100, 0.200]`,
    /// or `unconditional`.
    fn scope_text(&self) -> String {
        let when = self.when().filter(|w| !w.is_empty());
        match (when, self.between()) {
            (None, None) => "unconditional".to_owned(),
            (Some(w), None) => format!("when {w}"),
            (None, Some(b)) => format!("between {b}"),
            (Some(w), Some(b)) => format!("when {w} between {b}"),
        }
    }
}
