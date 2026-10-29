//! Geodesic linear referencing on WGS84, as Overture defines it:
//!
//! > linear reference = geodesic distance along the segment from its start ÷ its total
//! > geodesic length
//!
//! [`GeodesicLine`] answers both directions of the question:
//! * [`GeodesicLine::point_at`]: fraction → point;
//! * [`GeodesicLine::project`]: point → fraction. The Rust ecosystem otherwise only
//!   offers this in planar form (`geo`'s `LineLocatePoint` treats lon/lat as x/y).
//!
//! Projection minimises the geodesic distance to each sub-segment's geodesic arc by
//! golden-section search (Karney's algorithms underneath, via `geo`), skipping arcs that
//! the triangle inequality proves cannot hold the answer.
//!
//! [`planar`] has the naive planar equivalents, so callers can measure what they cost.

pub mod planar;

use geo::{Bearing, Coord, Distance, Geodesic, InterpolatePoint, LineString, Point};
use serde::Serialize;

/// Why a geometry cannot be linearly referenced.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum GeometryError {
    /// A line needs a start and an end.
    #[error("a line needs at least two vertices, found {0}")]
    TooFewVertices(usize),
    /// NaN or infinite coordinates.
    #[error("vertex {0} has a non-finite coordinate")]
    NonFinite(usize),
    /// A vertex is outside longitude -180..=180 or latitude -90..=90.
    #[error("vertex {0} is outside WGS84 longitude/latitude bounds")]
    OutOfRange(usize),
    /// A query point must have finite WGS84 longitude and latitude.
    #[error("query point must have finite longitude in -180..=180 and latitude in -90..=90")]
    InvalidPoint,
}

/// A WGS84 linestring together with the cumulative geodesic length at each vertex.
#[derive(Debug, Clone)]
pub struct GeodesicLine {
    geometry: LineString<f64>,
    cumulative_m: Vec<f64>,
}

/// A position along a line.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Located {
    /// Linear reference, `0..=1`.
    pub fraction: f64,
    /// Geodesic distance from the start, in metres.
    pub metres_from_start: f64,
    /// Longitude of the point.
    pub lon: f64,
    /// Latitude of the point.
    pub lat: f64,
    /// Index of the vertex that starts the sub-segment holding the point.
    pub vertex_index: usize,
}

impl Located {
    /// The position as a point.
    pub fn point(&self) -> Point<f64> {
        Point::new(self.lon, self.lat)
    }
}

/// The result of projecting a point onto a line.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Projection {
    /// The closest position on the line.
    pub at: Located,
    /// Geodesic distance from the query point to `at`, in metres.
    pub offset_m: f64,
}

impl GeodesicLine {
    /// Measures `geometry`. It needs at least two vertices, all valid WGS84 coordinates.
    pub fn new(geometry: LineString<f64>) -> Result<Self, GeometryError> {
        let n = geometry.0.len();
        if n < 2 {
            return Err(GeometryError::TooFewVertices(n));
        }
        if let Some(i) = geometry
            .0
            .iter()
            .position(|c| !(c.x.is_finite() && c.y.is_finite()))
        {
            return Err(GeometryError::NonFinite(i));
        }
        if let Some(i) = geometry
            .0
            .iter()
            .position(|c| validate_point(Point(*c)).is_err())
        {
            return Err(GeometryError::OutOfRange(i));
        }
        let mut cumulative_m = Vec::with_capacity(n);
        let mut acc = 0.0;
        cumulative_m.push(acc);
        for w in geometry.0.windows(2) {
            acc += Geodesic.distance(Point(w[0]), Point(w[1]));
            cumulative_m.push(acc);
        }
        Ok(Self {
            geometry,
            cumulative_m,
        })
    }

    /// The linestring.
    pub fn geometry(&self) -> &LineString<f64> {
        &self.geometry
    }

    /// Cumulative geodesic length at each vertex, in metres; starts at 0.
    pub fn cumulative_m(&self) -> &[f64] {
        &self.cumulative_m
    }

    /// Total geodesic length, in metres.
    pub fn length_m(&self) -> f64 {
        self.cumulative_m.last().copied().unwrap_or(0.0)
    }

    /// Fraction → metres from the start. The fraction is clamped to `0..=1`.
    pub fn fraction_to_metres(&self, fraction: f64) -> f64 {
        fraction.clamp(0.0, 1.0) * self.length_m()
    }

