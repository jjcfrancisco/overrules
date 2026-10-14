//! A compact, lossless snapshot of a set of segments, for places where the Parquet
//! reader is too heavy to ship (the browser build).
//!
//! A pack is MessagePack holding exactly what the resolver reads: ids, names, classes,
//! connectors, routes, the three rule lists, and geometry. Numbers are stored as
//! integers when that is exact: coordinates in 1e-7° (Overture carries seven decimals),
//! delta-encoded, and connector positions in 1e-9. Any list that would not survive the
//! round trip bit for bit is kept as `f64`, so [`decode`] reproduces the source values
//! exactly. Connector ids are interned, since each appears on every segment it touches.

use std::collections::HashMap;

use geo::{Coord, LineString};
use serde::{Deserialize, Serialize};

use crate::model::{
    AccessRestriction, ConnectorRef, ProhibitedTransition, Route, Segment, SpeedLimit,
};
use crate::{Error, Result};

/// Version of the pack layout this build reads and writes.
pub const FORMAT: u32 = 2;
const COORD_SCALE: f64 = 1e7;
const FRACTION_SCALE: f64 = 1e9;

#[derive(Serialize, Deserialize)]
struct Pack {
    format: u32,
    source: String,
    /// Interned connector ids; segments refer to them by index.
    connectors: Vec<String>,
    segments: Vec<PackedSegment>,
}

#[derive(Serialize, Deserialize)]
struct PackedSegment {
    id: String,
    subtype: Option<String>,
    class: Option<String>,
    subclass: Option<String>,
    name: Option<String>,
    /// Indices into [`Pack::connectors`], parallel to `connector_at`.
    connectors: Vec<u32>,
    connector_at: Nums,
    routes: Vec<Route>,
    access_restrictions: Vec<AccessRestriction>,
    speed_limits: Vec<SpeedLimit>,
    prohibited_transitions: Vec<ProhibitedTransition>,
    /// Interleaved `lon, lat, lon, lat, …`.
    coords: Nums,
}

/// A list of `f64`, stored as scaled integers whenever that is lossless.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum Nums {
    Quantised(Vec<i32>),
    Exact(Vec<f64>),
}

/// What [`encode`] did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct EncodeReport {
    /// Segments written.
    pub segments: usize,
    /// Vertices written.
    pub vertices: usize,
    /// Distinct connectors.
    pub connectors: usize,
    /// Lists kept as `f64` because integer quantisation would not have been exact.
    pub exact_fallbacks: usize,
}

/// A decoded pack.
#[derive(Debug)]
#[non_exhaustive]
pub struct Decoded {
    /// The segments.
    pub segments: Vec<Segment>,
    /// The `source` label given to [`encode`].
    pub source: String,
}

/// Packs `segments`. `source` is a free-text label (file name, release) kept with them.
pub fn encode(segments: &[Segment], source: &str) -> Result<(Vec<u8>, EncodeReport)> {
    let mut report = EncodeReport {
        segments: segments.len(),
        ..EncodeReport::default()
    };
    let mut table: Vec<String> = Vec::new();
    let mut index: HashMap<&str, u32> = HashMap::new();

    let mut packed = Vec::with_capacity(segments.len());
    for s in segments {
        report.vertices += s.geometry().0.len();
        let mut connectors = Vec::with_capacity(s.connectors.len());
        for c in &s.connectors {
            let next = u32::try_from(table.len())
                .map_err(|_| Error::Pack("too many connectors".to_owned()))?;
            let i = *index.entry(&c.connector_id).or_insert_with(|| {
                table.push(c.connector_id.clone());
                next
            });
            connectors.push(i);
        }
        let ats: Vec<f64> = s.connectors.iter().map(|c| c.at).collect();
        let flat: Vec<f64> = s.geometry().0.iter().flat_map(|c| [c.x, c.y]).collect();
        packed.push(PackedSegment {
            id: s.id.clone(),
            subtype: s.subtype.clone(),
            class: s.class.clone(),
            subclass: s.subclass.clone(),
            name: s.name.clone(),
            connectors,
            connector_at: pack_nums(&ats, FRACTION_SCALE, 0, &mut report),
            routes: s.routes.clone(),
            access_restrictions: s.access_restrictions.clone(),
            speed_limits: s.speed_limits.clone(),
            prohibited_transitions: s.prohibited_transitions.clone(),
            coords: pack_nums(&flat, COORD_SCALE, 2, &mut report),
        });
    }
    report.connectors = table.len();
    let pack = Pack {
        format: FORMAT,
        source: source.to_owned(),
        connectors: table,
        segments: packed,
    };
    Ok((rmp_serde::to_vec_named(&pack)?, report))
}

