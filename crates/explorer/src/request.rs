//! A question as it arrives over the wire: query-string parameters, all text.

use std::collections::HashMap;

use chrono::{Local, NaiveDateTime};
use geo::Point;
use overrules::{Heading, Situation, TravelMode};

use crate::{Error, Result};

/// Which segment, and where on it.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A GERS id (or unique prefix), optionally at a linear reference.
    Segment {
        /// Id or prefix.
        id: String,
        /// Linear reference, if given.
        at: Option<f64>,
    },
    /// A point, snapped onto the given segment or, without one, the nearest segment.
    Point {
        /// The point.
        point: Point<f64>,
        /// Segment id or prefix to snap onto.
        segment: Option<String>,
    },
}

/// A parsed `resolve` request.
#[derive(Debug, Clone)]
pub struct Request {
    /// What to resolve.
    pub target: Target,
    /// The traveller. `at` is filled in once the target is located.
    pub situation: Situation,
}

impl Request {
    /// Parses query-string parameters: `segment` and `at`, or `lon` and `lat` (optionally
    /// with `segment`), plus `heading`, `mode`, `time` (`2026-10-13T08:30` or `now`),
    /// comma-separated `using` and `recognized`, and `vehicle` such as
    /// `weight=12,height=4.2`.
    pub fn from_params(p: &HashMap<String, String>) -> Result<Self> {
        let get = |k: &str| p.get(k).map(|v| v.trim()).filter(|v| !v.is_empty());
        let num = |k: &str| -> Result<Option<f64>> {
            get(k)
                .map(|v| {
                    v.parse::<f64>()
                        .map_err(|_| Error::BadRequest(format!("{k}={v} is not a number")))
                })
                .transpose()
        };
        let list = |k: &str| -> Vec<&str> {
            get(k)
                .map(|v| {
                    v.split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        };

        let target = match (get("segment"), num("lon")?, num("lat")?) {
            (Some(id), None, None) => Target::Segment {
                id: id.to_owned(),
                at: num("at")?,
            },
            (segment, Some(lon), Some(lat)) => Target::Point {
                point: Point::new(lon, lat),
                segment: segment.map(str::to_owned),
            },
            _ => {
                return Err(Error::BadRequest(
                    "give segment (and at), or lon and lat".to_owned(),
                ));
            }
        };

        let mut s = Situation::new();
        if let Some(h) = get("heading") {
            s = s.heading(h.parse::<Heading>()?);
        }
        if let Some(m) = get("mode") {
            s = s.mode(m.parse::<TravelMode>()?);
        }
        if let Some(t) = get("time") {
            s = s.time(parse_time(t)?);
        }
        for u in list("using") {
            s = s.using(u);
        }
        for r in list("recognized") {
            s = s.recognized(r);
        }
        for v in list("vehicle") {
            s = s.vehicle_spec(v)?;
        }
        Ok(Self {
            target,
            situation: s,
        })
    }
}

/// Parses a local time of travel: `now`, or `YYYY-MM-DDTHH:MM[:SS]` (a space works in
/// place of the `T`).
pub fn parse_time(s: &str) -> Result<NaiveDateTime> {
    if s.eq_ignore_ascii_case("now") {
        return Ok(Local::now().naive_local());
    }
    [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ]
    .iter()
    .find_map(|fmt| NaiveDateTime::parse_from_str(s, fmt).ok())
    .ok_or_else(|| {
        Error::BadRequest(format!(
            "cannot read time {s:?}; use e.g. 2026-10-13T08:30 or now"
        ))
    })
}

#[cfg(test)]
mod tests {
    use overrules::Dimension;

    use super::*;

    fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn parses_a_full_request() {
        let r = Request::from_params(&params(&[
            ("segment", "645f3ec7"),
            ("at", "0.5"),
            ("heading", "forward"),
            ("mode", "truck"),
            ("time", "2026-10-13 08:30"),
            ("recognized", "as_private, as_permitted"),
            ("vehicle", "weight=12,height=4.2"),
        ]))
        .unwrap();
        assert_eq!(
            r.target,
            Target::Segment {
                id: "645f3ec7".into(),
                at: Some(0.5)
            }
        );
        assert_eq!(r.situation.mode, Some(TravelMode::Truck));
        assert_eq!(r.situation.recognized, ["as_private", "as_permitted"]);
        assert_eq!(r.situation.vehicle[&Dimension::Weight], 12.0);
    }

    #[test]
    fn rejects_bad_input() {
        for p in [
            params(&[]),
            params(&[("lon", "-5.99")]),
            params(&[("segment", "x"), ("at", "half")]),
            params(&[("segment", "x"), ("heading", "sideways")]),
            params(&[("segment", "x"), ("mode", "tractor")]),
            params(&[("segment", "x"), ("time", "teatime")]),
        ] {
            assert!(Request::from_params(&p).is_err(), "{p:?}");
        }
    }
}