    /// Metres from the start → fraction, clamped to `0..=1`. A zero-length line gives 0.
    pub fn metres_to_fraction(&self, metres: f64) -> f64 {
        let len = self.length_m();
        if len <= 0.0 {
            0.0
        } else {
            (metres / len).clamp(0.0, 1.0)
        }
    }

    fn vertex(&self, i: usize) -> Point<f64> {
        Point(self.geometry.0[i])
    }

    /// The point at a linear reference. The fraction is clamped to `0..=1`.
    pub fn point_at(&self, fraction: f64) -> Located {
        let target = self.fraction_to_metres(fraction);
        let last = self.cumulative_m.len() - 2;
        // The sub-segment [i, i+1] whose cumulative range holds `target`.
        let i = self
            .cumulative_m
            .partition_point(|&c| c <= target)
            .saturating_sub(1)
            .min(last);
        let d = (target - self.cumulative_m[i]).max(0.0);
        let p = Geodesic.point_at_distance_between(self.vertex(i), self.vertex(i + 1), d);
        Located {
            fraction: self.metres_to_fraction(target),
            metres_from_start: target,
            lon: p.x(),
            lat: p.y(),
            vertex_index: i,
        }
    }

    /// The closest point on the line to `q`, on the WGS84 ellipsoid.
    /// Returns an error for non-finite or out-of-range coordinates.
    pub fn project(&self, q: Point<f64>) -> Result<Projection, GeometryError> {
        validate_point(q)?;
        let n = self.geometry.0.len();
        // Distances from q to every vertex, reused for the pruning bound.
        let dv: Vec<f64> = (0..n)
            .map(|i| Geodesic.distance(q, self.vertex(i)))
            .collect();

        let mut best = (f64::INFINITY, 0.0, self.vertex(0), 0usize);
        for i in 0..n - 1 {
            let seg_len = self.cumulative_m[i + 1] - self.cumulative_m[i];
            // Triangle inequality: no point on this arc can be closer than this.
            let lower_bound = dv[i].min(dv[i + 1]) - seg_len;
            if lower_bound >= best.0 {
                continue;
            }
            let (t, d, p) = if seg_len <= 1e-9 {
                (0.0, dv[i], self.vertex(i))
            } else {
                golden_section(q, self.vertex(i), self.vertex(i + 1), seg_len)
            };
            if d < best.0 {
                best = (d, self.cumulative_m[i] + t * seg_len, p, i);
            }
        }
        let (offset_m, metres, p, vertex_index) = best;
        Ok(Projection {
            at: Located {
                fraction: self.metres_to_fraction(metres),
                metres_from_start: metres,
                lon: p.x(),
                lat: p.y(),
                vertex_index,
            },
            offset_m,
        })
    }

    /// Compass bearing (degrees clockwise from north, `0..360`) of the line's own
    /// direction at a position: the bearing of a `forward` traveller there.
    pub fn bearing_at(&self, at: &Located) -> f64 {
        let start = at.vertex_index.min(self.geometry.0.len() - 2);
        let nonzero = |&i: &usize| self.cumulative_m[i + 1] > self.cumulative_m[i];
        let Some(i) = (start..self.geometry.0.len() - 1)
            .find(nonzero)
            .or_else(|| (0..start).rev().find(nonzero))
        else {
            return 0.0;
        };
        let bearing = if at.metres_from_start >= self.cumulative_m[i + 1] {
            Geodesic.bearing(self.vertex(i + 1), self.vertex(i)) + 180.0
        } else if at.metres_from_start <= self.cumulative_m[i] {
            Geodesic.bearing(self.vertex(i), self.vertex(i + 1))
        } else {
            Geodesic.bearing(at.point(), self.vertex(i + 1))
        };
        bearing.rem_euclid(360.0)
    }

    /// The part of the line between two linear references, in line order.
    pub fn slice(&self, from: f64, to: f64) -> LineString<f64> {
        let (lo, hi) = (from.min(to), from.max(to));
        let a = self.point_at(lo);
        let b = self.point_at(hi);
        let mut coords = vec![Coord { x: a.lon, y: a.lat }];
        coords.extend(
            (a.vertex_index + 1..=b.vertex_index)
                .map(|i| self.geometry.0[i])
                .filter(|c| (c.x, c.y) != (a.lon, a.lat)),
        );
        if coords.len() == 1 || coords.last() != Some(&Coord { x: b.lon, y: b.lat }) {
            coords.push(Coord { x: b.lon, y: b.lat });
        }
        LineString(coords)
    }
}

