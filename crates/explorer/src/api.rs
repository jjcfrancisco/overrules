//! The four calls the page makes (`stats`, `network`, `find`, `resolve`) as JSON.

use std::collections::HashMap;

use geo::{Distance, Geodesic, Point};
use overrules::lr::{Located, Projection, planar};
use overrules::model::{ProhibitedTransition, Scoped};
use overrules::{Access, Network, Resolution, Segment};
use serde_json::{Value, json};

use crate::Result;
use crate::request::{Request, Target};

/// `GET /api/stats`: counts for the page header.
pub fn stats(net: &Network) -> Value {
    let s = net.stats();
    json!({
        "segments": s.segments,
        "km": s.length_km,
        "speed_rules": s.speed_limits.rules,
        "access_rules": s.access_restrictions.rules,
        "turn_rules": s.prohibited_transitions.rules,
        "segments_with_between": s.segments_with_between,
    })
}

/// `GET /api/network`: every segment as a GeoJSON `FeatureCollection` with properties `id`,
/// `n` (name), `c` (class) and `k` (bit flags: 1 speed, 2 access, 4 turns, 8 `between`).
pub fn network_geojson(net: &Network) -> String {
    let features: Vec<Value> = net
        .segments()
        .iter()
        .map(|s| {
            json!({
                "type": "Feature",
                "properties": {"id": s.id, "n": s.name, "c": s.class.as_deref().unwrap_or("?"), "k": kind_flags(s)},
                "geometry": {"type": "LineString", "coordinates": coords(s, 6)},
            })
        })
        .collect();
    json!({"type": "FeatureCollection", "features": features}).to_string()
}

/// `GET /api/find?q=`: by GERS id prefix first, then by name.
pub fn find(net: &Network, q: &str) -> Value {
    let q = q.trim();
    if q.len() < 2 {
        return json!([]);
    }
    let mut hits = net.find_by_prefix(q);
    if hits.is_empty() {
        hits = net.find_by_name(q);
    }
    hits.sort_by_key(|s| {
        (
            s.name.as_ref().map_or(usize::MAX, String::len),
            std::cmp::Reverse(rule_count(s)),
        )
    });
    hits.iter().take(12).map(|s| summary(s)).collect()
}

/// `GET /api/resolve?…`: see [`Request::from_params`] for the parameters.
pub fn resolve(net: &Network, params: &HashMap<String, String>) -> Result<Value> {
    resolve_request(net, Request::from_params(params)?)
}

/// Resolves a parsed request, adding everything the map needs to draw the answer:
/// geometry, rule boundaries and where planar maths would put them, and banned turns.
pub fn resolve_request(net: &Network, req: Request) -> Result<Value> {
    let Request {
        target,
        mut situation,
    } = req;
    let place = locate(net, target)?;
    situation.at = place.at;
    let answer = overrules::resolve(place.segment, &situation)?;
    let seg = place.segment;

    let speed = answer.speed_limit_rule().map(
        |r| json!({"display": r.to_string(), "max_speed": r.max_speed, "min_speed": r.min_speed}),
    );
    let access = match answer.access_outcome() {
        Access::Rule(t) => json!(t),
        Access::Default => Value::Null,
    };
    let nearby: Vec<Value> = place.free_point.map_or_else(Vec::new, |point| {
        net.nearest(point, 5)
            .into_iter()
            .filter(|h| h.segment.id != seg.id)
            .map(|h| json!({"id": h.segment.id, "label": h.segment.label(), "offset_m": h.projection.offset_m}))
            .collect()
    });

    Ok(json!({
        "segment": segment_json(seg),
        "position": place.position_json(),
        "situation": answer.situation,
        "access": property(&answer.access, &seg.access_restrictions, |r| r.access_type.to_string(), access),
        "speed_limit": property(&answer.speed_limit, &seg.speed_limits, ToString::to_string, json!(speed)),
        "prohibited_transitions": {
            "rules": rules_json(&answer.prohibited_transitions.rules, &seg.prohibited_transitions, |r| transition_head(net, r)),
            "in_force": answer.prohibited_transitions.in_force,
            "could_apply": answer.prohibited_transitions.could_apply,
        },
        "turns": turns_json(net, seg),
        "boundaries": boundaries_json(seg),
        "nearby": nearby,
    }))
}

/// Where on which segment a request lands.
struct Place<'a> {
    segment: &'a Segment,
    at: Option<f64>,
    /// Set when a point was snapped: the projection, the planar fraction for the same
    /// point, and how far that planar fraction lands from the true position.
    snapped: Option<(Projection, f64, f64)>,
    /// Set when the caller gave only a point, so the page can list other roads nearby.
    free_point: Option<Point<f64>>,
}

fn locate(net: &Network, target: Target) -> Result<Place<'_>> {
    let (segment, point, free) = match target {
        Target::Segment { id, at } => {
            return Ok(Place {
                segment: net.lookup(&id)?,
                at,
                snapped: None,
                free_point: None,
            });
        }
        Target::Point {
            point,
            segment: Some(id),
        } => (net.lookup(&id)?, point, false),
        Target::Point {
            point,
            segment: None,
        } => (net.snap(point)?.segment, point, true),
    };
    let line = segment.line();
    let projection = line.project(point);
    let planar_fraction = planar::fraction_of(line, point);
    let planar_error_m = Geodesic.distance(
        line.point_at(planar_fraction).point(),
        projection.at.point(),
    );
    Ok(Place {
        segment,
        at: Some(projection.at.fraction),
        snapped: Some((projection, planar_fraction, planar_error_m)),
        free_point: free.then_some(point),
    })
}

