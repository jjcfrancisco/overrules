# overrules

[![CI](https://github.com/jjcfrancisco/overrules/actions/workflows/ci.yml/badge.svg)](https://github.com/jjcfrancisco/overrules/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#licence)

**A Rust library that says which Overture transportation rules apply here, for this traveller,
and why.**

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

> Status: pre-release. The API is settling and the crate is not on crates.io yet. Python
> bindings are planned. The map explorer lives in
> [overrules-explorer](https://github.com/jjcfrancisco/overrules-explorer).

## Use

```toml
[dependencies]
overrules = { git = "https://github.com/jjcfrancisco/overrules", features = ["parquet"] }
```

```rust,no_run
use overrules::{Access, AccessType, Heading, Network, Situation, TravelMode};

fn main() -> Result<(), overrules::Error> {
    let net = Network::new(overrules::io::read_path("segment.parquet")?); // or .geojson
    let street = net.lookup("1b946971")?; // GERS id, or a unique prefix

    let driver = Situation::new().heading(Heading::Backward).mode(TravelMode::Car);
    let answer = overrules::resolve(street, &driver)?;
    assert_eq!(answer.access_outcome(), Access::Rule(AccessType::Denied));
    assert_eq!(answer.access.determining, Some(0)); // rule #0 decided

    // Residents are excepted by a later rule, and the later rule wins.
    let answer = overrules::resolve(street, &driver.recognized("as_private"))?;
    assert_eq!(answer.access.determining, Some(3));
    Ok(())
}
```

| Module | What |
|---|---|
| `model` | Typed Overture segments and rules: headings, travel modes and their taxonomy, vehicle selectors with units, `between` ranges. |
| `lr` | Geodesic linear referencing: fraction → point, point → fraction, bearings, slices. `lr::planar` measures what the planar shortcut would get wrong. |
| `eval` | Scope checks and rule resolution: last match for single values, all matches for prohibited turns, with reasons. |
| `Network` | Lookup by GERS id or prefix, by name, and nearest segment to a point. |
| `io` | GeoJSON (always), Overture GeoParquet (feature `parquet`). |

Errors are one `#[non_exhaustive]` `thiserror` enum, with an `ErrorKind` (not found, invalid
input, bad data) for callers that map errors onto a transport. The library never panics on bad
input or data, and compiles to WebAssembly.

## What it shows, on real data

The examples use Seville, Spain (Overture release `2026-09-23.1`, 43,749 segments, 6.8 MB).
`just data` downloads it with DuckDB. Every case below is also a [conformance
vector](tests/vectors/seville.json) that `cargo test` runs on a fixture of the same
segments.

**Same spot, opposite directions, different limits.** Segment `e26d3384` has 8 speed rules
scoped by both heading and `between`. Rule #0 says 90 km/h, which is what a naive reader sees.

```sh
just resolve e26d3384 --at 0.05 --heading forward  --mode car    # ⇒ max 80 km/h, rule #5
just resolve e26d3384 --at 0.05 --heading backward --mode car    # ⇒ max 50 km/h, rule #4
```

**A living street with exceptions.** `Calle Pintor Rosales`: no entry backward, except on foot
to a destination, or for residents. The travel-mode taxonomy applies: `car ⊂ motor_vehicle ⊂ vehicle`.

```sh
just resolve 1b946971 --heading backward --mode car                          # ⇒ denied, rule #0
just resolve 1b946971 --heading backward --mode car --recognized as_private  # ⇒ allowed, rule #3
just resolve 1b946971 --heading backward --mode foot --using at_destination  # ⇒ allowed, rule #1
```

**Facts you did not give.** `Avenida del Almendral` is closed to vehicles over 3.5 t.

```sh
just resolve 645f3ec7 --heading forward --mode truck --vehicle weight=12000kg  # ⇒ denied, rule #1
just resolve 645f3ec7 --heading forward --mode truck                           # ⇒ default, but rule #1 could override
```

**What planar maths would cost a city.** `just audit` moves every `between` boundary in Seville
from where Overture means it to where a planar implementation would put it:

```text
segments with between-scoped rules : 644
boundary fractions checked         : 1832
mean displacement                  : 1.97 m
histogram                          : <1 m 1317  · 1–5 m 297  · 5–10 m 111  · 10–25 m 96  · ≥25 m 11
segments with a boundary > 5 m off : 117
```

18% of Seville's `between`-scoped segments would have a rule boundary misplaced by more than
5 m. A speed-limit change on the SE-40 motorway would move by 50 m.

## Semantics

Where the spec is silent, `overrules` makes a documented choice rather than a silent one: the
travel-mode tree (where `truck` and `bus` sit), missing facts, `using`/`recognized` claims,
inverted `between` ranges, units such as the short ton, and what "default" means.
All of it is in **[docs/semantics.md](docs/semantics.md)**.

The [conformance vectors](tests/vectors) are a draft of a shared test suite: JSON
cases `(segment, situation) → expected answer` that any implementation can run.

## Development

Everything runs through [`just`](https://github.com/casey/just):

| recipe | what |
|---|---|
| `just test` | unit tests, conformance vectors and doc tests |
| `just clippy` · `just fmt` | lint (pedantic, warnings are errors) and format |
| `just check-wasm` | the library still compiles for `wasm32` |
| `just package` | build the crate as crates.io would receive it, and test that |
| `just ci` | everything CI runs |
| `just msrv` | build with the minimum supported Rust (1.95) |
| `just data` · `just fixtures` | download Seville; rebuild the test fixture from it |
| `just resolve <id> …` · `just audit` | the examples above, on the Seville extract |

Checking for `wasm32` on macOS needs LLVM's `ar` (`brew install llvm`): Apple's silently drops
the precompiled `wasm32` object of a transitive dependency (`psm`). The justfile finds it.

## Direction

The aim: be the reference answer to "what does this Overture segment say applies here, for this
traveller, right now, and which rule said so?"

[OvertureMaps/data#469](https://github.com/OvertureMaps/data/issues/469) asks for "a reference
implementation of the rule evaluation … as well as some sort of viewing/evaluation tool". The
only statement of "last matching rule wins" is a paragraph of prose in the docs; it is in neither
the JSON Schema nor the Pydantic models, and the official examples have data but no expected
outcomes. Three layers:

1. **Library** (this repository): evaluation with provenance; Rust first, Python next.
2. **Conformance suite**: shared JSON vectors ([draft](tests/vectors)).
3. **Explorer** ([overrules-explorer](https://github.com/jjcfrancisco/overrules-explorer)):
   the library on a map, compiled to WebAssembly.

Not a router, a downloader or a data pipeline. The research behind this is in
[docs/research](docs/research).

## Credits

`overrules` is built by [Francisco Jiménez](https://github.com/jjcfrancisco) in collaboration
with **Claude** (Anthropic's AI model, Claude Opus 5.5), working as a coding agent. Claude
carried out the restructuring into a standalone library: the crate layout, the typed model and
error handling, the three-valued evaluator's API, the GeoJSON reader, the tests and conformance
vectors, the examples, the justfile, CI, and the semantics document. Earlier commits were also
written with AI help, through GitHub Copilot. Francisco sets the direction, reviews every change
and makes the decisions. From the restructuring on, commits Claude contributed to carry a
`Co-authored-by: Claude` trailer.

## Licence

Code: [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

Overture transportation data, including the test fixtures, is © OpenStreetMap contributors and
Overture Maps Foundation, available under the [ODbL](https://opendatacommons.org/licenses/odbl/1-0/).
