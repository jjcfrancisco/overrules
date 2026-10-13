use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{Local, NaiveDateTime};
use clap::{Args, Parser, Subcommand};
use geo::Point;
use serde_json::json;

use gers_lr::lr;
use gers_lr::model::Segment;
use gers_lr::rules::{self, PropertyResolution, Situation, Verdict};
use gers_lr::{reader, Network};

#[derive(Parser)]
#[command(
    name = "gers-lr",
    version,
    about = "Geodesic linear referencing + rule resolver for Overture segments"
)]
struct Cli {
    /// Overture segment Parquet file (a bbox extract is plenty).
    #[arg(long, global = true, default_value = "data/seville/segment.parquet")]
    segments: PathBuf,
    /// Emit JSON instead of a human-readable report.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Summarise what the extract contains and how much of it is scoped.
    Stats,
    /// Find segments by (part of a) name or by GERS id prefix.
    Find {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, default_value_t = 15)]
        limit: usize,
    },
    /// Snap a lon/lat to the nearest segments: geodesic fraction vs the planar answer.
    Locate {
        #[arg(long, allow_negative_numbers = true)]
        lon: f64,
        #[arg(long, allow_negative_numbers = true)]
        lat: f64,
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    /// Geodesic fraction → point on a segment (and the planar point for comparison).
    At {
        #[arg(long)]
        segment: String,
        #[arg(long)]
        fraction: f64,
    },
    /// Resolve the rules in force for a traveller at a position on a segment.
    Resolve(ResolveArgs),
    /// Dump a segment (and its scoped rules) as GeoJSON.
    Show {
        #[arg(long)]
        segment: String,
    },
    /// City-wide check: how far would every `between` boundary move if LR were computed planar?
    Audit {
        /// Only list segments whose worst boundary error exceeds this many metres.
        #[arg(long, default_value_t = 5.0)]
        threshold_m: f64,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
}

#[derive(Args)]
struct ResolveArgs {
    /// GERS id (or unique prefix). Omit to pick the nearest segment to --lon/--lat.
    #[arg(long)]
    segment: Option<String>,
    /// Position as a geodesic fraction 0..1.
    #[arg(long)]
    at: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    lon: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    lat: Option<f64>,
    /// forward | backward (relative to the segment's digitising direction).
    #[arg(long)]
    heading: Option<String>,
    /// foot | bicycle | vehicle | motor_vehicle | car | truck | motorcycle | bus | hgv | hov | emergency
    #[arg(long)]
    mode: Option<String>,
    /// Local time, e.g. 2026-10-13T08:30, or "now".
    #[arg(long)]
    time: Option<String>,
    /// Purpose of use: as_customer | at_destination | to_deliver | to_farm | for_forestry (repeatable).
    #[arg(long = "using")]
    using: Vec<String>,
    /// Status: as_permitted | as_private | as_disabled | as_employee | as_student (repeatable).
    #[arg(long)]
    recognized: Vec<String>,
    /// Vehicle dimension, e.g. weight=3.5 (t), height=4.2 (m), length=12 (m), width=2.5 (m), axle_count=3.
    #[arg(long)]
    vehicle: Vec<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let t0 = Instant::now();
    let segments = reader::read_segments(&cli.segments)?;
    let net = Network::new(segments);
    if !cli.json {
        eprintln!(
            "loaded {} segments from {} in {:.1}s",
            net.segments.len(),
            cli.segments.display(),
            t0.elapsed().as_secs_f64()
        );
    }
    match cli.cmd {
        Cmd::Stats => stats(&net, cli.json),
        Cmd::Find { name, id, limit } => find(&net, name, id, limit, cli.json),
        Cmd::Locate { lon, lat, limit } => locate(&net, lon, lat, limit, cli.json),
        Cmd::At { segment, fraction } => at(&net, &segment, fraction, cli.json),
        Cmd::Resolve(a) => resolve(&net, a, cli.json),
        Cmd::Show { segment } => show(&net, &segment),
        Cmd::Audit { threshold_m, limit } => audit(&net, threshold_m, limit, cli.json),
    }
}

fn one<'a>(net: &'a Network, needle: &str) -> Result<&'a Segment> {
    let hits = net.find_id(needle);
    match hits.len() {
        0 => bail!("no segment with id (prefix) {needle}"),
        1 => Ok(hits[0]),
        n => bail!("{n} segments match prefix {needle}; be more specific"),
    }
}

