# overrules

[![CI](https://github.com/jjcfrancisco/overrules/actions/workflows/ci.yml/badge.svg)](https://github.com/jjcfrancisco/overrules/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#licence)

`overrules` is a Rust library for evaluating Overture transportation rules for pedestrians,
cyclists and drivers. It resolves speed limits, access restrictions and prohibited turns,
with rule-by-rule explanations.

For speed and access, the last matching rule wins. For prohibited turns, every matching
rule applies. Positions along segments use WGS84 geodesic distances.

| overrules | Overture schema |
|---|---|
| 0.1 | 2.0.0 |

## Use

Add `overrules` to your dependencies:

```toml
[dependencies]
overrules = { version = "0.1", features = ["parquet"] }
```

Overture publishes its data as GeoParquet, which needs the `parquet` feature. GeoJSON, handy
for small extracts, is always available. This example reads a segment file and asks about one
segment:

```rust,no_run
use overrules::{Heading, Network, Situation, TravelMode};

fn main() -> overrules::Result<()> {
    let net = Network::new(overrules::io::read_path("segment.parquet")?);
    let segment = net.lookup("93212c22-eee0-4a9d-872a-c63dcf56dfa9")?; // GERS id
    let traveller = Situation::new()
        .at(0.8)
        .heading(Heading::Forward)
        .mode(TravelMode::Car);

    let answer = overrules::resolve(segment, &traveller)?;
    assert_eq!(answer.speed_limit_rule().and_then(|r| r.max_speed).map(|s| s.value), Some(20));
    assert_eq!(answer.speed_limit.determining, Some(4));
    assert!(answer.speed_limit.is_settled());
    Ok(())
}
```

That segment, 21st Avenue in New York City, has five speed rules. The first says **15 mph**,
but at this position the answer is **20 mph travelling forward** and **25 mph travelling
backward**.

You can also build segments directly with `Segment::new`. The library provides geodesic
position conversion, line slicing, lookup by id or name and nearest-segment search, and
compiles to WebAssembly.

## Reading an answer

- Rule indices are zero-based. `determining` is the last rule **known** to match.
  `could_override` lists later rules with missing facts. Check `is_settled()` before treating
  the choice as final.
- `Access::Default` does **not** mean allowed: the application must supply the road-class
  default. No speed-limit rule means no numeric limit from the data.
- Prohibited turns have both `in_force` and `could_apply` lists.
- Omitted purposes and recognised statuses mean none were claimed, rather than unknown.
- Readers fail the whole read on an invalid record. `Error::InvalidRecord` identifies the
  segment and field. The [`Error` docs](https://docs.rs/overrules/latest/overrules/enum.Error.html)
  have an example.

The supported fields, schema-version limits and interpretation decisions are documented in
**[docs/semantics.md](docs/semantics.md)**. The [draft conformance vectors](tests/vectors)
provide shared test cases for other implementations.

## Development

Run `just test` for tests, `just ci` for local checks, or `just` to list recipes. Tests use
checked-in fixtures, so no data download is needed. See [CONTRIBUTING.md](CONTRIBUTING.md) for
setup, other checks and schema-update instructions.

## Licence

Code: [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Overture transportation data, including the test fixtures, is © OpenStreetMap contributors
and Overture Maps Foundation, under the [ODbL](https://opendatacommons.org/licenses/odbl/1-0/).
