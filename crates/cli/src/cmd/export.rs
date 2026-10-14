use anyhow::Result;
use overrules::Network;
use overrules::io::geojson;

#[derive(clap::Args)]
pub struct Args {
    /// GERS ids, or unique prefixes.
    #[arg(required = true, value_name = "ID")]
    ids: Vec<String>,
}

pub fn run(net: &Network, a: &Args) -> Result<()> {
    let segments = a
        .ids
        .iter()
        .map(|id| net.lookup(id))
        .collect::<overrules::Result<Vec<_>>>()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&geojson::to_feature_collection(segments))?
    );
    Ok(())
}