/// Checks longitude/latitude before passing them to geodesic maths or a spatial index.
pub(crate) fn validate_point(point: Point<f64>) -> Result<(), GeometryError> {
    if !(-180.0..=180.0).contains(&point.x()) || !(-90.0..=90.0).contains(&point.y()) {
        return Err(GeometryError::InvalidPoint);
    }
    Ok(())
}

/// Minimise the geodesic distance from `q` to the geodesic arc a→b over t ∈ [0, 1].
/// Returns (t, distance in metres, point).
fn golden_section(q: Point<f64>, a: Point<f64>, b: Point<f64>, len: f64) -> (f64, f64, Point<f64>) {
    let f = |t: f64| -> (f64, Point<f64>) {
        let p = Geodesic.point_at_distance_between(a, b, t * len);
        (Geodesic.distance(q, p), p)
    };
    let inv_phi = (5f64.sqrt() - 1.0) / 2.0;
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    let mut c = hi - inv_phi * (hi - lo);
    let mut d = lo + inv_phi * (hi - lo);
    let mut fc = f(c).0;
    let mut fd = f(d).0;
    // Converge well inside Between's 1e-9 fraction tolerance, including on short arcs.
    let tol = 1e-11;
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
    let t = f64::midpoint(lo, hi);
    // The minimum may sit at a vertex.
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
pub(crate) mod tests {
    use geo::coord;

    use super::*;

    /// Roughly 1.1 km east, then 1.1 km north, in New York City (40.73°N).
    pub(crate) fn l_shaped() -> GeodesicLine {
        GeodesicLine::new(LineString::new(vec![
            coord! { x: -73.9950, y: 40.7300 },
            coord! { x: -73.9818, y: 40.7300 },
            coord! { x: -73.9818, y: 40.7400 },
        ]))
        .unwrap()
    }

    #[test]
    fn rejects_degenerate_geometry() {
        let one = LineString::new(vec![coord! { x: 0.0, y: 0.0 }]);
        assert_eq!(
            GeodesicLine::new(one).unwrap_err(),
            GeometryError::TooFewVertices(1)
        );
        let nan = LineString::new(vec![
            coord! { x: 0.0, y: 0.0 },
            coord! { x: f64::NAN, y: 0.0 },
        ]);
        assert_eq!(
            GeodesicLine::new(nan).unwrap_err(),
            GeometryError::NonFinite(1)
        );
    }

    #[test]
    fn point_at_and_project_round_trip() {
        let line = l_shaped();
        for f in [0.0, 0.1, 0.37, 0.5, 0.73, 1.0] {
            let at = line.point_at(f);
            let back = line.project(at.point()).unwrap();
            assert!(
                (back.at.fraction - f).abs() < 1e-6,
                "f={f} got {}",
                back.at.fraction
            );
            assert!(back.offset_m < 1e-3);
        }
    }

    #[test]
    fn point_at_lands_on_vertices() {
        let line = l_shaped();
        let half = line.cumulative_m()[1] / line.length_m();
        let corner = line.point_at(half);
        assert!((corner.lon - -73.9818).abs() < 1e-9 && (corner.lat - 40.73).abs() < 1e-9);
        let end = line.point_at(1.0);
        assert!((end.lon - -73.9818).abs() < 1e-9 && (end.lat - 40.74).abs() < 1e-9);
    }

    #[test]
    fn bearing_follows_the_line() {
        let line = l_shaped();
        let east = line.bearing_at(&line.point_at(0.1));
        let north = line.bearing_at(&line.point_at(0.9));
        assert!((east - 90.0).abs() < 0.1, "{east}");
        assert!(north < 0.1 || north > 359.9, "{north}");
    }

    #[test]
    fn slice_keeps_interior_vertices() {
        let line = l_shaped();
        let s = line.slice(0.25, 0.75);
        assert_eq!(s.0.len(), 3);
        assert_eq!(s.0[1], coord! { x: -73.9818, y: 40.7300 });
    }
}
