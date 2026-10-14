//! For every `between` boundary in the data, compare the point Overture means (the
//! geodesic linear reference) with the point a planar implementation would use.

use anyhow::Result;
use overrules::lr::planar;
use overrules::{Network, Segment};
use serde_json::json;

use super::print_json;
use crate::render::{short, truncate};

#[derive(clap::Args)]
pub struct Args {
    /// Count segments whose worst boundary moves more than this many metres.
    #[arg(long, default_value_t = 5.0)]
    threshold_m: f64,
    /// How many of the worst segments to list.
    #[arg(long, default_value_t = 10)]
    limit: usize,
}

struct Row<'a> {
    seg: &'a Segment,
    boundaries: usize,
    worst_m: f64,
    worst_fraction: f64,
}

/// Upper bounds, in metres, of the histogram buckets; the last is open-ended.
const BUCKETS: [f64; 4] = [1.0, 5.0, 10.0, 25.0];

pub fn run(net: &Network, a: &Args, json: bool) -> Result<()> {
    let mut rows: Vec<Row<'_>> = Vec::new();
    let mut histogram = [0usize; BUCKETS.len() + 1];
    let (mut total, mut sum_m) = (0usize, 0.0);

    for seg in net.segments() {
        let fractions: Vec<f64> = seg
            .speed_limits
            .iter()
            .filter_map(|r| r.between)
            .chain(seg.access_restrictions.iter().filter_map(|r| r.between))
            .flat_map(|b| [b.start, b.end])
            .collect();
        if fractions.is_empty() {
            continue;
        }
        let mut row = Row {
            seg,
            boundaries: fractions.len(),
            worst_m: 0.0,
            worst_fraction: 0.0,
        };
        for f in fractions {
            let err = planar::error_at(seg.line(), f);
            total += 1;
            sum_m += err;
            histogram[BUCKETS.iter().take_while(|&&b| err >= b).count()] += 1;
            if err > row.worst_m {
                row.worst_m = err;
                row.worst_fraction = f;
            }
        }
        rows.push(row);
    }
    rows.sort_by(|a, b| b.worst_m.total_cmp(&a.worst_m));
    let over = rows.iter().filter(|r| r.worst_m > a.threshold_m).count();
    let mean_m = sum_m / total.max(1) as f64;

    if json {
        let worst: Vec<_> = rows
            .iter()
            .take(a.limit)
            .map(|r| {
                json!({
                    "id": r.seg.id, "name": r.seg.name, "class": r.seg.class, "length_m": r.seg.length_m(),
                    "boundaries": r.boundaries, "worst_error_m": r.worst_m, "at_fraction": r.worst_fraction,
                })
            })
            .collect();
        return print_json(&json!({
            "segments_with_between": rows.len(), "boundaries": total, "mean_error_m": mean_m,
            "histogram_m": {"<1": histogram[0], "1-5": histogram[1], "5-10": histogram[2], "10-25": histogram[3], ">=25": histogram[4]},
            "segments_over_threshold": over, "threshold_m": a.threshold_m, "worst": worst,
        }));
    }

    println!("Planar-vs-geodesic audit of every `between` boundary");
    println!("  segments with between-scoped rules : {}", rows.len());
    println!("  boundary fractions checked         : {total}");
    println!("  mean displacement                  : {mean_m:.2} m");
    println!(
        "  histogram                          : <1 m {}  · 1–5 m {}  · 5–10 m {}  · 10–25 m {}  · ≥25 m {}",
        histogram[0], histogram[1], histogram[2], histogram[3], histogram[4]
    );
    println!(
        "  segments with a boundary > {:.0} m off : {over}",
        a.threshold_m
    );
    println!("\n  worst offenders:");
    println!(
        "  {:<13} {:<44} {:>8}  {:>6}  {:>9}  {:>8}",
        "gers id", "segment", "length", "bounds", "worst Δ", "at frac"
    );
    for r in rows.iter().take(a.limit) {
        println!(
            "  {:<13} {:<44} {:>6.0} m  {:>6}  {:>7.2} m  {:>8.4}",
            short(&r.seg.id),
            truncate(&r.seg.label(), 44),
            r.seg.length_m(),
            r.boundaries,
            r.worst_m,
            r.worst_fraction
        );
    }
    println!(
        "\n  Δ = metres between where Overture's geodesic linear reference lands and where a planar (lon/lat as x/y) one does."
    );
    Ok(())
}
