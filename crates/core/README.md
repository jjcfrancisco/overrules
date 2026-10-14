# overrules

Evaluate [Overture Maps](https://overturemaps.org) transportation rules (speed limits, access
restrictions, prohibited turns) for any traveller, on foot, by bike or at the wheel, and say
which rule decided and why.

Overture attaches these rules to road segments as ordered lists, each rule scoped by `when`
(heading, time, travel mode, purpose, status, vehicle dimensions) and `between` (a stretch of the
road as geodesic fractions of its length). The last matching rule wins. `overrules` implements
that exactly, evaluates every condition with its reason, and treats facts you leave out as
*undetermined* rather than guessing.

```rust,no_run
use overrules::{Access, AccessType, Heading, Network, Situation, TravelMode};

let net = Network::new(overrules::io::read_path("segment.parquet")?); // or .geojson
let street = net.lookup("1b946971")?; // GERS id, or a unique prefix

let driver = Situation::new().heading(Heading::Backward).mode(TravelMode::Car);
let answer = overrules::resolve(street, &driver)?;
assert_eq!(answer.access_outcome(), Access::Rule(AccessType::Denied));
println!("decided by rule #{:?}", answer.access.determining);
# Ok::<(), overrules::Error>(())
```

- **Geodesic linear referencing**: fraction ⇄ point on WGS84 (`lr`), with the planar
  shortcut available for comparison only.
- **Three-valued evaluation with provenance**: every rule, every condition, `match`,
  `no_match` or `undetermined`, and which later rules `could_override` the answer.
- **Typed Overture model**: headings, travel modes with their taxonomy, vehicle units
  (converted to metres and tonnes), `between` ranges.
- **Reading data**: GeoJSON always; Overture GeoParquet with the `parquet` feature.
- Compiles to WebAssembly.

The choices made where the Overture spec is silent are documented in
[`docs/semantics.md`](https://github.com/jjcfrancisco/overrules/blob/main/docs/semantics.md).

Licensed under MIT or Apache-2.0, at your option.
