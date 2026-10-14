# Changelog

The project follows [Semantic Versioning](https://semver.org/). Before 1.0, minor releases may
break the API. Changes to rule interpretation are recorded here and in
[the semantics document](docs/semantics.md).

## [Unreleased]

### Changed

- Renamed `gers-lr` to `overrules` and made it a standalone Rust library. The CLI and map
  demo moved to [overrules-explorer](https://github.com/jjcfrancisco/overrules-explorer).
- Added typed rules, units and travel modes, and structured errors. Resolution results
  identify the determining rule and any later rules that could override it.
- Minimum Rust version: 1.95.

### Added

- GeoJSON reading and writing; opt-in Parquet support.
- Vehicle-unit conversion and inputs such as `weight=7500kg` or `weight=1e3kg`.
- Geodesic line slicing and bearings, network statistics and rule-boundary lookup.
- Declared schema support (`SCHEMA_VERSION`, currently 2.0.0) and a weekly check against
  Overture's release calendar. Unrecognised strict-enum values fail with the segment and
  field identified; unknown travel modes are retained.
- Draft conformance vectors, Seville fixtures and runnable examples.

### Fixed

- Buffered GeoJSON file reads to avoid byte-by-byte file access.
- Preserved structured record errors inside GeoJSON FeatureCollections. Missing files
  report `ErrorKind::NotFound`.
- Vehicle comparisons treat values within 1e-9 as equal for every operator, preventing
  unit-conversion rounding from crossing an exact limit.
- Inverted `between` ranges are identified in explanations.

## gers-lr (pre-release)

The prototype combined geodesic linear referencing, rule evaluation, a CLI and a web demo.
