//! One module per subcommand: its arguments and what it does.

pub mod at;
pub mod audit;
pub mod export;
pub mod find;
pub mod locate;
pub mod pack;
pub mod resolve;
#[cfg(feature = "serve")]
pub mod serve;
pub mod stats;

use anyhow::Result;
use serde::Serialize;

/// Prints a value as pretty JSON on stdout.
pub fn print_json(value: &impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