fn short(id: &str) -> &str {
    &id[..id.len().min(13)]
}

// ---------------------------------------------------------------------------

/// For every `between` boundary in the dataset, compare the point Overture means (geodesic
/// fraction) with the point a planar implementation would place there.
fn audit(net: &Network, threshold_m: f64, limit: usize, as_json: bool) -> Result<()> {
    use geo::{Distance, Euclidean, Geodesic, InterpolatableLine};

    struct Row<'a> {
        seg: &'a Segment,
        boundaries: usize,
        worst_m: f64,
        worst_fraction: f64,
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut total_boundaries = 0usize;
    let mut buckets = [0usize; 5]; // <1 m, 1-5 m, 5-10 m, 10-25 m, >=25 m
    let mut sum_m = 0.0;

    for s in &net.segments {
        let mut fractions: Vec<f64> = Vec::new();
        if let Some(r) = &s.rec.speed_limits {
            fractions.extend(
                r.iter()
                    .filter_map(|x| x.between.as_ref())
                    .flatten()
                    .copied(),
            );
        }
        if let Some(r) = &s.rec.access_restrictions {
            fractions.extend(
                r.iter()
                    .filter_map(|x| x.between.as_ref())
                    .flatten()
                    .copied(),
            );
        }
        if fractions.is_empty() || s.geometry.0.len() < 2 {
            continue;
        }
        let mut worst_m = 0.0;
        let mut worst_fraction = 0.0;
        for f in &fractions {
            let f = f.clamp(0.0, 1.0);
            let g = lr::point_at(&s.geometry, &s.measure, f);
            let Some(p) = s.geometry.point_at_ratio_from_start(&Euclidean, f) else {
                continue;
            };
            let err = Geodesic.distance(Point::new(g.lon, g.lat), p);
            total_boundaries += 1;
            sum_m += err;
            let b = match err {
                e if e < 1.0 => 0,
                e if e < 5.0 => 1,
                e if e < 10.0 => 2,
                e if e < 25.0 => 3,
                _ => 4,
            };
            buckets[b] += 1;
            if err > worst_m {
                worst_m = err;
                worst_fraction = f;
            }
        }
        rows.push(Row {
            seg: s,
            boundaries: fractions.len(),
            worst_m,
            worst_fraction,
        });
    }
    rows.sort_by(|a, b| b.worst_m.partial_cmp(&a.worst_m).unwrap());
    let over = rows.iter().filter(|r| r.worst_m > threshold_m).count();

    if as_json {
        let worst: Vec<_> = rows
            .iter()
            .take(limit)
            .map(|r| json!({"id": r.seg.id(), "name": r.seg.name(), "class": r.seg.class(), "length_m": r.seg.measure.length_m(),
                            "boundaries": r.boundaries, "worst_error_m": r.worst_m, "at_fraction": r.worst_fraction}))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "segments_with_between": rows.len(), "boundaries": total_boundaries,
                "mean_error_m": sum_m / total_boundaries.max(1) as f64,
                "histogram_m": {"<1": buckets[0], "1-5": buckets[1], "5-10": buckets[2], "10-25": buckets[3], ">=25": buckets[4]},
                "segments_over_threshold": over, "threshold_m": threshold_m, "worst": worst
            }))?
        );
        return Ok(());
    }

    println!("Planar-vs-geodesic audit of every `between` boundary");
    println!("  segments with between-scoped rules : {}", rows.len());
    println!(
        "  boundary fractions checked         : {}",
        total_boundaries
    );
    println!(
        "  mean displacement                  : {:.2} m",
        sum_m / total_boundaries.max(1) as f64
    );
    println!(
        "  histogram                          : <1 m {}  · 1–5 m {}  · 5–10 m {}  · 10–25 m {}  · ≥25 m {}",
        buckets[0], buckets[1], buckets[2], buckets[3], buckets[4]
    );
    println!("  segments with a boundary > {threshold_m:.0} m off : {over}");
    println!("\n  worst offenders:");
    println!(
        "  {:<13} {:<44} {:>8}  {:>6}  {:>9}  {:>8}",
        "gers id", "segment", "length", "bounds", "worst Δ", "at frac"
    );
    for r in rows.iter().take(limit) {
        println!(
            "  {:<13} {:<44} {:>6.0} m  {:>6}  {:>7.2} m  {:>8.4}",
            short(r.seg.id()),
            truncate(&r.seg.label(), 44),
            r.seg.measure.length_m(),
            r.boundaries,
            r.worst_m,
            r.worst_fraction
        );
    }
    println!("\n  Δ = metres along the road between where Overture's geodesic fraction lands and where a planar (lon/lat-as-xy) fraction lands.");
    Ok(())
}

