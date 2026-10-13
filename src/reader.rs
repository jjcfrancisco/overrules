//! Read Overture `segment` Parquet files straight into typed [`Segment`]s.
//!
//! Uses the parquet crate's row API (no Arrow) and lets serde do the nested
//! struct/list decoding via JSON values, which keeps the schema handling to a
//! few dozen lines at the cost of some speed — fine for a city or a country.

use std::fs::File;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use geo::{Geometry, LineString};
use geozero::wkb::Wkb;
use geozero::ToGeo;
use parquet::file::reader::{FileReader, SerializedFileReader};

use crate::lr::Measure;
use crate::model::{Segment, SegmentRecord};

pub fn read_segments(path: &Path) -> Result<Vec<Segment>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let reader = SerializedFileReader::new(file)?;
    let mut out = Vec::with_capacity(reader.metadata().file_metadata().num_rows() as usize);
    for row in reader.get_row_iter(None)? {
        let row = row?;
        let value = row.to_json_value();
        let rec: SegmentRecord = serde_json::from_value(value).context("decoding segment row")?;
        let geometry =
            decode_linestring(&rec.geometry).with_context(|| format!("geometry of {}", rec.id))?;
        let measure = Measure::new(&geometry);
        out.push(Segment {
            rec,
            geometry,
            measure,
        });
    }
    Ok(out)
}

fn decode_linestring(b64: &str) -> Result<LineString<f64>> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
    match Wkb(bytes).to_geo().map_err(|e| anyhow!("wkb: {e}"))? {
        Geometry::LineString(ls) => Ok(ls),
        other => bail!(
            "expected LineString, found {:?}",
            std::mem::discriminant(&other)
        ),
    }
}