impl Place<'_> {
    fn position_json(&self) -> Value {
        let line = self.segment.line();
        let located: Option<Located> = self
            .snapped
            .map(|(p, ..)| p.at)
            .or_else(|| self.at.map(|f| line.point_at(f)));
        let Some(l) = located else {
            return Value::Null;
        };
        let mut v = json!({
            "fraction": l.fraction, "metres_from_start": l.metres_from_start,
            "lon": l.lon, "lat": l.lat, "bearing_forward_deg": line.bearing_at(&l),
        });
        if let Some((p, planar_fraction, planar_error_m)) = self.snapped {
            v["snapped_from_m"] = json!(p.offset_m);
            v["planar_fraction"] = json!(planar_fraction);
            v["planar_error_m"] = json!(planar_error_m);
        }
        v
    }
}

fn segment_json(seg: &Segment) -> Value {
    let line = seg.line();
    json!({
        "id": seg.id, "name": seg.name, "subtype": seg.subtype, "class": seg.class.as_deref().unwrap_or("?"),
        "length_m": seg.length_m(), "vertices": seg.geometry().0.len(),
        "routes": seg.routes.iter().filter_map(|r| r.reference.clone().or_else(|| r.name.clone())).collect::<Vec<_>>(),
        "geometry": {"type": "LineString", "coordinates": coords(seg, 7)},
        "connectors": seg.connectors.iter().map(|c| {
            let p = line.point_at(c.at);
            json!({"id": c.connector_id, "at": c.at, "lon": p.lon, "lat": p.lat})
        }).collect::<Vec<_>>(),
    })
}

/// Where every `between` boundary really is, and where planar maths would put it.
fn boundaries_json(seg: &Segment) -> Vec<Value> {
    let line = seg.line();
    seg.boundaries()
        .into_iter()
        .map(|f| {
            let g = line.point_at(f);
            let p = planar::point_at(line, f);
            json!({
                "fraction": f, "lon": g.lon, "lat": g.lat,
                "planar_lon": p.x(), "planar_lat": p.y(),
                "planar_error_m": Geodesic.distance(g.point(), p),
            })
        })
        .collect()
}

/// Banned turns, with the connector each starts from so the map can mark it.
fn turns_json(net: &Network, seg: &Segment) -> Vec<Value> {
    seg.prohibited_transitions
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let first = r.sequence.first();
            let via = first
                .and_then(|st| {
                    seg.connectors
                        .iter()
                        .find(|c| c.connector_id == st.connector_id)
                })
                .map(|c| seg.line().point_at(c.at));
            json!({
                "index": i,
                "connector": via.map(|p| [p.lon, p.lat]),
                "target_id": first.map(|st| &st.segment_id),
                "target": first.map(|st| describe(net, &st.segment_id)),
                "final_heading": r.final_heading,
            })
        })
        .collect()
}

fn property<R: Scoped>(
    res: &Resolution,
    rules: &[R],
    head: impl Fn(&R) -> String,
    value: Value,
) -> Value {
    let mut v = json!({
        "rules": rules_json(&res.rules, rules, head),
        "determining": res.determining,
        "could_override": res.could_override,
    });
    v["value"] = value;
    v
}

fn rules_json<R: Scoped>(
    evals: &[overrules::eval::RuleEvaluation],
    rules: &[R],
    head: impl Fn(&R) -> String,
) -> Vec<Value> {
    evals
        .iter()
        .zip(rules)
        .map(|(e, r)| {
            json!({
                "index": e.index,
                "rule": format!("{} {}", head(r), r.scope_text()),
                "verdict": e.verdict,
                "scopes": e.scopes,
            })
        })
        .collect()
}

fn transition_head(net: &Network, r: &ProhibitedTransition) -> String {
    let path: Vec<String> = r
        .sequence
        .iter()
        .map(|st| {
            let short = st.connector_id.get(..12).unwrap_or(&st.connector_id);
            format!("via {short}… onto {}", describe(net, &st.segment_id))
        })
        .collect();
    format!(
        "no transition {} (final heading {})",
        path.join(", then "),
        r.final_heading
    )
}

/// A segment's label, or a note that it lies outside the loaded extract.
pub fn describe(net: &Network, id: &str) -> String {
    net.get(id).map_or_else(
        || format!("{}… (outside extract)", id.get(..8).unwrap_or(id)),
        Segment::label,
    )
}

fn coords(seg: &Segment, decimals: i32) -> Vec<[f64; 2]> {
    let k = 10f64.powi(decimals);
    seg.geometry()
        .0
        .iter()
        .map(|c| [(c.x * k).round() / k, (c.y * k).round() / k])
        .collect()
}

fn rule_count(s: &Segment) -> usize {
    s.speed_limits.len() + s.access_restrictions.len() + s.prohibited_transitions.len()
}

fn summary(s: &Segment) -> Value {
    let mid = s.line().point_at(0.5);
    json!({
        "id": s.id, "name": s.name, "class": s.class, "subtype": s.subtype,
        "length_m": s.length_m(),
        "speed": s.speed_limits.len(),
        "access": s.access_restrictions.len(),
        "turns": s.prohibited_transitions.len(),
        "between": s.has_between(),
        "center": [mid.lon, mid.lat],
    })
}

fn kind_flags(s: &Segment) -> u8 {
    u8::from(!s.speed_limits.is_empty())
        | u8::from(!s.access_restrictions.is_empty()) << 1
        | u8::from(!s.prohibited_transitions.is_empty()) << 2
        | u8::from(s.has_between()) << 3
}
