use std::fmt;

use serde::{Deserialize, Serialize};

use super::{Between, Heading, Scoped, When};

/// What an access restriction rule says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessType {
    /// Access is allowed.
    Allowed,
    /// Access is denied.
    Denied,
    /// Access is allowed, and the segment is designated for it (a cycleway for bicycles).
    Designated,
}

impl AccessType {
    /// The spelling Overture uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Denied => "denied",
            Self::Designated => "designated",
        }
    }
}

impl fmt::Display for AccessType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

/// One rule of a segment's `access_restrictions`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessRestriction {
    /// What the rule says when it applies.
    pub access_type: AccessType,
    /// Conditions on the traveller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<When>,
    /// The stretch of the segment it applies to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub between: Option<Between>,
}

/// Unit of a [`Speed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpeedUnit {
    /// Kilometres per hour.
    #[serde(rename = "km/h")]
    KilometresPerHour,
    /// Miles per hour.
    #[serde(rename = "mph")]
    MilesPerHour,
}

impl fmt::Display for SpeedUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::KilometresPerHour => "km/h",
            Self::MilesPerHour => "mph",
        })
    }
}

/// A speed with its unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Speed {
    /// Magnitude; the schema allows 1 to 350.
    pub value: u16,
    /// Unit.
    pub unit: SpeedUnit,
}

impl fmt::Display for Speed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.value, self.unit)
    }
}

/// One rule of a segment's `speed_limits`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeedLimit {
    /// Minimum speed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_speed: Option<Speed>,
    /// Maximum speed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_speed: Option<Speed>,
    /// Whether the maximum is variable (signs that change).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_max_speed_variable: Option<bool>,
    /// Conditions on the traveller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<When>,
    /// The stretch of the segment it applies to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub between: Option<Between>,
}

/// Renders the value of the rule, e.g. `max 50 km/h, min 20 km/h`.
impl fmt::Display for SpeedLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(s) = self.max_speed {
            parts.push(format!("max {s}"));
        }
        if let Some(s) = self.min_speed {
            parts.push(format!("min {s}"));
        }
        if self.is_max_speed_variable == Some(true) {
            parts.push("variable".to_owned());
        }
        if parts.is_empty() {
            f.write_str("(no value)")
        } else {
            f.write_str(&parts.join(", "))
        }
    }
}

/// One step of a prohibited transition: through a connector, onto a segment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransitionStep {
    /// GERS id of the connector passed through.
    pub connector_id: String,
    /// GERS id of the segment entered.
    pub segment_id: String,
}

/// One rule of a segment's `prohibited_transitions`: a turn (or longer manoeuvre)
/// that may not be made from this segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProhibitedTransition {
    /// The manoeuvre, step by step.
    #[serde(default)]
    pub sequence: Vec<TransitionStep>,
    /// Direction of travel on the last segment of the sequence.
    pub final_heading: Heading,
    /// Conditions on the traveller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<When>,
    /// The stretch of this segment it applies to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub between: Option<Between>,
}

macro_rules! impl_scoped {
    ($($t:ty),*) => {$(
        impl Scoped for $t {
            fn when(&self) -> Option<&When> {
                self.when.as_ref()
            }
            fn between(&self) -> Option<&Between> {
                self.between.as_ref()
            }
        }
    )*};
}

impl_scoped!(AccessRestriction, SpeedLimit, ProhibitedTransition);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialises_overture_json() {
        let r: SpeedLimit = serde_json::from_str(
            r#"{"max_speed": {"value": 50, "unit": "km/h"}, "min_speed": null,
                "when": {"heading": "backward", "mode": ["car"]}, "between": [0.1, 0.3]}"#,
        )
        .unwrap();
        assert_eq!(r.to_string(), "max 50 km/h");
        assert_eq!(r.between, Some(Between::new(0.1, 0.3)));
        let a: AccessRestriction =
            serde_json::from_str(r#"{"access_type": "designated", "when": null}"#).unwrap();
        assert_eq!(a.access_type, AccessType::Designated);
    }
}