/// Unpacks bytes written by [`encode`] (already decompressed).
pub fn decode(bytes: &[u8]) -> Result<Decoded> {
    let pack: Pack = rmp_serde::from_slice(bytes)?;
    if pack.format != FORMAT {
        return Err(Error::Pack(format!(
            "format {} (this build reads {FORMAT}); re-run `overrules pack`",
            pack.format
        )));
    }
    let table = &pack.connectors;
    let segments = pack
        .segments
        .into_iter()
        .map(|p| {
            let ats = unpack_nums(p.connector_at, FRACTION_SCALE, 0);
            if ats.len() != p.connectors.len() {
                return Err(Error::Pack(format!(
                    "segment {}: connector lists disagree",
                    p.id
                )));
            }
            let connectors = p
                .connectors
                .iter()
                .zip(ats)
                .map(|(&i, at)| {
                    let connector_id = table
                        .get(i as usize)
                        .ok_or_else(|| {
                            Error::Pack(format!("segment {}: connector #{i} unknown", p.id))
                        })?
                        .clone();
                    Ok(ConnectorRef { connector_id, at })
                })
                .collect::<Result<Vec<_>>>()?;
            let flat = unpack_nums(p.coords, COORD_SCALE, 2);
            let coords: Vec<Coord> = flat
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[x, y]| Coord { x, y })
                .collect();
            let mut s = Segment::new(p.id, LineString(coords))?;
            s.subtype = p.subtype;
            s.class = p.class;
            s.subclass = p.subclass;
            s.name = p.name;
            s.connectors = connectors;
            s.routes = p.routes;
            s.access_restrictions = p.access_restrictions;
            s.speed_limits = p.speed_limits;
            s.prohibited_transitions = p.prohibited_transitions;
            Ok(s)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Decoded {
        segments,
        source: pack.source,
    })
}

/// Scales to integers if every value survives the round trip exactly. With
/// `stride > 0`, each integer is also stored as a delta from the one `stride` positions
/// earlier (2 for interleaved lon/lat).
fn pack_nums(vals: &[f64], scale: f64, stride: usize, report: &mut EncodeReport) -> Nums {
    let Some(mut q) = vals
        .iter()
        .map(|&v| to_units(v, scale))
        .collect::<Option<Vec<i32>>>()
    else {
        report.exact_fallbacks += 1;
        return Nums::Exact(vals.to_vec());
    };
    if stride > 0 {
        for i in (stride..q.len()).rev() {
            q[i] = q[i].wrapping_sub(q[i - stride]);
        }
    }
    Nums::Quantised(q)
}

fn unpack_nums(n: Nums, scale: f64, stride: usize) -> Vec<f64> {
    match n {
        Nums::Exact(f) => f,
        Nums::Quantised(mut q) => {
            if stride > 0 {
                for i in stride..q.len() {
                    q[i] = q[i].wrapping_add(q[i - stride]);
                }
            }
            q.into_iter().map(|u| f64::from(u) / scale).collect()
        }
    }
}

fn to_units(v: f64, scale: f64) -> Option<i32> {
    let q = (v * scale).round();
    // NaN and out-of-range values cannot be represented; keep them as f64.
    if q.is_nan() || q.abs() > f64::from(i32::MAX) {
        return None;
    }
    let q = q as i32;
    (f64::from(q) / scale == v).then_some(q)
}

#[cfg(test)]
mod tests {
    use geo::line_string;

    use super::*;

    #[test]
    fn seven_decimal_coordinates_round_trip_exactly() {
        let vals = [
            -5.928_464_1,
            37.416_101_1,
            -5.928_190_5,
            37.416_184_4,
            -5.927_908_1,
            37.416_269,
        ];
        let mut rep = EncodeReport::default();
        let n = pack_nums(&vals, COORD_SCALE, 2, &mut rep);
        assert!(matches!(n, Nums::Quantised(_)));
        assert_eq!(rep.exact_fallbacks, 0);
        assert_eq!(unpack_nums(n, COORD_SCALE, 2), vals);
    }

    #[test]
    fn nine_decimal_fractions_round_trip_exactly() {
        let vals = [0.022_047_855, 0.663_314_523, 1.0, 0.0];
        let mut rep = EncodeReport::default();
        let n = pack_nums(&vals, FRACTION_SCALE, 0, &mut rep);
        assert!(matches!(n, Nums::Quantised(_)));
        assert_eq!(unpack_nums(n, FRACTION_SCALE, 0), vals);
    }

    #[test]
    fn inexact_values_fall_back_to_f64() {
        let vals = [-5.928_464_112_34, 37.416_101_1];
        let mut rep = EncodeReport::default();
        let n = pack_nums(&vals, COORD_SCALE, 2, &mut rep);
        assert_eq!(n, Nums::Exact(vals.to_vec()));
        assert_eq!(rep.exact_fallbacks, 1);
    }

    #[test]
    fn segments_round_trip() {
        let mut s = Segment::new(
            "a",
            line_string![(x: -5.99, y: 37.39), (x: -5.98, y: 37.391_234_5)],
        )
        .unwrap();
        s.name = Some("Calle".into());
        s.connectors = vec![
            ConnectorRef {
                connector_id: "c".into(),
                at: 0.0,
            },
            ConnectorRef {
                connector_id: "c".into(),
                at: 1.0,
            },
        ];
        let (bytes, report) = encode(std::slice::from_ref(&s), "test").unwrap();
        assert_eq!(report.connectors, 1);
        let d = decode(&bytes).unwrap();
        assert_eq!(d.source, "test");
        assert_eq!(d.segments[0].geometry(), s.geometry());
        assert_eq!(d.segments[0].connectors, s.connectors);
        assert_eq!(d.segments[0].name, s.name);
    }
}
