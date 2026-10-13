//! The resolver as JSON. Transport-agnostic: the HTTP server and the WASM
//! build both call these functions, so every front end gets identical answers.

use std::collections::{BTreeSet, HashMap};

use anyhow::{Context, Result};
use geo::{Bearing, Distance, Geodesic, Point};
use serde_json::{json, Value};

use crate::index::Network;
use crate::lr;
use crate::model::Segment;
use crate::query::{self, Query};

/// `GET /api/stats`
pub fn stats(net: &Network) -> Value {
    let (mut speed, mut access, mut turns, mut between, mut km) =
        (0usize, 0usize, 0usize, 0usize, 0.0);
    for s in &net.segments {
        speed += s.rec.speed_limits.as_ref().map_or(0, |r| r.len());
        access += s.rec.access_restrictions.as_ref().map_or(0, |r| r.len());
        turns += s.rec.prohibited_transitions.as_ref().map_or(0, |r| r.len());
        between += has_between(s) as usize;
        km += s.measure.length_m() / 1000.0;
    }
    json!({
        "segments": net.segments.len(), "km": km,
        "speed_rules": speed, "access_rules": access, "turn_rules": turns,
        "segments_with_between": between,
    })
}

/// `GET /api/network` — every segment as a GeoJSON FeatureCollection with
/// properties `id`, `n` (name), `c` (class) and `k` (kind flags, see [`kind_flags`]).
pub fn network_geojson(net: &Network) -> String {
    let features: Vec<Value> = net
        .segments
        .iter()
        .filter(|s| s.geometry.0.len() >= 2)
        .map(|s| {
            json!({
                "type": "Feature",
                "properties": {"id": s.id(), "n": s.name(), "c": s.class(), "k": kind_flags(s)},
                "geometry": {"type": "LineString", "coordinates": coords(s, 6)},
            })
        })
        .collect();
    serde_json::to_string(&json!({"type": "FeatureCollection", "features": features}))
        .expect("geojson")
}

/// `GET /api/find?q=` — by GERS id prefix first, then by name substring.
pub fn find(net: &Network, q: &str) -> Value {
    let q = q.trim();
    if q.len() < 2 {
        return json!([]);
    }
    let mut hits = net.find_id(q);
    if hits.is_empty() {
        hits = net.find_name(q);
    }
    hits.sort_by_key(|s| {
        (
            s.name().map(|n| n.len()).unwrap_or(usize::MAX),
            std::cmp::Reverse(rule_count(s)),
        )
    });
    Value::Array(hits.iter().take(12).map(|s| summary(s)).collect())
}

/// `GET /api/resolve?…` — the `resolve` command plus everything the map needs
/// to draw the answer (geometry, boundaries, planar ghosts, banned-turn connectors).
pub fn resolve(net: &Network, params: &HashMap<String, String>) -> Result<Value> {
    let q = to_query(params)?;
    let a = query::answer(net, &q)?;
    let seg = a.segment;

    let located = match (&a.snapped, a.fraction) {
        (Some(p), _) => Some(p.geodesic.clone()),
        (None, Some(f)) => Some(lr::point_at(&seg.geometry, &seg.measure, f)),
        (None, None) => None,
    };
    let position = located.as_ref().map(|l| {
        let mut v = json!({
            "fraction": l.fraction, "metres_from_start": l.metres_from_start,
            "lon": l.lon, "lat": l.lat,
            "bearing_forward_deg": forward_bearing(seg, l.vertex_index),
        });
        if let Some(s) = &a.snapped {
            v["snapped_from_m"] = json!(s.offset_m);
            v["planar_fraction"] = json!(s.planar_fraction);
            v["planar_error_m"] = json!(s.planar_error_m);
        }
        v
    });

    // Where every `between` boundary on this segment really is, and where planar maths puts it.
    let mut bounds: BTreeSet<u64> = BTreeSet::new();
    for r in seg.rec.speed_limits.iter().flatten() {
        bounds.extend(r.between.iter().flatten().map(|f| f.to_bits()));
    }
    for r in seg.rec.access_restrictions.iter().flatten() {
        bounds.extend(r.between.iter().flatten().map(|f| f.to_bits()));
    }
    let boundaries: Vec<Value> = bounds
        .into_iter()
        .map(f64::from_bits)
        .filter(|f| *f > 0.0 && *f < 1.0)
        .map(|f| {
            let g = lr::point_at(&seg.geometry, &seg.measure, f);
            let planar = planar_point(seg, f);
            json!({
                "fraction": f, "lon": g.lon, "lat": g.lat,
                "planar_lon": planar.x(), "planar_lat": planar.y(),
                "planar_error_m": Geodesic.distance(Point::new(g.lon, g.lat), planar),
            })
        })
        .collect();

    // Banned turns: the connector they start from, so the map can mark it.
    let turns: Vec<Value> = seg
        .rec
        .prohibited_transitions
        .iter()
        .flatten()
        .enumerate()
        .map(|(i, r)| {
            let first = r.sequence.first();
            let via = first.and_then(|st| {
                seg.rec
                    .connectors
                    .iter()
                    .flatten()
                    .find(|c| c.connector_id == st.connector_id)
                    .map(|c| lr::point_at(&seg.geometry, &seg.measure, c.at))
            });
            json!({
                "index": i,
                "connector": via.map(|p| [p.lon, p.lat]),
                "target_id": first.map(|st| st.segment_id.clone()),
                "target": first.map(|st| net.describe(&st.segment_id)),
                "final_heading": r.final_heading,
            })
        })
        .collect();

    let nearby: Vec<Value> = match (q.segment.is_none(), q.lon, q.lat) {
        (true, Some(lon), Some(lat)) => net
            .nearest(lon, lat, 5, 40)
            .into_iter()
            .filter(|h| h.segment.id() != seg.id())
            .map(|h| json!({"id": h.segment.id(), "label": h.segment.label(), "offset_m": h.projection.offset_m}))
            .collect(),
        _ => vec![],
    };

    Ok(json!({
        "segment": {
            "id": seg.id(), "name": seg.name(), "subtype": seg.subtype(), "class": seg.class(),
            "length_m": seg.measure.length_m(), "vertices": seg.geometry.0.len(),
            "routes": route_refs(seg),
            "geometry": {"type": "LineString", "coordinates": coords(seg, 7)},
            "connectors": seg.rec.connectors.iter().flatten().map(|c| {
                let p = lr::point_at(&seg.geometry, &seg.measure, c.at);
                json!({"id": c.connector_id, "at": c.at, "lon": p.lon, "lat": p.lat})
            }).collect::<Vec<_>>(),
        },
        "position": position,
        "situation": a.situation,
        "access": a.access,
        "speed_limit": a.speed,
        "prohibited_transitions": a.transitions,
        "turns": turns,
        "boundaries": boundaries,
        "nearby": nearby,
    }))
}

