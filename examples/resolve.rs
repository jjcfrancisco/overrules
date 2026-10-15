//! Resolve the rules for a traveller on one segment, and print why.
//!
//! ```sh
//! cargo run --example resolve --features parquet -- \
//!     data/nyc/segment.parquet 93212c22 --at 0.8 --heading forward --mode car
//! ```
//!
//! Options: `--at <0..1>`, `--heading forward|backward`, `--mode <travel mode>`,
//! `--time 2026-10-13T08:30`, `--using <purpose>`, `--recognized <status>`,
//! `--vehicle weight=12` (each repeatable where it makes sense). Reads GeoParquet or GeoJSON.

use std::error::Error;

use chrono::NaiveDateTime;
use overrules::eval::{Resolution, RuleEvaluation, Verdict};
use overrules::model::Scoped;
use overrules::{Access, Network, Situation};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: resolve <segments file> <segment id> [--at F] [--heading H] [--mode M] [--time T] [--using U] [--recognized R] [--vehicle D=V]";
    let path = args.next().ok_or(usage)?;
    let id = args.next().ok_or(usage)?;

    let mut s = Situation::new();
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        s = match flag.as_str() {
            "--at" => s.at(value.parse()?),
            "--heading" => s.heading(value.parse()?),
            "--mode" => s.mode(value.parse()?),
            "--time" => s.time(NaiveDateTime::parse_from_str(&value, "%Y-%m-%dT%H:%M")?),
            "--using" => s.using(value),
            "--recognized" => s.recognized(value),
            "--vehicle" => s.vehicle_spec(&value)?,
            _ => return Err(format!("unknown option {flag}\n{usage}").into()),
        };
    }

    let net = Network::new(overrules::io::read_path(&path)?);
    let segment = net.lookup(&id)?;
    let answer = overrules::resolve(segment, &s)?;

    println!(
        "{}  {}  ({:.1} m)",
        segment.id,
        segment.label(),
        segment.length_m()
    );
    println!("\nACCESS");
    print_rules(&answer.access, &segment.access_restrictions, |r| {
        r.access_type.to_string()
    });
    match answer.access_outcome() {
        Access::Rule(t) => println!("  ⇒ {t}"),
        Access::Default => println!("  ⇒ no rule matched: the default for the class applies"),
    }
    println!("\nSPEED LIMIT");
    print_rules(
        &answer.speed_limit,
        &segment.speed_limits,
        ToString::to_string,
    );
    match answer.speed_limit_rule() {
        Some(r) => println!("  ⇒ {r}"),
        None => println!("  ⇒ none in the data"),
    }
    let t = &answer.prohibited_transitions;
    println!(
        "\nPROHIBITED TRANSITIONS: {} of {} in force {:?}, {} could apply {:?}",
        t.in_force.len(),
        t.rules.len(),
        t.in_force,
        t.could_apply.len(),
        t.could_apply
    );
    Ok(())
}

fn print_rules<R: Scoped>(res: &Resolution, rules: &[R], head: impl Fn(&R) -> String) {
    if rules.is_empty() {
        println!("  none in the data");
    }
    for (e, r) in res.rules.iter().zip(rules) {
        let mark = match e.verdict {
            Verdict::Match if res.determining == Some(e.index) => "✔ decides",
            Verdict::Match => "✔",
            Verdict::NoMatch => "✘",
            Verdict::Undetermined => "?",
        };
        println!("  #{} {} {}  [{mark}]", e.index, head(r), r.scope_text());
        print_scopes(e);
    }
    if !res.could_override.is_empty() {
        println!(
            "  ? rules {:?} would override if their missing facts held",
            res.could_override
        );
    }
}

fn print_scopes(e: &RuleEvaluation) {
    for c in &e.scopes {
        println!(
            "      {:<10} {:<28} {:?}: {}",
            c.scope, c.requires, c.verdict, c.because
        );
    }
}
