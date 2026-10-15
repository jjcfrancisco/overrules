//! How far would every `between` boundary move if linear references were computed in the
//! plane (lon/lat as x/y) rather than geodesically, as Overture defines them?
//!
//! ```sh
//! cargo run --release --example planar_audit --features parquet -- data/nyc/segment.parquet
//! ```

use std::error::Error;

use overrules::lr::planar;

/// Upper bounds of the histogram buckets, in metres; the last bucket is open-ended.
const BUCKETS: [f64; 4] = [1.0, 5.0, 10.0, 25.0];

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: planar_audit <segments file>")?;
    let segments = overrules::io::read_path(&path)?;

    let mut histogram = [0usize; BUCKETS.len() + 1];
    let (mut checked, mut sum_m) = (0usize, 0.0);
    let mut worst: Vec<(f64, f64, &overrules::Segment)> = Vec::new();

    for seg in &segments {
        let fractions: Vec<f64> = seg
            .speed_limits
            .iter()
            .filter_map(|r| r.between)
            .chain(seg.access_restrictions.iter().filter_map(|r| r.between))
            .flat_map(|b| [b.start, b.end])
            .collect();
        let mut seg_worst = (0.0, 0.0);
        for f in fractions.iter().copied() {
            let err = planar::error_at(seg.line(), f);
            checked += 1;
            sum_m += err;
            histogram[BUCKETS.iter().take_while(|&&b| err >= b).count()] += 1;
            if err > seg_worst.0 {
                seg_worst = (err, f);
            }
        }
        if !fractions.is_empty() {
            worst.push((seg_worst.0, seg_worst.1, seg));
        }
    }
    worst.sort_by(|a, b| b.0.total_cmp(&a.0));

    println!("segments with between-scoped rules : {}", worst.len());
    println!("boundary fractions checked         : {checked}");
    println!(
        "mean displacement                  : {:.2} m",
        sum_m / checked.max(1) as f64
    );
    println!(
        "histogram                          : <1 m {}  · 1–5 m {}  · 5–10 m {}  · 10–25 m {}  · ≥25 m {}",
        histogram[0], histogram[1], histogram[2], histogram[3], histogram[4]
    );
    println!(
        "segments with a boundary > 5 m off : {}",
        worst.iter().filter(|w| w.0 > 5.0).count()
    );
    println!("\nworst:");
    for (err, f, seg) in worst.iter().take(5) {
        println!(
            "  {err:>6.2} m at {f:.4}  {}  {}  ({:.0} m)",
            seg.id,
            seg.label(),
            seg.length_m()
        );
    }
    Ok(())
}