fn stats(net: &Network, as_json: bool) -> Result<()> {
    let mut by_class: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_m = 0.0;
    let (mut sl, mut sl_when, mut sl_between, mut sl_during, mut sl_mode) = (0, 0, 0, 0, 0);
    let (
        mut ar,
        mut ar_heading,
        mut ar_during,
        mut ar_mode,
        mut ar_between,
        mut ar_using,
        mut ar_recognized,
        mut ar_vehicle,
    ) = (0, 0, 0, 0, 0, 0, 0, 0);
    let (mut pt, mut pt_when) = (0, 0);
    let (mut seg_sl, mut seg_ar, mut seg_pt, mut seg_any_between) = (0, 0, 0, 0);
    let mut connectors = 0;

    for s in &net.segments {
        *by_class
            .entry(format!("{}/{}", s.subtype(), s.class()))
            .or_default() += 1;
        total_m += s.measure.length_m();
        connectors += s.rec.connectors.as_ref().map(|c| c.len()).unwrap_or(0);
        let mut any_between = false;
        if let Some(r) = &s.rec.speed_limits {
            seg_sl += 1;
            for x in r {
                sl += 1;
                if let Some(w) = &x.when {
                    sl_when += 1;
                    sl_during += w.during.is_some() as usize;
                    sl_mode += w.mode.is_some() as usize;
                }
                if x.between.is_some() {
                    sl_between += 1;
                    any_between = true;
                }
            }
        }
        if let Some(r) = &s.rec.access_restrictions {
            seg_ar += 1;
            for x in r {
                ar += 1;
                if let Some(w) = &x.when {
                    ar_heading += w.heading.is_some() as usize;
                    ar_during += w.during.is_some() as usize;
                    ar_mode += w.mode.is_some() as usize;
                    ar_using += w.using.is_some() as usize;
                    ar_recognized += w.recognized.is_some() as usize;
                    ar_vehicle += w.vehicle.is_some() as usize;
                }
                if x.between.is_some() {
                    ar_between += 1;
                    any_between = true;
                }
            }
        }
        if let Some(r) = &s.rec.prohibited_transitions {
            seg_pt += 1;
            for x in r {
                pt += 1;
                pt_when += x.when.is_some() as usize;
            }
        }
        seg_any_between += any_between as usize;
    }

    let n = net.segments.len();
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "segments": n, "total_length_km": total_m / 1000.0, "connector_refs": connectors,
                "by_class": by_class,
                "speed_limits": {"rules": sl, "segments": seg_sl, "with_when": sl_when, "with_during": sl_during, "with_mode": sl_mode, "with_between": sl_between},
                "access_restrictions": {"rules": ar, "segments": seg_ar, "heading": ar_heading, "during": ar_during, "mode": ar_mode, "using": ar_using, "recognized": ar_recognized, "vehicle": ar_vehicle, "between": ar_between},
                "prohibited_transitions": {"rules": pt, "segments": seg_pt, "with_when": pt_when},
                "segments_with_any_between": seg_any_between,
            }))?
        );
        return Ok(());
    }
    println!(
        "Segments: {n}  ·  {:.1} km of geometry  ·  {connectors} connector references",
        total_m / 1000.0
    );
    println!("\nBy subtype/class:");
    let mut classes: Vec<_> = by_class.into_iter().collect();
    classes.sort_by_key(|a| std::cmp::Reverse(a.1));
    for (c, k) in classes {
        println!("  {k:>7}  {c}");
    }
    println!("\nspeed_limits:           {sl} rules on {seg_sl} segments  ·  {sl_when} with `when` ({sl_during} during, {sl_mode} mode)  ·  {sl_between} with `between`");
    println!("access_restrictions:    {ar} rules on {seg_ar} segments  ·  heading {ar_heading}, during {ar_during}, mode {ar_mode}, using {ar_using}, recognized {ar_recognized}, vehicle {ar_vehicle}  ·  {ar_between} with `between`");
    println!("prohibited_transitions: {pt} rules on {seg_pt} segments  ·  {pt_when} with `when`");
    println!("\n{seg_any_between} segments carry at least one linearly-referenced (`between`) rule → these need geodesic LR to interpret correctly.");
    Ok(())
}

