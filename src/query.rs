//! "Standing here, travelling like this — what applies?" Shared by the CLI and the web server.

use std::collections::BTreeMap;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{Local, NaiveDateTime};
use geo::Point;

use crate::index::Network;
use crate::lr::{self, Projection};
use crate::model::Segment;
use crate::rules::{self, PropertyResolution, Situation, TransitionResolution};

/// A question, as it arrives from a command line or a URL.
#[derive(Debug, Default, Clone)]
pub struct Query {
    /// GERS id or unique prefix. Omit to pick the nearest segment to lon/lat.
    pub segment: Option<String>,
    /// Geodesic fraction 0..1 along the segment.
    pub at: Option<f64>,
    pub lon: Option<f64>,
    pub lat: Option<f64>,
    pub heading: Option<String>,
    pub mode: Option<String>,
    /// e.g. "2026-10-13T08:30" or "now".
    pub time: Option<String>,
    pub using: Vec<String>,
    pub recognized: Vec<String>,
    /// "dimension=value" pairs, e.g. "weight=3.5".
    pub vehicle: Vec<String>,
}

pub struct Answer<'a> {
    pub segment: &'a Segment,
    pub fraction: Option<f64>,
    /// Present when the position came from a lon/lat that was snapped onto the segment.
    pub snapped: Option<Projection>,
    pub situation: Situation,
    pub access: PropertyResolution,
    pub speed: PropertyResolution,
    pub transitions: TransitionResolution,
}

/// Resolve a full or unique-prefix GERS id.
pub fn one<'a>(net: &'a Network, needle: &str) -> Result<&'a Segment> {
    let hits = net.find_id(needle);
    match hits.len() {
        0 => bail!("no segment with id (prefix) {needle}"),
        1 => Ok(hits[0]),
        n => bail!("{n} segments match prefix {needle}; be more specific"),
    }
}

pub fn parse_time(s: &str) -> Result<NaiveDateTime> {
    if s.eq_ignore_ascii_case("now") {
        return Ok(Local::now().naive_local());
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(t) = NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(t);
        }
    }
    bail!("cannot parse time {s:?}; use e.g. 2026-10-13T08:30 or now")
}

fn situation(q: &Query, fraction: Option<f64>) -> Result<Situation> {
    let mut vehicle = BTreeMap::new();
    for v in &q.vehicle {
        let (k, val) = v
            .split_once('=')
            .ok_or_else(|| anyhow!("vehicle expects dimension=value, got {v}"))?;
        vehicle.insert(
            k.trim().to_string(),
            val.trim()
                .parse::<f64>()
                .with_context(|| format!("vehicle value in {v}"))?,
        );
    }
    if let Some(h) = &q.heading {
        if h != "forward" && h != "backward" {
            bail!("heading must be forward or backward");
        }
    }
    if let Some(m) = &q.mode {
        if rules::mode_lineage(m).is_empty() {
            bail!("unknown mode {m}");
        }
    }
    Ok(Situation {
        fraction,
        heading: q.heading.clone(),
        mode: q.mode.clone(),
        time: q.time.as_deref().map(parse_time).transpose()?,
        using: q.using.clone(),
        recognized: q.recognized.clone(),
        vehicle,
    })
}

pub fn answer<'a>(net: &'a Network, q: &Query) -> Result<Answer<'a>> {
    let (segment, fraction, snapped): (&Segment, Option<f64>, Option<Projection>) =
        match (&q.segment, q.lon, q.lat) {
            (Some(id), None, None) => (one(net, id)?, q.at, None),
            (Some(id), Some(lon), Some(lat)) => {
                let s = one(net, id)?;
                let p = lr::project(&s.geometry, &s.measure, Point::new(lon, lat));
                (s, Some(p.geodesic.fraction), Some(p))
            }
            (None, Some(lon), Some(lat)) => {
                let hit = net
                    .nearest(lon, lat, 1, 40)
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("no segments"))?;
                (
                    hit.segment,
                    Some(hit.projection.geodesic.fraction),
                    Some(hit.projection),
                )
            }
            _ => bail!("give a segment, or lon/lat (optionally both)"),
        };
    if let Some(f) = fraction {
        if !(0.0..=1.0).contains(&f) {
            bail!("position must be within 0..1");
        }
    }
    let situation = situation(q, fraction)?;
    let access = rules::resolve_access(segment, &situation);
    let speed = rules::resolve_speed(segment, &situation);
    let transitions = rules::resolve_transitions(segment, &situation, |id| net.describe(id));
    Ok(Answer {
        segment,
        fraction,
        snapped,
        situation,
        access,
        speed,
        transitions,
    })
}
