//! Geodesic linear referencing on WGS84, as Overture specifies it:
//!
//! > linear_ref = geodetic_distance_along_segment_from_start / total_geodetic_length
//!
//! Two primitives are provided:
//! * [`point_at`] — fraction → point (the "direct" problem), and
//! * [`project`] — point → fraction (the "inverse" problem), which the Rust
//!   ecosystem lacks: `geo`'s `LineLocatePoint` is Euclidean on raw lon/lat.
//!
//! Projection is solved per sub-segment by bracketed golden-section search of
//! the geodesic distance from the query point to points along the geodesic arc
//! between consecutive vertices (Karney direct/inverse under the hood), with a
//! triangle-inequality bound to skip hopeless sub-segments.

use geo::{Distance, Geodesic, InterpolatePoint, LineLocatePoint, LineString, Point};
use serde::Serialize;

/// Cumulative geodesic length (metres) at each vertex of a linestring.
#[derive(Debug, Clone)]
pub struct Measure {
    pub cumulative_m: Vec<f64>,
}

impl Measure {
    pub fn new(ls: &LineString<f64>) -> Self {
        let pts: Vec<Point<f64>> = ls.points().collect();
        let mut cumulative_m = Vec::with_capacity(pts.len());
        let mut acc = 0.0;
        cumulative_m.push(0.0);
        for w in pts.windows(2) {
            acc += Geodesic.distance(w[0], w[1]);
            cumulative_m.push(acc);
        }
        Self { cumulative_m }
    }

    pub fn length_m(&self) -> f64 {
        *self.cumulative_m.last().unwrap_or(&0.0)
    }

    pub fn fraction_to_metres(&self, fraction: f64) -> f64 {
        fraction.clamp(0.0, 1.0) * self.length_m()
    }

    pub fn metres_to_fraction(&self, metres: f64) -> f64 {
        let len = self.length_m();
        if len <= 0.0 {
            0.0
        } else {
            (metres / len).clamp(0.0, 1.0)
        }
    }
}

/// Where a position along a segment falls.
#[derive(Debug, Clone, Serialize)]
pub struct Located {
    pub fraction: f64,
    pub metres_from_start: f64,
    pub lon: f64,
    pub lat: f64,
    /// Index of the vertex that starts the sub-segment containing the point.
    pub vertex_index: usize,
}

/// Geodesic fraction → point on the linestring.
pub fn point_at(ls: &LineString<f64>, m: &Measure, fraction: f64) -> Located {
    let pts: Vec<Point<f64>> = ls.points().collect();
    let target = m.fraction_to_metres(fraction);
    if pts.len() < 2 {
        let p = pts.first().copied().unwrap_or(Point::new(0.0, 0.0));
        return Located {
            fraction: 0.0,
            metres_from_start: 0.0,
            lon: p.x(),
            lat: p.y(),
            vertex_index: 0,
        };
    }
    // Find the sub-segment [i, i+1] whose cumulative range contains `target`.
    let mut i = match m
        .cumulative_m
        .binary_search_by(|c| c.partial_cmp(&target).unwrap())
    {
        Ok(idx) => idx,
        Err(idx) => idx.saturating_sub(1),
    };
    if i >= pts.len() - 1 {
        i = pts.len() - 2;
    }
    let d = (target - m.cumulative_m[i]).max(0.0);
    let p = Geodesic.point_at_distance_between(pts[i], pts[i + 1], d);
    Located {
        fraction: m.metres_to_fraction(target),
        metres_from_start: target,
        lon: p.x(),
        lat: p.y(),
        vertex_index: i,
    }
}

/// Result of projecting a point onto a segment.
#[derive(Debug, Clone, Serialize)]
pub struct Projection {
    pub geodesic: Located,
    /// Geodesic distance from the query point to the snapped point, in metres.
    pub offset_m: f64,
    /// What a naive planar (lon/lat-as-xy) `line_locate_point` would have said.
    pub planar_fraction: f64,
    /// If the planar fraction were stored in the data and read back per Overture's geodesic
    /// definition, this is how many metres along the road it would land from the true point.
    pub planar_error_m: f64,
}

