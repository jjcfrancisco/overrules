# overrules

[![CI](https://github.com/jjcfrancisco/overrules/actions/workflows/ci.yml/badge.svg)](https://github.com/jjcfrancisco/overrules/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#licence)

**A Rust library that explains which Overture transportation rules apply to a traveller at a
position on a segment.** It handles speed limits, access restrictions and prohibited turns
for pedestrians, cyclists and drivers.

Overture stores these as ordered lists of rules, scoped by direction, time, travel mode,
vehicle dimensions and position. For speed and access, the last matching rule wins; for
prohibited turns, every matching rule applies. Positions are measured along the Earth's
curved surface (geodesically on WGS84), not with flat longitude/latitude maths.

`overrules` reports which rule decided, why each condition matched or failed, and which
missing facts could change the answer.

> Pre-release: supports Overture schema **2.0.0**. Not published to crates.io yet; the API may
> change. Python bindings are planned. The map demo lives in
> [overrules-explorer](https://github.com/jjcfrancisco/overrules-explorer).

## Use

The private repository requires GitHub access:

```toml
[dependencies]
overrules = { git = "ssh://git@github.com/jjcfrancisco/overrules.git", features = ["parquet"] }
```

If Cargo cannot use your SSH credentials, set `CARGO_NET_GIT_FETCH_WITH_CLI=true`.
Pin a `rev` to keep an application on a specific commit.

This example uses the Seville extract downloaded by `just data`:

```rust,no_run
use overrules::{Heading, Network, Situation, TravelMode};

fn main() -> Result<(), overrules::Error> {
    let net = Network::new(overrules::io::read_path("data/seville/segment.parquet")?);
    let segment = net.lookup("e26d3384")?; // GERS id or unique prefix
    let traveller = Situation::new()
        .at(0.05)
        .heading(Heading::Forward)
        .mode(TravelMode::Car);

    let answer = overrules::resolve(segment, &traveller)?;
    assert_eq!(answer.speed_limit_rule().and_then(|r| r.max_speed).map(|s| s.value), Some(80));
    assert_eq!(answer.speed_limit.determining, Some(5));
    assert!(answer.speed_limit.is_settled());
    Ok(())
}
```

That segment's first rule says **90 km/h**, but at this position the result is **80 forward**
or **50 backward**. To see the full explanations:

```sh
just resolve e26d3384 --at 0.05 --heading forward --mode car
just resolve e26d3384 --at 0.05 --heading backward --mode car
```

GeoJSON reading is always available; Parquet needs the `parquet` feature. You can also build
segments directly with `Segment::new`. The library provides geodesic position conversion,
line slicing, lookup by id/name and nearest-segment search, and compiles to WebAssembly.

## Reading an answer

- Rule indices are zero-based. `determining` is the last rule **known** to match;
  `could_override` lists later rules with missing facts. Check `is_settled()` before treating
  the choice as final.
- `Access::Default` does **not** mean allowed: the application must supply the road-class
  default. No speed-limit rule means no numeric limit from the data.
- Prohibited turns have both `in_force` and `could_apply` lists.
- Omitted purposes and recognised statuses mean none were claimed, rather than unknown.
- Readers fail the whole read on an invalid record. `Error::InvalidRecord` identifies the
  segment and field; see the `Error` API docs for an example (`just doc --open`).

The supported fields, schema-version limits and interpretation decisions are documented in
**[docs/semantics.md](docs/semantics.md)**. The [draft conformance vectors](tests/vectors)
provide shared test cases for other implementations.

## Development

Run `just test` for tests, `just ci` for local checks, or `just` to list recipes. Tests use
checked-in fixtures; no data download is needed. See [CONTRIBUTING.md](CONTRIBUTING.md) for
setup, other checks and schema-update instructions.

The goal is a reference evaluator and shared test suite, addressing
[OvertureMaps/data#469](https://github.com/OvertureMaps/data/issues/469)—not a router or data
pipeline. [Historical research](docs/research) records the reasoning behind the project.

## Credits

Developed by [Francisco Jiménez](https://github.com/jjcfrancisco) with AI coding assistance.
Git history records Copilot and Claude contributions; subsequent fixes and documentation
were assisted by GPT models through GitHub Copilot in OpenCode. Francisco sets the direction
and makes the decisions; AI-assisted commits identify their co-author.

## Licence

Code: [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Overture transportation data, including the test fixtures, is © OpenStreetMap contributors
and Overture Maps Foundation, under the [ODbL](https://opendatacommons.org/licenses/odbl/1-0/).
