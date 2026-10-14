//! Overture segments as GeoJSON.
//!
//! Reads a `FeatureCollection` (or a single `Feature`) whose features have `LineString`
//! geometries and Overture segment properties, as written by `overturemaps download -f
//! geojson` or by [`to_feature_collection`]. The GERS id is taken from `properties.id`, or
//! from the feature's own `id`.

use std::io::Read;

use geo::{Coord, LineString};
use serde_json::{Map, Value, json};

use crate::model::{Record, Segment};
use crate::{Error, Result};

/// Reads segments from GeoJSON text.
pub fn read(reader: impl Read) -> Result<Vec<Segment>> {
    from_value(serde_json::from_reader(reader)?)
}

/// Reads segments from a parsed GeoJSON value.
pub fn from_value(value: Value) -> Result<Vec<Segment>> {
    let Value::Object(mut obj) = value else {
        return Err(Error::GeoJson("expected a JSON object".to_owned()));
    };
    match obj.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => match obj.remove("features") {
            Some(Value::Array(features)) => features
                .into_iter()
                .enumerate()
                .map(|(i, f)| feature(f).map_err(|e| Error::GeoJson(format!("feature #{i}: {e}"))))
                .collect(),
            _ => Err(Error::GeoJson(
                "FeatureCollection without a features array".to_owned(),
            )),
        },
        Some("Feature") => Ok(vec![feature(Value::Object(obj))?]),
        other => Err(Error::GeoJson(format!(
            "expected a Feature or FeatureCollection, found type {other:?}"
        ))),
    }
}

fn feature(value: Value) -> Result<Segment> {
    let Value::Object(mut f) = value else {
        return Err(Error::GeoJson("feature is not an object".to_owned()));
    };
    let mut props = match f.remove("properties") {
        Some(Value::Object(p)) => p,
        Some(Value::Null) | None => Map::new(),
        Some(_) => return Err(Error::GeoJson("properties is not an object".to_owned())),
    };
    if !props.contains_key("id") {
        match f.remove("id") {
            Some(id @ Value::String(_)) => {
                props.insert("id".to_owned(), id);
            }
            _ => {
                return Err(Error::GeoJson(
                    "no GERS id in properties.id or id".to_owned(),
                ));
            }
        }
    }
    let record: Record = serde_json::from_value(Value::Object(props))?;
    let geometry = linestring(f.remove("geometry"))
        .map_err(|e| Error::GeoJson(format!("segment {}: {e}", record.id)))?;
    Segment::from_record(record, geometry)
}

fn linestring(geometry: Option<Value>) -> Result<LineString<f64>, String> {
    let Some(Value::Object(g)) = geometry else {
        return Err("no geometry".to_owned());
    };
    if g.get("type").and_then(Value::as_str) != Some("LineString") {
        return Err(format!("expected a LineString, found {:?}", g.get("type")));
    }
    let coords = g
        .get("coordinates")
        .and_then(Value::as_array)
        .ok_or("LineString without coordinates")?;
    coords
        .iter()
        .map(|c| match c.as_array().map(Vec::as_slice) {
            Some([x, y, ..]) => match (x.as_f64(), y.as_f64()) {
                (Some(x), Some(y)) => Ok(Coord { x, y }),
                _ => Err("a coordinate is not numeric".to_owned()),
            },
            _ => Err("a position has fewer than two numbers".to_owned()),
        })
        .collect::<Result<Vec<_>, _>>()
        .map(LineString)
}

/// A segment as a GeoJSON `Feature`, with Overture-shaped properties.
pub fn to_feature(segment: &Segment) -> Value {
    let coords: Vec<[f64; 2]> = segment.geometry().0.iter().map(|c| [c.x, c.y]).collect();
    json!({
        "type": "Feature",
        "id": segment.id,
        "geometry": {"type": "LineString", "coordinates": coords},
        "properties": segment.to_record(),
    })
}

/// Segments as a GeoJSON `FeatureCollection` that [`read`] reads back losslessly.
pub fn to_feature_collection<'a>(segments: impl IntoIterator<Item = &'a Segment>) -> Value {
    let features: Vec<Value> = segments.into_iter().map(to_feature).collect();
    json!({"type": "FeatureCollection", "features": features})
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEATURE: &str = r#"{
        "type": "Feature", "id": "seg-1",
        "geometry": {"type": "LineString", "coordinates": [[-5.99, 37.39, 12.0], [-5.98, 37.39]]},
        "properties": {
            "subtype": "road", "class": "residential", "names": {"primary": "Calle Feria"},
            "connectors": [{"connector_id": "c1", "at": 0}, {"connector_id": "c2", "at": 1}],
            "speed_limits": [{"max_speed": {"value": 30, "unit": "km/h"}}],
            "access_restrictions": null
        }
    }"#;

    #[test]
    fn reads_a_feature() {
        let segs = read(FEATURE.as_bytes()).unwrap();
        let s = &segs[0];
        assert_eq!(s.id, "seg-1");
        assert_eq!(s.name.as_deref(), Some("Calle Feria"));
        assert_eq!(s.connectors.len(), 2);
        assert_eq!(s.speed_limits.len(), 1);
        assert_eq!(s.access_restrictions.len(), 0);
        assert!(s.length_m() > 800.0);
    }

    #[test]
    fn round_trips_through_a_feature_collection() {
        let segs = read(FEATURE.as_bytes()).unwrap();
        let fc = to_feature_collection(&segs);
        let back = from_value(fc).unwrap();
        assert_eq!(back[0].geometry(), segs[0].geometry());
        assert_eq!(back[0].speed_limits, segs[0].speed_limits);
        assert_eq!(back[0].name, segs[0].name);
    }

    #[test]
    fn rejects_non_linestrings() {
        let bad = r#"{"type": "Feature", "id": "p", "geometry": {"type": "Point", "coordinates": [0, 0]}}"#;
        assert!(matches!(read(bad.as_bytes()), Err(Error::GeoJson(_))));
    }
}
