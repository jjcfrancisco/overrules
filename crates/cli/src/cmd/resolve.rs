use anyhow::{Result, bail};
use chrono::NaiveDateTime;
use geo::{Distance as _, Geodesic, Point};
use overrules::eval::{RuleEvaluation, Verdict};
use overrules::lr::{Projection, planar};
use overrules::model::Scoped;
use overrules::{Access, Heading, Network, Resolution, Segment, Situation, TravelMode};
use overrules_explorer::api::describe;
use overrules_explorer::request::parse_time;
use serde_json::json;

use super::print_json;
use crate::render::{plural, truncate};

#[derive(clap::Args)]
pub struct Args {
    /// GERS id, or a unique prefix. Without it, --lon/--lat snap to the nearest segment.
    #[arg(long)]
    segment: Option<String>,
    /// Position as a linear reference, 0 to 1.
    #[arg(long, conflicts_with_all = ["lon", "lat"])]
    at: Option<f64>,
    /// Longitude of the position, snapped onto the segment.
    #[arg(long, allow_negative_numbers = true, requires = "lat")]
    lon: Option<f64>,
    /// Latitude of the position, snapped onto the segment.
    #[arg(long, allow_negative_numbers = true, requires = "lon")]
    lat: Option<f64>,
    /// forward or backward, relative to the order of the segment's coordinates.
    #[arg(long, value_parser = parse_heading)]
    heading: Option<Heading>,
    /// foot, bicycle, vehicle, motor_vehicle, car, truck, motorcycle, bus, hgv, hov or emergency.
    #[arg(long, value_parser = parse_mode)]
    mode: Option<TravelMode>,
    /// Local time of travel, e.g. 2026-10-13T08:30, or "now".
    #[arg(long, value_parser = parse_time_arg)]
    time: Option<NaiveDateTime>,
    /// Purpose of use: as_customer, at_destination, to_deliver, to_farm, for_forestry. Repeatable.
    #[arg(long = "using")]
    using: Vec<String>,
    /// Status: as_permitted, as_private, as_disabled, as_employee, as_student. Repeatable.
    #[arg(long)]
    recognized: Vec<String>,
    /// Vehicle dimension, e.g. weight=3.5 (t), height=4.2 (m), weight=7500kg, height=14ft,
    /// axle_count=3. Repeatable.
    #[arg(long)]
    vehicle: Vec<String>,
}

fn parse_heading(s: &str) -> overrules::Result<Heading> {
    s.parse()
}

fn parse_mode(s: &str) -> overrules::Result<TravelMode> {
    s.parse()
}

fn parse_time_arg(s: &str) -> overrules_explorer::Result<NaiveDateTime> {
    parse_time(s)
}

pub fn run(net: &Network, a: Args, json: bool) -> Result<()> {
    let point = a.lon.zip(a.lat).map(|(lon, lat)| Point::new(lon, lat));
    let (seg, snapped): (&Segment, Option<Projection>) = match (&a.segment, point) {
        (Some(id), None) => (net.lookup(id)?, None),
        (Some(id), Some(p)) => {
            let s = net.lookup(id)?;
            (s, Some(s.line().project(p)))
        }
        (None, Some(p)) => {
            let hit = net.snap(p)?;
            (hit.segment, Some(hit.projection))
        }
        (None, None) => bail!("give --segment, or --lon and --lat"),
    };

    let mut s = Situation::new();
    s.at = snapped.map(|p| p.at.fraction).or(a.at);
    s.heading = a.heading;
    s.mode = a.mode;
    s.time = a.time;
    s.using = a.using;
    s.recognized = a.recognized;
    for v in &a.vehicle {
        s = s.vehicle_spec(v)?;
    }
    let answer = overrules::resolve(seg, &s)?;
    if json {
        return print_json(&json!({
            "segment": {"id": seg.id, "name": seg.name, "subtype": seg.subtype, "class": seg.class, "length_m": seg.length_m()},
            "position": snapped.map_or_else(|| json!({"fraction": s.at}), |p| json!(p)),
            "answer": answer,
            "access": answer.access_outcome(),
            "speed_limit": answer.speed_limit_rule(),
        }));
    }
    print_header(seg, snapped, point, &answer.situation);

    let access = match answer.access_outcome() {
        Access::Rule(t) => t.to_string().to_uppercase(),
        Access::Default => "IMPLIED: no rule matched; the default for the class applies".to_owned(),
    };
    print_property(
        "ACCESS",
        &answer.access,
        &seg.access_restrictions,
        |r| r.access_type.to_string(),
        &access,
    );
    let speed = answer.speed_limit_rule().map_or_else(
        || "NONE IN DATA: fall back to the class or local default".to_owned(),
        ToString::to_string,
    );
    print_property(
        "SPEED LIMIT",
        &answer.speed_limit,
        &seg.speed_limits,
        ToString::to_string,
        &speed,
    );

    let t = &answer.prohibited_transitions;
    println!(
        "│ PROHIBITED TRANSITIONS  ({} rule{})",
        t.rules.len(),
        plural(t.rules.len())
    );
    if t.rules.is_empty() {
        println!("│   none in data");
    }
    for (e, r) in t.rules.iter().zip(&seg.prohibited_transitions) {
        let path: Vec<String> = r
            .sequence
            .iter()
            .map(|st| {
                let short = st.connector_id.get(..12).unwrap_or(&st.connector_id);
                format!("via {short}… onto {}", describe(net, &st.segment_id))
            })
            .collect();
        let head = format!(
            "no transition {} (final heading {})",
            path.join(", then "),
            r.final_heading
        );
        print_rule(e, &head, r);
    }
    if !t.in_force.is_empty() {
        println!(
            "│   ⇒ {} prohibited turn(s) in force here: {:?}",
            t.in_force.len(),
            t.in_force
        );
    }
    if !t.could_apply.is_empty() {
        println!(
            "│   ? {:?} could apply if the missing facts hold",
            t.could_apply
        );
    }
    println!(
        "└ semantics: every condition must hold; the last matching rule wins; otherwise the default applies."
    );
    Ok(())
}