fn find(
    net: &Network,
    name: Option<String>,
    id: Option<String>,
    limit: usize,
    as_json: bool,
) -> Result<()> {
    let hits: Vec<&Segment> = match (&name, &id) {
        (Some(n), _) => net.find_name(n),
        (None, Some(i)) => net.find_id(i),
        _ => bail!("give --name or --id"),
    };
    if as_json {
        let v: Vec<_> = hits.iter().take(limit).map(|s| summary_json(s)).collect();
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    println!(
        "{} match(es){}",
        hits.len(),
        if hits.len() > limit {
            format!(", showing {limit}")
        } else {
            String::new()
        }
    );
    for s in hits.iter().take(limit) {
        println!(
            "  {}  {:<48} {:>7.0} m  {}",
            s.id(),
            truncate(&s.label(), 48),
            s.measure.length_m(),
            rule_flags(s)
        );
    }
    Ok(())
}

fn rule_flags(s: &Segment) -> String {
    let mut f = Vec::new();
    if let Some(r) = &s.rec.access_restrictions {
        f.push(format!("access×{}", r.len()));
    }
    if let Some(r) = &s.rec.speed_limits {
        f.push(format!("speed×{}", r.len()));
    }
    if let Some(r) = &s.rec.prohibited_transitions {
        f.push(format!("no-turn×{}", r.len()));
    }
    let between = s
        .rec
        .access_restrictions
        .iter()
        .flatten()
        .any(|r| r.between.is_some())
        || s.rec
            .speed_limits
            .iter()
            .flatten()
            .any(|r| r.between.is_some());
    if between {
        f.push("LR".into());
    }
    f.join(" ")
}

fn summary_json(s: &Segment) -> serde_json::Value {
    json!({
        "id": s.id(), "name": s.name(), "subtype": s.subtype(), "class": s.class(),
        "length_m": s.measure.length_m(),
        "access_restrictions": s.rec.access_restrictions, "speed_limits": s.rec.speed_limits,
        "prohibited_transitions": s.rec.prohibited_transitions,
    })
}

fn locate(net: &Network, lon: f64, lat: f64, limit: usize, as_json: bool) -> Result<()> {
    let hits = net.nearest(lon, lat, limit, 40);
    if as_json {
        let v: Vec<_> = hits
            .iter()
            .map(|h| json!({"id": h.segment.id(), "name": h.segment.name(), "class": h.segment.class(), "projection": h.projection}))
            .collect();
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    println!("Nearest segments to ({lon}, {lat}):");
    println!(
        "  {:<13} {:<44} {:>7}  {:>9}  {:>9}  {:>9}",
        "gers id", "segment", "offset", "geodesic", "planar", "Δ planar"
    );
    for h in hits {
        let p = &h.projection;
        println!(
            "  {:<13} {:<44} {:>5.1} m  {:>9.4}  {:>9.4}  {:>7.2} m",
            short(h.segment.id()),
            truncate(&h.segment.label(), 44),
            p.offset_m,
            p.geodesic.fraction,
            p.planar_fraction,
            p.planar_error_m
        );
    }
    println!("\n  geodesic = WGS84 fraction (Overture's definition); planar = lon/lat-as-xy line_locate_point; Δ = metres between the two answers.");
    Ok(())
}

fn at(net: &Network, segment: &str, fraction: f64, as_json: bool) -> Result<()> {
    let s = one(net, segment)?;
    let g = lr::point_at(&s.geometry, &s.measure, fraction);
    let planar =
        geo::InterpolatableLine::point_at_ratio_from_start(&s.geometry, &geo::Euclidean, fraction)
            .unwrap_or(Point::new(g.lon, g.lat));
    let err = geo::Distance::distance(&geo::Geodesic, planar, Point::new(g.lon, g.lat));
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"segment": s.id(), "name": s.name(), "length_m": s.measure.length_m(), "geodesic": g, "planar": {"lon": planar.x(), "lat": planar.y()}, "planar_error_m": err})
            )?
        );
        return Ok(());
    }
    println!(
        "{}  {}  length {:.1} m, {} vertices",
        s.id(),
        s.label(),
        s.measure.length_m(),
        s.geometry.0.len()
    );
    println!(
        "  fraction {fraction:.4} → {:.1} m from start",
        g.metres_from_start
    );
    println!("  geodesic point : {:.7}, {:.7}", g.lon, g.lat);
    println!(
        "  planar point   : {:.7}, {:.7}   ({err:.2} m away)",
        planar.x(),
        planar.y()
    );
    Ok(())
}

