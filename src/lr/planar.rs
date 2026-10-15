//! The planar shortcuts that Overture's geodesic definition rules out, kept so their
//! cost can be measured.
//!
//! "Planar" means treating longitude and latitude as x and y. Away from the equator a
//! degree of longitude is shorter than a degree of latitude, so planar fractions drift
//! from geodesic ones wherever a line changes direction.

use geo::{Distance, Euclidean, Geodesic, InterpolatableLine, LineLocatePoint, Point};

use super::GeodesicLine;

/// Where a planar implementation would put `fraction` along the line.
pub fn point_at(line: &GeodesicLine, fraction: f64) -> Point<f64> {
    line.geometry()
        .point_at_ratio_from_start(&Euclidean, fraction.clamp(0.0, 1.0))
        .unwrap_or_else(|| line.point_at(fraction).point())
}

/// The fraction a planar `line_locate_point` gives for `q`.
pub fn fraction_of(line: &GeodesicLine, q: Point<f64>) -> f64 {
    line.geometry().line_locate_point(&q).unwrap_or(0.0)
}

/// How far apart, in metres, the geodesic and planar readings of the same fraction are.
///
/// This is the error a consumer makes if it computes `between` positions in the plane:
/// the stretch it thinks a rule covers starts or ends this far from where Overture means.
pub fn error_at(line: &GeodesicLine, fraction: f64) -> f64 {
    Geodesic.distance(line.point_at(fraction).point(), point_at(line, fraction))
}

#[cfg(test)]
mod tests {
    use super::super::tests::l_shaped;
    use super::*;

    #[test]
    fn planar_and_geodesic_disagree_on_mixed_orientation_lines() {
        // A longitude degree is ~20% shorter than a latitude degree at 37°N, so a planar
        // fraction along an L-shaped line is measurably wrong.
        let line = l_shaped();
        let corner = Point::new(-73.9818, 40.7300);
        let geodesic = line.project(corner).at.fraction;
        let planar = fraction_of(&line, corner);
        assert!((geodesic - 0.5).abs() < 0.01, "geodesic={geodesic}");
        assert!((planar - 0.5).abs() > 0.03, "planar={planar}");
        assert!(error_at(&line, 0.5) > 50.0);
    }
}
