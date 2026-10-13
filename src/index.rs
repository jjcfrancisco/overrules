//! In-memory network: segments by id, name search, and nearest-segment lookup.

use std::collections::HashMap;

use geo::{BoundingRect, Point};
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::RTree;

use crate::lr::{self, Projection};
use crate::model::Segment;

pub struct Network {
    pub segments: Vec<Segment>,
    by_id: HashMap<String, usize>,
    tree: RTree<GeomWithData<Rectangle<[f64; 2]>, usize>>,
    /// cos(mean latitude): used to make degree-space distances roughly isotropic.
    kx: f64,
}

pub struct Hit<'a> {
    pub segment: &'a Segment,
    pub projection: Projection,
}

impl Network {
    pub fn new(segments: Vec<Segment>) -> Self {
        let mean_lat = {
            let (sum, n) = segments
                .iter()
                .filter_map(|s| s.geometry.0.first())
                .fold((0.0, 0usize), |(a, n), c| (a + c.y, n + 1));
            if n == 0 {
                0.0
            } else {
                sum / n as f64
            }
        };
        let kx = mean_lat.to_radians().cos().max(1e-6);
        let by_id = segments
            .iter()
            .enumerate()
            .map(|(i, s)| (s.rec.id.clone(), i))
            .collect();
        let items = segments
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let r = s.geometry.bounding_rect()?;
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

    pub fn get(&self, id: &str) -> Option<&Segment> {
        self.by_id.get(id).map(|&i| &self.segments[i])
    }

    /// Resolve a full or prefix GERS id.
    pub fn find_id(&self, needle: &str) -> Vec<&Segment> {
        if let Some(s) = self.get(needle) {
            return vec![s];
        }
        self.segments
            .iter()
            .filter(|s| s.rec.id.starts_with(needle))
            .collect()
    }

    pub fn find_name(&self, needle: &str) -> Vec<&Segment> {
        let n = needle.to_lowercase();
        self.segments
            .iter()
            .filter(|s| {
                s.name()
                    .map(|x| x.to_lowercase().contains(&n))
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Nearest segments to a lon/lat, by exact geodesic projection over the
    /// `candidates` nearest bounding boxes.
    pub fn nearest(&self, lon: f64, lat: f64, limit: usize, candidates: usize) -> Vec<Hit<'_>> {
        let q = Point::new(lon, lat);
        let mut hits: Vec<Hit<'_>> = self
            .tree
            .nearest_neighbor_iter([lon * self.kx, lat])
            .take(candidates)
            .map(|g| {
                let seg = &self.segments[g.data];
                Hit {
                    segment: seg,
                    projection: lr::project(&seg.geometry, &seg.measure, q),
                }
            })
            .collect();
        hits.sort_by(|a, b| a.projection.offset_m.total_cmp(&b.projection.offset_m));
        hits.truncate(limit);
        hits
    }

    pub fn describe(&self, id: &str) -> String {
        match self.get(id) {
            Some(s) => s.label(),
            None => format!("{}… (outside extract)", &id[..id.len().min(8)]),
        }
    }
}
