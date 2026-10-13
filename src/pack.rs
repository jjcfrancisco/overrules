//! A compact, self-contained copy of a segment extract for the browser build.
//!
//! The Parquet reader (Arrow, zstd, …) is far too heavy to ship as
//! WebAssembly, so `gers-lr pack` converts an extract once, natively, into a
//! MessagePack blob holding exactly what [`crate::api`] reads: ids, names,
//! classes, connectors, routes and the three rule lists, plus geometry.
//!
//! Numbers are stored as integers when that is exact: coordinates in 1e-7°
//! (Overture carries seven decimals) and connector fractions in 1e-9, with
//! coordinates additionally delta-encoded. Any value that would not survive
//! the round trip bit-for-bit is kept as f64, so decoding reproduces the
//! source values exactly. Connector ids are interned, since each appears in
//! every segment that touches it.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use geo::{Coord, LineString};
use serde::{Deserialize, Serialize};

use crate::lr::Measure;
use crate::model::{
    AccessRestrictionRule, ConnectorRef, Names, ProhibitedTransitionRule, Route, Segment,
    SegmentRecord, SpeedLimitRule,
};

pub const FORMAT: u32 = 1;
const COORD_SCALE: f64 = 1e7;
const FRACTION_SCALE: f64 = 1e9;

#[derive(Serialize, Deserialize)]
pub struct Pack {
    pub format: u32,
    /// Where the data came from, for the "about" line (file name, release).
    pub source: String,
    /// Interned connector ids; segments refer to them by index.
    pub connectors: Vec<String>,
    pub segments: Vec<PackedSegment>,
}

#[derive(Serialize, Deserialize)]
pub struct PackedSegment {
    pub id: String,
    pub subtype: Option<String>,
    pub class: Option<String>,
    pub subclass: Option<String>,
    pub name: Option<String>,
    /// Indices into [`Pack::connectors`], parallel to `connector_at`.
    pub connectors: Vec<u32>,
    pub connector_at: Nums,
    pub routes: Option<Vec<Route>>,
    pub access_restrictions: Option<Vec<AccessRestrictionRule>>,
    pub speed_limits: Option<Vec<SpeedLimitRule>>,
    pub prohibited_transitions: Option<Vec<ProhibitedTransitionRule>>,
    /// Interleaved `lon, lat, lon, lat, …`.
    pub coords: Nums,
}

/// A list of f64 that is stored as scaled integers whenever that is lossless.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Nums {
    Quantised(Vec<i32>),
    Exact(Vec<f64>),
}

/// Summary of what [`encode`] did, for the CLI report.
#[derive(Debug, Default, Clone, Copy)]
pub struct EncodeReport {
    pub segments: usize,
    pub vertices: usize,
    pub connectors: usize,
    /// Lists kept as f64 because integer quantisation was not exact.
    pub exact_fallbacks: usize,
}

pub fn encode(segments: &[Segment], source: &str) -> Result<(Vec<u8>, EncodeReport)> {
    let mut report = EncodeReport {
        segments: segments.len(),
        ..Default::default()
    };
    let mut table: Vec<String> = Vec::new();
    let mut index: HashMap<String, u32> = HashMap::new();
    let mut intern = |id: &str| -> u32 {
        if let Some(&i) = index.get(id) {
            return i;
        }
        let i = table.len() as u32;
        table.push(id.to_string());
        index.insert(id.to_string(), i);
        i
    };

    let mut packed = Vec::with_capacity(segments.len());
    for s in segments {
        let r = &s.rec;
        report.vertices += s.geometry.0.len();
        let refs: Vec<&ConnectorRef> = r.connectors.iter().flatten().collect();
        let ats: Vec<f64> = refs.iter().map(|c| c.at).collect();
        let flat: Vec<f64> = s.geometry.0.iter().flat_map(|c| [c.x, c.y]).collect();
        let connector_at = pack_nums(&ats, FRACTION_SCALE, 0, &mut report);
        let coords = pack_nums(&flat, COORD_SCALE, 2, &mut report);
        packed.push(PackedSegment {
            id: r.id.clone(),
            subtype: r.subtype.clone(),
            class: r.class.clone(),
            subclass: r.subclass.clone(),
            name: r.names.as_ref().and_then(|n| n.primary.clone()),
            connectors: refs.iter().map(|c| intern(&c.connector_id)).collect(),
            connector_at,
            routes: r.routes.clone(),
            access_restrictions: r.access_restrictions.clone(),
            speed_limits: r.speed_limits.clone(),
            prohibited_transitions: r.prohibited_transitions.clone(),
            coords,
        });
    }
    report.connectors = table.len();
    let pack = Pack {
        format: FORMAT,
        source: source.to_string(),
        connectors: table,
        segments: packed,
    };
    let bytes = rmp_serde::to_vec(&pack).context("encoding pack")?;
    Ok((bytes, report))
}

