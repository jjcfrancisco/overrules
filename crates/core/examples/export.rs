//! Write chosen segments as GeoJSON, the format of the test fixtures.
//!
//! ```sh
//! cargo run --example export --features parquet -- data/seville/segment.parquet e26d3384 1b946971
//! ```

use std::error::Error;

use overrules::Network;
use overrules::io::geojson;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: export <segments file> <id>...")?;
    let net = Network::new(overrules::io::read_path(&path)?);
    let segments = args
        .map(|id| net.lookup(&id))
        .collect::<overrules::Result<Vec<_>>>()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&geojson::to_feature_collection(segments))?
    );
    Ok(())
}
