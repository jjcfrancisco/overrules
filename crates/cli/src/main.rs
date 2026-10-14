//! `overrules`: resolve Overture transportation rules from the command line.

// Doc comments here double as `--help` text, where backticks would show literally.
#![allow(clippy::doc_markdown)]

mod cmd;
mod render;

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand};
use overrules::Network;

#[derive(Parser)]
#[command(name = "overrules", version, about, long_about = None)]
/// Which Overture transportation rules apply here, for this traveller, and why.
struct Cli {
    /// Overture segments: a GeoParquet file from a release (a bbox extract is plenty), or
    /// GeoJSON.
    #[arg(short, long, global = true, env = "OVERRULES_SEGMENTS")]
    segments: Option<PathBuf>,
    /// Print JSON instead of a report.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// What the data holds, and how much of it is scoped.
    Stats,
    /// Find segments by GERS id prefix or by name.
    Find(cmd::find::Args),
    /// Snap a point to the nearest segments: geodesic linear reference vs the planar one.
    Locate(cmd::locate::Args),
    /// The point at a linear reference on a segment, and where planar maths would put it.
    At(cmd::at::Args),
    /// The rules in force for a traveller at a position on a segment, and why.
    Resolve(cmd::resolve::Args),
    /// Segments as GeoJSON, in the shape `--segments` reads back.
    Export(cmd::export::Args),
    /// How far every `between` boundary moves if linear references are computed planar.
    Audit(cmd::audit::Args),
    /// Write a compact snapshot for the browser build of the explorer.
    Pack(cmd::pack::Args),
    /// Serve the web explorer and its JSON API.
    #[cfg(feature = "serve")]
    Serve(cmd::serve::Args),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = cli
        .segments
        .context("no segments given: pass --segments <FILE> or set OVERRULES_SEGMENTS")?;
    let t0 = Instant::now();
    let segments =
        overrules::io::read_path(&path).with_context(|| format!("reading {}", path.display()))?;
    let net = Network::new(segments);
    if !cli.json {
        eprintln!(
            "loaded {} segments from {} in {:.1}s",
            net.len(),
            path.display(),
            t0.elapsed().as_secs_f64()
        );
    }
    let json = cli.json;
    match cli.command {
        Command::Stats => cmd::stats::run(&net, json),
        Command::Find(a) => cmd::find::run(&net, &a, json),
        Command::Locate(a) => cmd::locate::run(&net, &a, json),
        Command::At(a) => cmd::at::run(&net, &a, json),
        Command::Resolve(a) => cmd::resolve::run(&net, a, json),
        Command::Export(a) => cmd::export::run(&net, &a),
        Command::Audit(a) => cmd::audit::run(&net, &a, json),
        Command::Pack(a) => cmd::pack::run(&net, &path, &a, json),
        #[cfg(feature = "serve")]
        Command::Serve(a) => cmd::serve::run(net, &a),
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory as _;

    use super::Cli;

    #[test]
    fn cli_definition_is_consistent() {
        Cli::command().debug_assert();
    }
}