/// Query-string parameters → [`Query`]. Lists are comma-separated; `vehicle` is `weight=12,height=4.2`.
pub fn to_query(p: &HashMap<String, String>) -> Result<Query> {
    let get = |k: &str| {
        p.get(k)
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    let num = |k: &str| -> Result<Option<f64>> {
        get(k)
            .map(|v| {
                v.parse::<f64>()
                    .with_context(|| format!("{k}={v} is not a number"))
            })
            .transpose()
    };
    let list = |k: &str| -> Vec<String> {
        get(k)
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };
    Ok(Query {
        segment: get("segment"),
        at: num("at")?,
        lon: num("lon")?,
        lat: num("lat")?,
        heading: get("heading"),
        mode: get("mode"),
        time: get("time"),
        using: list("using"),
        recognized: list("recognized"),
        vehicle: list("vehicle"),
    })
}

/// Compass bearing of the segment's drawn direction at the sub-segment containing the point.
fn forward_bearing(seg: &Segment, vertex_index: usize) -> f64 {
    let pts: Vec<Point<f64>> = seg.geometry.points().collect();
    if pts.len() < 2 {
        return 0.0;
    }
    let i = vertex_index.min(pts.len() - 2);
    let b = Geodesic.bearing(pts[i], pts[i + 1]);
    (b + 360.0) % 360.0
}

/// Where a planar (lon/lat-as-xy) implementation would place a fraction.
fn planar_point(seg: &Segment, f: f64) -> Point<f64> {
    use geo::{Euclidean, InterpolatableLine};
    seg.geometry
        .point_at_ratio_from_start(&Euclidean, f)
        .unwrap_or_else(|| {
            let g = lr::point_at(&seg.geometry, &seg.measure, f);
            Point::new(g.lon, g.lat)
        })
}

fn coords(seg: &Segment, decimals: i32) -> Vec<[f64; 2]> {
    let k = 10f64.powi(decimals);
    seg.geometry
        .0
        .iter()
        .map(|c| [(c.x * k).round() / k, (c.y * k).round() / k])
        .collect()
}

fn route_refs(seg: &Segment) -> Vec<String> {
    seg.rec
        .routes
        .iter()
        .flatten()
        .filter_map(|r| r.reference.clone().or_else(|| r.name.clone()))
        .collect()
}

fn rule_count(s: &Segment) -> usize {
    s.rec.speed_limits.as_ref().map_or(0, |r| r.len())
        + s.rec.access_restrictions.as_ref().map_or(0, |r| r.len())
        + s.rec.prohibited_transitions.as_ref().map_or(0, |r| r.len())
}

fn has_between(s: &Segment) -> bool {
    s.rec
        .speed_limits
        .iter()
        .flatten()
        .any(|r| r.between.is_some())
        || s.rec
            .access_restrictions
            .iter()
            .flatten()
            .any(|r| r.between.is_some())
}

fn summary(s: &Segment) -> Value {
    let mid = lr::point_at(&s.geometry, &s.measure, 0.5);
    json!({
        "id": s.id(), "name": s.name(), "class": s.class(), "subtype": s.subtype(),
        "length_m": s.measure.length_m(),
        "speed": s.rec.speed_limits.as_ref().map_or(0, |r| r.len()),
        "access": s.rec.access_restrictions.as_ref().map_or(0, |r| r.len()),
        "turns": s.rec.prohibited_transitions.as_ref().map_or(0, |r| r.len()),
        "between": has_between(s),
        "center": [mid.lon, mid.lat],
    })
}

/// Bit flags per segment so the map can style it: 1 speed, 2 access, 4 turns, 8 between.
fn kind_flags(s: &Segment) -> u8 {
    (s.rec.speed_limits.as_ref().is_some_and(|r| !r.is_empty()) as u8)
        | ((s
            .rec
            .access_restrictions
            .as_ref()
            .is_some_and(|r| !r.is_empty()) as u8)
            << 1)
        | ((s
            .rec
            .prohibited_transitions
            .as_ref()
            .is_some_and(|r| !r.is_empty()) as u8)
            << 2)
        | ((has_between(s) as u8) << 3)
}