fn print_header(
    seg: &Segment,
    snapped: Option<Projection>,
    point: Option<Point<f64>>,
    s: &Situation,
) {
    let line = seg.line();
    println!("┌ {}  {}", seg.id, seg.label());
    let routes: Vec<&str> = seg
        .routes
        .iter()
        .filter_map(|r| r.reference.as_deref().or(r.name.as_deref()))
        .collect();
    println!(
        "│ length {:.1} m · {} vertices · {} connectors{}",
        seg.length_m(),
        seg.geometry().0.len(),
        seg.connectors.len(),
        if routes.is_empty() {
            String::new()
        } else {
            format!(" · routes [{}]", routes.join(", "))
        }
    );
    match (snapped, s.at) {
        (Some(p), _) => {
            let q = point.unwrap_or_else(|| p.at.point());
            let planar_fraction = planar::fraction_of(line, q);
            let planar_error_m =
                Geodesic.distance(line.point_at(planar_fraction).point(), p.at.point());
            println!(
                "│ position {:.4} = {:.1} m from start (snapped from {:.1} m away; planar maths would say {planar_fraction:.4}, {planar_error_m:.1} m off)",
                p.at.fraction, p.at.metres_from_start, p.offset_m
            );
        }
        (None, Some(f)) => println!(
            "│ position {f:.4} = {:.1} m from start",
            line.fraction_to_metres(f)
        ),
        (None, None) => println!("│ position: not given, so `between` rules are undetermined"),
    }
    println!("│ traveller: {}", describe_situation(s));
    println!("│");
}

fn describe_situation(s: &Situation) -> String {
    let unknown = || "?".to_owned();
    let mut p = vec![
        format!(
            "heading={}",
            s.heading.map_or_else(unknown, |h| h.to_string())
        ),
        format!(
            "mode={}",
            s.mode.as_ref().map_or_else(unknown, ToString::to_string)
        ),
        format!(
            "time={}",
            s.time
                .map_or_else(unknown, |t| t.format("%a %Y-%m-%d %H:%M").to_string())
        ),
    ];
    if !s.using.is_empty() {
        p.push(format!("using={}", s.using.join(",")));
    }
    if !s.recognized.is_empty() {
        p.push(format!("recognized={}", s.recognized.join(",")));
    }
    p.extend(s.vehicle.iter().map(|(k, v)| format!("{k}={v}")));
    p.join(" ")
}

fn print_property<R: Scoped>(
    title: &str,
    res: &Resolution,
    rules: &[R],
    head: impl Fn(&R) -> String,
    value: &str,
) {
    println!(
        "│ {title}  ({} rule{})",
        res.rules.len(),
        plural(res.rules.len())
    );
    if res.rules.is_empty() {
        println!("│   none in data");
    }
    for (e, r) in res.rules.iter().zip(rules) {
        print_rule(e, &head(r), r);
    }
    match res.determining {
        Some(i) => println!("│   ⇒ {value}   [determining rule #{i}]"),
        None => println!("│   ⇒ {value}"),
    }
    if !res.could_override.is_empty() {
        println!(
            "│   ? rule(s) {:?} come later and are undetermined: each would override if its missing facts hold",
            res.could_override
        );
    }
    println!("│");
}

fn print_rule(e: &RuleEvaluation, head: &str, rule: &impl Scoped) {
    let mark = match e.verdict {
        Verdict::Match => "✔",
        Verdict::NoMatch => "✘",
        Verdict::Undetermined => "?",
    };
    println!("│   {mark} #{} {head} {}", e.index, rule.scope_text());
    for c in &e.scopes {
        let m = match c.verdict {
            Verdict::Match => "ok",
            Verdict::NoMatch => "no",
            Verdict::Undetermined => "??",
        };
        println!(
            "│        {m} {:<10} {:<34} {}",
            c.scope,
            truncate(&c.requires, 34),
            c.because
        );
    }
}
