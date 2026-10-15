//! Overture `segment` GeoParquet files.
//!
//! Uses the parquet crate's row API and lets serde decode the nested structs and lists
//! through JSON values. That keeps schema handling to a few dozen lines, at some cost in
//! speed: fine for a city or region, not for the planet.

use std::fs::File;
use std::path::Path;

use base64::Engine as _;
use geo::{Geometry, LineString};
use geozero::ToGeo as _;
use geozero::wkb::Wkb;
use parquet::file::reader::{FileReader as _, SerializedFileReader};

use crate::model::{Record, Segment};
use crate::{Error, Result};

/// Reads every segment in an Overture `segment` Parquet file.
pub fn read_path(path: impl AsRef<Path>) -> Result<Vec<Segment>> {
    read(File::open(path)?)
}

/// Reads every segment from an open Overture `segment` Parquet file.
pub fn read(file: File) -> Result<Vec<Segment>> {
    let reader = SerializedFileReader::new(file)?;
    let rows = usize::try_from(reader.metadata().file_metadata().num_rows()).unwrap_or_default();
    let mut out = Vec::with_capacity(rows);
    for row in reader.get_row_iter(None)? {
        let mut value = row?.to_json_value();
        // WKB, base64-encoded: that is how the parquet crate renders binary columns as JSON.
        let geometry = value
            .as_object_mut()
            .and_then(|o| o.remove("geometry"))
            .and_then(|g| g.as_str().map(str::to_owned));
        let record = Record::from_value(value)?;
        let geometry = geometry
            .ok_or_else(|| "no geometry".to_owned())
            .and_then(|g| decode_linestring(&g))
            .map_err(|reason| Error::InvalidGeometry {
                id: record.id.clone(),
                reason,
            })?;
        out.push(Segment::from_record(record, geometry)?);
    }
    Ok(out)
}

fn decode_linestring(b64: &str) -> Result<LineString<f64>, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("geometry is not base64: {e}"))?;
    match Wkb(bytes).to_geo().map_err(|e| format!("WKB: {e}"))? {
        Geometry::LineString(ls) => Ok(ls),
        other => Err(format!("expected a LineString, found {}", kind(&other))),
    }
}

fn kind(g: &Geometry<f64>) -> &'static str {
    match g {
        Geometry::Point(_) => "Point",
        Geometry::Line(_) => "Line",
        Geometry::LineString(_) => "LineString",
        Geometry::Polygon(_) => "Polygon",
        Geometry::MultiPoint(_) => "MultiPoint",
        Geometry::MultiLineString(_) => "MultiLineString",
        Geometry::MultiPolygon(_) => "MultiPolygon",
        Geometry::GeometryCollection(_) => "GeometryCollection",
        Geometry::Rect(_) => "Rect",
        Geometry::Triangle(_) => "Triangle",
    }
}