/// Point → geodesic fraction along the linestring (closest point on the WGS84 ellipsoid).
pub fn project(ls: &LineString<f64>, m: &Measure, q: Point<f64>) -> Projection {
    let pts: Vec<Point<f64>> = ls.points().collect();
    assert!(pts.len() >= 2, "linestring needs at least two vertices");

    let mut best_d = f64::INFINITY;
    let mut best_metres = 0.0;
    let mut best_pt = pts[0];
    let mut best_i = 0usize;

    // Distances from q to every vertex, reused for the pruning bound.
    let dv: Vec<f64> = pts.iter().map(|p| Geodesic.distance(q, *p)).collect();

    for i in 0..pts.len() - 1 {
        let seg_len = m.cumulative_m[i + 1] - m.cumulative_m[i];
        // Triangle inequality: no point on this arc can be closer than this.
        let lower_bound = dv[i].min(dv[i + 1]) - seg_len;
        if lower_bound >= best_d {
            continue;
        }
        let (t, d, p) = if seg_len <= 1e-9 {
            (0.0, dv[i], pts[i])
        } else {
            golden_section(q, pts[i], pts[i + 1], seg_len)
        };
        if d < best_d {
            best_d = d;
            best_metres = m.cumulative_m[i] + t * seg_len;
            best_pt = p;
            best_i = i;
        }
    }

    let geodesic = Located {
        fraction: m.metres_to_fraction(best_metres),
        metres_from_start: best_metres,
        lon: best_pt.x(),
        lat: best_pt.y(),
        vertex_index: best_i,
    };

    // The naive answer, for comparison: where does its fraction land when read geodesically?
    let planar_fraction = ls.line_locate_point(&q).unwrap_or(0.0);
    let planar_as_read = point_at(ls, m, planar_fraction);
    let planar_error_m =
        Geodesic.distance(Point::new(planar_as_read.lon, planar_as_read.lat), best_pt);

    Projection {
        geodesic,
        offset_m: best_d,
        planar_fraction,
        planar_error_m,
    }
}

/// Minimise geodesic distance from `q` to the geodesic arc a→b over t ∈ [0, 1].
/// Returns (t, distance_m, point).
fn golden_section(q: Point<f64>, a: Point<f64>, b: Point<f64>, len: f64) -> (f64, f64, Point<f64>) {
    let f = |t: f64| -> (f64, Point<f64>) {
        let p = Geodesic.point_at_distance_between(a, b, t * len);
        (Geodesic.distance(q, p), p)
    };
    let inv_phi = (5f64.sqrt() - 1.0) / 2.0;
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    let mut c = hi - inv_phi * (hi - lo);
    let mut d = lo + inv_phi * (hi - lo);
    let (mut fc, _) = f(c);
    let (mut fd, _) = f(d);
    // Stop when the bracket is below 0.1 mm along the arc.
    let tol = (1e-4 / len.max(1e-9)).min(0.5);
    let mut iters = 0;
    while (hi - lo) > tol && iters < 200 {
        if fc < fd {
            hi = d;
            d = c;
            fd = fc;
            c = hi - inv_phi * (hi - lo);
            fc = f(c).0;
        } else {
            lo = c;
            c = d;
            fc = fd;
            d = lo + inv_phi * (hi - lo);
            fd = f(d).0;
        }
        iters += 1;
    }
    let t = 0.5 * (lo + hi);
    // Also check the endpoints, in case the minimum is at a vertex.
    let (ft, pt) = f(t);
    let (f0, p0) = f(0.0);
    let (f1, p1) = f(1.0);
    if f0 <= ft && f0 <= f1 {
        (0.0, f0, p0)
    } else if f1 <= ft {
        (1.0, f1, p1)
    } else {
        (t, ft, pt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::coord;

    fn seville_line() -> LineString<f64> {
        // Roughly 1.1 km east then 1.1 km north around Seville (37.39°N).
        LineString::new(vec![
            coord! { x: -5.9950, y: 37.3900 },
            coord! { x: -5.9825, y: 37.3900 },
            coord! { x: -5.9825, y: 37.4000 },
        ])
    }

    #[test]
    fn round_trip_fraction() {
        let ls = seville_line();
        let m = Measure::new(&ls);
        for &f in &[0.0, 0.1, 0.37, 0.5, 0.73, 1.0] {
            let at = point_at(&ls, &m, f);
            let back = project(&ls, &m, Point::new(at.lon, at.lat));
            assert!(
                (back.geodesic.fraction - f).abs() < 1e-6,
                "f={f} got {}",
                back.geodesic.fraction
            );
            assert!(back.offset_m < 1e-3);
        }
    }

    #[test]
    fn planar_and_geodesic_disagree_on_mixed_orientation_lines() {
        // Longitude degrees are ~20% shorter than latitude degrees at 37°N, so a
        // planar fraction along an L-shaped line is measurably wrong.
        let ls = seville_line();
        let m = Measure::new(&ls);
        let corner = project(&ls, &m, Point::new(-5.9825, 37.3900));
        assert!((corner.geodesic.fraction - 0.5).abs() < 0.01);
        assert!(
            (corner.planar_fraction - 0.5).abs() > 0.03,
            "planar={}",
            corner.planar_fraction
        );
    }
}
