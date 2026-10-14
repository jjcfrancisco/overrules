use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use flate2::Compression;
use flate2::write::GzEncoder;
use overrules::{Network, pack};
use serde_json::json;

use super::print_json;

#[derive(clap::Args)]
pub struct Args {
    /// Where to write the gzip-compressed pack (browsers decompress it natively).
    #[arg(long, short)]
    out: PathBuf,
    /// Label stored in the pack and shown by the explorer; defaults to the input file name.
    #[arg(long)]
    source: Option<String>,
}

pub fn run(net: &Network, input: &Path, a: &Args, json: bool) -> Result<()> {
    let source = a.source.clone().unwrap_or_else(|| {
        input
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    let (bytes, report) = pack::encode(net.segments(), &source)?;
    if let Some(dir) = a.out.parent() {
        fs::create_dir_all(dir)?;
    }
    let file = File::create(&a.out).with_context(|| format!("creating {}", a.out.display()))?;
    let mut gz = GzEncoder::new(file, Compression::best());
    gz.write_all(&bytes)?;
    let compressed = gz.finish()?.metadata()?.len();
    if json {
        return print_json(&json!({
            "out": a.out, "segments": report.segments, "vertices": report.vertices,
            "connectors": report.connectors, "exact_fallbacks": report.exact_fallbacks,
            "bytes": bytes.len(), "gzip_bytes": compressed,
        }));
    }
    let exactness = if report.exact_fallbacks == 0 {
        "; all numbers quantised losslessly".to_owned()
    } else {
        format!("; {} list(s) kept as f64", report.exact_fallbacks)
    };
    println!(
        "wrote {}: {} segments, {} vertices, {} connectors; {:.1} MB packed → {:.1} MB gzipped{exactness}",
        a.out.display(),
        report.segments,
        report.vertices,
        report.connectors,
        bytes.len() as f64 / 1e6,
        compressed as f64 / 1e6,
    );
    Ok(())
}
