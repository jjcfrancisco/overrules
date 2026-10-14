use anyhow::{Result, ensure};
use geo::{Distance as _, Geodesic};
use overrules::Network;
use overrules::lr::planar;
use serde_json::json;

use super::print_json;

#[derive(clap::Args)]
pub struct Args {
    /// GERS id, or a unique prefix.
    segment: String,
    /// Linear reference, 0 to 1.
    fraction: f64,
}

pub fn run(net: &Network, a: &Args, json: bool) -> Result<()> {
    ensure!(
        (0.0..=1.0).contains(&a.fraction),
        "fraction must be within 0..=1"
    );
    let s = net.lookup(&a.segment)?;
    let g = s.line().point_at(a.fraction);
    let p = planar::point_at(s.line(), a.fraction);
    let err = Geodesic.distance(g.point(), p);
    if json {
        return print_json(&json!({
            "segment": s.id, "name": s.name, "length_m": s.length_m(),
            "geodesic": g, "planar": {"lon": p.x(), "lat": p.y()}, "planar_error_m": err,
        }));
    }
    println!(
        "{}  {}  length {:.1} m, {} vertices",
        s.id,
        s.label(),
        s.length_m(),
        s.geometry().0.len()
    );
    println!(
        "  fraction {:.4} → {:.1} m from start",
        a.fraction, g.metres_from_start
    );
    println!("  geodesic point : {:.7}, {:.7}", g.lon, g.lat);
    println!(
        "  planar point   : {:.7}, {:.7}   ({err:.2} m away)",
        p.x(),
        p.y()
    );
    Ok(())
}