fn parse_time(s: &str) -> Result<NaiveDateTime> {
    if s.eq_ignore_ascii_case("now") {
        return Ok(Local::now().naive_local());
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(t) = NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(t);
        }
    }
    bail!("cannot parse time {s:?}; use e.g. 2026-10-13T08:30 or now")
}

fn resolve(net: &Network, a: ResolveArgs, as_json: bool) -> Result<()> {
    // 1. Which segment, and where on it.
    let (seg, fraction, snapped): (&Segment, Option<f64>, Option<lr::Projection>) =
        match (&a.segment, a.lon, a.lat) {
            (Some(id), None, None) => (one(net, id)?, a.at, None),
            (Some(id), Some(lon), Some(lat)) => {
                let s = one(net, id)?;
                let p = lr::project(&s.geometry, &s.measure, Point::new(lon, lat));
                (s, Some(p.geodesic.fraction), Some(p))
            }
            (None, Some(lon), Some(lat)) => {
                let hit = net
                    .nearest(lon, lat, 1, 40)
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("no segments"))?;
                (
                    hit.segment,
                    Some(hit.projection.geodesic.fraction),
                    Some(hit.projection),
                )
            }
            _ => bail!("give --segment, or --lon/--lat (optionally both)"),
        };
    if let Some(f) = fraction {
        if !(0.0..=1.0).contains(&f) {
            bail!("--at must be within 0..1");
        }
    }

    // 2. The situation.
    let mut vehicle = BTreeMap::new();
    for v in &a.vehicle {
        let (k, val) = v
            .split_once('=')
            .ok_or_else(|| anyhow!("--vehicle expects dimension=value, got {v}"))?;
        vehicle.insert(
            k.trim().to_string(),
            val.trim()
                .parse::<f64>()
                .with_context(|| format!("vehicle value in {v}"))?,
        );
    }
    if let Some(h) = &a.heading {
        if h != "forward" && h != "backward" {
            bail!("--heading must be forward or backward");
        }
    }
    if let Some(m) = &a.mode {
        if rules::mode_lineage(m).is_empty() {
            bail!("unknown --mode {m}");
        }
    }
    let situation = Situation {
        fraction,
        heading: a.heading.clone(),
        mode: a.mode.clone(),
        time: a.time.as_deref().map(parse_time).transpose()?,
        using: a.using.clone(),
        recognized: a.recognized.clone(),
        vehicle,
    };

    // 3. Resolve.
    let access = rules::resolve_access(seg, &situation);
    let speed = rules::resolve_speed(seg, &situation);
    let transitions = rules::resolve_transitions(seg, &situation, |id| net.describe(id));

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "segment": {"id": seg.id(), "name": seg.name(), "subtype": seg.subtype(), "class": seg.class(), "length_m": seg.measure.length_m()},
                "position": snapped.as_ref().map(|p| json!(p)).unwrap_or(json!({"fraction": fraction})),
                "situation": situation,
                "access": access, "speed_limit": speed, "prohibited_transitions": transitions,
            }))?
        );
        return Ok(());
    }

    // 4. Report.
    println!("┌ {}  {}", seg.id(), seg.label());
    println!(
        "│ length {:.1} m · {} vertices · {} connectors{}",
        seg.measure.length_m(),
        seg.geometry.0.len(),
        seg.rec.connectors.as_ref().map(|c| c.len()).unwrap_or(0),
        routes(seg)
    );
    match (&snapped, fraction) {
        (Some(p), _) => println!(
            "│ position {:.4} = {:.1} m from start (query snapped from {:.1} m away; planar LR would say {:.4}, {:.1} m off)",
            p.geodesic.fraction, p.geodesic.metres_from_start, p.offset_m, p.planar_fraction, p.planar_error_m
        ),
        (None, Some(f)) => println!("│ position {:.4} = {:.1} m from start", f, seg.measure.fraction_to_metres(f)),
        (None, None) => println!("│ position: whole segment (no --at; `between` rules will be undetermined)"),
    }
    println!("│ traveller: {}", describe_situation(&situation));
    println!("│");
    print_property(&access, "ACCESS");
    print_property(&speed, "SPEED LIMIT");
    println!(
        "│ PROHIBITED TRANSITIONS  ({} rule{})",
        transitions.rules.len(),
        plural(transitions.rules.len())
    );
    if transitions.rules.is_empty() {
        println!("│   none in data");
    }
    for r in &transitions.rules {
        print_rule(r);
    }
    if !transitions.in_force.is_empty() {
        println!(
            "│   ⇒ {} prohibited turn(s) in force here: {:?}",
            transitions.in_force.len(),
            transitions.in_force
        );
    }
    if !transitions.could_apply.is_empty() {
        println!(
            "│   ? {:?} could apply if the missing facts hold",
            transitions.could_apply
        );
    }
    println!("└ semantics: all scopes must hold (AND); the last matching rule in the list wins; otherwise the property default applies.");
    Ok(())
}