/// Returns the segments and the pack's `source` label.
pub fn decode(bytes: &[u8]) -> Result<(Vec<Segment>, String)> {
    let pack: Pack = rmp_serde::from_slice(bytes).context("decoding pack")?;
    if pack.format != FORMAT {
        bail!("pack format {} (this build reads {FORMAT})", pack.format);
    }
    let table = &pack.connectors;
    let mut out = Vec::with_capacity(pack.segments.len());
    for p in pack.segments {
        let ats = unpack_nums(p.connector_at, FRACTION_SCALE, 0);
        if ats.len() != p.connectors.len() {
            bail!("segment {}: connector lists disagree", p.id);
        }
        let connectors = p
            .connectors
            .iter()
            .zip(ats)
            .map(|(&i, at)| {
                let connector_id = table
                    .get(i as usize)
                    .with_context(|| format!("segment {}: connector #{i} unknown", p.id))?
                    .clone();
                Ok(ConnectorRef { connector_id, at })
            })
            .collect::<Result<Vec<_>>>()?;
        let flat = unpack_nums(p.coords, COORD_SCALE, 2);
        let geometry = LineString::from(
            flat.as_chunks::<2>()
                .0
                .iter()
                .map(|&[x, y]| Coord { x, y })
                .collect::<Vec<_>>(),
        );
        let measure = Measure::new(&geometry);
        let rec = SegmentRecord {
            id: p.id,
            geometry: String::new(),
            version: None,
            subtype: p.subtype,
            class: p.class,
            subclass: p.subclass,
            names: p.name.map(|primary| Names {
                primary: Some(primary),
                ..Default::default()
            }),
            connectors: Some(connectors),
            routes: p.routes,
            access_restrictions: p.access_restrictions,
            speed_limits: p.speed_limits,
            prohibited_transitions: p.prohibited_transitions,
            road_surface: None,
            road_flags: None,
            level_rules: None,
            subclass_rules: None,
            width_rules: None,
        };
        out.push(Segment {
            rec,
            geometry,
            measure,
        });
    }
    Ok((out, pack.source))
}

/// Scale to integers if every value survives the round trip exactly; with
/// `stride > 0`, additionally store each integer as a delta from the one
/// `stride` positions earlier (2 for interleaved lon/lat).
fn pack_nums(vals: &[f64], scale: f64, stride: usize, report: &mut EncodeReport) -> Nums {
    let mut q = Vec::with_capacity(vals.len());
    for &v in vals {
        match to_units(v, scale) {
            Some(u) => q.push(u),
            None => {
                report.exact_fallbacks += 1;
                return Nums::Exact(vals.to_vec());
            }
        }
    }
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
            q.into_iter().map(|u| u as f64 / scale).collect()
        }
    }
}

fn to_units(v: f64, scale: f64) -> Option<i32> {
    let q = (v * scale).round();
    // NaN and out-of-range values cannot be represented; keep them as f64.
    if q.is_nan() || q.abs() > i32::MAX as f64 {
        return None;
    }
    let q = q as i32;
    (q as f64 / scale == v).then_some(q)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seven_decimal_coordinates_round_trip_exactly() {
        let vals = [
            -5.9284641, 37.4161011, -5.9281905, 37.4161844, -5.9279081, 37.416269,
        ];
        let mut rep = EncodeReport::default();
        let n = pack_nums(&vals, COORD_SCALE, 2, &mut rep);
        assert!(matches!(n, Nums::Quantised(_)));
        assert_eq!(rep.exact_fallbacks, 0);
        assert_eq!(unpack_nums(n, COORD_SCALE, 2), vals);
    }

    #[test]
    fn nine_decimal_fractions_round_trip_exactly() {
        let vals = [0.022047855, 0.663314523, 1.0, 0.0];
        let mut rep = EncodeReport::default();
        let n = pack_nums(&vals, FRACTION_SCALE, 0, &mut rep);
        assert!(matches!(n, Nums::Quantised(_)));
        assert_eq!(unpack_nums(n, FRACTION_SCALE, 0), vals);
    }

    #[test]
    fn inexact_values_fall_back_to_f64() {
        let vals = [-5.92846411234, 37.4161011];
        let mut rep = EncodeReport::default();
        let n = pack_nums(&vals, COORD_SCALE, 2, &mut rep);
        assert_eq!(n, Nums::Exact(vals.to_vec()));
        assert_eq!(rep.exact_fallbacks, 1);
    }
}
