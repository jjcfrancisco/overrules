use geo::LineString;
use serde::{Deserialize, Deserializer, Serialize};

use super::{AccessRestriction, Between, ProhibitedTransition, SpeedLimit};
use crate::lr::GeodesicLine;
use crate::{Error, Result};

/// Where a connector sits along a segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectorRef {
    /// GERS id of the connector.
    pub connector_id: String,
    /// Linear reference of the connector along the segment.
    pub at: f64,
}

/// A route (e.g. a numbered road) the segment is part of.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Route {
    /// Full name of the route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Highway system the route belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    /// Code or number of the route, e.g. `SE-30`.
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// The stretch of the segment that is on the route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub between: Option<Between>,
}

/// An Overture transportation segment: its geometry and the properties `overrules` reads.
///
/// Rule lists keep the order they have in the data, which is what gives them meaning:
/// the last matching rule wins.
#[derive(Debug, Clone)]
pub struct Segment {
    /// GERS id.
    pub id: String,
    /// `road`, `rail` or `water`.
    pub subtype: Option<String>,
    /// e.g. `residential`, `footway`, `cycleway`.
    pub class: Option<String>,
    /// e.g. `sidewalk`, `crosswalk`, `link`.
    pub subclass: Option<String>,
    /// `names.primary`.
    pub name: Option<String>,
    /// Connectors along the segment.
    pub connectors: Vec<ConnectorRef>,
    /// Routes the segment is part of.
    pub routes: Vec<Route>,
    /// `access_restrictions`, in data order.
    pub access_restrictions: Vec<AccessRestriction>,
    /// `speed_limits`, in data order.
    pub speed_limits: Vec<SpeedLimit>,
    /// `prohibited_transitions`, in data order.
    pub prohibited_transitions: Vec<ProhibitedTransition>,
    line: GeodesicLine,
}

impl Segment {
    /// A segment with this id and geometry and no properties yet.
    ///
    /// Fails if the geometry cannot be linearly referenced (fewer than two vertices, or
    /// coordinates that are not finite).
    pub fn new(id: impl Into<String>, geometry: LineString<f64>) -> Result<Self> {
        let id = id.into();
        let line = GeodesicLine::new(geometry).map_err(|source| Error::InvalidGeometry {
            id: id.clone(),
            reason: source.to_string(),
        })?;
        Ok(Self {
            id,
            subtype: None,
            class: None,
            subclass: None,
            name: None,
            connectors: Vec::new(),
            routes: Vec::new(),
            access_restrictions: Vec::new(),
            speed_limits: Vec::new(),
            prohibited_transitions: Vec::new(),
            line,
        })
    }

    /// The geometry, with its geodesic measure.
    pub fn line(&self) -> &GeodesicLine {
        &self.line
    }

    /// The geometry.
    pub fn geometry(&self) -> &LineString<f64> {
        self.line.geometry()
    }

    /// Geodesic length on WGS84, in metres.
    pub fn length_m(&self) -> f64 {
        self.line.length_m()
    }

    /// `"Calle Feria" (road/residential)`, or `<unnamed> (road/footway)`.
    pub fn label(&self) -> String {
        let kind = format!(
            "{}/{}",
            self.subtype.as_deref().unwrap_or("?"),
            self.class.as_deref().unwrap_or("?")
        );
        match &self.name {
            Some(n) => format!("\"{n}\" ({kind})"),
            None => format!("<unnamed> ({kind})"),
        }
    }

    /// Whether any speed limit or access restriction applies to only part of the segment.
    pub fn has_between(&self) -> bool {
        self.access_restrictions.iter().any(|r| r.between.is_some())
            || self.speed_limits.iter().any(|r| r.between.is_some())
    }

    /// Every distinct `between` boundary strictly inside the segment, ascending.
    pub fn boundaries(&self) -> Vec<f64> {
        let mut out: Vec<f64> = self
            .access_restrictions
            .iter()
            .filter_map(|r| r.between)
            .chain(self.speed_limits.iter().filter_map(|r| r.between))
            .flat_map(|b| [b.start, b.end])
            .filter(|f| *f > 0.0 && *f < 1.0)
            .collect();
        out.sort_by(f64::total_cmp);
        out.dedup();
        out
    }

    pub(crate) fn from_record(record: Record, geometry: LineString<f64>) -> Result<Self> {
        let mut s = Self::new(record.id, geometry)?;
        s.subtype = record.subtype;
        s.class = record.class;
        s.subclass = record.subclass;
        s.name = record.names.and_then(|n| n.primary);
        s.connectors = record.connectors;
        s.routes = record.routes;
        s.access_restrictions = record.access_restrictions;
        s.speed_limits = record.speed_limits;
        s.prohibited_transitions = record.prohibited_transitions;
        Ok(s)
    }

    pub(crate) fn to_record(&self) -> Record {
        Record {
            id: self.id.clone(),
            subtype: self.subtype.clone(),
            class: self.class.clone(),
            subclass: self.subclass.clone(),
            names: self.name.clone().map(|primary| Names {
                primary: Some(primary),
            }),
            connectors: self.connectors.clone(),
            routes: self.routes.clone(),
            access_restrictions: self.access_restrictions.clone(),
            speed_limits: self.speed_limits.clone(),
            prohibited_transitions: self.prohibited_transitions.clone(),
        }
    }
}

/// The properties of a segment as Overture writes them (GeoJSON properties, or a Parquet
/// row as JSON), minus the geometry. Readers and writers go through this.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Record {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subclass: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub names: Option<Names>,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub connectors: Vec<ConnectorRef>,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub routes: Vec<Route>,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub access_restrictions: Vec<AccessRestriction>,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub speed_limits: Vec<SpeedLimit>,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub prohibited_transitions: Vec<ProhibitedTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Names {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<String>,
}

/// Overture writes absent lists as `null`; treat that like an empty list.
fn null_as_empty<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(d)?.unwrap_or_default())
}
