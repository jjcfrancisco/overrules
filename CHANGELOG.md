# Changelog

The project follows [Semantic Versioning](https://semver.org/). Before 1.0, minor releases may
break the API. Changes to rule interpretation are recorded here and in
[the semantics document](docs/semantics.md).

## 0.2.0 (2026-10-29)

Supports Overture schema 2.0.0 and Rust 1.95 or later.

- Keep holiday and solar time expressions undetermined when their context is unavailable.
  Dates outside 1900 through 9999 are also undetermined.
- Find nearest segments using conservative Earth-centred bounds, without a fixed candidate
  cutoff. Covers the date line and poles.
- Reject invalid WGS84 coordinates. **Breaking:** `GeodesicLine::project` and
  `Network::nearest` now return `Result`.
- Tighten projection convergence so computed boundary positions respect the existing
  inclusive range tolerance.
- Compute bearings at the supplied position, including arc endpoints. Entirely zero length
  lines have bearing zero. Degenerate slices retain two coordinates.
- Reject GeoJSON collection members whose type is not `Feature`.
- Add `PreparedSegment` to cache parsed time expressions, including repairs and errors.
  Add `resolve_compact` to return rule choices and uncertainty without explanation strings.
- Add geographic regressions and Parquet fixture tests for Snappy, ZSTD, invalid nested
  records and malformed WKB. Check compact results against the conformance vectors.
- Require patched ZSTD dependencies for Parquet, addressing RUSTSEC-2026-0391,
  RUSTSEC-2026-0398, RUSTSEC-2026-0403 and RUSTSEC-2026-0410.
- Add an explicit visual demo section linking to overrules-city.

## 0.1.0 (2026-10-15)

First release. Supports Overture schema 2.0.0 and Rust 1.95 or later.

- Resolves `speed_limits`, `access_restrictions` and `prohibited_transitions` for a traveller
  at a position on a segment, with an explanation for every rule and condition.
- Missing facts leave a rule undetermined. Answers list the rules that could still override
  the result.
- Time rules (`during`) missing a separator, as some Overture values are, are read with the
  separator restored. The explanation shows how the value was read.
- Typed Overture model: headings, travel modes and their taxonomy, vehicle selectors with
  unit conversion, `between` ranges.
- Geodesic linear referencing on WGS84: fraction to point, point to fraction, bearings and
  slices.
- `Network` lookup by GERS id, id prefix, name and nearest segment.
- Reads GeoJSON, and Overture GeoParquet with the `parquet` feature. Errors name the segment
  and field that failed.
- Draft conformance vectors and runnable examples.