fn routes(seg: &Segment) -> String {
    let refs: Vec<String> = seg
        .rec
        .routes
        .iter()
        .flatten()
        .filter_map(|r| r.reference.clone().or_else(|| r.name.clone()))
        .collect();
    if refs.is_empty() {
        String::new()
    } else {
        format!(" · routes [{}]", refs.join(", "))
    }
}

fn describe_situation(s: &Situation) -> String {
    let mut p = Vec::new();
    p.push(format!("heading={}", s.heading.as_deref().unwrap_or("?")));
    p.push(format!("mode={}", s.mode.as_deref().unwrap_or("?")));
    p.push(format!(
        "time={}",
        s.time
            .map(|t| t.format("%a %Y-%m-%d %H:%M").to_string())
            .unwrap_or("?".into())
    ));
    if !s.using.is_empty() {
        p.push(format!("using={}", s.using.join(",")));
    }
    if !s.recognized.is_empty() {
        p.push(format!("recognized={}", s.recognized.join(",")));
    }
    for (k, v) in &s.vehicle {
        p.push(format!("{k}={v}"));
    }
    p.join(" ")
}

fn print_property(r: &PropertyResolution, title: &str) {
    println!(
        "│ {title}  ({} rule{})",
        r.rules.len(),
        plural(r.rules.len())
    );
    if r.rules.is_empty() {
        println!("│   none in data");
    }
    for e in &r.rules {
        print_rule(e);
    }
    match r.determining {
        Some(i) => println!("│   ⇒ {}   [determining rule #{i}]", r.value),
        None => println!("│   ⇒ {}", r.value),
    }
    if !r.could_override.is_empty() {
        println!("│   ? rule(s) {:?} come later and are undetermined — they would override if their missing facts hold", r.could_override);
    }
    if let Some(n) = &r.note {
        println!("│   ({n})");
    }
    println!("│");
}

fn print_rule(e: &rules::RuleEvaluation) {
    let mark = match e.verdict {
        Verdict::Match => "✔",
        Verdict::NoMatch => "✘",
        Verdict::Undetermined => "?",
    };
    println!("│   {mark} #{} {}", e.index, e.rule);
    for s in &e.scopes {
        let m = match s.verdict {
            Verdict::Match => "ok",
            Verdict::NoMatch => "no",
            Verdict::Undetermined => "??",
        };
        println!(
            "│        {m} {:<10} {:<34} {}",
            s.scope,
            truncate(&s.requires, 34),
            s.because
        );
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let cut: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

fn show(net: &Network, segment: &str) -> Result<()> {
    let s = one(net, segment)?;
    let coords: Vec<[f64; 2]> = s.geometry.0.iter().map(|c| [c.x, c.y]).collect();
    let mut features = vec![json!({
        "type": "Feature",
        "properties": summary_json(s),
        "geometry": {"type": "LineString", "coordinates": coords},
    })];
    for c in s.rec.connectors.iter().flatten() {
        let p = lr::point_at(&s.geometry, &s.measure, c.at);
        features.push(json!({
            "type": "Feature",
            "properties": {"connector_id": c.connector_id, "at": c.at, "metres_from_start": p.metres_from_start},
            "geometry": {"type": "Point", "coordinates": [p.lon, p.lat]},
        }));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"type": "FeatureCollection", "features": features}))?
    );
    Ok(())
}
