//! Print, as JSON, every value this crate knows for each Overture enum, and the schema version
//! it reads. `scripts/check_schema.py` compares this against Overture's schema.
//!
//! ```sh
//! cargo run --example supported_values
//! ```

use overrules::model::{AccessType, Comparison, Quantity, SpeedUnit, Unit};
use overrules::{Dimension, Heading, TravelMode};
use serde_json::json;

fn main() {
    let units = |q: Quantity| -> Vec<&str> {
        Unit::ALL
            .into_iter()
            .filter(|u| u.quantity() == q)
            .map(Unit::as_str)
            .collect()
    };
    let values = json!({
        "schema_version": overrules::SCHEMA_VERSION,
        "lists": {
            "heading": Heading::ALL.map(Heading::as_str),
            "travel_mode": TravelMode::ALL.iter().map(TravelMode::as_str).collect::<Vec<_>>(),
            "vehicle_dimension": Dimension::ALL.map(Dimension::as_str),
            "comparison": Comparison::ALL.map(Comparison::as_str),
            "length_unit": units(Quantity::Length),
            "weight_unit": units(Quantity::Weight),
            "speed_unit": SpeedUnit::ALL.map(SpeedUnit::as_str),
            "access_type": AccessType::ALL.map(AccessType::as_str),
        },
    });
    println!("{values:#}");
}
