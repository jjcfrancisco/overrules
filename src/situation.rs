//! The facts about a traveller that rules are evaluated against.

use std::collections::BTreeMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::model::{Dimension, Heading, TravelMode, Unit};
use crate::{Error, Result};

/// Who is travelling, where on the segment, which way, and when.
///
/// Every fact is optional. A rule scoped by a fact you leave out is *undetermined*
/// rather than guessed at, and the answer says which later rules could change it.
///
/// ```
/// use overrules::{Dimension, Heading, Situation, TravelMode};
///
/// let s = Situation::new()
///     .at(0.25)
///     .heading(Heading::Forward)
///     .mode(TravelMode::Truck)
///     .vehicle(Dimension::Weight, 12.0);
/// assert_eq!(s.vehicle[&Dimension::Weight], 12.0);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Situation {
    /// Position along the segment, as a geodesic linear reference `0..=1`.
    pub at: Option<f64>,
    /// Direction of travel relative to the segment's geometry.
    pub heading: Option<Heading>,
    /// How the traveller is travelling.
    pub mode: Option<TravelMode>,
    /// Local time of travel.
    pub time: Option<NaiveDateTime>,
    /// Purposes of use the traveller claims (`as_customer`, `at_destination`, `to_deliver`,
    /// `to_farm`, `for_forestry`).
    pub using: Vec<String>,
    /// Statuses the traveller holds (`as_permitted`, `as_private`, `as_disabled`,
    /// `as_employee`, `as_student`).
    pub recognized: Vec<String>,
    /// Vehicle dimensions in canonical units: metres, metric tonnes, or a count.
    pub vehicle: BTreeMap<Dimension, f64>,
}

impl Situation {
    /// A traveller about whom nothing is known yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the position along the segment.
    #[must_use]
    pub fn at(mut self, fraction: f64) -> Self {
        self.at = Some(fraction);
        self
    }

    /// Sets the direction of travel.
    #[must_use]
    pub fn heading(mut self, heading: Heading) -> Self {
        self.heading = Some(heading);
        self
    }

    /// Sets the travel mode.
    #[must_use]
    pub fn mode(mut self, mode: TravelMode) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Sets the local time of travel.
    #[must_use]
    pub fn time(mut self, time: NaiveDateTime) -> Self {
        self.time = Some(time);
        self
    }

    /// Adds a purpose of use.
    #[must_use]
    pub fn using(mut self, purpose: impl Into<String>) -> Self {
        self.using.push(purpose.into());
        self
    }

    /// Adds a recognised status.
    #[must_use]
    pub fn recognized(mut self, status: impl Into<String>) -> Self {
        self.recognized.push(status.into());
        self
    }

    /// Sets a vehicle dimension, in metres, metric tonnes, or a count.
    #[must_use]
    pub fn vehicle(mut self, dimension: Dimension, value: f64) -> Self {
        self.vehicle.insert(dimension, value);
        self
    }

    /// Sets a vehicle dimension given in any unit Overture knows, converting it.
    pub fn vehicle_in(self, dimension: Dimension, value: f64, unit: Unit) -> Result<Self> {
        if unit.quantity() != dimension.quantity() {
            return Err(Error::invalid(
                "vehicle unit",
                unit.as_str(),
                format!("cannot measure {dimension}"),
            ));
        }
        Ok(self.vehicle(dimension, unit.to_canonical(value)))
    }

    /// Sets a vehicle dimension from text such as `weight=12`, `height=4.2`,
    /// `weight=7500kg` or `height=14ft`. Without a unit, lengths are metres and weights
    /// metric tonnes.
    pub fn vehicle_spec(self, spec: &str) -> Result<Self> {
        let bad = |reason: &str| Error::invalid("vehicle", spec, reason);
        let (key, value) = spec
            .split_once('=')
            .ok_or_else(|| bad("expected dimension=value, e.g. weight=3.5"))?;
        let dimension: Dimension = key.trim().parse()?;
        let value = value.trim();
        // The unit is the trailing run of letters, so `1e3kg` reads as 1e3 kg.
        let number = value.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        let unit = &value[number.len()..];
        let number: f64 = number
            .trim()
            .parse()
            .map_err(|_| bad("the value is not a number"))?;
        if unit.is_empty() {
            return Ok(self.vehicle(dimension, number));
        }
        let unit: Unit = unit.parse()?;
        self.vehicle_in(dimension, number, unit)
    }

    /// Checks the facts are usable: the position is within `0..=1` and dimensions are
    /// finite and not negative.
    pub fn validate(&self) -> Result<()> {
        if let Some(f) = self.at
            && !(0.0..=1.0).contains(&f)
        {
            return Err(Error::FractionOutOfRange(f));
        }
        for (d, v) in &self.vehicle {
            if !v.is_finite() || *v < 0.0 {
                return Err(Error::invalid(
                    "vehicle dimension",
                    format!("{d}={v}"),
                    "must be a finite, non-negative number",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_position_and_dimensions() {
        assert!(Situation::new().at(0.5).validate().is_ok());
        assert!(matches!(
            Situation::new().at(1.5).validate(),
            Err(Error::FractionOutOfRange(_))
        ));
        assert!(
            Situation::new()
                .vehicle(Dimension::Height, -1.0)
                .validate()
                .is_err()
        );
    }

    #[test]
    fn converts_vehicle_units() {
        let s = Situation::new()
            .vehicle_in(Dimension::Height, 14.0, Unit::Ft)
            .unwrap();
        assert!((s.vehicle[&Dimension::Height] - 4.2672).abs() < 1e-9);
        assert!(
            Situation::new()
                .vehicle_in(Dimension::Weight, 3.0, Unit::M)
                .is_err()
        );
    }

    #[test]
    fn parses_vehicle_specs() {
        let s = Situation::new()
            .vehicle_spec("weight=7500kg")
            .and_then(|s| s.vehicle_spec("height = 4.2"))
            .and_then(|s| s.vehicle_spec("axle_count=3"))
            .unwrap();
        assert!((s.vehicle[&Dimension::Weight] - 7.5).abs() < 1e-12);
        assert_eq!(s.vehicle[&Dimension::Height], 4.2);
        assert_eq!(s.vehicle[&Dimension::AxleCount], 3.0);
        let s = Situation::new().vehicle_spec("weight=1e3kg").unwrap();
        assert!((s.vehicle[&Dimension::Weight] - 1.0).abs() < 1e-12);
        for bad in [
            "weight",
            "mass=3",
            "weight=heavy",
            "weight=3furlongs",
            "weight=3ft",
        ] {
            assert!(Situation::new().vehicle_spec(bad).is_err(), "{bad}");
        }
    }
}
