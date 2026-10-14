# overrules

[![CI](https://github.com/jjcfrancisco/overrules/actions/workflows/ci.yml/badge.svg)](https://github.com/jjcfrancisco/overrules/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#licence)

**Which Overture transportation rules apply here, for this traveller, and why.**

> *Standing at **this point** on **this road**, travelling **this way**, on foot, by bike or at
> the wheel, **right now**: what is the speed limit, may I pass, which turns are banned, and
> which rule said so?*

[Overture Maps](https://overturemaps.org) attaches `speed_limits`, `access_restrictions` and
`prohibited_transitions` to road segments as **ordered lists of scoped rules**. Each rule may be
limited by `when` (heading, time, travel mode, purpose, status, vehicle dimensions) and by
`between` (a stretch of the road, as fractions of its length). The spec says:

- a rule applies when **all** its conditions hold;
- the **last** applicable rule in the list wins;
- otherwise a default applies;
- `between` fractions are **geodesic on WGS84**, not planar.

Most consumers read `speed_limits[0]`, or compute positions in the plane. `overrules`
implements the spec as written and shows its working: every rule, every condition, and which
rule decided. Where you leave a fact out (the time, the vehicle's weight) it says which rules
*could* change the answer instead of guessing.

It is a Rust library first, with a CLI, a web explorer that also runs as WebAssembly, and a
draft set of conformance vectors. Python bindings are planned.

## Quick start

### Library

```toml
[dependencies]
overrules = { git = "https://github.com/jjcfrancisco/overrules", features = ["parquet"] }
```

```rust
use overrules::{Access, Heading, Network, Situation, TravelMode};

let net = Network::new(overrules::io::read_path("segment.parquet")?);
let street = net.lookup("1b946971")?; // GERS id or unique prefix

let driver = Situation::new().heading(Heading::Backward).mode(TravelMode::Car);
let answer = overrules::resolve(street, &driver)?;
assert_eq!(answer.access_outcome(), Access::Rule(overrules::AccessType::Denied));
assert_eq!(answer.access.determining, Some(0)); // rule #0 decided

// Residents are excepted by a later rule, and the later rule wins.
let resident = driver.recognized("as_private");
let answer = overrules::resolve(street, &resident)?;
assert_eq!(answer.access.determining, Some(3));
```

Features: `parquet` reads Overture GeoParquet; `pack` writes compact snapshots for the
browser. GeoJSON is always available. API docs: `just doc --open`.

### Command line

```sh
cargo install --git https://github.com/jjcfrancisco/overrules overrules-cli
just data                                   # or: duckdb < data/extract_seville.sql (needs DuckDB, ~2 min)
export OVERRULES_SEGMENTS=data/seville/segment.parquet
overrules resolve --segment e26d3384 --at 0.05 --heading forward --mode car
```

### Web explorer

```sh
just serve                                  # http://127.0.0.1:8787, the explorer over HTTP
just web && just web-serve                  # the same page as static files, resolver in WebAssembly
```

## What it shows, on real data

The demo data is Seville, Spain (Overture release `2026-09-23.1`, 43,749 segments, 6.8 MB):
`just data` downloads it. The cases below are also [conformance
vectors](crates/core/tests/vectors/seville.json), run on every `cargo test`.

**Same spot, opposite directions, different limits.** Segment `e26d3384` carries 8 speed
rules scoped by both heading and `between`. Rule #0 says 90 km/h, which is what a naive reader
sees.

```sh
overrules resolve --segment e26d3384 --at 0.05 --heading forward  --mode car   # ⇒ max 80 km/h [rule #5]
overrules resolve --segment e26d3384 --at 0.05 --heading backward --mode car   # ⇒ max 50 km/h [rule #4]
```

**A living street with exceptions.** `Calle Pintor Rosales`: no entry backward, except on foot
to a destination, or for residents.

```sh
overrules resolve --segment 1b946971 --heading backward --mode car                          # ⇒ DENIED  [rule #0]
overrules resolve --segment 1b946971 --heading backward --mode car --recognized as_private  # ⇒ ALLOWED [rule #3]
overrules resolve --segment 1b946971 --heading backward --mode foot --using at_destination  # ⇒ ALLOWED [rule #1]
```

The mode taxonomy is applied, not string equality: `car ⊂ motor_vehicle ⊂ vehicle`.

**Vehicle dimensions, and facts you did not give.** `Avenida del Almendral` is closed to
vehicles over 3.5 t.

```sh
overrules resolve --segment 645f3ec7 --heading forward --mode truck --vehicle weight=3.5      # ⇒ IMPLIED (3.5 is not > 3.5)
overrules resolve --segment 645f3ec7 --heading forward --mode truck --vehicle weight=12000kg  # ⇒ DENIED  [rule #1]
overrules resolve --segment 645f3ec7 --heading forward --mode truck
#   ⇒ IMPLIED, but rule [1] comes later and is undetermined: it would override if its missing facts hold
```

**Banned turns.** `Paseo de las Delicias` (`a022afe5`) has 17 prohibited transitions; 9 are in
force travelling forward, 8 backward, and with no heading all 17 *could* apply.

**Geodesic linear referencing.** Snap a point to the nearest roads, and see what planar maths
would have said:

```sh
overrules locate --lon -5.9926 --lat 37.3861        # the Giralda
```

```
  gers id       segment                                       offset   geodesic     planar   Δ planar
  2e61507f-7b23 "Puerta de Palos" (road/footway)               7.4 m     1.0000     1.0000     0.00 m
  3bbf96ee-ff34 "Plaza Virgen de los Reyes" (road/footway)    24.7 m     0.1402     0.1206     2.54 m
  563b148c-1c9b "Calle Alemanes" (road/living_street)         28.0 m     0.6408     0.6197     3.07 m
  ae3f22e4-7fef <unnamed> (road/unknown)                      41.2 m     0.2043     0.0399     9.13 m
```

**What planar maths would cost a whole city.** `overrules audit` moves every `between`
boundary in Seville from where Overture means it to where a planar implementation would put it:

```
  segments with between-scoped rules : 644
  boundary fractions checked         : 1832
  mean displacement                  : 1.97 m
  histogram                          : <1 m 1317  · 1–5 m 297  · 5–10 m 111  · 10–25 m 96  · ≥25 m 11
  segments with a boundary > 5 m off : 117
```

18% of Seville's `between`-scoped segments would have a rule boundary misplaced by more than
5 m. A speed-limit change on the SE-40 motorway would move by 50 m.

## The explorer

The same resolver, on a map. The page asks the question in a sentence ("I'm driving a car
eastbound on Ronda Norte de Sevilla (SE-20), 150 m into a 166 m stretch. What applies here?"),
answers with three tiles (speed limit, access, banned turns), and lists every rule with ✔ / ✘ / ?
per condition. On the map: hollow rings where rules change along the road, red dots where planar
maths would have put those boundaries, and red circles on connectors with banned turns.

It runs two ways with one codebase: `overrules serve` answers over HTTP, or `just web` builds a
static site where the resolver runs in the browser as WebAssembly (about 1 MB of code plus a
4.4 MB data pack). The pack is lossless: coordinates are delta-encoded integers in 1e-7°,
connector positions in 1e-9, and any value that would not round-trip exactly stays a float.
Both transports return the same answers; positions found by projection agree to well under a
millimetre (`wasm32` uses Rust's `libm`, native builds the platform's).

## Semantics

Where the spec is silent, `overrules` makes a documented choice rather than a silent one: the
travel-mode tree (where `truck` and `bus` sit), missing facts, `using`/`recognized` claims,
inverted `between` ranges, units such as the short ton, and what "default" means.
All of it is in **[docs/semantics.md](docs/semantics.md)**.

## Workspace

| crate | what |
|---|---|
| [`crates/core`](crates/core) (`overrules`) | The library: typed Overture segments and rules, geodesic linear referencing, three-valued rule evaluation, a spatial index, GeoJSON/Parquet readers, packs. No I/O unless asked. |
| [`crates/cli`](crates/cli) (`overrules-cli`) | The `overrules` binary: `stats · find · locate · at · resolve · export · audit · pack · serve`. |
| [`crates/explorer`](crates/explorer) (`overrules-explorer`) | The web explorer: the JSON API, the embedded page (`web/`), and an axum server. |
| [`crates/wasm`](crates/wasm) (`overrules-wasm`) | The explorer API compiled to WebAssembly for the static site. |

Errors follow the pattern in [rustac](https://github.com/stac-utils/rustac) and
[Martin](https://github.com/maplibre/martin): one `thiserror` enum per library crate,
`#[non_exhaustive]`, with an `ErrorKind` that front ends map to a transport (404, 400, 500)
without matching every variant. The CLI uses `anyhow`.

## Development

Everything runs through [`just`](https://github.com/casey/just):

| recipe | what |
|---|---|
| `just test` | unit, integration and doc tests, and the conformance vectors |
| `just clippy` · `just fmt` | lint (pedantic, warnings are errors) and format |
| `just ci` | everything CI runs |
| `just msrv` | build with the minimum supported Rust (1.95) |
| `just data` · `just fixtures` | download Seville; rebuild the test fixture from it |
| `just run <args>` · `just serve` | the CLI; the explorer |
| `just web` · `just wasm-smoke` | the static site; a Node test of the WebAssembly build |

Building for `wasm32` on macOS needs LLVM's `ar` (`brew install llvm`): Apple's silently drops
the precompiled `wasm32` object of a transitive dependency (`psm`). The justfile finds it.

## Direction

The aim: be the reference answer to "what does this Overture segment say applies here, for this
traveller, right now, and which rule said so?"

[OvertureMaps/data#469](https://github.com/OvertureMaps/data/issues/469) asks for "a reference
implementation of the rule evaluation … as well as some sort of viewing/evaluation tool". The
only statement of "last matching rule wins" is a paragraph of prose in the docs; it is in neither
the JSON Schema nor the Pydantic models, and the official examples have data but no expected
outcomes. Three layers, in order:

1. **Library**: pure evaluation with provenance, Rust first, WebAssembly today, Python next.
2. **Conformance suite**: JSON vectors `(segment, situation) → expected answer` that any
   implementation can run ([draft](crates/core/tests/vectors)).
3. **Explorer**: the human-readable face of both, linked from the docs rather than merged into
   them.

Not a router, a downloader or a data pipeline. The research behind this is in
[docs/research](docs/research): [niche](docs/research/niche-research.md) and
[naming](docs/research/naming.md).

## Credits

`overrules` is built by [Francisco Jiménez](https://github.com/jjcfrancisco) in collaboration
with **Claude** (Anthropic's AI model, Claude Opus 5.5), working as a coding agent. Claude
carried out the restructuring into a library-first workspace: the crate layout, the typed model
and error handling, the three-valued evaluator's API, the GeoJSON reader, the tests and
conformance vectors, the justfile, CI, and the semantics document. Earlier commits were also
written with AI help, through GitHub Copilot. Francisco sets the direction, reviews every change
and makes the decisions. From this restructuring on, commits Claude contributed to carry a
`Co-authored-by: Claude` trailer.

## Licence

Code: [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

Overture transportation data, including the test fixtures, is © OpenStreetMap contributors and
Overture Maps Foundation, available under the [ODbL](https://opendatacommons.org/licenses/odbl/1-0/).
