use anyhow::Result;
use geo::{Distance as _, Geodesic, Point};
use overrules::Network;
use overrules::lr::planar;
use serde_json::json;

use super::print_json;
use crate::render::{short, truncate};

#[derive(clap::Args)]
pub struct Args {
    /// Longitude.
    #[arg(long, allow_negative_numbers = true)]
    lon: f64,
    /// Latitude.
    #[arg(long, allow_negative_numbers = true)]
    lat: f64,
    /// How many segments to list.
    #[arg(long, default_value_t = 5)]
    limit: usize,
}

pub fn run(net: &Network, a: &Args, json: bool) -> Result<()> {
    let q = Point::new(a.lon, a.lat);
    let rows: Vec<_> = net
        .nearest(q, a.limit)
        .into_iter()
        .map(|h| {
            let line = h.segment.line();
            let planar_fraction = planar::fraction_of(line, q);
            // If the planar fraction were stored and read back geodesically, as Overture
            // defines it, this is how far along the road from the true point it would land.
            let planar_error_m = Geodesic.distance(
                line.point_at(planar_fraction).point(),
                h.projection.at.point(),
            );
            (h, planar_fraction, planar_error_m)
        })
        .collect();
    if json {
        let v: Vec<_> = rows
            .iter()
            .map(|(h, pf, pe)| {
                json!({
                    "id": h.segment.id, "name": h.segment.name, "class": h.segment.class,
                    "projection": h.projection, "planar_fraction": pf, "planar_error_m": pe,
                })
            })
            .collect();
        return print_json(&v);
    }
    println!("Nearest segments to ({}, {}):", a.lon, a.lat);
    println!(
        "  {:<13} {:<44} {:>7}  {:>9}  {:>9}  {:>9}",
        "gers id", "segment", "offset", "geodesic", "planar", "Δ planar"
    );
    for (h, planar_fraction, planar_error_m) in &rows {
        println!(
            "  {:<13} {:<44} {:>5.1} m  {:>9.4}  {:>9.4}  {:>7.2} m",
            short(&h.segment.id),
            truncate(&h.segment.label(), 44),
            h.projection.offset_m,
            h.projection.at.fraction,
            planar_fraction,
            planar_error_m
        );
    }
    println!(
        "\n  geodesic = WGS84 linear reference (Overture's definition); planar = lon/lat as x/y; Δ = metres between the two."
    );
    Ok(())
}
