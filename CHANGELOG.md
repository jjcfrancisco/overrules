# Changelog

The project follows [Semantic Versioning](https://semver.org/). Before 1.0, minor releases may
break the API. Changes to rule interpretation are recorded here and in
[the semantics document](docs/semantics.md).

## 0.1.0 (unreleased)

First release. Supports Overture schema 2.0.0 and Rust 1.95 or later.

- Resolves `speed_limits`, `access_restrictions` and `prohibited_transitions` for a traveller
  at a position on a segment, with an explanation for every rule and condition.
- Missing facts leave a rule undetermined. Answers list the rules that could still override
  the result.
- Typed Overture model: headings, travel modes and their taxonomy, vehicle selectors with
  unit conversion, `between` ranges.
- Geodesic linear referencing on WGS84: fraction to point, point to fraction, bearings and
  slices.
- `Network` lookup by GERS id, id prefix, name and nearest segment.
- Reads GeoJSON, and Overture GeoParquet with the `parquet` feature. Errors name the segment
  and field that failed.
- Draft conformance vectors and runnable examples.
