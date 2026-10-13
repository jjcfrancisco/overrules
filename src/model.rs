//! Data model for Overture Maps transportation segments (schema v1.18-era).
//!
//! Only the properties the resolver needs are typed; everything else is kept
//! as raw JSON so nothing is lost when printing a feature.

use geo::LineString;
use serde::{Deserialize, Serialize};

/// A `when` scoping block shared by all rule-based properties.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct When {
    #[serde(default)]
    pub heading: Option<String>,
    #[serde(default)]
    pub during: Option<String>,
    #[serde(default)]
    pub using: Option<Vec<String>>,
    #[serde(default)]
    pub recognized: Option<Vec<String>>,
    #[serde(default)]
    pub mode: Option<Vec<String>>,
    #[serde(default)]
    pub vehicle: Option<Vec<VehicleSelector>>,
}

impl When {
    pub fn is_empty(&self) -> bool {
        self.heading.is_none()
            && self.during.is_none()
            && self.using.is_none()
            && self.recognized.is_none()
            && self.mode.is_none()
            && self.vehicle.is_none()
    }

    /// Compact, human-readable rendering such as `{heading: backward, mode: [motor_vehicle]}`.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(h) = &self.heading {
            parts.push(format!("heading: {h}"));
        }
        if let Some(d) = &self.during {
            parts.push(format!("during: \"{d}\""));
        }
        if let Some(m) = &self.mode {
            parts.push(format!("mode: [{}]", m.join(", ")));
        }
        if let Some(u) = &self.using {
            parts.push(format!("using: [{}]", u.join(", ")));
        }
        if let Some(r) = &self.recognized {
            parts.push(format!("recognized: [{}]", r.join(", ")));
        }
        if let Some(v) = &self.vehicle {
            let sel: Vec<String> = v.iter().map(|s| s.summary()).collect();
            parts.push(format!("vehicle: [{}]", sel.join(", ")));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("{{{}}}", parts.join(", "))
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct VehicleSelector {
    pub dimension: String,
    pub comparison: String,
    pub value: f64,
    #[serde(default)]
    pub unit: Option<String>,
}

impl VehicleSelector {
    pub fn summary(&self) -> String {
        let op = match self.comparison.as_str() {
            "greater_than" => ">",
            "greater_than_equal" => ">=",
            "equal" => "==",
            "less_than" => "<",
            "less_than_equal" => "<=",
            other => other,
        };
        format!(
            "{} {} {}{}",
            self.dimension,
            op,
            self.value,
            self.unit
                .as_deref()
                .map(|u| format!(" {u}"))
                .unwrap_or_default()
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Speed {
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpeedLimitRule {
    #[serde(default)]
    pub min_speed: Option<Speed>,
    #[serde(default)]
    pub max_speed: Option<Speed>,
    #[serde(default)]
    pub is_max_speed_variable: Option<bool>,
    #[serde(default)]
    pub when: Option<When>,
    #[serde(default)]
    pub between: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AccessRestrictionRule {
    pub access_type: String,
    #[serde(default)]
    pub when: Option<When>,
    #[serde(default)]
    pub between: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransitionStep {
    pub connector_id: String,
    pub segment_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProhibitedTransitionRule {
    #[serde(default)]
    pub sequence: Vec<TransitionStep>,
    pub final_heading: String,
    #[serde(default)]
    pub when: Option<When>,
    #[serde(default)]
    pub between: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectorRef {
    pub connector_id: String,
    pub at: f64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Names {
    #[serde(default)]
    pub primary: Option<String>,
    #[serde(default)]
    pub common: Option<serde_json::Value>,
    #[serde(default)]
    pub rules: Option<serde_json::Value>,
}

/// `{value, between}` pattern used by road_surface, subclass_rules, level_rules, width_rules.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScopedValue {
    pub value: serde_json::Value,
    #[serde(default)]
    pub between: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RoadFlagRule {
    #[serde(default)]
    pub values: Vec<String>,
    #[serde(default)]
    pub between: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Route {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub network: Option<String>,
    #[serde(default, rename = "ref")]
    pub reference: Option<String>,
    #[serde(default)]
    pub between: Option<Vec<f64>>,
}

/// One row of an Overture `segment` Parquet file, as deserialised from JSON.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SegmentRecord {
    pub id: String,
    /// WKB geometry, base64-encoded (that is how the parquet crate renders binary columns).
    pub geometry: String,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub subclass: Option<String>,
    #[serde(default)]
    pub names: Option<Names>,
    #[serde(default)]
    pub connectors: Option<Vec<ConnectorRef>>,
    #[serde(default)]
    pub routes: Option<Vec<Route>>,
    #[serde(default)]
    pub access_restrictions: Option<Vec<AccessRestrictionRule>>,
    #[serde(default)]
    pub speed_limits: Option<Vec<SpeedLimitRule>>,
    #[serde(default)]
    pub prohibited_transitions: Option<Vec<ProhibitedTransitionRule>>,
    #[serde(default)]
    pub road_surface: Option<Vec<ScopedValue>>,
    #[serde(default)]
    pub road_flags: Option<Vec<RoadFlagRule>>,
    #[serde(default)]
    pub level_rules: Option<Vec<ScopedValue>>,
    #[serde(default)]
    pub subclass_rules: Option<Vec<ScopedValue>>,
    #[serde(default)]
    pub width_rules: Option<Vec<ScopedValue>>,
}

/// A decoded segment: typed properties plus geometry and its geodesic measure.
#[derive(Debug, Clone)]
pub struct Segment {
    pub rec: SegmentRecord,
    pub geometry: LineString<f64>,
    pub measure: crate::lr::Measure,
}

impl Segment {
    pub fn id(&self) -> &str {
        &self.rec.id
    }

    pub fn name(&self) -> Option<&str> {
        self.rec.names.as_ref().and_then(|n| n.primary.as_deref())
    }

    pub fn class(&self) -> &str {
        self.rec.class.as_deref().unwrap_or("?")
    }

    pub fn subtype(&self) -> &str {
        self.rec.subtype.as_deref().unwrap_or("?")
    }

    pub fn label(&self) -> String {
        match self.name() {
            Some(n) => format!("\"{n}\" ({}/{})", self.subtype(), self.class()),
            None => format!("<unnamed> ({}/{})", self.subtype(), self.class()),
        }
    }
}
