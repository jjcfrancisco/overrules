//! An in-memory set of segments: lookup by id or name, and nearest-segment search.

use std::collections::{BTreeMap, HashMap};

use geo::{BoundingRect, Point};
use rstar::RTree;
use rstar::primitives::{GeomWithData, Rectangle};
use serde::Serialize;

use crate::lr::Projection;
use crate::model::Segment;
use crate::{Error, Result};

/// How many candidate bounding boxes [`Network::nearest`] projects onto exactly.
const NEAREST_CANDIDATES: usize = 40;

/// Segments, indexed by GERS id and by space.
#[derive(Debug)]
pub struct Network {
    segments: Vec<Segment>,
    by_id: HashMap<String, usize>,
    tree: RTree<GeomWithData<Rectangle<[f64; 2]>, usize>>,
    /// cos(mean latitude): scales longitudes so that bounding-box distances are
    /// roughly isotropic for the pre-filter.
    kx: f64,
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
        let (sum, n) = segments
            .iter()
            .filter_map(|s| s.geometry().0.first())
            .fold((0.0, 0usize), |(a, n), c| (a + c.y, n + 1));
        let mean_lat = if n == 0 { 0.0 } else { sum / n as f64 };
        let kx = mean_lat.to_radians().cos().max(1e-6);
        let by_id = segments
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.clone(), i))
            .collect();
        let items = segments
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let r = s.geometry().bounding_rect()?;
                Some(GeomWithData::new(
                    Rectangle::from_corners(
                        [r.min().x * kx, r.min().y],
                        [r.max().x * kx, r.max().y],
                    ),
                    i,
                ))
            })
            .collect();
        Self {
            segments,
            by_id,
            tree: RTree::bulk_load(items),
            kx,
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
    /// distance.
    pub fn nearest(&self, point: Point<f64>, limit: usize) -> Vec<Hit<'_>> {
        let mut hits: Vec<Hit<'_>> = self
            .tree
            .nearest_neighbor_iter([point.x() * self.kx, point.y()])
            .take(NEAREST_CANDIDATES.max(limit))
            .map(|g| {
                let segment = &self.segments[g.data];
                Hit {
                    segment,
                    projection: segment.line().project(point),
                }
            })
            .collect();
        hits.sort_by(|a, b| a.projection.offset_m.total_cmp(&b.projection.offset_m));
        hits.truncate(limit);
        hits
    }

    /// The nearest segment to `point`.
    pub fn snap(&self, point: Point<f64>) -> Result<Hit<'_>> {
        self.nearest(point, 1)
            .into_iter()
            .next()
            .ok_or(Error::EmptyNetwork)
    }

    /// Counts of what the network holds and how much of it is scoped.
    pub fn stats(&self) -> Stats {
        let mut st = Stats::default();
        for s in &self.segments {
            st.segments += 1;
            st.length_km += s.length_m() / 1000.0;
            st.connector_refs += s.connectors.len();
            let kind = format!(
                "{}/{}",
                s.subtype.as_deref().unwrap_or("?"),
                s.class.as_deref().unwrap_or("?")
            );
            *st.by_class.entry(kind).or_default() += 1;
            st.segments_with_between += usize::from(s.has_between());
            st.speed_limits.add(
                s.speed_limits
                    .iter()
                    .map(|r| (r.when.as_ref(), r.between.is_some())),
            );
            st.access_restrictions.add(
                s.access_restrictions
                    .iter()
                    .map(|r| (r.when.as_ref(), r.between.is_some())),
            );
            st.prohibited_transitions.add(
                s.prohibited_transitions
                    .iter()
                    .map(|r| (r.when.as_ref(), r.between.is_some())),
            );
        }
        st
    }
}

impl FromIterator<Segment> for Network {
    fn from_iter<I: IntoIterator<Item = Segment>>(iter: I) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

/// What a [`Network`] holds; see [`Network::stats`].
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Stats {
    /// Number of segments.
    pub segments: usize,
    /// Total geodesic length, in kilometres.
    pub length_km: f64,
    /// Connector references, counted per segment.
    pub connector_refs: usize,
    /// Segments per `subtype/class`.
    pub by_class: BTreeMap<String, usize>,
    /// Segments with at least one `between`-scoped speed limit or access restriction.
    pub segments_with_between: usize,
    /// `speed_limits` counts.
    pub speed_limits: RuleStats,
    /// `access_restrictions` counts.
    pub access_restrictions: RuleStats,
    /// `prohibited_transitions` counts.
    pub prohibited_transitions: RuleStats,
}

/// Counts for one rule-based property.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RuleStats {
    /// Rules in total.
    pub rules: usize,
    /// Segments with at least one rule.
    pub segments: usize,
    /// Rules scoped by `between`.
    pub between: usize,
    /// Rules scoped by `when.heading`.
    pub heading: usize,
    /// Rules scoped by `when.during`.
    pub during: usize,
    /// Rules scoped by `when.mode`.
    pub mode: usize,
    /// Rules scoped by `when.using`.
    pub using: usize,
    /// Rules scoped by `when.recognized`.
    pub recognized: usize,
    /// Rules scoped by `when.vehicle`.
    pub vehicle: usize,
}

impl RuleStats {
    fn add<'w>(&mut self, rules: impl Iterator<Item = (Option<&'w crate::model::When>, bool)>) {
        let before = self.rules;
        for (when, between) in rules {
            self.rules += 1;
            self.between += usize::from(between);
            if let Some(w) = when {
                self.heading += usize::from(w.heading.is_some());
                self.during += usize::from(w.during.is_some());
                self.mode += usize::from(w.mode.is_some());
                self.using += usize::from(w.using.is_some());
                self.recognized += usize::from(w.recognized.is_some());
                self.vehicle += usize::from(w.vehicle.is_some());
            }
        }
        self.segments += usize::from(self.rules > before);
    }
}

#[cfg(test)]
mod tests {
    use geo::line_string;

    use super::*;

    fn seg(id: &str, y: f64) -> Segment {
        Segment::new(id, line_string![(x: -5.99, y: y), (x: -5.98, y: y)]).unwrap()
    }

    #[test]
    fn looks_up_ids_and_prefixes() {
        let net: Network = [seg("abc1", 37.0), seg("abc2", 37.1), seg("xyz", 37.2)]
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
        let net: Network = [seg("south", 37.0), seg("north", 37.1)]
            .into_iter()
            .collect();
        let hit = net.snap(Point::new(-5.985, 37.09)).unwrap();
        assert_eq!(hit.segment.id, "north");
        assert!((hit.projection.at.fraction - 0.5).abs() < 0.01);
        assert!(Network::new(vec![]).snap(Point::new(0.0, 0.0)).is_err());
    }
}
