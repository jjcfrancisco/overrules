use anyhow::Result;
use overrules::{Network, RuleStats};

use super::print_json;

pub fn run(net: &Network, json: bool) -> Result<()> {
    let s = net.stats();
    if json {
        return print_json(&s);
    }
    println!(
        "Segments: {}  ·  {:.1} km of geometry  ·  {} connector references",
        s.segments, s.length_km, s.connector_refs
    );
    println!("\nBy subtype/class:");
    let mut classes: Vec<_> = s.by_class.iter().collect();
    classes.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (class, n) in classes {
        println!("  {n:>7}  {class}");
    }
    println!();
    line("speed_limits", &s.speed_limits);
    line("access_restrictions", &s.access_restrictions);
    line("prohibited_transitions", &s.prohibited_transitions);
    println!(
        "\n{} segments carry at least one `between` rule: reading them correctly needs geodesic linear referencing.",
        s.segments_with_between
    );
    Ok(())
}

fn line(name: &str, r: &RuleStats) {
    println!(
        "{name:<23} {} rules on {} segments  ·  heading {}, during {}, mode {}, using {}, recognized {}, vehicle {}  ·  {} with `between`",
        r.rules,
        r.segments,
        r.heading,
        r.during,
        r.mode,
        r.using,
        r.recognized,
        r.vehicle,
        r.between
    );
}
