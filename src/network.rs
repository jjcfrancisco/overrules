//! An in-memory set of segments: lookup by id or name, and nearest-segment search.

use std::collections::HashMap;

use geo::{Geodesic, InterpolatePoint, Point};
use rstar::RTree;
use rstar::primitives::{GeomWithData, Rectangle};

use crate::lr::{Projection, validate_point};
use crate::model::Segment;
use crate::{Error, Result};

/// Segments, indexed by GERS id and by space.
#[derive(Debug)]
pub struct Network {
    segments: Vec<Segment>,
    by_id: HashMap<String, usize>,
    tree: RTree<GeomWithData<Rectangle<[f64; 3]>, usize>>,
}

/// A segment near a point, with the exact geodesic projection onto it.
#[derive(Debug, Clone)]
pub struct Hit<'a> {
    /// The segment.
    pub segment: &'a Segment,
    /// Where the point projects onto it, and how far away it is.
    pub projection: Projection,
}

impl Network {
    /// Indexes `segments`. With duplicate ids, lookups find the last.
    pub fn new(segments: Vec<Segment>) -> Self {
        let by_id = segments
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.clone(), i))
            .collect();
        let items = segments
            .iter()
            .enumerate()
            .map(|(i, s)| GeomWithData::new(envelope(s), i))
            .collect();
        Self {
            segments,
            by_id,
            tree: RTree::bulk_load(items),
        }
    }

    /// All segments, in the order given.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Number of segments.
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Whether there are no segments.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// The segment with exactly this id.
    pub fn get(&self, id: &str) -> Option<&Segment> {
        self.by_id.get(id).map(|&i| &self.segments[i])
    }

    /// The segment with this id, or the only one whose id starts with it.
    pub fn lookup(&self, id_or_prefix: &str) -> Result<&Segment> {
        if let Some(s) = self.get(id_or_prefix) {
            return Ok(s);
        }
        match self.find_by_prefix(id_or_prefix).as_slice() {
            [] => Err(Error::SegmentNotFound(id_or_prefix.to_owned())),
            [one] => Ok(one),
            many => Err(Error::AmbiguousSegmentId {
                prefix: id_or_prefix.to_owned(),
                matches: many.len(),
            }),
        }
    }

    /// Segments whose id starts with `prefix`.
    pub fn find_by_prefix(&self, prefix: &str) -> Vec<&Segment> {
        self.segments
            .iter()
            .filter(|s| s.id.starts_with(prefix))
            .collect()
    }

    /// Segments whose primary name contains `needle`, ignoring case.
    pub fn find_by_name(&self, needle: &str) -> Vec<&Segment> {
        let n = needle.to_lowercase();
        self.segments
            .iter()
            .filter(|s| {
                s.name
                    .as_ref()
                    .is_some_and(|x| x.to_lowercase().contains(&n))
            })
            .collect()
    }

    /// Up to `limit` segments nearest to `point`, closest first, by exact geodesic
    /// distance. Invalid WGS84 coordinates return an error, even when `limit` is zero.
    pub fn nearest(&self, point: Point<f64>, limit: usize) -> Result<Vec<Hit<'_>>> {
        validate_point(point)
            .map_err(|e| Error::invalid("point", format!("{point:?}"), e.to_string()))?;
        let limit = limit.min(self.len());
        let mut hits: Vec<Hit<'_>> = Vec::with_capacity(limit);
        if limit == 0 {
            return Ok(hits);
        }
        for (candidate, distance_2) in self
            .tree
            .nearest_neighbor_iter_with_distance_2(geocentric(point))
        {
            // Distance to an enclosing box is a lower bound on chord distance, which
            // is itself no greater than surface distance. This also works at the poles
            // and across the date line. No fixed candidate cutoff is needed.
            if hits.len() == limit && distance_2.sqrt() > hits[limit - 1].projection.offset_m + 1e-5
            {
                break;
            }
            let segment = &self.segments[candidate.data];
            let projection = segment
                .line()
                .project(point)
                .map_err(|e| Error::invalid("point", format!("{point:?}"), e.to_string()))?;
            let hit = Hit {
                segment,
                projection,
            };
            let compare = |a: &Hit<'_>, b: &Hit<'_>| {
                a.projection
                    .offset_m
                    .total_cmp(&b.projection.offset_m)
                    .then_with(|| a.segment.id.cmp(&b.segment.id))
            };
            if hits.len() < limit {
                hits.push(hit);
                if hits.len() == limit {
                    hits.sort_by(compare);
                }
            } else {
                let rank = hits.partition_point(|existing| compare(existing, &hit).is_le());
                if rank < limit {
                    hits.insert(rank, hit);
                    hits.pop();
                }
            }
        }
        Ok(hits)
    }

    /// The nearest segment to `point`.
    pub fn snap(&self, point: Point<f64>) -> Result<Hit<'_>> {
        self.nearest(point, 1)?
            .into_iter()
            .next()
            .ok_or(Error::EmptyNetwork)
    }
}

