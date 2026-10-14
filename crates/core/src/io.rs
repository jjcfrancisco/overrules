//! Reading and writing segments.
//!
//! * [`geojson`]: Overture segments as GeoJSON features (always available).
//! * [`parquet`]: Overture's GeoParquet release files (feature `parquet`).

pub mod geojson;
#[cfg(feature = "parquet")]
pub mod parquet;

use std::path::Path;

use crate::model::Segment;
use crate::{Error, Result};

/// Reads segments from a file, choosing the format by extension: `.parquet`, or
/// `.geojson`/`.json`.
pub fn read_path(path: impl AsRef<Path>) -> Result<Vec<Segment>> {
    let path = path.as_ref();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("geojson" | "json") => geojson::read(std::fs::File::open(path)?),
        #[cfg(feature = "parquet")]
        Some("parquet") => parquet::read_path(path),
        _ => Err(Error::invalid(
            "input file",
            path.display().to_string(),
            if cfg!(feature = "parquet") {
                "expected a .parquet, .geojson or .json file"
            } else {
                "expected a .geojson or .json file (build with feature `parquet` for Parquet)"
            },
        )),
    }
}
