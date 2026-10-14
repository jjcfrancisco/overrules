# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/). Before 1.0, minor versions may break the API, and
any change to [the semantics](docs/semantics.md) is listed here.

## [Unreleased]

### Changed

- Renamed from `gers-lr` to **overrules**, and restructured as a Cargo workspace:
  `overrules` (library), `overrules-cli` (the `overrules` binary), `overrules-explorer`
  (web API, page and server) and `overrules-wasm`.
- The library is I/O-free by default. Parquet reading (`parquet`) and packs (`pack`) are opt-in
  features.
- Typed model: `Heading`, `TravelMode`, `Dimension`, `Unit`, `Comparison`, `AccessType`,
  `SpeedUnit` and `Between` replace strings and raw arrays. Unknown travel modes in data are kept
  as `TravelMode::Other` rather than failing the read.
- Errors: one `thiserror` enum per library crate, with `ErrorKind` for mapping to a transport.
  `GeodesicLine` rejects degenerate geometry instead of panicking.
- Rule resolution returns owned, serialisable results (`Resolution`, `SetResolution`) with typed
  scope checks. `Answer` gives `access_outcome()`, `speed_limit_rule()` and
  `transitions_in_force()`.
- Explanations no longer mention CLI flags.
- CLI: `--segments` (or `OVERRULES_SEGMENTS`) is required. `find` and `at` take positional
  arguments. `show` is replaced by `export`, which writes GeoJSON that `--segments` reads back.
  `serve` takes `--host` and stops cleanly on Ctrl-C.
- Pack format 2 (named MessagePack fields); rebuild packs with `overrules pack`.
- Minimum supported Rust is 1.95, set by a transitive dependency of `opening-hours`.

### Added

- Vehicle thresholds are converted to metres and tonnes from any Overture unit, including the
  short ton (`st`). Travellers can give dimensions with units: `--vehicle weight=7500kg`.
- Inverted `between` ranges are named in the explanation.
- Reading and writing Overture segments as GeoJSON.
- `Network::stats`, `Segment::boundaries`, `GeodesicLine::bearing_at` and `GeodesicLine::slice`.
- Draft conformance vectors, run by `cargo test`, with a Seville fixture.
- `docs/semantics.md`: every interpretation the evaluator makes, with sources.
- A justfile, GitHub Actions CI (lint, test on three OSes, docs, MSRV, WebAssembly smoke test,
  cargo-deny), Dependabot, and dual MIT/Apache-2.0 licence files.

## gers-lr (pre-release)

The prototype: geodesic linear referencing and the rule resolver as a single crate, with a CLI,
an HTTP map demo and a static WebAssembly build, on a Seville extract.