/// WGS84 Earth-centred coordinates in metres. Chords cannot exceed surface distances.
fn geocentric(point: Point<f64>) -> [f64; 3] {
    const A: f64 = 6_378_137.0;
    const E2: f64 = 6.694_379_990_141_316_5e-3;
    let (sin_lat, cos_lat) = point.y().to_radians().sin_cos();
    let (sin_lon, cos_lon) = point.x().to_radians().sin_cos();
    let n = A / (1.0 - E2 * sin_lat * sin_lat).sqrt();
    [
        n * cos_lat * cos_lon,
        n * cos_lat * sin_lon,
        n * (1.0 - E2) * sin_lat,
    ]
}

fn envelope(segment: &Segment) -> Rectangle<[f64; 3]> {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for (i, pair) in segment.geometry().0.windows(2).enumerate() {
        let length = segment.line().cumulative_m()[i + 1] - segment.line().cumulative_m()[i];
        let midpoint =
            Geodesic.point_at_distance_between(Point(pair[0]), Point(pair[1]), length / 2.0);
        let centre = geocentric(midpoint);
        // Every point on this arc is at most half its length from its midpoint.
        // Expand slightly to absorb geodesic and floating-point rounding.
        let radius = length / 2.0 + 1e-5;
        for axis in 0..3 {
            min[axis] = min[axis].min(centre[axis] - radius);
            max[axis] = max[axis].max(centre[axis] + radius);
        }
    }
    Rectangle::from_corners(min, max)
}

impl FromIterator<Segment> for Network {
    fn from_iter<I: IntoIterator<Item = Segment>>(iter: I) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use geo::line_string;

    use super::*;

    fn seg(id: &str, y: f64) -> Segment {
        Segment::new(id, line_string![(x: -73.99, y: y), (x: -73.98, y: y)]).unwrap()
    }

    #[test]
    fn looks_up_ids_and_prefixes() {
        let net: Network = [seg("abc1", 40.7), seg("abc2", 40.8), seg("xyz", 40.9)]
            .into_iter()
            .collect();
        assert_eq!(net.lookup("xy").unwrap().id, "xyz");
        assert!(matches!(
            net.lookup("abc"),
            Err(Error::AmbiguousSegmentId { matches: 2, .. })
        ));
        assert!(matches!(net.lookup("nope"), Err(Error::SegmentNotFound(_))));
    }

    #[test]
    fn nearest_is_by_geodesic_distance() {
        let net: Network = [seg("south", 40.7), seg("north", 40.8)]
            .into_iter()
            .collect();
        let hit = net.snap(Point::new(-73.985, 40.79)).unwrap();
        assert_eq!(hit.segment.id, "north");
        assert!((hit.projection.at.fraction - 0.5).abs() < 0.01);
        assert!(Network::new(vec![]).snap(Point::new(0.0, 0.0)).is_err());
    }
}
