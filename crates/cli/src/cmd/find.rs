use anyhow::Result;
use overrules::{Network, Segment};
use serde_json::json;

use super::print_json;
use crate::render::{rule_flags, truncate};

#[derive(clap::Args)]
pub struct Args {
    /// A GERS id prefix, or part of a street name.
    query: String,
    /// Show at most this many.
    #[arg(long, default_value_t = 15)]
    limit: usize,
}

pub fn run(net: &Network, a: &Args, json: bool) -> Result<()> {
    let mut hits: Vec<&Segment> = net.find_by_prefix(&a.query);
    if hits.is_empty() {
        hits = net.find_by_name(&a.query);
    }
    if json {
        let v: Vec<_> = hits
            .iter()
            .take(a.limit)
            .map(|s| {
                json!({
                    "id": s.id, "name": s.name, "subtype": s.subtype, "class": s.class,
                    "length_m": s.length_m(),
                    "access_restrictions": s.access_restrictions.len(),
                    "speed_limits": s.speed_limits.len(),
                    "prohibited_transitions": s.prohibited_transitions.len(),
                })
            })
            .collect();
        return print_json(&v);
    }
    let shown = if hits.len() > a.limit {
        format!(", showing {}", a.limit)
    } else {
        String::new()
    };
    println!("{} match(es){shown}", hits.len());
    for s in hits.iter().take(a.limit) {
        println!(
            "  {}  {:<48} {:>7.0} m  {}",
            s.id,
            truncate(&s.label(), 48),
            s.length_m(),
            rule_flags(s)
        );
    }
    Ok(())
}
