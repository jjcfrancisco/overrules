# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/). Before 1.0, minor versions may break the API, and
any change to [the semantics](docs/semantics.md) is listed here.

## [Unreleased]

### Changed

- Renamed from `gers-lr` to **overrules**, and made a standalone library: the `overrules` crate.
  The CLI, the map explorer, its HTTP server, the WebAssembly bindings and the browser data pack
  moved to [overrules-explorer](https://github.com/jjcfrancisco/overrules-explorer).
- The library does no file reading by default. Reading Parquet is the opt-in `parquet` feature;
  GeoJSON is always available.
- Typed model: `Heading`, `TravelMode`, `Dimension`, `Unit`, `Comparison`, `AccessType`,
  `SpeedUnit` and `Between` replace strings and raw arrays. Unknown travel modes in data are kept
  as `TravelMode::Other` rather than failing the read.
- Errors: one `thiserror` enum with `ErrorKind`, for mapping to a transport. A missing input
  file is `ErrorKind::NotFound`. `GeodesicLine`
  rejects degenerate geometry instead of panicking.
- Rule resolution returns owned, serialisable results (`Resolution`, `SetResolution`) with typed
  scope checks, from `eval::last_match` and `eval::all_matches`. `Answer` gives
  `access_outcome()`, `speed_limit_rule()` and `transitions_in_force()`.
- Explanations no longer mention CLI flags.
- Minimum supported Rust is 1.95, set by a transitive dependency of `opening-hours`.

### Added

- Vehicle thresholds are converted to metres and tonnes from any Overture unit, including the
  short ton (`st`). `Situation::vehicle_spec("weight=7500kg")` parses dimensions with units,
  including scientific notation (`weight=1e3kg`).
- `SCHEMA_VERSION` (2.0.0): the Overture schema the crate reads. Data with a value outside
  that schema's lists fails with `Error::InvalidRecord`, naming the segment and the field
  (e.g. `access_restrictions[2].access_type`).
- A weekly CI job (`scripts/check_schema.py`) compares the crate's lists with the schema of
  Overture's newest announced release, from the release calendar, and warns ahead of major
  change months. The `supported_values` example prints the lists it compares.
- `ALL` constants for `Heading`, `Comparison`, `AccessType` and `SpeedUnit`, and `as_str` for
  `Comparison` and `SpeedUnit`.
- Inverted `between` ranges are named in the explanation.
- Vehicle comparisons treat values within 1e-9 as equal for every operator, so converting a
  limit between units cannot flip `<` or `>` at the boundary.
- Reading and writing Overture segments as GeoJSON.
- `Network::stats`, `Segment::boundaries`, `GeodesicLine::bearing_at`, `GeodesicLine::slice`
  and `Scoped::scope_text`.
- Examples: `resolve`, `planar_audit` and `export`.
- Draft conformance vectors, run by `cargo test`, with a Seville fixture.
- `docs/semantics.md`: every interpretation the evaluator makes, with sources.
- A justfile; GitHub Actions CI (lint, tests on three OSes, docs, MSRV, a `wasm32` build,
  packaging, cargo-deny); Dependabot; dual MIT/Apache-2.0 licence files.

## gers-lr (pre-release)

The prototype: geodesic linear referencing and the rule resolver as a single crate, with a CLI,
an HTTP map demo and a static WebAssembly build, on a Seville extract.
